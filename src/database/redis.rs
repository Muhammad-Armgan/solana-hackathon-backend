use anyhow::{Context, Result};
use redis::{aio::ConnectionManager, Client};
use tokio::time::{timeout, Duration};

use crate::config::Settings;

pub type RedisPool = ConnectionManager;

pub async fn create_redis_client(settings: &Settings) -> Result<RedisPool> {
    let client = Client::open(settings.redis_url.as_str())
        .with_context(|| format!("Failed to parse REDIS_URL: {}", settings.redis_url))?;

    // 5-second timeout so a missing Redis doesn't hang startup forever
    let manager = timeout(Duration::from_secs(5), client.get_connection_manager())
        .await
        .context("Redis connection timed out after 5s — is Redis running?")?
        .context("Failed to establish Redis connection manager")?;

    // Sanity-check the connection
    let mut ping_conn = manager.clone();
    let pong: String = timeout(
        Duration::from_secs(3),
        redis::cmd("PING").query_async(&mut ping_conn),
    )
    .await
    .context("Redis PING timed out")?
    .context("Redis PING failed")?;

    if pong != "PONG" {
        anyhow::bail!("Unexpected Redis PING response: {pong}");
    }

    tracing::info!("Redis connected at {}", settings.redis_url);
    Ok(manager)
}
