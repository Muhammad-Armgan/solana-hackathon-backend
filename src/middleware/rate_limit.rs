use anyhow::Result;
use axum::{
    extract::State,
    middleware::Next,
    response::Response,
};
use redis::AsyncCommands;

use crate::{
    middleware::auth::require_auth,
    state::{AppState, SharedState},
    utils::error::AppError,
};

pub async fn enforce_daily_limit(state: &AppState, key: &str, max: u64, ttl_seconds: u64) -> Result<()> {
    let redis = match &state.redis {
        Some(r) => r,
        None => return Ok(()), // Redis unavailable — skip rate limiting
    };
    let mut conn = redis.clone();
    let count: u64 = conn.incr(key, 1_u64).await?;
    if count == 1 {
        let _: () = conn.expire(key, ttl_seconds as i64).await?;
    }
    if count > max {
        anyhow::bail!("rate_limit_exceeded");
    }
    Ok(())
}

pub async fn enforce_cooldown(state: &AppState, key: &str, cooldown_seconds: u64) -> Result<()> {
    let redis = match &state.redis {
        Some(r) => r,
        None => return Ok(()), // Redis unavailable — skip cooldown
    };
    let mut conn = redis.clone();
    let exists: bool = conn.exists(key).await?;
    if exists {
        anyhow::bail!("cooldown_active");
    }
    let _: () = conn.set_ex(key, 1_u8, cooldown_seconds).await?;
    Ok(())
}

pub async fn endpoint_rate_limit_middleware(
    State(state): State<SharedState>,
    request: axum::extract::Request,
    next: Next,
) -> Result<Response, AppError> {
    let path = request.uri().path().to_string();
    let user_id = require_auth(request.headers(), &state.auth)?;

    match path.as_str() {
        "/api/quiz/generate" => {
            enforce_daily_limit(&state, &format!("rl:quiz:{}", user_id), state.settings.rate_limit_quizzes_per_day, 86_400)
                .await
                .map_err(|_| AppError::Forbidden("Quiz daily limit reached".to_string()))?;
            enforce_cooldown(&state, &format!("cd:quiz:{}", user_id), state.settings.quiz_cooldown_seconds)
                .await
                .map_err(|_| AppError::Forbidden("Quiz cooldown active".to_string()))?;
        }
        "/api/paper/generate" => {
            enforce_daily_limit(&state, &format!("rl:paper:{}", user_id), state.settings.rate_limit_papers_per_day, 86_400)
                .await
                .map_err(|_| AppError::Forbidden("Paper daily limit reached".to_string()))?;
        }
        "/api/upload/submit" => {
            enforce_daily_limit(&state, &format!("rl:upload:{}", user_id), state.settings.rate_limit_uploads_per_day, 86_400)
                .await
                .map_err(|_| AppError::Forbidden("Upload daily limit reached".to_string()))?;
        }
        "/api/token/send" => {
            enforce_daily_limit(&state, &format!("rl:send:{}", user_id), 50, 86_400)
                .await
                .map_err(|_| AppError::Forbidden("Token send daily limit reached".to_string()))?;
        }
        "/api/token/buy" => {
            enforce_daily_limit(&state, &format!("rl:buy:{}", user_id), 5, 3_600)
                .await
                .map_err(|_| AppError::Forbidden("Buy token hourly limit reached".to_string()))?;
        }
        _ => {}
    }

    Ok(next.run(request).await)
}
