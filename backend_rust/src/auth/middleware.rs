use axum::{
    extract::{FromRequestParts, FromRef},
    http::request::Parts,
};
use crate::auth::jwt::Claims;
use crate::errors::AppError;
use crate::state::AppState;

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub claims: Claims,
}

#[axum::async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = AppState::from_ref(state);

        // 1. Get the Authorization header
        let auth_header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| AppError::Unauthorized("Missing Authorization header".to_string()))?;

        // 2. Check if header starts with Bearer
        if !auth_header.starts_with("Bearer ") {
            return Err(AppError::Unauthorized("Invalid Authorization header format".to_string()));
        }

        let token = &auth_header[7..];

        // 3. Decode and validate access token
        let claims = crate::auth::jwt::decode_access_token(token, &app_state.config)?;

        Ok(AuthUser { claims })
    }
}

impl AuthUser {
    /// Enforce that the user has a specific role, otherwise return a 403 Forbidden error.
    pub fn require_role(&self, role: &str) -> Result<(), AppError> {
        if self.claims.role == role {
            Ok(())
        } else {
            Err(AppError::Forbidden("Forbidden: insufficient permissions".to_string()))
        }
    }

    /// Enforce that the user has one of the allowed roles, otherwise return a 403 Forbidden.
    pub fn require_any_role(&self, roles: &[&str]) -> Result<(), AppError> {
        if roles.contains(&self.claims.role.as_str()) {
            Ok(())
        } else {
            Err(AppError::Forbidden("Forbidden: insufficient permissions".to_string()))
        }
    }
}

/// Standalone helper to verify user's role.
pub fn require_role(auth_user: &AuthUser, role: &str) -> Result<(), AppError> {
    auth_user.require_role(role)
}
