use sqlx::PgPool;
use uuid::Uuid;
use serde_json::Value;
use crate::errors::AppError;

pub async fn log_action(
    pool: &PgPool,
    user_id: Option<Uuid>,
    user_type: &str,
    action: &str,
    resource_type: &str,
    resource_id: Option<Uuid>,
    ip_address: Option<&str>,
    user_agent: Option<&str>,
    metadata: Option<Value>,
) -> Result<(), AppError> {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO audit_logs (id, user_id, user_type, action, resource_type, resource_id, ip_address, user_agent, metadata)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"
    )
    .bind(id)
    .bind(user_id)
    .bind(user_type)
    .bind(action)
    .bind(resource_type)
    .bind(resource_id)
    .bind(ip_address)
    .bind(user_agent)
    .bind(metadata)
    .execute(pool)
    .await
    .map_err(AppError::Database)?;

    Ok(())
}
