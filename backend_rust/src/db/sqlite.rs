use sqlx::SqlitePool;
use std::time::Duration;
use tokio::time::sleep;
use chrono::Utc;

pub async fn create_pool(sqlite_url: &str) -> Result<SqlitePool, sqlx::Error> {
    // Ensure parent directories exist
    if let Some(mut path_str) = sqlite_url.strip_prefix("sqlite://") {
        path_str = path_str.split('?').next().unwrap_or(path_str);
        if path_str != ":memory:" {
            if let Some(parent) = std::path::Path::new(path_str).parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)?;
                }
            }
        }
    }

    sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(5)
        .connect(sqlite_url)
        .await
}

pub async fn run_sqlite_setup(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS rate_limits (
            key TEXT NOT NULL,
            timestamp INTEGER NOT NULL
        );"
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_rate_limits_key_timestamp ON rate_limits (key, timestamp);"
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS cache (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            expires_at INTEGER NOT NULL
        );"
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub fn start_cleanup_worker(pool: SqlitePool) {
    tokio::spawn(async move {
        tracing::info!("SQLite rate limit and cache cleanup background worker started.");
        loop {
            sleep(Duration::from_secs(300)).await; // every 5 minutes
            let now = Utc::now().timestamp();
            let rate_limit_cutoff = now - 600; // 10 minutes sliding window

            // Clean up rate limits
            match sqlx::query("DELETE FROM rate_limits WHERE timestamp < ?")
                .bind(rate_limit_cutoff)
                .execute(&pool)
                .await
            {
                Ok(result) => {
                    let rows_affected = result.rows_affected();
                    if rows_affected > 0 {
                        tracing::info!("SQLite rate limit cleanup worker purged {} stale records.", rows_affected);
                    }
                }
                Err(e) => {
                    tracing::error!("SQLite rate limit cleanup background worker failed: {:?}", e);
                }
            }

            // Clean up expired cache
            match sqlx::query("DELETE FROM cache WHERE expires_at < ?")
                .bind(now)
                .execute(&pool)
                .await
            {
                Ok(result) => {
                    let rows_affected = result.rows_affected();
                    if rows_affected > 0 {
                        tracing::info!("SQLite cache cleanup worker purged {} expired records.", rows_affected);
                    }
                }
                Err(e) => {
                    tracing::error!("SQLite cache cleanup background worker failed: {:?}", e);
                }
            }
        }
    });
}
