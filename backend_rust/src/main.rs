use std::net::SocketAddr;
use axum::Router;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use axum::http::header::HeaderName;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod config;
mod db;
pub mod auth;
mod errors;
mod middleware;
pub mod models;
mod routes;
pub mod services;
mod state;
mod utils;
pub mod ws;

use config::Config;
use state::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize dotenvy and Config
    let config = Config::load().expect("Failed to load configuration from env");

    // 2. Initialize tracing subscriber
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            // Use config's rust_log value
            config.rust_log.clone().into()
        }))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Initializing Fly & Flourish backend services...");

    // Create base upload and tmp directories programmatically
    tracing::info!("Ensuring base upload directories exist at {}...", config.upload_dir);
    tokio::fs::create_dir_all(&config.upload_dir).await?;
    tokio::fs::create_dir_all(format!("{}/tmp", config.upload_dir)).await?;

    // 3. Create Postgres and SQLite pools

    let pg_pool = db::postgres::create_pool(&config.database_url).await?;
    tracing::info!("PostgreSQL connection pool initialized.");

    let sqlite_pool = db::sqlite::create_pool(&config.sqlite_url).await?;
    tracing::info!("SQLite connection pool initialized.");

    // 5. Run Database migrations and setups
    tracing::info!("Running PostgreSQL schema migrations...");
    db::postgres::run_migrations(&pg_pool).await?;

    tracing::info!("Running SQLite schema setup...");
    db::sqlite::run_sqlite_setup(&sqlite_pool).await?;

    // 6. Seed initial data (admin user and default pipeline stages)
    tracing::info!("Checking for seed data...");
    db::postgres::seed_initial_data(&pg_pool, &config).await?;

    // 7. Start the SQLite rate limit background cleanup worker
    db::sqlite::start_cleanup_worker(sqlite_pool.clone());

    // 7.5. Initialize Email Service
    let email_service = std::sync::Arc::new(services::email_service::EmailService::new(&config));

    // 8. Build the application state
    let state = AppState {
        pg_pool,
        sqlite_pool,
        config: config.clone(),
        email_service,
    };

    // 9. Build router with all middlewares
    let x_request_id = HeaderName::from_static("x-request-id");
    let api_router = routes::create_router(state.clone());

    let app = Router::new()
        .route("/ws/chat", axum::routing::get(ws::handler::ws_handler))
        .nest_service("/api", api_router)
        .with_state(state.clone())
        .layer(axum::middleware::from_fn_with_state(state.clone(), middleware::audit_trail::audit_logger))
        .layer(axum::middleware::from_fn_with_state(state.clone(), middleware::rate_limit::rate_limiter))
        .layer(axum::middleware::from_fn(middleware::security_headers::security_headers))
        .layer(PropagateRequestIdLayer::new(x_request_id.clone()))
        .layer(SetRequestIdLayer::new(x_request_id, MakeRequestUuid))
        .layer(middleware::cors::cors_layer())
        .layer(tower_http::trace::TraceLayer::new_for_http());

    // 10. Bind and serve
    let addr = format!("{}:{}", config.host, config.port);
    tracing::info!("Fly & Flourish Axum server running on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}
