use sqlx::PgPool;
use uuid::Uuid;
use chrono::Utc;
use serde::{Serialize, Deserialize};

use crate::models::application::Application;
use crate::models::progress::StudentStageProgress;
use crate::errors::AppError;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct StageProgressDetail {
    pub stage_id: Uuid,
    pub stage_name: String,
    pub order_index: i32,
    pub description: Option<String>,
    pub is_active: bool,
    pub progress_id: Option<Uuid>,
    pub status: Option<String>,
    pub notes: Option<String>,
    pub updated_by: Option<Uuid>,
    pub completed_at: Option<chrono::DateTime<Utc>>,
}

/// Inserts a new application record in the `applications` table.
/// It also queries the student's assigned agent and assigns them if exists.
pub async fn create_application(
    pool: &PgPool,
    student_id: Uuid,
    university_id: i32,
    university_name: &str,
    course_name: &str,
    degree_level: &str,
    status: Option<&str>,
    metadata: Option<serde_json::Value>,
) -> Result<Application, AppError> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM applications WHERE student_id = $1 AND LOWER(university_name) = LOWER($2) AND LOWER(course_name) = LOWER($3))"
    )
    .bind(student_id)
    .bind(university_name)
    .bind(course_name)
    .fetch_one(pool)
    .await
    .map_err(AppError::Database)?;

    if exists {
        return Err(AppError::BadRequest("You have already applied to this course at this university".to_string()));
    }

    let mut tx = pool.begin().await.map_err(AppError::Database)?;

    let id = Uuid::new_v4();
    let status_val = status.unwrap_or("draft");

    // Fetch the assigned agent ID from student profile if exists
    let agent_id: Option<Uuid> = sqlx::query_scalar::<_, Option<Uuid>>(
        "SELECT assigned_agent_id FROM students WHERE id = $1"
    )
    .bind(student_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(AppError::Database)?
    .flatten();

    let app = sqlx::query_as::<_, Application>(
        "INSERT INTO applications (id, student_id, agent_id, university_id, university_name, course_name, degree_level, status, metadata) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) \
         RETURNING id, student_id, agent_id, university_id, university_name, course_name, degree_level, status, metadata, submitted_at, created_at"
    )
    .bind(id)
    .bind(student_id)
    .bind(agent_id)
    .bind(university_id)
    .bind(university_name)
    .bind(course_name)
    .bind(degree_level)
    .bind(status_val)
    .bind(metadata)
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::Database)?;

    // Automatically initialize default status='not_started' records for all active stages
    let stage_ids: Vec<Uuid> = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM pipeline_stages WHERE is_active = true ORDER BY order_index ASC"
    )
    .fetch_all(&mut *tx)
    .await
    .map_err(AppError::Database)?;

    for stage_id in stage_ids {
        let progress_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO student_stage_progress (id, application_id, stage_id, status, notes, updated_by, completed_at) \
             VALUES ($1, $2, $3, 'not_started', NULL, NULL, NULL)"
        )
        .bind(progress_id)
        .bind(id)
        .bind(stage_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Database)?;
    }

    tx.commit().await.map_err(AppError::Database)?;

    Ok(app)
}

/// Fetches an application record by its ID.
pub async fn get_application_by_id(
    pool: &PgPool,
    id: Uuid,
) -> Result<Application, AppError> {
    sqlx::query_as::<_, Application>(
        "SELECT id, student_id, agent_id, university_id, university_name, course_name, degree_level, status, metadata, submitted_at, created_at \
         FROM applications \
         WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound(format!("Application with ID {} not found", id)))
}

/// Updates basic application details.
pub async fn update_application(
    pool: &PgPool,
    id: Uuid,
    university_id: i32,
    university_name: &str,
    course_name: &str,
    degree_level: &str,
    status: Option<&str>,
    metadata: Option<serde_json::Value>,
) -> Result<Application, AppError> {
    let current = get_application_by_id(pool, id).await?;
    let status_val = status.unwrap_or(current.status.as_deref().unwrap_or("draft"));

    let submitted_at = if status_val == "submitted" && current.submitted_at.is_none() {
        Some(Utc::now())
    } else {
        current.submitted_at
    };

    sqlx::query_as::<_, Application>(
        "UPDATE applications \
         SET university_id = $2, university_name = $3, course_name = $4, degree_level = $5, status = $6, metadata = $7, submitted_at = $8 \
         WHERE id = $1 \
         RETURNING id, student_id, agent_id, university_id, university_name, course_name, degree_level, status, metadata, submitted_at, created_at"
    )
    .bind(id)
    .bind(university_id)
    .bind(university_name)
    .bind(course_name)
    .bind(degree_level)
    .bind(status_val)
    .bind(metadata)
    .bind(submitted_at)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound(format!("Application with ID {} not found", id)))
}

/// Retrieves all applications submitted by a student.
pub async fn list_student_applications(
    pool: &PgPool,
    student_id: Uuid,
) -> Result<Vec<Application>, AppError> {
    sqlx::query_as::<_, Application>(
        "SELECT id, student_id, agent_id, university_id, university_name, course_name, degree_level, status, metadata, submitted_at, created_at \
         FROM applications \
         WHERE student_id = $1 \
         ORDER BY created_at DESC"
    )
    .bind(student_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::Database)
}

/// Retrieves all applications assigned to an agent.
pub async fn list_agent_applications(
    pool: &PgPool,
    agent_id: Uuid,
) -> Result<Vec<Application>, AppError> {
    sqlx::query_as::<_, Application>(
        "SELECT id, student_id, agent_id, university_id, university_name, course_name, degree_level, status, metadata, submitted_at, created_at \
         FROM applications \
         WHERE agent_id = $1 \
         ORDER BY created_at DESC"
    )
    .bind(agent_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::Database)
}

/// List all applications for administrators.
pub async fn list_all_applications(
    pool: &PgPool,
) -> Result<Vec<Application>, AppError> {
    sqlx::query_as::<_, Application>(
        "SELECT id, student_id, agent_id, university_id, university_name, course_name, degree_level, status, metadata, submitted_at, created_at \
         FROM applications \
         ORDER BY created_at DESC"
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::Database)
}

/// Assigns or updates the agent in charge of an application.
pub async fn assign_agent_to_application(
    pool: &PgPool,
    app_id: Uuid,
    agent_id: Option<Uuid>,
) -> Result<Application, AppError> {
    assign_agent(pool, app_id, agent_id).await
}

/// Assigns or updates the agent in charge of an application (alias for route compatibility).
pub async fn assign_agent(
    pool: &PgPool,
    app_id: Uuid,
    agent_id: Option<Uuid>,
) -> Result<Application, AppError> {
    sqlx::query_as::<_, Application>(
        "UPDATE applications \
         SET agent_id = $2 \
         WHERE id = $1 \
         RETURNING id, student_id, agent_id, university_id, university_name, course_name, degree_level, status, metadata, submitted_at, created_at"
    )
    .bind(app_id)
    .bind(agent_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound(format!("Application with ID {} not found", app_id)))
}

/// For a newly created application, automatically insert default status='not_started' records
/// in the `student_stage_progress` table for all active pipeline stages defined in `pipeline_stages`.
pub async fn initialize_application_progress(
    pool: &PgPool,
    application_id: Uuid,
) -> Result<(), AppError> {
    let stage_ids: Vec<Uuid> = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM pipeline_stages WHERE is_active = true ORDER BY order_index ASC"
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::Database)?;

    for stage_id in stage_ids {
        let progress_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO student_stage_progress (id, application_id, stage_id, status, notes, updated_by, completed_at) \
             VALUES ($1, $2, $3, 'not_started', NULL, NULL, NULL)"
        )
        .bind(progress_id)
        .bind(application_id)
        .bind(stage_id)
        .execute(pool)
        .await
        .map_err(AppError::Database)?;
    }

    Ok(())
}

/// Retrieves a joined list of pipeline stages and the student's progress for this application,
/// ordered by stage `order_index`.
pub async fn get_application_progress(
    pool: &PgPool,
    application_id: Uuid,
) -> Result<Vec<StageProgressDetail>, AppError> {
    sqlx::query_as::<_, StageProgressDetail>(
        "SELECT \
             ps.id AS stage_id, \
             ps.name AS stage_name, \
             ps.order_index, \
             ps.description, \
             ps.is_active, \
             ssp.id AS progress_id, \
             ssp.status, \
             ssp.notes, \
             ssp.updated_by, \
             ssp.completed_at \
         FROM pipeline_stages ps \
         LEFT JOIN student_stage_progress ssp \
             ON ps.id = ssp.stage_id AND ssp.application_id = $1 \
         WHERE ps.is_active = true \
         ORDER BY ps.order_index ASC"
    )
    .bind(application_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::Database)
}

/// Inserts or updates a progress status (not_started, in_progress, completed, blocked) and notes for a specific stage.
pub async fn update_stage_progress(
    pool: &PgPool,
    application_id: Uuid,
    stage_id: Uuid,
    status: &str,
    notes: Option<&str>,
    updated_by: Option<Uuid>,
) -> Result<StudentStageProgress, AppError> {
    // Check if progress record already exists
    let existing: Option<StudentStageProgress> = sqlx::query_as::<_, StudentStageProgress>(
        "SELECT id, application_id, stage_id, status, notes, updated_by, completed_at, created_at \
         FROM student_stage_progress \
         WHERE application_id = $1 AND stage_id = $2"
    )
    .bind(application_id)
    .bind(stage_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Database)?;

    let completed_at = if status == "completed" {
        if let Some(ref ext) = existing {
            if ext.status.as_deref() == Some("completed") {
                ext.completed_at.or_else(|| Some(Utc::now()))
            } else {
                Some(Utc::now())
            }
        } else {
            Some(Utc::now())
        }
    } else {
        None
    };

    if let Some(ext) = existing {
        sqlx::query_as::<_, StudentStageProgress>(
            "UPDATE student_stage_progress \
             SET status = $2, notes = $3, updated_by = $4, completed_at = $5 \
             WHERE id = $1 \
             RETURNING id, application_id, stage_id, status, notes, updated_by, completed_at, created_at"
        )
        .bind(ext.id)
        .bind(status)
        .bind(notes)
        .bind(updated_by)
        .bind(completed_at)
        .fetch_optional(pool)
        .await
        .map_err(AppError::Database)?
        .ok_or_else(|| AppError::NotFound(format!("Stage progress with ID {} not found", ext.id)))
    } else {
        let new_id = Uuid::new_v4();
        sqlx::query_as::<_, StudentStageProgress>(
            "INSERT INTO student_stage_progress (id, application_id, stage_id, status, notes, updated_by, completed_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) \
             RETURNING id, application_id, stage_id, status, notes, updated_by, completed_at, created_at"
        )
        .bind(new_id)
        .bind(application_id)
        .bind(stage_id)
        .bind(status)
        .bind(notes)
        .bind(updated_by)
        .bind(completed_at)
        .fetch_one(pool)
        .await
        .map_err(AppError::Database)
    }
}
