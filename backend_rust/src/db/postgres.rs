use sqlx::PgPool;
use uuid::Uuid;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use crate::config::Config;

pub async fn create_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
}

pub async fn run_migrations(pool: &PgPool) -> Result<(), anyhow::Error> {
    tracing::info!("Running PostgreSQL database migrations...");
    sqlx::migrate!("./migrations")
        .run(pool)
        .await?;
    tracing::info!("PostgreSQL database migrations completed successfully.");
    Ok(())
}

pub async fn seed_initial_data(pool: &PgPool, config: &Config) -> Result<(), anyhow::Error> {
    // 1. Check if there are any admin users
    let admin_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM admin_users WHERE role = 'superadmin')"
    )
    .fetch_one(pool)
    .await?;

    if !admin_exists {
        tracing::info!("No admin users found. Seeding default superadmin user...");
        
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(config.admin_password.as_bytes(), &salt)
            .map_err(|e| anyhow::anyhow!("Argon2 error: {}", e))?
            .to_string();

        let admin_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO admin_users (id, email, password_hash, full_name, role) VALUES ($1, $2, $3, $4, 'superadmin')"
        )
        .bind(admin_id)
        .bind(&config.admin_email)
        .bind(password_hash)
        .bind("System Super Admin")
        .execute(pool)
        .await?;
        
        tracing::info!("Default superadmin user seeded successfully.");
    }


    // 2. Check if pipeline_stages has records
    let stages_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM pipeline_stages"
    )
    .fetch_one(pool)
    .await?;

    if stages_count == 0 {
        tracing::info!("No pipeline stages found. Seeding default stages...");
        let default_stages = vec![
            "Profile Complete",
            "Documents Submitted",
            "University Applied",
            "Offer Received",
            "Visa Applied",
            "Enrolled",
        ];

        for (i, stage_name) in default_stages.into_iter().enumerate() {
            let stage_id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO pipeline_stages (id, name, order_index, is_active) VALUES ($1, $2, $3, $4)"
            )
            .bind(stage_id)
            .bind(stage_name)
            .bind((i + 1) as i32)
            .bind(true)
            .execute(pool)
            .await?;
        }
        tracing::info!("Default pipeline stages seeded successfully.");
    }

    Ok(())
}
