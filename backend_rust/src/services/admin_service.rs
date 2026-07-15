use sqlx::PgPool;
use uuid::Uuid;
use crate::models::student::Student;
use crate::models::agent::Agent;
use crate::models::enquiry::Enquiry;
use crate::models::audit::AuditLog;
use crate::errors::AppError;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DashboardStats {
    pub total_students: i64,
    pub total_agents: i64,
    pub total_applications: i64,
    pub total_enquiries: i64,
    pub applications_by_status: std::collections::HashMap<String, i64>,
    pub pending_documents: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PaginatedStudents {
    pub students: Vec<Student>,
    pub total: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PaginatedEnquiries {
    pub enquiries: Vec<Enquiry>,
    pub total: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PaginatedAuditLogs {
    pub logs: Vec<AuditLog>,
    pub total: i64,
}

/// Aggregate platform statistics for the admin dashboard.
pub async fn get_dashboard_stats(pool: &PgPool) -> Result<DashboardStats, AppError> {
    let total_students = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM students")
        .fetch_one(pool)
        .await
        .map_err(AppError::Database)?;

    let total_agents = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM agents")
        .fetch_one(pool)
        .await
        .map_err(AppError::Database)?;

    let total_applications = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM applications")
        .fetch_one(pool)
        .await
        .map_err(AppError::Database)?;

    let total_enquiries = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM enquiries")
        .fetch_one(pool)
        .await
        .map_err(AppError::Database)?;

    let pending_documents = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM documents WHERE status = 'pending'")
        .fetch_one(pool)
        .await
        .map_err(AppError::Database)?;

    let rows = sqlx::query_as::<_, (Option<String>, i64)>(
        "SELECT status, COUNT(*) FROM applications GROUP BY status"
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::Database)?;

    let mut applications_by_status = std::collections::HashMap::new();
    for (status, count) in rows {
        applications_by_status.insert(status.unwrap_or_else(|| "unknown".to_string()), count);
    }

    Ok(DashboardStats {
        total_students,
        total_agents,
        total_applications,
        total_enquiries,
        applications_by_status,
        pending_documents,
    })
}

/// Retrieve a paginated list of students with optional filters.
pub async fn list_students(
    pool: &PgPool,
    page: u32,
    page_size: u32,
    search: Option<&str>,
    is_active: Option<bool>,
) -> Result<PaginatedStudents, AppError> {
    let search_pattern = search.map(|s| format!("%{}%", s));
    let limit = page_size as i64;
    let offset = ((page.max(1) - 1) * page_size) as i64;

    let students = sqlx::query_as::<_, Student>(
        "SELECT id, email, '' as password_hash, full_name, phone, country, preferred_destination, \
         preferred_degree_level, preferred_intake, profile_data, is_verified, is_active, status, created_at, assigned_agent_id \
         FROM students \
         WHERE ($1::text IS NULL OR (full_name ILIKE $1 OR email ILIKE $1 OR phone ILIKE $1)) \
           AND ($2::boolean IS NULL OR is_active = $2) \
         ORDER BY created_at DESC \
         LIMIT $3 OFFSET $4"
    )
    .bind(search_pattern.clone())
    .bind(is_active)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(AppError::Database)?;

    let total = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) \
         FROM students \
         WHERE ($1::text IS NULL OR (full_name ILIKE $1 OR email ILIKE $1 OR phone ILIKE $1)) \
           AND ($2::boolean IS NULL OR is_active = $2)"
    )
    .bind(search_pattern)
    .bind(is_active)
    .fetch_one(pool)
    .await
    .map_err(AppError::Database)?;

    Ok(PaginatedStudents { students, total })
}

/// Retrieve all agents (without password_hash).
pub async fn list_agents(pool: &PgPool) -> Result<Vec<Agent>, AppError> {
    let agents = sqlx::query_as::<_, Agent>(
        "SELECT id, email, '' as password_hash, full_name, phone, specializations, \
         is_active, is_online, last_seen, created_at \
         FROM agents \
         ORDER BY created_at DESC"
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::Database)?;

    Ok(agents)
}

/// Create a new agent profile (admin action).
pub async fn create_agent(
    pool: &PgPool,
    email: &str,
    password_hash: &str,
    full_name: &str,
    phone: Option<&str>,
    specializations: Option<&str>,
) -> Result<Agent, AppError> {
    // Check if user already exists
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM agents WHERE email = $1)"
    )
    .bind(email)
    .fetch_one(pool)
    .await
    .map_err(AppError::Database)?;

    if exists {
        return Err(AppError::Conflict("Agent email already registered".to_string()));
    }

    let agent = sqlx::query_as::<_, Agent>(
        "INSERT INTO agents (id, email, password_hash, full_name, phone, specializations, is_active, is_online) \
         VALUES ($1, $2, $3, $4, $5, $6, TRUE, FALSE) \
         RETURNING id, email, '' as password_hash, full_name, phone, specializations, is_active, is_online, last_seen, created_at"
    )
    .bind(Uuid::new_v4())
    .bind(email)
    .bind(password_hash)
    .bind(full_name)
    .bind(phone)
    .bind(specializations)
    .fetch_one(pool)
    .await
    .map_err(AppError::Database)?;

    Ok(agent)
}

/// Update an agent's details (admin action).
pub async fn update_agent(
    pool: &PgPool,
    id: Uuid,
    full_name: &str,
    phone: Option<&str>,
    specializations: Option<&str>,
    is_active: bool,
) -> Result<Agent, AppError> {
    let agent = sqlx::query_as::<_, Agent>(
        "UPDATE agents \
         SET full_name = $2, phone = $3, specializations = $4, is_active = $5 \
         WHERE id = $1 \
         RETURNING id, email, '' as password_hash, full_name, phone, specializations, is_active, is_online, last_seen, created_at"
    )
    .bind(id)
    .bind(full_name)
    .bind(phone)
    .bind(specializations)
    .bind(is_active)
    .fetch_one(pool)
    .await
    .map_err(AppError::Database)?;

    Ok(agent)
}

/// Activate or deactivate a student.
pub async fn update_student_status(
    pool: &PgPool,
    id: Uuid,
    is_active: bool,
    status: Option<&str>,
) -> Result<(), AppError> {
    let final_status = match status {
        Some(s) => s.to_string(),
        None => if is_active { "in_progress".to_string() } else { "inactive".to_string() },
    };
    let final_active = final_status != "inactive";

    let result = sqlx::query(
        "UPDATE students SET is_active = $2, status = $3 WHERE id = $1"
    )
    .bind(id)
    .bind(final_active)
    .bind(final_status)
    .execute(pool)
    .await
    .map_err(AppError::Database)?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Student with ID {} not found", id)));
    }

    Ok(())
}

/// Retrieve a paginated list of enquiries with optional status filter.
pub async fn list_enquiries(
    pool: &PgPool,
    page: u32,
    page_size: u32,
    status: Option<&str>,
) -> Result<PaginatedEnquiries, AppError> {
    let limit = page_size as i64;
    let offset = ((page.max(1) - 1) * page_size) as i64;

    let enquiries = sqlx::query_as::<_, Enquiry>(
        "SELECT id, student_id, name, email, phone, preferred_destination, message, status, assigned_agent, created_at \
         FROM enquiries \
         WHERE ($1::text IS NULL OR status = $1) \
         ORDER BY created_at DESC \
         LIMIT $2 OFFSET $3"
    )
    .bind(status)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(AppError::Database)?;

    let total = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) \
         FROM enquiries \
         WHERE ($1::text IS NULL OR status = $1)"
    )
    .bind(status)
    .fetch_one(pool)
    .await
    .map_err(AppError::Database)?;

    Ok(PaginatedEnquiries { enquiries, total })
}

/// Update enquiry status and agent assignment.
pub async fn update_enquiry(
    pool: &PgPool,
    id: Uuid,
    status: &str,
    assigned_agent: Option<Uuid>,
) -> Result<Enquiry, AppError> {
    sqlx::query_as::<_, Enquiry>(
        "UPDATE enquiries \
         SET status = $2, assigned_agent = $3 \
         WHERE id = $1 \
         RETURNING id, student_id, name, email, phone, preferred_destination, message, status, assigned_agent, created_at"
    )
    .bind(id)
    .bind(status)
    .bind(assigned_agent)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound(format!("Enquiry with ID {} not found", id)))
}

/// Retrieve paginated audit logs.
pub async fn list_audit_logs(
    pool: &PgPool,
    page: u32,
    page_size: u32,
) -> Result<PaginatedAuditLogs, AppError> {
    let limit = page_size as i64;
    let offset = ((page.max(1) - 1) * page_size) as i64;

    let logs = sqlx::query_as::<_, AuditLog>(
        "SELECT id, user_id, user_type, action, resource_type, resource_id, ip_address, user_agent, metadata, created_at \
         FROM audit_logs \
         ORDER BY created_at DESC \
         LIMIT $1 OFFSET $2"
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(AppError::Database)?;

    let total = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM audit_logs"
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::Database)?;

    Ok(PaginatedAuditLogs { logs, total })
}
