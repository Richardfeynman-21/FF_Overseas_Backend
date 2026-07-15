use axum::{
    extract::{State, Path},
    routing::{get, put},
    Json,
    Router,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::state::AppState;
use crate::errors::AppError;
use crate::auth::middleware::AuthUser;
use crate::services::{agent_service, visa_service, student_service};
use crate::models::agent::Agent;
use crate::models::student::Student;
use crate::models::shortlist::Shortlist;
use crate::models::visa::VisaStep;
use crate::routes::students::UpdateStudentProfileRequest;

#[derive(Deserialize)]
pub struct UpdateStatusRequest {
    pub is_online: bool,
}

#[derive(Deserialize)]
pub struct CreateStudentRequest {
    pub email: String,
    pub password: String,
    pub full_name: String,
    pub phone: Option<String>,
}

#[derive(Deserialize)]
pub struct AgentUpdateVisaStepRequest {
    pub status: String,
    pub date_completed: Option<String>,
    pub checklist: serde_json::Value,
}

pub fn agents_router() -> Router<AppState> {
    Router::new()
        .route("/me", get(get_profile))
        .route("/me/status", put(update_status))
        .route("/me/students", get(get_assigned_students).post(create_student))
        .route("/students/:student_id/shortlist", get(get_student_shortlist))
        .route("/students/:student_id/profile", put(agent_update_student_profile))
        .route("/students/:student_id/visa-steps", get(get_student_visa_steps))
        .route("/students/:student_id/visa-steps/:step_index", put(update_student_visa_step))
}

/// GET /api/agents/me
async fn get_profile(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> Result<Json<Agent>, AppError> {
    auth_user.require_role("agent")?;
    let agent = agent_service::get_agent_profile(&state.pg_pool, auth_user.claims.sub).await?;
    Ok(Json(agent))
}

/// PUT /api/agents/me/status
async fn update_status(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Json(payload): Json<UpdateStatusRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    auth_user.require_role("agent")?;
    
    agent_service::update_agent_status(&state.pg_pool, auth_user.claims.sub, payload.is_online).await?;
    
    Ok(Json(json!({
        "message": "Status updated successfully",
        "is_online": payload.is_online
    })))
}

/// GET /api/agents/me/students
async fn get_assigned_students(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> Result<Json<Vec<Student>>, AppError> {
    auth_user.require_role("agent")?;
    let students = agent_service::get_assigned_students(&state.pg_pool, auth_user.claims.sub).await?;
    Ok(Json(students))
}

/// POST /api/agents/me/students
async fn create_student(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Json(payload): Json<CreateStudentRequest>,
) -> Result<Json<Student>, AppError> {
    auth_user.require_role("agent")?;
    
    // Hash password using Argon2id
    let password_hash = crate::auth::password::hash_password(&payload.password)?;
    
    let student = agent_service::create_student_by_agent(
        &state.pg_pool,
        auth_user.claims.sub,
        &payload.email,
        &password_hash,
        &payload.full_name,
        payload.phone.as_deref(),
    )
    .await?;
    
    Ok(Json(student))
}

/// GET /api/agents/students/:student_id/shortlist
async fn get_student_shortlist(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(student_id): Path<Uuid>,
) -> Result<Json<Vec<Shortlist>>, AppError> {
    auth_user.require_any_role(&["agent", "superadmin", "admin"])?;
    
    // Check if the agent is assigned to the student
    if auth_user.claims.role == "agent" {
        let is_assigned = crate::services::document_service::is_assigned_agent(&state.pg_pool, student_id, auth_user.claims.sub).await?;
        if !is_assigned {
            return Err(AppError::Forbidden("You are not the assigned advisor for this student".to_string()));
        }
    }

    let mut shortlists = sqlx::query_as::<_, Shortlist>(
        "SELECT id, student_id, university_id, university_name, course_name, degree_level, country, ranking, tuition, scholarship, acceptance_rate, created_at \
         FROM student_shortlists WHERE student_id = $1 ORDER BY created_at DESC"
    )
    .bind(student_id)
    .fetch_all(&state.pg_pool)
    .await
    .map_err(AppError::Database)?;

    let applications = sqlx::query!(
        "SELECT id, university_id, university_name, course_name, degree_level, metadata, created_at \
         FROM applications WHERE student_id = $1 ORDER BY created_at DESC",
        student_id
    )
    .fetch_all(&state.pg_pool)
    .await
    .map_err(AppError::Database)?;

    for app in applications {
        let exists = shortlists.iter().any(|s| {
            s.university_name == app.university_name && s.course_name == app.course_name
        });
        if !exists {
            let country = app.metadata
                .as_ref()
                .and_then(|m| m.get("country"))
                .and_then(|c| c.as_str())
                .unwrap_or("Unknown")
                .to_string();

            shortlists.push(Shortlist {
                id: app.id,
                student_id,
                university_id: app.university_id,
                university_name: app.university_name,
                course_name: app.course_name,
                degree_level: app.degree_level,
                country,
                ranking: Some("N/A".to_string()),
                tuition: Some("N/A".to_string()),
                scholarship: Some("None".to_string()),
                acceptance_rate: Some("N/A".to_string()),
                created_at: Some(app.created_at),
            });
        }
    }

    Ok(Json(shortlists))
}

/// GET /api/agents/students/:student_id/visa-steps
async fn get_student_visa_steps(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(student_id): Path<Uuid>,
) -> Result<Json<Vec<VisaStep>>, AppError> {
    auth_user.require_any_role(&["agent", "superadmin", "admin"])?;

    // Check if the agent is assigned to the student
    if auth_user.claims.role == "agent" {
        let is_assigned = crate::services::document_service::is_assigned_agent(&state.pg_pool, student_id, auth_user.claims.sub).await?;
        if !is_assigned {
            return Err(AppError::Forbidden("You are not the assigned advisor for this student".to_string()));
        }
    }

    let steps = visa_service::get_or_init_visa_steps(&state.pg_pool, student_id).await?;
    Ok(Json(steps))
}

/// PUT /api/agents/students/:student_id/visa-steps/:step_index
async fn update_student_visa_step(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path((student_id, step_index)): Path<(Uuid, i32)>,
    Json(payload): Json<AgentUpdateVisaStepRequest>,
) -> Result<Json<VisaStep>, AppError> {
    auth_user.require_role("agent")?;
    let step = visa_service::update_visa_step(
        &state.pg_pool,
        student_id,
        step_index,
        &payload.status,
        payload.checklist,
        payload.date_completed,
    )
    .await?;
    Ok(Json(step))
}

/// PUT /api/agents/students/:student_id/profile
async fn agent_update_student_profile(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(student_id): Path<Uuid>,
    Json(payload): Json<UpdateStudentProfileRequest>,
) -> Result<Json<Student>, AppError> {
    auth_user.require_any_role(&["agent", "superadmin", "admin"])?;

    // Check if the agent is assigned to the student
    if auth_user.claims.role == "agent" {
        let is_assigned = crate::services::document_service::is_assigned_agent(&state.pg_pool, student_id, auth_user.claims.sub).await?;
        if !is_assigned {
            return Err(AppError::Forbidden("You are not the assigned advisor for this student".to_string()));
        }
    }

    if payload.full_name.trim().is_empty() {
        return Err(AppError::BadRequest("Full name is required".to_string()));
    }

    let student = student_service::update_student_profile(
        &state.pg_pool,
        student_id,
        &payload.full_name,
        payload.phone.as_deref(),
        payload.country.as_deref(),
        payload.preferred_destination.as_deref(),
        payload.preferred_degree_level.as_deref(),
        payload.preferred_intake.as_deref(),
        payload.profile_data,
    )
    .await?;

    Ok(Json(student))
}
