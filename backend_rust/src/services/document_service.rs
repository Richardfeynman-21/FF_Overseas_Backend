use sqlx::PgPool;
use uuid::Uuid;
use crate::models::document::Document;
use crate::errors::AppError;

/// Create a new document metadata record in PostgreSQL.
pub async fn create_document(
    pool: &PgPool,
    id: Uuid,
    student_id: Option<Uuid>,
    application_id: Option<Uuid>,
    file_name: &str,
    file_path: &str,
    mime_type: &str,
    file_size_bytes: i64,
    doc_type: &str,
) -> Result<Document, AppError> {
    sqlx::query_as::<_, Document>(
        "INSERT INTO documents (id, student_id, application_id, file_name, file_path, mime_type, file_size_bytes, doc_type, status, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'pending', CURRENT_TIMESTAMP) \
         RETURNING id, student_id, application_id, file_name, file_path, mime_type, file_size_bytes, doc_type, status, verified_by, created_at"
    )
    .bind(id)
    .bind(student_id)
    .bind(application_id)
    .bind(file_name)
    .bind(file_path)
    .bind(mime_type)
    .bind(file_size_bytes)
    .bind(doc_type)
    .fetch_one(pool)
    .await
    .map_err(AppError::Database)
}

/// Retrieve a document metadata record by its ID.
pub async fn get_document(
    pool: &PgPool,
    id: Uuid,
) -> Result<Document, AppError> {
    sqlx::query_as::<_, Document>(
        "SELECT id, student_id, application_id, file_name, file_path, mime_type, file_size_bytes, doc_type, status, verified_by, created_at \
         FROM documents \
         WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound(format!("Document with ID {} not found", id)))
}

/// Update document verification status.
pub async fn verify_document(
    pool: &PgPool,
    id: Uuid,
    is_verified: bool,
    verified_by: Uuid,
) -> Result<Document, AppError> {
    let status = if is_verified { "verified" } else { "rejected" };
    sqlx::query_as::<_, Document>(
        "UPDATE documents \
         SET status = $1, verified_by = $2 \
         WHERE id = $3 \
         RETURNING id, student_id, application_id, file_name, file_path, mime_type, file_size_bytes, doc_type, status, verified_by, created_at"
    )
    .bind(status)
    .bind(verified_by)
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound(format!("Document with ID {} not found", id)))
}

/// Delete a document metadata record by its ID.
pub async fn delete_document(
    pool: &PgPool,
    id: Uuid,
) -> Result<(), AppError> {
    let result = sqlx::query(
        "DELETE FROM documents WHERE id = $1"
    )
    .bind(id)
    .execute(pool)
    .await
    .map_err(AppError::Database)?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Document with ID {} not found", id)));
    }

    Ok(())
}

/// Check if a counselor/agent is assigned to a student.
pub async fn is_assigned_agent(
    pool: &PgPool,
    student_id: Uuid,
    agent_id: Uuid,
) -> Result<bool, AppError> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS ( \
             SELECT 1 FROM students s \
             LEFT JOIN applications a ON s.id = a.student_id \
             WHERE s.id = $1 AND (s.assigned_agent_id = $2 OR a.agent_id = $2) \
         )"
    )
    .bind(student_id)
    .bind(agent_id)
    .fetch_one(pool)
    .await
    .map_err(AppError::Database)?;

    Ok(exists)
}
