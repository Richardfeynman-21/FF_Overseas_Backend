use axum::{extract::State, Json};
use serde_json::{json, Value};
use crate::state::AppState;
use crate::errors::AppError;

pub async fn health_check(State(state): State<AppState>) -> Result<Json<Value>, AppError> {
    // 1. Check Postgres connection
    let postgres_status = match sqlx::query("SELECT 1").execute(&state.pg_pool).await {
        Ok(_) => "ok",
        Err(e) => {
            tracing::error!("Postgres health check failed: {:?}", e);
            "error"
        }
    };

    // 2. Check SQLite connection
    let sqlite_status = match sqlx::query("SELECT 1").execute(&state.sqlite_pool).await {
        Ok(_) => "ok",
        Err(e) => {
            tracing::error!("SQLite health check failed: {:?}", e);
            "error"
        }
    };

    let overall_status = if postgres_status == "ok" && sqlite_status == "ok" {
        "ok"
    } else {
        "error"
    };

    Ok(Json(json!({
        "status": overall_status,
        "postgres": postgres_status,
        "sqlite": sqlite_status
    })))
}
