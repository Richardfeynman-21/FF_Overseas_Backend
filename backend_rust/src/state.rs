use crate::config::Config;
use sqlx::{PgPool, SqlitePool};

#[derive(Clone, Debug)]
pub struct AppState {
    pub pg_pool: PgPool,
    pub sqlite_pool: SqlitePool,
    pub config: Config,
    pub email_service: std::sync::Arc<crate::services::email_service::EmailService>,
}
