use axum::{extract::{Path, Query, State}, http::HeaderMap, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    middleware::auth::require_auth,
    models::{paper::Paper, transaction::Transaction, user::Balance},
    state::SharedState,
    utils::error::AppError,
};

const VERIFIED_PAPER_COST: i64 = 5;   // 5 COIN for verified paper
const UNVERIFIED_PAPER_COST: i64 = 2; // 2 COIN for community paper

// ---------------------------------------------------------------------------
// POST /api/paper/generate  (verified — costs 5 COIN)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct GeneratePaperRequest {
    pub subject: String,
}

#[derive(Serialize)]
pub struct GeneratePaperResponse {
    pub paper: Paper,
    /// Always None — on-chain burn requires user's private key (not held server-side).
    pub solana_tx: Option<String>,
}

pub async fn generate_paper(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<GeneratePaperRequest>,
) -> Result<Json<GeneratePaperResponse>, AppError> {
    let user_id = require_auth(&headers, &state.auth)?;
    let subject = req.subject.trim();
    if subject.is_empty() {
        return Err(AppError::BadRequest("subject is required".to_string()));
    }

    let bal = Balance::get_by_user_id(&state.db, user_id)
        .await?
        .map(|b| b.token_balance)
        .unwrap_or(0);

    if bal < VERIFIED_PAPER_COST {
        return Err(AppError::Forbidden(format!(
            "Insufficient COIN. You need {} COIN to generate a verified paper but have {}.",
            VERIFIED_PAPER_COST, bal
        )));
    }

    let payload = state.ai_client.generate_paper(subject).await;
    let download_url = Some(format!("/api/paper/download/{}", Uuid::new_v4()));
    let paper = Paper::create_generated(
        &state.db,
        user_id,
        subject,
        Some(payload),
        download_url.as_deref(),
        VERIFIED_PAPER_COST,
    )
    .await?;

    Balance::set_balance(&state.db, user_id, bal - VERIFIED_PAPER_COST).await?;

    let _ = Transaction::create(
        &state.db,
        Some(user_id),
        None,
        VERIFIED_PAPER_COST,
        "paper_spend",
        Some(paper.id),
        Some("Verified paper generation (5 COIN)"),
    )
    .await;

    tracing::info!(
        "paper: {} COIN deducted from user={} for verified paper",
        VERIFIED_PAPER_COST, user_id
    );

    Ok(Json(GeneratePaperResponse { paper, solana_tx: None }))
}

// ---------------------------------------------------------------------------
// POST /api/paper/generate-unverified  (community — costs 2 COIN)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct GenerateUnverifiedPaperRequest {
    pub subject: String,
}

pub async fn generate_unverified_paper(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<GenerateUnverifiedPaperRequest>,
) -> Result<Json<GeneratePaperResponse>, AppError> {
    let user_id = require_auth(&headers, &state.auth)?;
    let subject = req.subject.trim();
    if subject.is_empty() {
        return Err(AppError::BadRequest("subject is required".to_string()));
    }

    let bal = Balance::get_by_user_id(&state.db, user_id)
        .await?
        .map(|b| b.token_balance)
        .unwrap_or(0);

    if bal < UNVERIFIED_PAPER_COST {
        return Err(AppError::Forbidden(format!(
            "Insufficient COIN. You need {} COIN to generate a community paper but have {}.",
            UNVERIFIED_PAPER_COST, bal
        )));
    }

    let payload = state.ai_client.generate_paper(subject).await;
    let download_url = Some(format!("/api/paper/download/{}", Uuid::new_v4()));
    let paper = Paper::create_generated(
        &state.db,
        user_id,
        subject,
        Some(payload),
        download_url.as_deref(),
        UNVERIFIED_PAPER_COST,
    )
    .await?;

    Balance::set_balance(&state.db, user_id, bal - UNVERIFIED_PAPER_COST).await?;

    let _ = Transaction::create(
        &state.db,
        Some(user_id),
        None,
        UNVERIFIED_PAPER_COST,
        "unverified_paper_spend",
        Some(paper.id),
        Some("Community paper generation (2 COIN)"),
    )
    .await;

    tracing::info!(
        "paper: {} COIN deducted from user={} for community paper",
        UNVERIFIED_PAPER_COST, user_id
    );

    Ok(Json(GeneratePaperResponse { paper, solana_tx: None }))
}

// ---------------------------------------------------------------------------
// POST /api/paper/record          (verified — record-only, no deduction)
// POST /api/paper/record-unverified (community — record-only, no deduction)
//
// Called after a successful client-side Phantom burn. Saves the paper row
// to DB for history without touching the balance.
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct RecordPaperRequest {
    pub subject: String,
    pub tokens_spent: i64,
}

pub async fn record_paper(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<RecordPaperRequest>,
) -> Result<Json<GeneratePaperResponse>, AppError> {
    let user_id = require_auth(&headers, &state.auth)?;
    let subject = req.subject.trim();
    if subject.is_empty() {
        return Err(AppError::BadRequest("subject is required".to_string()));
    }

    let download_url = Some(format!("/api/paper/download/{}", Uuid::new_v4()));
    let paper = Paper::create_generated(
        &state.db,
        user_id,
        subject,
        None,
        download_url.as_deref(),
        req.tokens_spent,
    )
    .await?;

    tracing::info!(
        "paper: recorded history row for user={} subject={} (COIN burned on-chain)",
        user_id, subject
    );

    Ok(Json(GeneratePaperResponse { paper, solana_tx: None }))
}

pub async fn record_unverified_paper(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<RecordPaperRequest>,
) -> Result<Json<GeneratePaperResponse>, AppError> {
    let user_id = require_auth(&headers, &state.auth)?;
    let subject = req.subject.trim();
    if subject.is_empty() {
        return Err(AppError::BadRequest("subject is required".to_string()));
    }

    let download_url = Some(format!("/api/paper/download/{}", Uuid::new_v4()));
    let paper = Paper::create_generated(
        &state.db,
        user_id,
        subject,
        None,
        download_url.as_deref(),
        req.tokens_spent,
    )
    .await?;

    tracing::info!(
        "paper: recorded unverified history row for user={} subject={} (COIN burned on-chain)",
        user_id, subject
    );

    Ok(Json(GeneratePaperResponse { paper, solana_tx: None }))
}

// ---------------------------------------------------------------------------
// GET /api/paper/download/:paper_id
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct DownloadPaperResponse {
    pub id: Uuid,
    pub subject: String,
    pub download_url: Option<String>,
    pub paper_payload: Option<serde_json::Value>,
}

pub async fn download_paper(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(paper_id): Path<Uuid>,
) -> Result<Json<DownloadPaperResponse>, AppError> {
    let user_id = require_auth(&headers, &state.auth)?;
    let paper = Paper::find_by_id(&state.db, paper_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Paper not found".to_string()))?;
    if paper.user_id != user_id {
        return Err(AppError::Forbidden("Access denied".to_string()));
    }

    Ok(Json(DownloadPaperResponse {
        id: paper.id,
        subject: paper.subject,
        download_url: paper.download_url,
        paper_payload: paper.paper_payload,
    }))
}

// ---------------------------------------------------------------------------
// GET /api/paper/history
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct Paging {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Serialize)]
pub struct PagedPaperHistory {
    pub items: Vec<Paper>,
    pub limit: i64,
    pub offset: i64,
}

pub async fn paper_history(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(paging): Query<Paging>,
) -> Result<Json<PagedPaperHistory>, AppError> {
    let user_id = require_auth(&headers, &state.auth)?;
    let limit = paging.limit.unwrap_or(20).clamp(1, 100);
    let offset = paging.offset.unwrap_or(0).max(0);
    let rows = Paper::history_by_user(&state.db, user_id, limit, offset).await?;
    Ok(Json(PagedPaperHistory { items: rows, limit, offset }))
}
