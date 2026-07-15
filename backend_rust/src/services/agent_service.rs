use sqlx::PgPool;
use uuid::Uuid;
use crate::models::agent::Agent;
use crate::models::student::Student;
use crate::errors::AppError;

/// Retrieve an agent's profile by their ID (without password_hash).
pub async fn get_agent_profile(
    pool: &PgPool,
    id: Uuid,
) -> Result<Agent, AppError> {
    sqlx::query_as::<_, Agent>(
        "SELECT id, email, '' as password_hash, full_name, phone, specializations, \
         is_active, is_online, last_seen, created_at \
         FROM agents \
         WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound(format!("Agent with ID {} not found", id)))
}

/// Set active agent online/offline presence status.
pub async fn update_agent_status(
    pool: &PgPool,
    id: Uuid,
    is_online: bool,
) -> Result<(), AppError> {
    let result = sqlx::query(
        "UPDATE agents \
         SET is_online = $2, last_seen = $3 \
         WHERE id = $1 AND is_active = TRUE"
    )
    .bind(id)
    .bind(is_online)
    .bind(chrono::Utc::now())
    .execute(pool)
    .await
    .map_err(AppError::Database)?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Active agent with ID {} not found", id)));
    }

    Ok(())
}

/// Query all student profiles assigned to this agent (via their direct assigned_agent_id link or linked applications).
pub async fn get_assigned_students(
    pool: &PgPool,
    agent_id: Uuid,
) -> Result<Vec<Student>, AppError> {
    let students = sqlx::query_as::<_, Student>(
        "SELECT DISTINCT s.id, s.email, '' as password_hash, s.full_name, s.phone, s.country, \
                s.preferred_destination, s.preferred_degree_level, s.preferred_intake, s.profile_data, s.is_verified, \
                s.is_active, s.status, s.created_at, s.assigned_agent_id \
         FROM students s \
         LEFT JOIN applications a ON s.id = a.student_id \
         WHERE a.agent_id = $1 OR s.assigned_agent_id = $1"
    )
    .bind(agent_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::Database)?;

    Ok(students)
}

/// Create a new student profile linked directly to the agent.
pub async fn create_student_by_agent(
    pool: &PgPool,
    agent_id: Uuid,
    email: &str,
    password_hash: &str,
    full_name: &str,
    phone: Option<&str>,
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
        return Err(AppError::Conflict("Email already registered".to_string()));
    }

    let student = sqlx::query_as::<_, Student>(
        "INSERT INTO students (id, email, password_hash, full_name, phone, assigned_agent_id, is_verified, is_active, status) \
         VALUES ($1, $2, $3, $4, $5, $6, TRUE, TRUE, 'lead') \
         RETURNING id, email, '' as password_hash, full_name, phone, country, \
                   preferred_destination, preferred_degree_level, NULL as preferred_intake, profile_data, \
                   is_verified, is_active, status, created_at, assigned_agent_id"
    )
    .bind(Uuid::new_v4())
    .bind(email)
    .bind(password_hash)
    .bind(full_name)
    .bind(phone)
    .bind(agent_id)
    .fetch_one(pool)
    .await
    .map_err(AppError::Database)?;

    Ok(student)
}

