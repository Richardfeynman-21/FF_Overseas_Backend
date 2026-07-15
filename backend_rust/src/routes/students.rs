use axum::{
    extract::{State, Path},
    routing::{get, put, delete, post},
    Json,
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::state::AppState;
use crate::errors::AppError;
use crate::auth::middleware::AuthUser;
use crate::auth::password::{verify_password, hash_password};
use crate::services::{student_service, shortlist_service, visa_service};
use crate::models::student::Student;
use crate::models::shortlist::Shortlist;
use crate::models::visa::VisaStep;

#[derive(Deserialize)]
pub struct UpdateStudentProfileRequest {
    pub full_name: String,
    pub phone: Option<String>,
    pub country: Option<String>,
    pub preferred_destination: Option<String>,
    pub preferred_degree_level: Option<String>,
    pub preferred_intake: Option<String>,
    pub profile_data: Option<serde_json::Value>,
}

#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Deserialize)]
pub struct AddShortlistRequest {
    pub university_id: i32,
    pub course_name: String,
    pub degree_level: String,
}

#[derive(Deserialize)]
pub struct UpdateVisaStepRequest {
    pub status: String,
    pub checklist: serde_json::Value,
    pub date_completed: Option<String>,
}

pub fn students_router() -> Router<AppState> {
    Router::new()
        .route("/me", get(get_profile).put(update_profile))
        .route("/me/password", put(change_password))
        .route("/shortlist", get(get_shortlist).post(add_shortlist))
        .route("/shortlist/:id", delete(delete_shortlist))
        .route("/me/visa-steps", get(get_my_visa_steps))
        .route("/me/visa-steps/:step_index", put(update_my_visa_step))
        .route("/me/consultations/availability", get(get_consultation_availability))
        .route("/me/consultations/book", post(book_consultation))
        .route("/me/active", post(transition_status_active))
}

/// GET /api/students/me
async fn get_profile(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> Result<Json<Student>, AppError> {
    auth_user.require_role("student")?;
    let student = student_service::get_student_profile(&state.pg_pool, auth_user.claims.sub).await?;
    Ok(Json(student))
}

/// PUT /api/students/me
async fn update_profile(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Json(payload): Json<UpdateStudentProfileRequest>,
) -> Result<Json<Student>, AppError> {
    auth_user.require_role("student")?;
    
    if payload.full_name.trim().is_empty() {
        return Err(AppError::BadRequest("Full name is required".to_string()));
    }

    let student = student_service::update_student_profile(
        &state.pg_pool,
        auth_user.claims.sub,
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

/// PUT /api/students/me/password
async fn change_password(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Json(payload): Json<ChangePasswordRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    auth_user.require_role("student")?;

    if payload.current_password.is_empty() || payload.new_password.is_empty() {
        return Err(AppError::BadRequest("Current password and new password are required".to_string()));
    }

    // 1. Fetch current password hash from database securely
    let password_hash = sqlx::query_scalar::<_, String>(
        "SELECT password_hash FROM students WHERE id = $1"
    )
    .bind(auth_user.claims.sub)
    .fetch_optional(&state.pg_pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound("Student not found".to_string()))?;

    // 2. Verify current password
    let valid = verify_password(&payload.current_password, &password_hash)?;
    if !valid {
        return Err(AppError::Unauthorized("Invalid current password".to_string()));
    }

    // 3. Hash and save new password
    let hashed = hash_password(&payload.new_password)?;
    student_service::change_student_password(&state.pg_pool, auth_user.claims.sub, &hashed).await?;

    Ok(Json(json!({ "message": "Password changed successfully" })))
}

/// GET /api/students/shortlist
async fn get_shortlist(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> Result<Json<Vec<Shortlist>>, AppError> {
    auth_user.require_role("student")?;
    let shortlist = shortlist_service::get_student_shortlist(&state.pg_pool, auth_user.claims.sub).await?;
    Ok(Json(shortlist))
}

/// POST /api/students/shortlist
async fn add_shortlist(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Json(payload): Json<AddShortlistRequest>,
) -> Result<Json<Shortlist>, AppError> {
    auth_user.require_role("student")?;
    let shortlist = shortlist_service::add_to_shortlist(
        &state.pg_pool,
        &state.config.frontend_api_key,
        auth_user.claims.sub,
        payload.university_id,
        &payload.course_name,
        &payload.degree_level,
    )
    .await?;
    Ok(Json(shortlist))
}

/// DELETE /api/students/shortlist/:id
async fn delete_shortlist(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    auth_user.require_role("student")?;
    shortlist_service::remove_from_shortlist(&state.pg_pool, auth_user.claims.sub, id).await?;
    Ok(Json(json!({ "message": "Shortlist item removed successfully" })))
}

/// GET /api/students/me/visa-steps
async fn get_my_visa_steps(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> Result<Json<Vec<VisaStep>>, AppError> {
    auth_user.require_role("student")?;
    let steps = visa_service::get_or_init_visa_steps(&state.pg_pool, auth_user.claims.sub).await?;
    Ok(Json(steps))
}

/// PUT /api/students/me/visa-steps/:step_index
async fn update_my_visa_step(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(step_index): Path<i32>,
    Json(payload): Json<UpdateVisaStepRequest>,
) -> Result<Json<VisaStep>, AppError> {
    auth_user.require_role("student")?;
    let step = visa_service::update_visa_step(
        &state.pg_pool,
        auth_user.claims.sub,
        step_index,
        &payload.status,
        payload.checklist,
        payload.date_completed,
    )
    .await?;
    Ok(Json(step))
}

use chrono::NaiveDate;

#[derive(Deserialize)]
pub struct BookConsultationRequest {
    pub booking_date: String,
    pub booking_time: String,
}

#[derive(Serialize)]
pub struct SlotAvailability {
    pub time: String,
    pub available: bool,
    pub status: String,
}

#[derive(Deserialize)]
pub struct AvailabilityQuery {
    pub date: String,
}

/// GET /api/students/me/consultations/availability
async fn get_consultation_availability(
    State(state): State<AppState>,
    auth_user: AuthUser,
    axum::extract::Query(params): axum::extract::Query<AvailabilityQuery>,
) -> Result<Json<Vec<SlotAvailability>>, AppError> {
    auth_user.require_role("student")?;
    let parsed_date = NaiveDate::parse_from_str(&params.date, "%Y-%m-%d")
        .map_err(|_| AppError::BadRequest("invalid date format, use YYYY-MM-DD".to_string()))?;

    let slots = vec![
        "10:00 AM",
        "11:00 AM",
        "12:00 PM",
        "02:00 PM",
        "03:00 PM",
        "04:00 PM",
    ];

    let mut availability = Vec::new();
    for slot in slots {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM consultations WHERE booking_date = $1 AND booking_time = $2 AND status = 'booked'"
        )
        .bind(parsed_date)
        .bind(slot)
        .fetch_one(&state.pg_pool)
        .await
        .map_err(AppError::Database)?;

        let available = count < 2;
        let status = if count == 0 {
            "available".to_string()
        } else if count == 1 {
            "limited".to_string()
        } else {
            "booked".to_string()
        };

        availability.push(SlotAvailability {
            time: slot.to_string(),
            available,
            status,
        });
    }

    Ok(Json(availability))
}

/// POST /api/students/me/consultations/book
async fn book_consultation(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Json(payload): Json<BookConsultationRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    auth_user.require_role("student")?;
    let parsed_date = NaiveDate::parse_from_str(&payload.booking_date, "%Y-%m-%d")
        .map_err(|_| AppError::BadRequest("invalid date format, use YYYY-MM-DD".to_string()))?;

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM consultations WHERE booking_date = $1 AND booking_time = $2 AND status = 'booked'"
    )
    .bind(parsed_date)
    .bind(&payload.booking_time)
    .fetch_one(&state.pg_pool)
    .await
    .map_err(AppError::Database)?;

    if count >= 2 {
        return Err(AppError::BadRequest("This slot is already fully booked. Please select another slot.".to_string()));
    }

    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO consultations (id, student_id, booking_date, booking_time, status) VALUES ($1, $2, $3, $4, 'booked')"
    )
    .bind(id)
    .bind(auth_user.claims.sub)
    .bind(parsed_date)
    .bind(&payload.booking_time)
    .execute(&state.pg_pool)
    .await
    .map_err(AppError::Database)?;

    Ok(Json(serde_json::json!({
        "message": "Consultation slot booked successfully!",
        "booking_id": id,
        "date": payload.booking_date,
        "time": payload.booking_time
    })))
}

/// POST /api/students/me/active
async fn transition_status_active(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, AppError> {
    auth_user.require_role("student")?;
    
    sqlx::query(
        "UPDATE students SET status = 'in_progress' WHERE id = $1 AND status = 'lead'"
    )
    .bind(auth_user.claims.sub)
    .execute(&state.pg_pool)
    .await
    .map_err(AppError::Database)?;

    Ok(Json(serde_json::json!({ "message": "Student status transitioned to in_progress" })))
}
