use axum::{
    extract::{State, Path},
    routing::{get, put, post},
    Json,
    Router,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::state::AppState;
use crate::errors::AppError;
use crate::auth::middleware::AuthUser;
use crate::services::application_service::{self, StageProgressDetail};
use crate::models::application::Application;
use crate::models::progress::StudentStageProgress;

#[derive(Deserialize)]
pub struct CreateApplicationRequest {
    pub university_id: i32,
    pub university_name: String,
    pub course_name: String,
    pub degree_level: String,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Deserialize)]
pub struct UpdateApplicationRequest {
    pub university_id: i32,
    pub university_name: String,
    pub course_name: String,
    pub degree_level: String,
    pub status: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Deserialize)]
pub struct UpdateStageProgressRequest {
    pub status: String,
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct AssignAgentRequest {
    pub agent_id: Uuid,
}

pub fn applications_router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_applications).post(create_application))
        .route("/agent-create-for/:student_id", post(agent_create_application))
        .route("/:id", get(get_application).put(update_application))
        .route("/:id/progress", get(get_progress))
        .route("/:id/progress/:stage_id", put(update_progress))
        .route("/:id/assign", put(assign_agent))
}

/// GET /api/applications
async fn list_applications(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> Result<Json<Vec<Application>>, AppError> {
    match auth_user.claims.role.as_str() {
        "student" => {
            let apps = application_service::list_student_applications(&state.pg_pool, auth_user.claims.sub).await?;
            Ok(Json(apps))
        }
        "agent" => {
            let apps = application_service::list_agent_applications(&state.pg_pool, auth_user.claims.sub).await?;
            Ok(Json(apps))
        }
        "superadmin" => {
            let apps = application_service::list_all_applications(&state.pg_pool).await?;
            Ok(Json(apps))
        }
        _ => Err(AppError::Forbidden("Forbidden: Insufficient permissions".to_string())),
    }
}

/// Helper: Enforce access to an application details modal.
/// Only owning student, assigned agent, or a superadmin are allowed.
async fn check_application_acl(
    state: &AppState,
    auth_user: &AuthUser,
    app: &Application,
) -> Result<(), AppError> {
    match auth_user.claims.role.as_str() {
        "superadmin" => Ok(()), // Superadmin can access everything
        "student" => {
            if app.student_id != Some(auth_user.claims.sub) {
                return Err(AppError::Forbidden("Forbidden: You do not own this application".to_string()));
            }
            Ok(())
        }
        "agent" => {
            // Check if agent is explicitly assigned to the application
            if app.agent_id == Some(auth_user.claims.sub) {
                return Ok(());
            }
            // Check if agent is assigned to the student owning the application
            if let Some(student_id) = app.student_id {
                let student_agent = sqlx::query_scalar::<_, Option<Uuid>>(
                    "SELECT assigned_agent_id FROM students WHERE id = $1"
                )
                .bind(student_id)
                .fetch_optional(&state.pg_pool)
                .await
                .map_err(AppError::Database)?
                .flatten();

                if student_agent == Some(auth_user.claims.sub) {
                    return Ok(());
                }
            }
            Err(AppError::Forbidden("Forbidden: You are not the assigned agent for this application".to_string()))
        }
        _ => Err(AppError::Forbidden("Forbidden: Insufficient permissions".to_string())),
    }
}

/// POST /api/applications
async fn create_application(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Json(payload): Json<CreateApplicationRequest>,
) -> Result<Json<Application>, AppError> {
    // Accessible only by students
    auth_user.require_role("student")?;

    if payload.university_name.trim().is_empty() || payload.course_name.trim().is_empty() || payload.degree_level.trim().is_empty() {
        return Err(AppError::BadRequest("University name, course name, and degree level are required".to_string()));
    }

    let app = application_service::create_application(
        &state.pg_pool,
        auth_user.claims.sub,
        payload.university_id,
        &payload.university_name,
        &payload.course_name,
        &payload.degree_level,
        Some("draft"),
        payload.metadata,
    )
    .await?;

    // Automatically transition lead status to in_progress when student submits an application
    sqlx::query!(
        "UPDATE students SET status = 'in_progress' WHERE id = $1 AND status = 'lead'",
        auth_user.claims.sub
    )
    .execute(&state.pg_pool)
    .await
    .map_err(AppError::Database)?;

    Ok(Json(app))
}

/// GET /api/applications/:id
async fn get_application(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Application>, AppError> {
    let app = application_service::get_application_by_id(&state.pg_pool, id).await?;
    
    // Ownership checks
    check_application_acl(&state, &auth_user, &app).await?;

    Ok(Json(app))
}

/// PUT /api/applications/:id
async fn update_application(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateApplicationRequest>,
) -> Result<Json<Application>, AppError> {
    let app = application_service::get_application_by_id(&state.pg_pool, id).await?;

    // Only allow updates if:
    // 1. The user is the owning student AND the application status is 'draft'.
    // 2. The user is the assigned agent or a superadmin (who can update at any status).
    match auth_user.claims.role.as_str() {
        "superadmin" => {}
        "student" => {
            if app.student_id != Some(auth_user.claims.sub) {
                return Err(AppError::Forbidden("Forbidden: You do not own this application".to_string()));
            }
            if app.status.as_deref() != Some("draft") {
                return Err(AppError::Forbidden("Forbidden: Students can only edit draft applications".to_string()));
            }
        }
        "agent" => {
            let mut is_assigned = app.agent_id == Some(auth_user.claims.sub);
            if !is_assigned {
                if let Some(student_id) = app.student_id {
                    let student_agent = sqlx::query_scalar::<_, Option<Uuid>>(
                        "SELECT assigned_agent_id FROM students WHERE id = $1"
                    )
                    .bind(student_id)
                    .fetch_optional(&state.pg_pool)
                    .await
                    .map_err(AppError::Database)?
                    .flatten();

                    if student_agent == Some(auth_user.claims.sub) {
                        is_assigned = true;
                    }
                }
            }
            if !is_assigned {
                return Err(AppError::Forbidden("Forbidden: You are not assigned to this application".to_string()));
            }
        }
        _ => return Err(AppError::Forbidden("Forbidden: Insufficient permissions".to_string())),
    }

    if payload.university_name.trim().is_empty() || payload.course_name.trim().is_empty() || payload.degree_level.trim().is_empty() {
        return Err(AppError::BadRequest("University name, course name, and degree level are required".to_string()));
    }

    // Students cannot update the status directly, status remains unchanged or updated only if user is agent/superadmin
    let new_status = if auth_user.claims.role == "student" {
        None
    } else {
        payload.status.as_deref()
    };

    let updated_app = application_service::update_application(
        &state.pg_pool,
        id,
        payload.university_id,
        &payload.university_name,
        &payload.course_name,
        &payload.degree_level,
        new_status,
        payload.metadata,
    )
    .await?;

    let status_changed = app.status != updated_app.status;
    if status_changed {
        if let Some(student_id) = updated_app.student_id {
            let pg_pool = state.pg_pool.clone();
            let email_service = state.email_service.clone();
            let uni_name = updated_app.university_name.clone();
            let course_name = updated_app.course_name.clone();
            let new_status_str = updated_app.status.clone().unwrap_or_default();
            tokio::spawn(async move {
                #[derive(sqlx::FromRow)]
                struct StudentEmailName {
                    email: String,
                    full_name: String,
                }

                let student_res = sqlx::query_as::<_, StudentEmailName>(
                    "SELECT email, full_name FROM students WHERE id = $1"
                )
                .bind(student_id)
                .fetch_optional(&pg_pool)
                .await;



                match student_res {
                    Ok(Some(student)) => {
                        if let Err(err) = email_service
                            .send_status_change_email(
                                &student.email,
                                &student.full_name,
                                &uni_name,
                                &course_name,
                                &new_status_str,
                            )
                            .await
                        {
                            tracing::error!("Failed to send status change email: {:?}", err);
                        }
                    }
                    Ok(None) => {
                        tracing::warn!("No student found for id {} to send status change email", student_id);
                    }
                    Err(err) => {
                        tracing::error!("Failed to fetch student details for status change email: {:?}", err);
                    }
                }
            });
        }
    }

    Ok(Json(updated_app))
}

/// GET /api/applications/:id/progress
async fn get_progress(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<StageProgressDetail>>, AppError> {
    let app = application_service::get_application_by_id(&state.pg_pool, id).await?;
    
    // Check access
    check_application_acl(&state, &auth_user, &app).await?;

    let student_id = app.student_id;
    let apps = sqlx::query!(
        "SELECT id, metadata FROM applications WHERE student_id = $1",
        student_id
    )
    .fetch_all(&state.pg_pool)
    .await
    .map_err(AppError::Database)?;

    // Try to find an application with custom milestones metadata first
    let mut found_metadata = None;

    for a in &apps {
        if let Some(ref meta_val) = a.metadata {
            if meta_val.get("milestones").is_some() {
                found_metadata = Some(meta_val.clone());
                break;
            }
        }
    }

    if let Some(ref metadata) = found_metadata {
        if let Some(milestones_val) = metadata.get("milestones") {
            if let Ok(milestones) = serde_json::from_value::<Vec<serde_json::Value>>(milestones_val.clone()) {
                let mut progress = Vec::new();
                for m in milestones {
                    let id_val = m.get("id").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                    let name = m.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let desc = m.get("description").and_then(|v| v.as_str()).map(|s| s.to_string());
                    let status = m.get("status").and_then(|v| v.as_str()).unwrap_or("pending");
                    
                    let mapped_status = match status {
                        "completed" => "completed",
                        "current" => "in_progress",
                        _ => "not_started",
                    };

                    let db_stage_id_str = m.get("dbStageId").and_then(|v| v.as_str()).unwrap_or("");
                    let db_stage_id = Uuid::parse_str(db_stage_id_str).unwrap_or_else(|_| Uuid::new_v4());

                    progress.push(StageProgressDetail {
                        stage_id: db_stage_id,
                        stage_name: name,
                        order_index: id_val,
                        description: desc.clone(),
                        is_active: true,
                        progress_id: None,
                        status: Some(mapped_status.to_string()),
                        notes: desc,
                        updated_by: None,
                        completed_at: None,
                    });
                }
                return Ok(Json(progress));
            }
        }
    }

    // Fallback: If no application has custom metadata, find the one with the most completed stages in DB
    let mut best_app_id = id;
    let mut max_completed: i64 = -1;

    for a in &apps {
        let completed_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM student_stage_progress WHERE application_id = $1 AND status = 'completed'"
        )
        .bind(a.id)
        .fetch_one(&state.pg_pool)
        .await
        .map_err(AppError::Database)?;

        if completed_count > max_completed {
            max_completed = completed_count;
            best_app_id = a.id;
        }
    }

    let progress = application_service::get_application_progress(&state.pg_pool, best_app_id).await?;
    Ok(Json(progress))
}

/// PUT /api/applications/:id/progress/:stage_id
async fn update_progress(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path((id, stage_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<UpdateStageProgressRequest>,
) -> Result<Json<StudentStageProgress>, AppError> {
    let app = application_service::get_application_by_id(&state.pg_pool, id).await?;

    // Accessible only by the assigned agent or superadmin
    match auth_user.claims.role.as_str() {
        "superadmin" => {}
        "agent" => {
            let mut is_assigned = app.agent_id == Some(auth_user.claims.sub);
            if !is_assigned {
                if let Some(student_id) = app.student_id {
                    let student_agent = sqlx::query_scalar::<_, Option<Uuid>>(
                        "SELECT assigned_agent_id FROM students WHERE id = $1"
                    )
                    .bind(student_id)
                    .fetch_optional(&state.pg_pool)
                    .await
                    .map_err(AppError::Database)?
                    .flatten();

                    if student_agent == Some(auth_user.claims.sub) {
                        is_assigned = true;
                    }
                }
            }
            if !is_assigned {
                return Err(AppError::Forbidden("Forbidden: You are not assigned to this application".to_string()));
            }
        }
        _ => return Err(AppError::Forbidden("Forbidden: Only the assigned agent or superadmin can update stage progress".to_string())),
    }

    let progress = application_service::update_stage_progress(
        &state.pg_pool,
        id,
        stage_id,
        &payload.status,
        payload.notes.as_deref(),
        Some(auth_user.claims.sub),
    )
    .await?;

    if app.student_id.is_some() {
        let pg_pool = state.pg_pool.clone();
        let email_service = state.email_service.clone();
        let uni_name = app.university_name.clone();
        let course_name = app.course_name.clone();
        let progress_id = progress.id;
        let progress_status = progress.status.clone();
        tokio::spawn(async move {
            #[derive(sqlx::FromRow)]
            struct StudentAndStageDetails {
                email: String,
                full_name: String,
                stage_name: String,
            }

            let details_res = sqlx::query_as::<_, StudentAndStageDetails>(
                "SELECT s.email, s.full_name, st.name as stage_name \
                 FROM student_stage_progress sp \
                 JOIN applications a ON sp.application_id = a.id \
                 JOIN students s ON a.student_id = s.id \
                 JOIN pipeline_stages st ON sp.stage_id = st.id \
                 WHERE sp.id = $1"
            )
            .bind(progress_id)
            .fetch_optional(&pg_pool)
            .await;

            match details_res {
                Ok(Some(details)) => {
                    let course_display = format!("{} (Stage: {})", course_name, details.stage_name);
                    if let Err(err) = email_service
                        .send_status_change_email(
                            &details.email,
                            &details.full_name,
                            &uni_name,
                            &course_display,
                            progress_status.as_deref().unwrap_or("unknown"),
                        )
                        .await
                    {
                        tracing::error!("Failed to send stage progress update email: {:?}", err);
                    }
                }
                Ok(None) => {
                    tracing::warn!("No details found for stage progress {} to send email", progress_id);
                }
                Err(err) => {
                    tracing::error!("Failed to fetch details for stage progress email: {:?}", err);
                }
            }
        });
    }

    Ok(Json(progress))

}

/// PUT /api/applications/:id/assign
async fn assign_agent(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<AssignAgentRequest>,
) -> Result<Json<Application>, AppError> {
    // Accessible by superadmin only
    auth_user.require_role("superadmin")?;

    let app = application_service::assign_agent_to_application(
        &state.pg_pool,
        id,
        Some(payload.agent_id),
    )
    .await?;

    Ok(Json(app))
}

/// POST /api/applications/agent-create-for/:student_id
async fn agent_create_application(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(student_id): Path<Uuid>,
    Json(payload): Json<CreateApplicationRequest>,
) -> Result<Json<Application>, AppError> {
    // Accessible by agents and superadmins
    auth_user.require_any_role(&["agent", "superadmin"])?;

    // Check if the agent is assigned to the student
    if auth_user.claims.role == "agent" {
        let is_assigned = crate::services::document_service::is_assigned_agent(&state.pg_pool, student_id, auth_user.claims.sub).await?;
        if !is_assigned {
            return Err(AppError::Forbidden("You are not the assigned advisor for this student".to_string()));
        }
    }

    if payload.university_name.trim().is_empty() || payload.course_name.trim().is_empty() || payload.degree_level.trim().is_empty() {
        return Err(AppError::BadRequest("University name, course name, and degree level are required".to_string()));
    }

    // Default status for new agent-submitted applications is "Applied"
    let app = application_service::create_application(
        &state.pg_pool,
        student_id,
        payload.university_id,
        &payload.university_name,
        &payload.course_name,
        &payload.degree_level,
        Some("Applied"),
        payload.metadata,
    )
    .await?;

    // Also transition student status to in_progress if currently a lead
    sqlx::query!(
        "UPDATE students SET status = 'in_progress' WHERE id = $1 AND status = 'lead'",
        student_id
    )
    .execute(&state.pg_pool)
    .await
    .map_err(AppError::Database)?;

    Ok(Json(app))
}
