use axum::{
    extract::{State, Path, Query},
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
use crate::auth::password::hash_password;
use crate::services::admin_service::{self, DashboardStats, PaginatedStudents, PaginatedEnquiries, PaginatedAuditLogs};
use crate::services::student_service;
use crate::models::student::Student;
use crate::models::agent::Agent;
use crate::models::enquiry::Enquiry;

#[derive(Deserialize)]
pub struct StudentsFilter {
    pub page: Option<u32>,
    pub limit: Option<u32>,
    pub search: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Deserialize)]
pub struct UpdateStudentStatusRequest {
    pub is_active: bool,
    pub status: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateAgentRequest {
    pub email: String,
    pub password: String,
    pub full_name: String,
    pub phone: Option<String>,
    pub specializations: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateAgentRequest {
    pub full_name: String,
    pub phone: Option<String>,
    pub specializations: Option<String>,
    pub is_active: bool,
}

#[derive(Deserialize)]
pub struct UpdateEnquiryRequest {
    pub status: String,
    pub assigned_agent: Option<Uuid>,
}

#[derive(Deserialize)]
pub struct EnquiriesQuery {
    pub page: Option<u32>,
    pub limit: Option<u32>,
    pub status: Option<String>,
}

#[derive(Deserialize)]
pub struct AuditLogQuery {
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

pub fn admin_router() -> Router<AppState> {
    Router::new()
        .route("/dashboard", get(get_dashboard))
        .route("/students", get(list_students))
        .route("/students/:id", get(get_student))
        .route("/students/:id/status", put(change_student_status))
        .route("/agents", get(list_agents).post(add_agent))
        .route("/agents/:id", put(edit_agent))
        .route("/enquiries", get(list_enquiries))
        .route("/enquiries/:id", put(edit_enquiry))
        .route("/audit-log", get(list_audit_log))
}

/// Helper to assert the user is a superadmin
fn enforce_superadmin(auth_user: &AuthUser) -> Result<(), AppError> {
    auth_user.require_role("superadmin")
}

/// GET /api/admin/dashboard
async fn get_dashboard(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> Result<Json<DashboardStats>, AppError> {
    enforce_superadmin(&auth_user)?;
    let stats = admin_service::get_dashboard_stats(&state.pg_pool).await?;
    Ok(Json(stats))
}

/// GET /api/admin/students
async fn list_students(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(filter): Query<StudentsFilter>,
) -> Result<Json<PaginatedStudents>, AppError> {
    enforce_superadmin(&auth_user)?;
    let page = filter.page.unwrap_or(1);
    let limit = filter.limit.unwrap_or(10);
    let response = admin_service::list_students(
        &state.pg_pool,
        page,
        limit,
        filter.search.as_deref(),
        filter.is_active,
    )
    .await?;
    Ok(Json(response))
}

/// GET /api/admin/students/:id
async fn get_student(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Student>, AppError> {
    enforce_superadmin(&auth_user)?;
    let student = student_service::get_student_profile(&state.pg_pool, id).await?;
    Ok(Json(student))
}

/// PUT /api/admin/students/:id/status
async fn change_student_status(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateStudentStatusRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    enforce_superadmin(&auth_user)?;
    admin_service::update_student_status(
        &state.pg_pool, 
        id, 
        payload.is_active, 
        payload.status.as_deref()
    ).await?;
    Ok(Json(json!({ "message": "Student status updated successfully" })))
}

/// GET /api/admin/agents
async fn list_agents(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> Result<Json<Vec<Agent>>, AppError> {
    enforce_superadmin(&auth_user)?;
    let agents = admin_service::list_agents(&state.pg_pool).await?;
    Ok(Json(agents))
}

/// POST /api/admin/agents
async fn add_agent(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Json(payload): Json<CreateAgentRequest>,
) -> Result<Json<Agent>, AppError> {
    enforce_superadmin(&auth_user)?;
    
    if payload.email.trim().is_empty() || payload.password.trim().is_empty() || payload.full_name.trim().is_empty() {
        return Err(AppError::BadRequest("Email, password, and full name are required".to_string()));
    }

    let hashed = hash_password(&payload.password)?;
    let agent = admin_service::create_agent(
        &state.pg_pool,
        &payload.email,
        &hashed,
        &payload.full_name,
        payload.phone.as_deref(),
        payload.specializations.as_deref(),
    )
    .await?;

    Ok(Json(agent))
}

/// PUT /api/admin/agents/:id
async fn edit_agent(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateAgentRequest>,
) -> Result<Json<Agent>, AppError> {
    enforce_superadmin(&auth_user)?;

    if payload.full_name.trim().is_empty() {
        return Err(AppError::BadRequest("Full name is required".to_string()));
    }

    let agent = admin_service::update_agent(
        &state.pg_pool,
        id,
        &payload.full_name,
        payload.phone.as_deref(),
        payload.specializations.as_deref(),
        payload.is_active,
    )
    .await?;

    Ok(Json(agent))
}

/// GET /api/admin/enquiries
async fn list_enquiries(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<EnquiriesQuery>,
) -> Result<Json<PaginatedEnquiries>, AppError> {
    enforce_superadmin(&auth_user)?;
    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(50);
    let response = admin_service::list_enquiries(
        &state.pg_pool,
        page,
        limit,
        query.status.as_deref(),
    )
    .await?;
    Ok(Json(response))
}

/// PUT /api/admin/enquiries/:id
async fn edit_enquiry(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateEnquiryRequest>,
) -> Result<Json<Enquiry>, AppError> {
    enforce_superadmin(&auth_user)?;
    let enquiry = admin_service::update_enquiry(
        &state.pg_pool,
        id,
        &payload.status,
        payload.assigned_agent,
    )
    .await?;
    Ok(Json(enquiry))
}

/// GET /api/admin/audit-log
async fn list_audit_log(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<AuditLogQuery>,
) -> Result<Json<PaginatedAuditLogs>, AppError> {
    enforce_superadmin(&auth_user)?;
    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20);
    let logs = admin_service::list_audit_logs(&state.pg_pool, page, limit).await?;
    Ok(Json(logs))
}
