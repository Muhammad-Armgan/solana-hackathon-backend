use std::time::Duration;

use anyhow::{Context, Result};
use sqlx::{postgres::PgPoolOptions, PgPool};

use crate::config::Settings;

pub type DbPool = PgPool;

pub async fn create_pool(settings: &Settings) -> Result<DbPool> {
    let pool = PgPoolOptions::new()
        .max_connections(settings.database_max_connections)
        .min_connections(settings.database_min_connections)
        .acquire_timeout(Duration::from_secs(settings.database_connect_timeout_seconds))
        .connect(&settings.database_url)
        .await
        .with_context(|| {
            format!(
                "Failed to connect to PostgreSQL. Check DATABASE_URL and DB availability: {}",
                settings.database_url
            )
        })?;

    sqlx::query("SELECT 1")
        .execute(&pool)
        .await
        .context("PostgreSQL connection established but health check failed")?;

    Ok(pool)
}
