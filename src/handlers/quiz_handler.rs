use axum::{extract::{Query, State}, http::HeaderMap, Json};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    middleware::auth::require_auth,
    models::{quiz::Quiz, transaction::Transaction, user::Balance},
    state::SharedState,
    utils::error::AppError,
};

const QUIZ_COST: i64 = 5; // 5 COIN per verified quiz

#[derive(Deserialize)]
pub struct GenerateQuizRequest {
    pub subject: String,
}

#[derive(Serialize)]
pub struct GenerateQuizResponse {
    pub quiz: Quiz,
    /// Always None — on-chain burn requires user's private key (not held server-side).
    /// The DB deduction is the authoritative record.
    pub solana_tx: Option<String>,
}

pub async fn generate_quiz(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<GenerateQuizRequest>,
) -> Result<Json<GenerateQuizResponse>, AppError> {
    let user_id = require_auth(&headers, &state.auth)?;
    let subject = req.subject.trim();
    if subject.is_empty() {
        return Err(AppError::BadRequest("subject is required".to_string()));
    }

    // Check balance BEFORE generating (fail fast — don't waste AI calls)
    let bal = Balance::get_by_user_id(&state.db, user_id)
        .await?
        .map(|b| b.token_balance)
        .unwrap_or(0);

    if bal < QUIZ_COST {
        return Err(AppError::Forbidden(format!(
            "Insufficient COIN. You need {} COIN to generate a quiz but have {}.",
            QUIZ_COST, bal
        )));
    }

    // Generate quiz via AI client
    let quiz_payload = state.ai_client.generate_quiz(subject).await;
    let questions = quiz_payload
        .get("questions")
        .cloned()
        .unwrap_or(Value::Array(vec![]));
    let quiz = Quiz::create_generated(&state.db, user_id, subject, questions, QUIZ_COST).await?;

    // DB: deduct COIN (on-chain burn requires user's private key — not held server-side)
    Balance::set_balance(&state.db, user_id, bal - QUIZ_COST).await?;

    let _ = Transaction::create(
        &state.db,
        Some(user_id),
        None,
        QUIZ_COST,
        "quiz_spend",
        Some(quiz.id),
        Some("Verified quiz generation (5 COIN)"),
    )
    .await;

    tracing::info!(
        "quiz: {} COIN deducted from user={} (DB-only; on-chain burn requires user signature)",
        QUIZ_COST, user_id
    );

    Ok(Json(GenerateQuizResponse { quiz, solana_tx: None }))
}

// ---------------------------------------------------------------------------
// POST /api/quiz/record  (record-only — COIN already burned on-chain)
//
// Called after a successful client-side Phantom burn. Saves the quiz row
// to DB for history without touching the balance.
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct RecordQuizRequest {
    pub subject: String,
    pub tokens_spent: i64,
}

pub async fn record_quiz(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<RecordQuizRequest>,
) -> Result<Json<GenerateQuizResponse>, AppError> {
    let user_id = require_auth(&headers, &state.auth)?;
    let subject = req.subject.trim();
    if subject.is_empty() {
        return Err(AppError::BadRequest("subject is required".to_string()));
    }

    // Insert quiz row with empty questions — no balance deduction
    let quiz = Quiz::create_generated(
        &state.db,
        user_id,
        subject,
        Value::Array(vec![]),
        req.tokens_spent,
    )
    .await?;

    tracing::info!(
        "quiz: recorded history row for user={} subject={} (COIN burned on-chain)",
        user_id, subject
    );

    Ok(Json(GenerateQuizResponse { quiz, solana_tx: None }))
}

#[derive(Deserialize)]
pub struct SubmitQuizRequest {
    pub quiz_id: Uuid,
    pub answers: Value,
    pub score: f64,
}

pub async fn submit_quiz(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<SubmitQuizRequest>,
) -> Result<Json<Quiz>, AppError> {
    let user_id = require_auth(&headers, &state.auth)?;
    if !(0.0..=100.0).contains(&req.score) {
        return Err(AppError::BadRequest(
            "score must be between 0 and 100".to_string(),
        ));
    }
    let existing = Quiz::find_by_id(&state.db, req.quiz_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Quiz not found".to_string()))?;
    if existing.user_id != user_id {
        return Err(AppError::Forbidden("Access denied".to_string()));
    }
    let quiz = Quiz::submit_answers(&state.db, req.quiz_id, req.answers, req.score)
        .await?
        .ok_or_else(|| AppError::NotFound("Quiz not found".to_string()))?;
    Ok(Json(quiz))
}

#[derive(Deserialize)]
pub struct Paging {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Serialize)]
pub struct PagedQuizHistory {
    pub items: Vec<Quiz>,
    pub limit: i64,
    pub offset: i64,
}

pub async fn quiz_history(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(paging): Query<Paging>,
) -> Result<Json<PagedQuizHistory>, AppError> {
    let user_id = require_auth(&headers, &state.auth)?;
    let limit = paging.limit.unwrap_or(20).clamp(1, 100);
    let offset = paging.offset.unwrap_or(0).max(0);
    let rows = Quiz::history_by_user(&state.db, user_id, limit, offset).await?;
    Ok(Json(PagedQuizHistory { items: rows, limit, offset }))
}
