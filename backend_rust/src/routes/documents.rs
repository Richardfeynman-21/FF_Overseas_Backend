use axum::{
    body::Body,
    extract::{State, Path, Multipart},
    http::{header, HeaderMap, Response, StatusCode},
    response::IntoResponse,
    Json,
    Router,
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;
use tokio_util::io::ReaderStream;

use crate::state::AppState;
use crate::errors::AppError;
use crate::auth::middleware::AuthUser;
use crate::services::document_service;
use crate::models::document::Document;

#[derive(Deserialize)]
pub struct VerifyDocumentRequest {
    pub is_verified: bool,
}

pub fn documents_router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_documents))
        .route("/upload", post(upload_document))
        .route("/:id", get(download_document).delete(delete_document))
        .route("/:id/verify", axum::routing::put(verify_document_endpoint))
}

/// POST /api/documents/upload
async fn upload_document(
    State(state): State<AppState>,
    auth_user: AuthUser,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<Document>, AppError> {
    // Enforce role check
    auth_user.require_any_role(&["student", "agent", "superadmin"])?;

    // 1. Check Content-Length header (first line of defense)
    if let Some(content_length_header) = headers.get(header::CONTENT_LENGTH) {
        if let Ok(content_length_str) = content_length_header.to_str() {
            if let Ok(content_length) = content_length_str.parse::<u64>() {
                if content_length > 25 * 1024 * 1024 {
                    return Err(AppError::BadRequest("File exceeds maximum size limit of 25 MB".to_string()));
                }
            }
        }
    }

    let mut doc_type: Option<String> = None;
    let mut application_id: Option<Uuid> = None;
    let mut student_id_field: Option<Uuid> = None;

    // Create a temporary file path under the upload directory
    let temp_dir = format!("{}/tmp", state.config.upload_dir);
    tokio::fs::create_dir_all(&temp_dir).await.map_err(|e| {
        AppError::Internal(anyhow::anyhow!("Failed to create temporary directory: {}", e))
    })?;
    
    let temp_file_id = Uuid::new_v4();
    let temp_path = std::path::PathBuf::from(format!("{}/{}", temp_dir, temp_file_id));

    let mut file_saved = false;
    let mut original_filename = String::new();
    let mut file_mime_type = String::new();
    let mut file_size: u64 = 0;
    let mut file_ext = String::new();

    // Stream the multipart payload
    let result: Result<(), AppError> = async {
        while let Some(field) = multipart.next_field().await.map_err(|e| {
            AppError::BadRequest(format!("Multipart error: {}", e))
        })? {
            let name = field.name().unwrap_or_default().to_string();

            if name == "doc_type" {
                let val = field.text().await.map_err(|e| {
                    AppError::BadRequest(format!("Failed to read doc_type field: {}", e))
                })?;
                doc_type = Some(val.trim().to_string());
            } else if name == "application_id" {
                let val = field.text().await.map_err(|e| {
                    AppError::BadRequest(format!("Failed to read application_id field: {}", e))
                })?;
                if !val.trim().is_empty() {
                    let uuid_val = Uuid::parse_str(val.trim()).map_err(|_| {
                        AppError::BadRequest("Invalid application_id UUID format".to_string())
                    })?;
                    application_id = Some(uuid_val);
                }
            } else if name == "student_id" {
                let val = field.text().await.map_err(|e| {
                    AppError::BadRequest(format!("Failed to read student_id field: {}", e))
                })?;
                if !val.trim().is_empty() {
                    let uuid_val = Uuid::parse_str(val.trim()).map_err(|_| {
                        AppError::BadRequest("Invalid student_id UUID format".to_string())
                    })?;
                    student_id_field = Some(uuid_val);
                }
            } else if name == "file" {
                original_filename = field.file_name().unwrap_or_default().to_string();
                file_mime_type = field.content_type().unwrap_or_default().to_string();

                let ext = std::path::Path::new(&original_filename)
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_lowercase())
                    .ok_or_else(|| AppError::BadRequest("Uploaded file must have an extension".to_string()))?;

                // Enforce: PDF, JPEG, and JPG only (extension and mimetype matching)
                let is_valid_format = match ext.as_str() {
                    "pdf" => file_mime_type == "application/pdf",
                    "jpeg" | "jpg" => file_mime_type == "image/jpeg",
                    _ => false,
                };

                if !is_valid_format {
                    return Err(AppError::BadRequest(
                        "Invalid file format. Only PDF (application/pdf) and JPEG/JPG (image/jpeg) files are allowed.".to_string()
                    ));
                }

                file_ext = ext;

                let mut file = tokio::fs::File::create(&temp_path).await.map_err(|e| {
                    AppError::Internal(anyhow::anyhow!("Failed to create temporary file on disk: {}", e))
                })?;

                let mut total_bytes = 0;
                let mut field = field;

                // Read chunks and enforce size validation
                while let Some(chunk) = field.chunk().await.map_err(|e| {
                    AppError::BadRequest(format!("Failed to read stream chunk: {}", e))
                })? {
                    total_bytes += chunk.len() as u64;
                    if total_bytes > 25 * 1024 * 1024 {
                        return Err(AppError::BadRequest("File exceeds maximum size limit of 25 MB".to_string()));
                    }

                    use tokio::io::AsyncWriteExt;
                    file.write_all(&chunk).await.map_err(|e| {
                        AppError::Internal(anyhow::anyhow!("Failed to write chunk to temporary file: {}", e))
                    })?;
                }

                file_size = total_bytes;
                file_saved = true;
            }
        }
        Ok(())
    }.await;

    // Clean up temporary file on failure
    if let Err(err) = result {
        let _ = tokio::fs::remove_file(&temp_path).await;
        return Err(err);
    }

    if !file_saved {
        let _ = tokio::fs::remove_file(&temp_path).await;
        return Err(AppError::BadRequest("No file field was uploaded".to_string()));
    }

    // 2. Validate doc_type
    let doc_type = match doc_type {
        Some(dt) => {
            let dt = dt.to_lowercase();
            if !["passport", "transcript", "sop", "lor", "resume", "financial", "other"].contains(&dt.as_str()) {
                let _ = tokio::fs::remove_file(&temp_path).await;
                return Err(AppError::BadRequest(
                    "Invalid doc_type. Allowed types: passport, transcript, sop, lor, resume, financial, other".to_string()
                ));
            }
            dt
        }
        None => {
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(AppError::BadRequest("doc_type field is required".to_string()));
        }
    };

    // 3. Resolve student_id based on role
    let student_id = match auth_user.claims.role.as_str() {
        "student" => auth_user.claims.sub,
        "agent" => {
            if let Some(sid) = student_id_field {
                let is_assigned = document_service::is_assigned_agent(&state.pg_pool, sid, auth_user.claims.sub).await?;
                if !is_assigned {
                    let _ = tokio::fs::remove_file(&temp_path).await;
                    return Err(AppError::Forbidden("You are not the assigned agent for this student".to_string()));
                }
                sid
            } else {
                let _ = tokio::fs::remove_file(&temp_path).await;
                return Err(AppError::BadRequest("student_id field is required for agents".to_string()));
            }
        }
        "superadmin" => {
            if let Some(sid) = student_id_field {
                sid
            } else {
                let _ = tokio::fs::remove_file(&temp_path).await;
                return Err(AppError::BadRequest("student_id field is required for admins".to_string()));
            }
        }
        _ => {
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(AppError::Forbidden("Insufficient permissions".to_string()));
        }
    };

    // 4. Move file from temp folder to student/doc_type directory
    let uuid_filename = format!("{}.{}", Uuid::new_v4(), file_ext);
    let student_dir = format!("{}/{}", state.config.upload_dir, student_id);
    let doc_type_dir = format!("{}/{}", student_dir, doc_type);

    if let Err(e) = tokio::fs::create_dir_all(&doc_type_dir).await {
        let _ = tokio::fs::remove_file(&temp_path).await;
        return Err(AppError::Internal(anyhow::anyhow!("Failed to create destination directories: {}", e)));
    }

    let final_file_path = format!("{}/{}", doc_type_dir, uuid_filename);

    if let Err(_) = tokio::fs::rename(&temp_path, &final_file_path).await {
        // Fallback to copy & remove if cross-device rename fails
        if let Err(e) = tokio::fs::copy(&temp_path, &final_file_path).await {
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(AppError::Internal(anyhow::anyhow!("Failed to copy file to destination: {}", e)));
        }
        let _ = tokio::fs::remove_file(&temp_path).await;
    }

    // 5. Create database record in Postgres
    let doc_id = Uuid::new_v4();
    let doc = match document_service::create_document(
        &state.pg_pool,
        doc_id,
        Some(student_id),
        application_id,
        &original_filename,
        &final_file_path,
        &file_mime_type,
        file_size as i64,
        &doc_type,
    ).await {
        Ok(d) => d,
        Err(err) => {
            // Delete file if DB insert fails
            let _ = tokio::fs::remove_file(&final_file_path).await;
            return Err(err);
        }
    };

    Ok(Json(doc))
}

/// GET /api/documents/:id
async fn download_document(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    // 1. Get document metadata
    let document = document_service::get_document(&state.pg_pool, id).await?;

    // 2. Enforce ACL checks
    let is_superadmin = auth_user.claims.role == "superadmin";
    if !is_superadmin {
        if let Some(doc_student_id) = document.student_id {
            if auth_user.claims.role == "student" {
                if auth_user.claims.sub != doc_student_id {
                    return Err(AppError::Forbidden("You do not have access to this document".to_string()));
                }
            } else if auth_user.claims.role == "agent" {
                let is_assigned = document_service::is_assigned_agent(&state.pg_pool, doc_student_id, auth_user.claims.sub).await?;
                if !is_assigned {
                    return Err(AppError::Forbidden("You are not the assigned agent for this student".to_string()));
                }
            } else {
                return Err(AppError::Forbidden("Access denied".to_string()));
            }
        } else {
            return Err(AppError::Forbidden("Access denied".to_string()));
        }
    }

    // 3. Open local file and stream back
    let file = match tokio::fs::File::open(&document.file_path).await {
        Ok(f) => f,
        Err(_) => return Err(AppError::NotFound("Physical file not found on disk".to_string())),
    };

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, &document.mime_type)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", document.file_name),
        )
        .body(body)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to build download stream: {}", e)))?;

    Ok(response)
}

/// DELETE /api/documents/:id
async fn delete_document(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    // 1. Get document metadata
    let document = document_service::get_document(&state.pg_pool, id).await?;

    // 2. Enforce ACL checks: Only owning student (if pending) or superadmin
    let is_superadmin = auth_user.claims.role == "superadmin";
    let is_student = auth_user.claims.role == "student";

    if is_superadmin {
        // Allowed
    } else if is_student {
        if let Some(doc_student_id) = document.student_id {
            if auth_user.claims.sub != doc_student_id {
                return Err(AppError::Forbidden("You do not own this document".to_string()));
            }
            if document.status.as_deref() != Some("pending") {
                return Err(AppError::Forbidden("Only pending documents can be deleted".to_string()));
            }
        } else {
            return Err(AppError::Forbidden("You do not own this document".to_string()));
        }
    } else {
        return Err(AppError::Forbidden("Insufficient permissions to delete document".to_string()));
    }

    // 3. Delete metadata in PG
    document_service::delete_document(&state.pg_pool, id).await?;

    // 4. Remove file from filesystem
    let _ = tokio::fs::remove_file(&document.file_path).await;

    Ok(Json(json!({ "message": "Document deleted successfully" })))
}

/// PUT /api/documents/:id/verify
async fn verify_document_endpoint(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<VerifyDocumentRequest>,
) -> Result<Json<Document>, AppError> {
    // Enforce role: agent or superadmin only
    auth_user.require_any_role(&["agent", "superadmin"])?;

    let updated_doc = document_service::verify_document(
        &state.pg_pool,
        id,
        payload.is_verified,
        auth_user.claims.sub,
    ).await?;

    Ok(Json(updated_doc))
}

#[derive(Deserialize)]
pub struct ListDocumentsQuery {
    pub student_id: Option<Uuid>,
}

/// GET /api/documents
async fn list_documents(
    State(state): State<AppState>,
    auth_user: AuthUser,
    axum::extract::Query(params): axum::extract::Query<ListDocumentsQuery>,
) -> Result<Json<Vec<Document>>, AppError> {
    auth_user.require_any_role(&["student", "agent", "superadmin"])?;

    let student_id = match auth_user.claims.role.as_str() {
        "student" => {
            if let Some(req_id) = params.student_id {
                if req_id != auth_user.claims.sub {
                    return Err(AppError::Forbidden("You can only list your own documents".to_string()));
                }
            }
            Some(auth_user.claims.sub)
        }
        "agent" => {
            if let Some(req_id) = params.student_id {
                let is_assigned = document_service::is_assigned_agent(&state.pg_pool, req_id, auth_user.claims.sub).await?;
                if !is_assigned {
                    return Err(AppError::Forbidden("You are not the assigned agent for this student".to_string()));
                }
                Some(req_id)
            } else {
                None
            }
        }
        "superadmin" => params.student_id,
        _ => return Err(AppError::Forbidden("Insufficient permissions".to_string())),
    };

    let docs = match (auth_user.claims.role.as_str(), student_id) {
        ("superadmin", Some(sid)) => {
            sqlx::query_as::<_, Document>(
                "SELECT id, student_id, application_id, file_name, file_path, mime_type, file_size_bytes, doc_type, status, verified_by, created_at \
                 FROM documents WHERE student_id = $1"
            )
            .bind(sid)
            .fetch_all(&state.pg_pool)
            .await
            .map_err(AppError::Database)?
        }
        ("superadmin", None) => {
            sqlx::query_as::<_, Document>(
                "SELECT id, student_id, application_id, file_name, file_path, mime_type, file_size_bytes, doc_type, status, verified_by, created_at \
                 FROM documents"
            )
            .fetch_all(&state.pg_pool)
            .await
            .map_err(AppError::Database)?
        }
        ("agent", Some(sid)) => {
            sqlx::query_as::<_, Document>(
                "SELECT id, student_id, application_id, file_name, file_path, mime_type, file_size_bytes, doc_type, status, verified_by, created_at \
                 FROM documents WHERE student_id = $1"
            )
            .bind(sid)
            .fetch_all(&state.pg_pool)
            .await
            .map_err(AppError::Database)?
        }
        ("agent", None) => {
            sqlx::query_as::<_, Document>(
                "SELECT DISTINCT d.id, d.student_id, d.application_id, d.file_name, d.file_path, d.mime_type, d.file_size_bytes, d.doc_type, d.status, d.verified_by, d.created_at \
                 FROM documents d \
                 JOIN students s ON d.student_id = s.id \
                 LEFT JOIN applications a ON s.id = a.student_id \
                 WHERE s.assigned_agent_id = $1 OR a.agent_id = $1"
            )
            .bind(auth_user.claims.sub)
            .fetch_all(&state.pg_pool)
            .await
            .map_err(AppError::Database)?
        }
        (_, Some(sid)) => {
            sqlx::query_as::<_, Document>(
                "SELECT id, student_id, application_id, file_name, file_path, mime_type, file_size_bytes, doc_type, status, verified_by, created_at \
                 FROM documents WHERE student_id = $1"
            )
            .bind(sid)
            .fetch_all(&state.pg_pool)
            .await
            .map_err(AppError::Database)?
        }
        _ => vec![],
    };

    Ok(Json(docs))
}
