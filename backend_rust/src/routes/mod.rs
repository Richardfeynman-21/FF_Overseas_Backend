use axum::{
    routing::get,
    Router,
};
use crate::state::AppState;

pub mod health;
pub mod auth;
pub mod students;
pub mod agents;
pub mod admin;
pub mod applications;
pub mod documents;
pub mod chat;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::health_check))
        .nest("/auth", auth::auth_router())
        .nest("/students", students::students_router())
        .nest("/agents", agents::agents_router())
        .nest("/admin", admin::admin_router())
        .nest("/applications", applications::applications_router())
        .nest("/documents", documents::documents_router())
        .nest("/chat", chat::chat_router())
        .with_state(state)
}

