use axum::{
    extract::{State, ConnectInfo},
    routing::post,
    Json,
    Router,
    http::HeaderMap,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use uuid::Uuid;
use chrono::Utc;

use crate::state::AppState;
use crate::errors::AppError;
use crate::auth::password::{hash_password, verify_password};
use crate::auth::jwt;
use crate::auth::middleware::AuthUser;
use crate::services::db_auth_service;

#[derive(Deserialize)]
pub struct RegisterStudentRequest {
    pub email: String,
    pub password: String,
    pub full_name: String,
    pub phone: Option<String>,
    pub country: Option<String>,
    pub preferred_destination: Option<String>,
    pub preferred_degree_level: Option<String>,
    pub preferred_intake: Option<String>,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
    pub device_fingerprint: Option<String>,
}

#[derive(Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
    pub device_fingerprint: Option<String>,
}

#[derive(Deserialize)]
pub struct LogoutRequest {
    pub refresh_token: String,
}

#[derive(Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
}

/// Helper to extract client IP address from headers or connection info
fn get_client_ip(headers: &HeaderMap, connect_info: Option<&ConnectInfo<SocketAddr>>) -> String {
    if let Some(forwarded) = headers.get("x-forwarded-for") {
        if let Ok(f_str) = forwarded.to_str() {
            return f_str.split(',').next().unwrap_or("unknown").trim().to_string();
        }
    }
    if let Some(real_ip) = headers.get("x-real-ip") {
        if let Ok(ip_str) = real_ip.to_str() {
            return ip_str.to_string();
        }
    }
    if let Some(ConnectInfo(addr)) = connect_info {
        return addr.ip().to_string();
    }
    "unknown".to_string()
}

pub fn auth_router() -> Router<AppState> {
    Router::new()
        .route("/student/register", post(register_student))
        .route("/student/login", post(login_student))
        .route("/agent/login", post(login_agent))
        .route("/admin/login", post(login_admin))
        .route("/refresh", post(refresh_token))
        .route("/logout", post(logout))
        .route("/logout-all", post(logout_all))
}

/// POST /api/auth/student/register
async fn register_student(
    State(state): State<AppState>,
    Json(payload): Json<RegisterStudentRequest>,
) -> Result<Json<crate::models::student::Student>, AppError> {
    if payload.email.trim().is_empty() || payload.password.trim().is_empty() || payload.full_name.trim().is_empty() {
        return Err(AppError::BadRequest("Email, password, and full name are required".to_string()));
    }

    let hashed = hash_password(&payload.password)?;

    let student = db_auth_service::create_student(
        &state.pg_pool,
        &payload.email,
        &hashed,
        &payload.full_name,
        payload.phone.as_deref(),
        payload.country.as_deref(),
        payload.preferred_destination.as_deref(),
        payload.preferred_degree_level.as_deref(),
        payload.preferred_intake.as_deref(),
    )
    .await?;

    let email_service = state.email_service.clone();
    let email = student.email.clone();
    let name = student.full_name.clone();
    tokio::spawn(async move {
        if let Err(err) = email_service.send_welcome_email(&email, &name).await {
            tracing::error!("Failed to send welcome email to {}: {:?}", email, err);
        }
    });

    Ok(Json(student))
}

/// POST /api/auth/student/login
async fn login_student(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<TokenResponse>, AppError> {
    let student = db_auth_service::find_student_by_email(&state.pg_pool, &payload.email)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid credentials".to_string()))?;

    if !student.is_active {
        return Err(AppError::Forbidden("Account is deactivated".to_string()));
    }

    let verified = verify_password(&payload.password, &student.password_hash)?;
    if !verified {
        return Err(AppError::Unauthorized("Invalid credentials".to_string()));
    }

    let access_token = jwt::generate_access_token(
        student.id,
        "student",
        payload.device_fingerprint.clone(),
        &state.config,
    )?;

    let token_family = Uuid::new_v4().to_string();
    let refresh_token = jwt::generate_refresh_token(
        student.id,
        "student",
        &token_family,
        &state.config,
    )?;

    let ip_addr = get_client_ip(&headers, connect_info.as_ref());
    let expires_at = Utc::now() + chrono::Duration::days(state.config.jwt_refresh_token_expire_days);

    db_auth_service::create_refresh_token(
        &state.pg_pool,
        student.id,
        "student",
        &refresh_token,
        &token_family,
        payload.device_fingerprint.as_deref(),
        Some(&ip_addr),
        expires_at,
    )
    .await?;

    Ok(Json(TokenResponse {
        access_token,
        refresh_token,
    }))
}

/// POST /api/auth/agent/login
async fn login_agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<TokenResponse>, AppError> {
    let agent = db_auth_service::find_agent_by_email(&state.pg_pool, &payload.email)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid credentials".to_string()))?;

    if !agent.is_active {
        return Err(AppError::Forbidden("Account is deactivated".to_string()));
    }

    let verified = verify_password(&payload.password, &agent.password_hash)?;
    if !verified {
        return Err(AppError::Unauthorized("Invalid credentials".to_string()));
    }

    let access_token = jwt::generate_access_token(
        agent.id,
        "agent",
        payload.device_fingerprint.clone(),
        &state.config,
    )?;

    let token_family = Uuid::new_v4().to_string();
    let refresh_token = jwt::generate_refresh_token(
        agent.id,
        "agent",
        &token_family,
        &state.config,
    )?;

    let ip_addr = get_client_ip(&headers, connect_info.as_ref());
    let expires_at = Utc::now() + chrono::Duration::days(state.config.jwt_refresh_token_expire_days);

    db_auth_service::create_refresh_token(
        &state.pg_pool,
        agent.id,
        "agent",
        &refresh_token,
        &token_family,
        payload.device_fingerprint.as_deref(),
        Some(&ip_addr),
        expires_at,
    )
    .await?;

    Ok(Json(TokenResponse {
        access_token,
        refresh_token,
    }))
}

/// POST /api/auth/admin/login
async fn login_admin(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<TokenResponse>, AppError> {
    let admin = db_auth_service::find_admin_by_email(&state.pg_pool, &payload.email)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid credentials".to_string()))?;

    if !admin.is_active {
        return Err(AppError::Forbidden("Account is deactivated".to_string()));
    }

    let verified = verify_password(&payload.password, &admin.password_hash)?;
    if !verified {
        return Err(AppError::Unauthorized("Invalid credentials".to_string()));
    }

    // Role could be superadmin
    let access_token = jwt::generate_access_token(
        admin.id,
        &admin.role,
        payload.device_fingerprint.clone(),
        &state.config,
    )?;

    let token_family = Uuid::new_v4().to_string();
    let refresh_token = jwt::generate_refresh_token(
        admin.id,
        &admin.role,
        &token_family,
        &state.config,
    )?;

    let ip_addr = get_client_ip(&headers, connect_info.as_ref());
    let expires_at = Utc::now() + chrono::Duration::days(state.config.jwt_refresh_token_expire_days);

    db_auth_service::create_refresh_token(
        &state.pg_pool,
        admin.id,
        &admin.role,
        &refresh_token,
        &token_family,
        payload.device_fingerprint.as_deref(),
        Some(&ip_addr),
        expires_at,
    )
    .await?;

    Ok(Json(TokenResponse {
        access_token,
        refresh_token,
    }))
}

/// POST /api/auth/refresh
async fn refresh_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    Json(payload): Json<RefreshRequest>,
) -> Result<Json<TokenResponse>, AppError> {
    // 1. Decode incoming refresh token
    let _claims = jwt::decode_refresh_token(&payload.refresh_token, &state.config)?;

    // 2. Query Postgres database to find details of this refresh token
    let db_token = db_auth_service::find_refresh_token_by_hash(&state.pg_pool, &payload.refresh_token)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Refresh token not found".to_string()))?;

    // 3. Rotation & Family Reuse Detection
    if db_token.is_revoked {
        // Reuse detected! Someone is trying to refresh using an already revoked token.
        // Revoke the entire family immediately to prevent further abuse.
        db_auth_service::revoke_token_family(&state.pg_pool, &db_token.token_family).await?;
        tracing::warn!(
            "Refresh token reuse detected for family {}. Revoking all tokens in the family.",
            db_token.token_family
        );
        return Err(AppError::Unauthorized("Token reuse detected. Family revoked.".to_string()));
    }

    if db_token.expires_at < Utc::now() {
        return Err(AppError::Unauthorized("Refresh token has expired".to_string()));
    }

    // 4. Revoke the old token so it can't be used again
    db_auth_service::revoke_refresh_token(&state.pg_pool, db_token.id).await?;

    // 5. Generate new pair
    let access_token = jwt::generate_access_token(
        db_token.user_id,
        &db_token.user_type,
        payload.device_fingerprint.clone(),
        &state.config,
    )?;

    // Keep the SAME token family for rotation tracking
    let new_refresh_token = jwt::generate_refresh_token(
        db_token.user_id,
        &db_token.user_type,
        &db_token.token_family,
        &state.config,
    )?;

    let ip_addr = get_client_ip(&headers, connect_info.as_ref());
    let expires_at = Utc::now() + chrono::Duration::days(state.config.jwt_refresh_token_expire_days);

    // 6. Record the new refresh token in Postgres
    db_auth_service::create_refresh_token(
        &state.pg_pool,
        db_token.user_id,
        &db_token.user_type,
        &new_refresh_token,
        &db_token.token_family,
        payload.device_fingerprint.as_deref(),
        Some(&ip_addr),
        expires_at,
    )
    .await?;

    Ok(Json(TokenResponse {
        access_token,
        refresh_token: new_refresh_token,
    }))
}

/// POST /api/auth/logout
async fn logout(
    State(state): State<AppState>,
    Json(payload): Json<LogoutRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let db_token = db_auth_service::find_refresh_token_by_hash(&state.pg_pool, &payload.refresh_token)
        .await?;

    if let Some(token) = db_token {
        // Revoke the token family (session)
        db_auth_service::revoke_token_family(&state.pg_pool, &token.token_family).await?;
    }

    Ok(Json(serde_json::json!({ "status": "success" })))
}

/// POST /api/auth/logout-all
async fn logout_all(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, AppError> {
    db_auth_service::revoke_all_tokens_for_user(&state.pg_pool, auth_user.claims.sub).await?;
    Ok(Json(serde_json::json!({ "status": "success" })))
}
