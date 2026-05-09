use axum::{
    extract::State,
    http::HeaderMap,
    middleware::Next,
    response::Response,
};

use crate::{services::auth_service::AuthService, state::SharedState, utils::error::AppError};

pub fn extract_bearer_token(headers: &HeaderMap) -> Result<String, AppError> {
    let auth_value = headers
        .get(axum::http::header::AUTHORIZATION)
        .ok_or_else(|| AppError::Unauthorized("Missing Authorization header".to_string()))?
        .to_str()
        .map_err(|_| AppError::Unauthorized("Invalid Authorization header".to_string()))?;

    let token = auth_value
        .strip_prefix("Bearer ")
        .ok_or_else(|| AppError::Unauthorized("Authorization must use Bearer token".to_string()))?;

    if token.is_empty() {
        return Err(AppError::Unauthorized("Bearer token cannot be empty".to_string()));
    }

    Ok(token.to_string())
}

pub fn require_auth(headers: &HeaderMap, auth_service: &AuthService) -> Result<uuid::Uuid, AppError> {
    let token = extract_bearer_token(headers)?;
    let claims = auth_service.decode_jwt(&token)?;
    Ok(claims.sub)
}

pub async fn require_auth_middleware(
    State(state): State<SharedState>,
    request: axum::extract::Request,
    next: Next,
) -> Result<Response, AppError> {
    let _ = require_auth(request.headers(), &state.auth)?;
    Ok(next.run(request).await)
}
