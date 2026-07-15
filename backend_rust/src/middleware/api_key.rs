use axum::{
    body::Body,
    extract::State,
    http::Request,
    response::Response,
    middleware::Next,
};
use crate::state::AppState;
use crate::errors::AppError;

#[allow(dead_code)]
pub async fn validate_api_key(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    let api_key_header = request
        .headers()
        .get("x-orbit-api-key")
        .and_then(|value| value.to_str().ok());

    match api_key_header {
        Some(key) if key == state.config.frontend_api_key => {
            Ok(next.run(request).await)
        }
        _ => {
            tracing::warn!("API key validation failed or header missing");
            Err(AppError::Unauthorized("Invalid or missing API key".to_string()))
        }
    }
}
