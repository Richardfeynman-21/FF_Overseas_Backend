use sqlx::PgPool;
use uuid::Uuid;
use crate::models::student::Student;
use crate::errors::AppError;

/// Retrieve a student's profile by their ID (with password_hash blanked out).
pub async fn get_student_profile(
    pool: &PgPool,
    id: Uuid,
) -> Result<Student, AppError> {
    sqlx::query_as::<_, Student>(
        "SELECT s.id, s.email, '' as password_hash, s.full_name, s.phone, s.country, s.preferred_destination, \
         s.preferred_degree_level, s.preferred_intake, s.profile_data, s.is_verified, s.is_active, s.status, s.created_at, s.assigned_agent_id, \
         a.full_name as assigned_agent_name \
         FROM students s \
         LEFT JOIN agents a ON s.assigned_agent_id = a.id \
         WHERE s.id = $1"
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound(format!("Student with ID {} not found", id)))
}

/// Update a student's profile fields and return the updated Student record (without password_hash).
pub async fn update_student_profile(
    pool: &PgPool,
    id: Uuid,
    full_name: &str,
    phone: Option<&str>,
    country: Option<&str>,
    preferred_destination: Option<&str>,
    preferred_degree_level: Option<&str>,
    preferred_intake: Option<&str>,
    profile_data: Option<serde_json::Value>,
) -> Result<Student, AppError> {
    sqlx::query_as::<_, Student>(
        "UPDATE students \
         SET full_name = $2, phone = $3, country = $4, preferred_destination = $5, \
             preferred_degree_level = $6, preferred_intake = $7, profile_data = $8 \
         WHERE id = $1 \
         RETURNING id, email, '' as password_hash, full_name, phone, country, preferred_destination, \
                   preferred_degree_level, preferred_intake, profile_data, is_verified, is_active, status, created_at, assigned_agent_id, \
                   (SELECT full_name FROM agents WHERE id = assigned_agent_id) as assigned_agent_name"
    )
    .bind(id)
    .bind(full_name)
    .bind(phone)
    .bind(country)
    .bind(preferred_destination)
    .bind(preferred_degree_level)
    .bind(preferred_intake)
    .bind(profile_data)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound(format!("Student with ID {} not found", id)))
}

/// Change a student's password_hash.
pub async fn change_student_password(
    pool: &PgPool,
    id: Uuid,
    new_password_hash: &str,
) -> Result<(), AppError> {
    let result = sqlx::query(
        "UPDATE students \
         SET password_hash = $2 \
         WHERE id = $1"
    )
    .bind(id)
    .bind(new_password_hash)
    .execute(pool)
    .await
    .map_err(AppError::Database)?;

    if result.rows_affected() == 0 {
         return Err(AppError::NotFound(format!("Student with ID {} not found", id)));
    }

    Ok(())
}
