use sqlx::PgPool;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use crate::models::student::Student;
use crate::models::agent::Agent;
use crate::models::admin::AdminUser;
use crate::models::token::RefreshToken;
use crate::errors::AppError;

/// Create a new student in the database.
pub async fn create_student(
    pool: &PgPool,
    email: &str,
    password_hash: &str,
    full_name: &str,
    phone: Option<&str>,
    country: Option<&str>,
    preferred_destination: Option<&str>,
    preferred_degree_level: Option<&str>,
    preferred_intake: Option<&str>,
) -> Result<Student, AppError> {
    // Check if user already exists
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM students WHERE email = $1)"
    )
    .bind(email)
    .fetch_one(pool)
    .await
    .map_err(AppError::Database)?;

    if exists {
        return Err(AppError::BadRequest("A student with this email already exists".to_string()));
    }

    let student = sqlx::query_as::<_, Student>(
        "INSERT INTO students (id, email, password_hash, full_name, phone, country, preferred_destination, preferred_degree_level, preferred_intake, is_verified, is_active)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
         RETURNING id, email, password_hash, full_name, phone, country, preferred_destination, preferred_degree_level, preferred_intake, profile_data, is_verified, is_active, created_at, assigned_agent_id"
    )
    .bind(Uuid::new_v4())
    .bind(email)
    .bind(password_hash)
    .bind(full_name)
    .bind(phone)
    .bind(country)
    .bind(preferred_destination)
    .bind(preferred_degree_level)
    .bind(preferred_intake)
    .bind(false)
    .bind(true)
    .fetch_one(pool)
    .await
    .map_err(AppError::Database)?;

    Ok(student)
}

/// Find a student by email.
pub async fn find_student_by_email(pool: &PgPool, email: &str) -> Result<Option<Student>, AppError> {
    sqlx::query_as::<_, Student>(
        "SELECT id, email, password_hash, full_name, phone, country, preferred_destination, preferred_degree_level, preferred_intake, profile_data, is_verified, is_active, status, created_at, assigned_agent_id FROM students WHERE email = $1"
    )
    .bind(email)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Database)
}

/// Find an agent by email.
pub async fn find_agent_by_email(pool: &PgPool, email: &str) -> Result<Option<Agent>, AppError> {
    sqlx::query_as::<_, Agent>(
        "SELECT id, email, password_hash, full_name, phone, specializations, is_active, is_online, last_seen, created_at FROM agents WHERE email = $1"
    )
    .bind(email)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Database)
}

/// Find an admin/superadmin user by email.
pub async fn find_admin_by_email(pool: &PgPool, email: &str) -> Result<Option<AdminUser>, AppError> {
    sqlx::query_as::<_, AdminUser>(
        "SELECT id, email, password_hash, full_name, role, is_active, last_login, created_at FROM admin_users WHERE email = $1"
    )
    .bind(email)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Database)
}

/// Insert a refresh token log.
pub async fn create_refresh_token(
    pool: &PgPool,
    user_id: Uuid,
    user_type: &str,
    token_hash: &str,
    token_family: &str,
    device_fingerprint: Option<&str>,
    ip_address: Option<&str>,
    expires_at: DateTime<Utc>,
) -> Result<(), AppError> {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO refresh_tokens (id, user_id, user_type, token_hash, token_family, device_fingerprint, ip_address, expires_at, is_revoked)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, FALSE)"
    )
    .bind(id)
    .bind(user_id)
    .bind(user_type)
    .bind(token_hash)
    .bind(token_family)
    .bind(device_fingerprint)
    .bind(ip_address)
    .bind(expires_at)
    .execute(pool)
    .await
    .map_err(AppError::Database)?;

    Ok(())
}

/// Find a refresh token by its hash.
pub async fn find_refresh_token_by_hash(
    pool: &PgPool,
    token_hash: &str,
) -> Result<Option<RefreshToken>, AppError> {
    sqlx::query_as::<_, RefreshToken>(
        "SELECT id, user_id, user_type, token_hash, token_family, device_fingerprint, ip_address, expires_at, is_revoked, created_at FROM refresh_tokens WHERE token_hash = $1"
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Database)
}

/// Revoke a specific refresh token by ID.
pub async fn revoke_refresh_token(
    pool: &PgPool,
    token_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query("UPDATE refresh_tokens SET is_revoked = TRUE WHERE id = $1")
        .bind(token_id)
        .execute(pool)
        .await
        .map_err(AppError::Database)?;
    Ok(())
}

/// Revoke all refresh tokens belonging to a family.
pub async fn revoke_token_family(
    pool: &PgPool,
    token_family: &str,
) -> Result<(), AppError> {
    sqlx::query("UPDATE refresh_tokens SET is_revoked = TRUE WHERE token_family = $1")
        .bind(token_family)
        .execute(pool)
        .await
        .map_err(AppError::Database)?;
    Ok(())
}

/// Revoke all refresh tokens for a user.
pub async fn revoke_all_tokens_for_user(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query("UPDATE refresh_tokens SET is_revoked = TRUE WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(AppError::Database)?;
    Ok(())
}
