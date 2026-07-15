use axum::{
    body::Body,
    extract::State,
    http::Request,
    response::Response,
    middleware::Next,
    extract::ConnectInfo,
};
use std::net::SocketAddr;
use crate::state::AppState;
use crate::errors::AppError;
use crate::services::audit_service;

pub async fn audit_logger(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    // 1. Get client IP address
    let ip_address = request.headers()
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or("").trim().to_string())
        .or_else(|| {
            request.headers()
                .get("x-real-ip")
                .and_then(|h| h.to_str().ok())
                .map(|s| s.to_string())
        })
        .or_else(|| {
            request.extensions()
                .get::<ConnectInfo<SocketAddr>>()
                .map(|addr| addr.ip().to_string())
        });

    // 2. Get User-Agent
    let user_agent = request.headers()
        .get(axum::http::header::USER_AGENT)
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());

    // 3. Extract route information
    let method = request.method().clone();
    let path = request.uri().path().to_string();

    // 4. Try to authenticate request to get claims
    let auth_header = request.headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    let auth_claims = if let Some(header_val) = auth_header {
        if header_val.starts_with("Bearer ") {
            let token = &header_val[7..];
            crate::auth::jwt::decode_access_token(token, &state.config).ok()
        } else {
            None
        }
    } else {
        None
    };

    // 5. Run next middleware/handler
    let response = next.run(request).await;

    // 6. Log if request is authenticated
    if let Some(claims) = auth_claims {
        let status = response.status();
        if status.is_success() || status.is_redirection() {
            // Extract resource_id if a UUID segment is in the path
            let mut resource_id = None;
            for segment in path.split('/') {
                if let Ok(uuid) = uuid::Uuid::parse_str(segment) {
                    resource_id = Some(uuid);
                    break;
                }
            }

            // Map method + path to action name and resource type
            let (action, resource_type) = if path.starts_with("/api/auth/student/register") {
                ("REGISTER_STUDENT".to_string(), "student".to_string())
            } else if path.starts_with("/api/auth/student/login") {
                ("LOGIN_STUDENT".to_string(), "student".to_string())
            } else if path.starts_with("/api/auth/agent/login") {
                ("LOGIN_AGENT".to_string(), "agent".to_string())
            } else if path.starts_with("/api/auth/admin/login") {
                ("LOGIN_ADMIN".to_string(), "admin".to_string())
            } else if path.starts_with("/api/auth/refresh") {
                ("REFRESH_TOKEN".to_string(), "token".to_string())
            } else if path.starts_with("/api/auth/logout") {
                ("LOGOUT".to_string(), "token".to_string())
            } else if path.starts_with("/api/applications") {
                (format!("{}_APPLICATION", method), "application".to_string())
            } else if path.starts_with("/api/documents") {
                (format!("{}_DOCUMENT", method), "document".to_string())
            } else if path.starts_with("/api/enquiries") {
                (format!("{}_ENQUIRY", method), "enquiry".to_string())
            } else if path.starts_with("/api/chat") {
                (format!("{}_CHAT", method), "chat".to_string())
            } else {
                (format!("{}_{}", method, path.replace('/', "_").trim_start_matches('_')), "other".to_string())
            };

            let metadata = serde_json::json!({
                "method": method.as_str(),
                "path": path,
                "status_code": status.as_u16(),
            });

            // Spawn database logging task in the background so it doesn't block the HTTP response
            let pg_pool = state.pg_pool.clone();
            let user_id = claims.sub;
            let role = claims.role;
            let ip_addr = ip_address.clone();
            let u_agent = user_agent.clone();
            
            tokio::spawn(async move {
                if let Err(e) = audit_service::log_action(
                    &pg_pool,
                    Some(user_id),
                    &role,
                    &action,
                    &resource_type,
                    resource_id,
                    ip_addr.as_deref(),
                    u_agent.as_deref(),
                    Some(metadata),
                ).await {
                    tracing::error!("Failed to write audit log to database: {:?}", e);
                }
            });
        }
    }

    Ok(response)
}
