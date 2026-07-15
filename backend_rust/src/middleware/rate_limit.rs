use axum::{
    body::Body,
    extract::State,
    http::Request,
    response::Response,
    middleware::Next,
};
use std::net::SocketAddr;
use axum::extract::ConnectInfo;
use chrono::Utc;
use crate::state::AppState;
use crate::errors::AppError;

const RATE_LIMIT_WINDOW: i64 = 600; // 10 minutes in seconds
const RATE_LIMIT_DEFAULT_MAX: i64 = 20; // 20 requests default limit
const RATE_LIMIT_UNIVERSITY_MAX: i64 = 1000; // 1000 requests for university list/shortlist

pub async fn rate_limiter(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    let path = request.uri().path();
    let method = request.method();
    let max_limit = if path == "/api/applications" && method == axum::http::Method::POST {
        RATE_LIMIT_DEFAULT_MAX
    } else {
        RATE_LIMIT_UNIVERSITY_MAX
    };

    // 1. Extract the client identifier (authorization token or IP address)
    let key = if let Some(auth_header) = request.headers().get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            format!("auth:{}", auth_str)
        } else {
            "auth:invalid".to_string()
        }
    } else if let Some(forwarded) = request.headers().get("x-forwarded-for") {
        if let Ok(f_str) = forwarded.to_str() {
            f_str.split(',').next().unwrap_or("unknown").trim().to_string()
        } else {
            "unknown".to_string()
        }
    } else if let Some(real_ip) = request.headers().get("x-real-ip") {
        real_ip.to_str().unwrap_or("unknown").to_string()
    } else if let Some(ConnectInfo(addr)) = request.extensions().get::<ConnectInfo<SocketAddr>>() {
        addr.ip().to_string()
    } else {
        "unknown".to_string()
    };

    let now = Utc::now().timestamp();
    let cutoff = now - RATE_LIMIT_WINDOW;

    // 2. Query-based cleanup of all expired logs globally
    if let Err(e) = sqlx::query("DELETE FROM rate_limits WHERE timestamp < ?")
        .bind(cutoff)
        .execute(&state.sqlite_pool)
        .await
    {
        tracing::error!("Failed to clean up stale rate limits in SQLite: {:?}", e);
    }

    // 3. Count current requests for this key within the sliding window
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM rate_limits WHERE key = ? AND timestamp >= ?")
        .bind(&key)
        .bind(cutoff)
        .fetch_one(&state.sqlite_pool)
        .await
        .map_err(AppError::Database)?;

    // 4. Return 429 if the request limit is exceeded
    if count >= max_limit {
        return Err(AppError::RateLimitExceeded);
    }

    // 5. Insert log of this request
    sqlx::query("INSERT INTO rate_limits (key, timestamp) VALUES (?, ?)")
        .bind(&key)
        .bind(now)
        .execute(&state.sqlite_pool)
        .await
        .map_err(AppError::Database)?;

    Ok(next.run(request).await)
}
