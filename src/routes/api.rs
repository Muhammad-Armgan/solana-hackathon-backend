use axum::{middleware::from_fn_with_state, routing::{get, post}, Json, Router};
use serde_json::{json, Value};

use crate::{
    handlers::{auth_handler, paper_handler, quiz_handler, solana_handler, token_handler, upload_handler},
    middleware::{auth::require_auth_middleware, rate_limit::endpoint_rate_limit_middleware},
    state::SharedState,
};

/// Simple health check — no auth, no DB. Used by Railway's health check probe.
async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

pub fn build_api_router(state: SharedState) -> Router {
    let protected = Router::new()
        .route("/api/auth/me", get(auth_handler::me))
        .route("/api/quiz/generate", post(quiz_handler::generate_quiz))
        .route("/api/quiz/record", post(quiz_handler::record_quiz))
        .route("/api/quiz/submit", post(quiz_handler::submit_quiz))
        .route("/api/quiz/history", get(quiz_handler::quiz_history))
        .route("/api/paper/generate", post(paper_handler::generate_paper))
        .route("/api/paper/generate-unverified", post(paper_handler::generate_unverified_paper))
        .route("/api/paper/record", post(paper_handler::record_paper))
        .route("/api/paper/record-unverified", post(paper_handler::record_unverified_paper))
        .route("/api/paper/download/:paper_id", get(paper_handler::download_paper))
        .route("/api/paper/history", get(paper_handler::paper_history))
        .route("/api/upload/submit", post(upload_handler::submit_upload))
        .route("/api/upload/status/:upload_id", get(upload_handler::upload_status))
        .route("/api/upload/history", get(upload_handler::upload_history))
        .route("/api/token/balance", get(token_handler::token_balance))
        .route("/api/token/send", post(token_handler::token_send))
        .route("/api/token/history", get(token_handler::token_history))
        .route("/api/token/buy", post(token_handler::token_buy))
        .route("/api/token/submit-signed-tx", post(solana_handler::submit_signed_tx))
        .route("/api/solana/blockhash", get(solana_handler::get_blockhash))
        .route("/api/solana/prepare-transfer", post(solana_handler::prepare_transfer))
        .route_layer(from_fn_with_state(state.clone(), endpoint_rate_limit_middleware))
        .route_layer(from_fn_with_state(state.clone(), require_auth_middleware));

    Router::new()
        .route("/health", get(health))          // public — no auth, used by Railway
        .route("/api/auth/signup", post(auth_handler::signup))
        .route("/api/auth/login", post(auth_handler::login))
        .merge(protected)
        .with_state(state)
}
