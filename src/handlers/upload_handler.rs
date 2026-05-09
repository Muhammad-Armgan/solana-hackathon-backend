use axum::{extract::{Multipart, Path, Query, State}, http::HeaderMap, Json};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    middleware::auth::require_auth,
    models::{transaction::Transaction, upload::Upload, user::{Balance, User}},
    state::SharedState,
    utils::error::AppError,
};

pub async fn submit_upload(
    State(state): State<SharedState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<Upload>, AppError> {
    let user_id = require_auth(&headers, &state.auth)?;

    let mut file_name = "upload.bin".to_string();
    let mut bytes = Vec::new();
    let mut content_type = "application/octet-stream".to_string();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::BadRequest("Invalid multipart data".to_string()))?
    {
        if let Some(name) = field.file_name() {
            file_name = name.to_string();
        }
        if let Some(ct) = field.content_type() {
            content_type = ct.to_string();
        }
        bytes = field
            .bytes()
            .await
            .map_err(|_| AppError::BadRequest("Invalid upload bytes".to_string()))?
            .to_vec();
        break;
    }

    if bytes.is_empty() {
        return Err(AppError::BadRequest("No file uploaded".to_string()));
    }
    if bytes.len() > state.settings.max_upload_size {
        return Err(AppError::BadRequest("File too large".to_string()));
    }

    // Store file
    let stored = state
        .storage
        .store_bytes(&file_name, &content_type, bytes)
        .await
        .map_err(|_| AppError::Internal)?;

    let upload = Upload::create(&state.db, user_id, &stored.file_name, &stored.storage_path).await?;

    // Score via AI client (returns ai_score 0.0–100.0 and reward_tokens)
    let score_result = state.ai_client.score_upload(&stored.file_name).await;
    let reward_coins = score_result.reward_tokens;

    let scored = Upload::update_scoring(
        &state.db,
        upload.id,
        "scored",
        Some(score_result.ai_score),
        reward_coins,
    )
    .await?
    .ok_or(AppError::Internal)?;

    // Only credit if reward > 0
    if reward_coins > 0 {
        let bal = Balance::get_by_user_id(&state.db, user_id)
            .await?
            .map(|b| b.token_balance)
            .unwrap_or(0);

        // On-chain mint for upload reward
        let mut solana_tx: Option<String> = None;
        if let Some(ref token_svc) = state.solana {
            use solana_sdk::pubkey::Pubkey;
            use std::str::FromStr;

            // Get user wallet address
            match User::find_by_id(&state.db, user_id).await {
                Ok(Some(user)) => {
                    match Pubkey::from_str(&user.wallet_address) {
                        Ok(pubkey) => {
                            let raw_amount = (reward_coins as u64) * 100; // 2 decimals
                            match token_svc.mint_tokens_to_user(&pubkey, raw_amount).await {
                                Ok(sig) => {
                                    tracing::info!(
                                        "upload: minted {} COIN to user={} tx={}",
                                        reward_coins, user_id, sig
                                    );
                                    solana_tx = Some(sig);
                                }
                                Err(e) => {
                                    tracing::error!(
                                        "upload: on-chain mint failed — {}. DB still credited.",
                                        e
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!("upload: invalid wallet pubkey: {}", e);
                        }
                    }
                }
                Ok(None) => tracing::warn!("upload: user not found for id={}", user_id),
                Err(e) => tracing::error!("upload: DB error fetching user: {}", e),
            }
        }

        // DB credit — always runs regardless of on-chain result
        Balance::set_balance(&state.db, user_id, bal + reward_coins).await?;

        let note = solana_tx
            .as_deref()
            .map(|sig| {
                format!(
                    "Upload reward {} COIN (score={:.2}) tx={}",
                    reward_coins, score_result.ai_score, sig
                )
            })
            .unwrap_or_else(|| {
                format!(
                    "Upload reward {} COIN (score={:.2})",
                    reward_coins, score_result.ai_score
                )
            });

        // Fire-and-forget — don't fail the upload if transaction recording fails
        let _ = Transaction::create(
            &state.db,
            None,
            Some(user_id),
            reward_coins,
            "upload_reward",
            Some(upload.id),
            Some(&note),
        )
        .await;

        tracing::info!(
            "upload: {} COIN rewarded to user={} (AI score={:.2})",
            reward_coins, user_id, score_result.ai_score
        );
    } else {
        tracing::info!(
            "upload: no reward for user={} (AI score={:.2} → 0 COIN)",
            user_id, score_result.ai_score
        );
    }

    Ok(Json(scored))
}

pub async fn upload_status(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(upload_id): Path<Uuid>,
) -> Result<Json<Upload>, AppError> {
    let user_id = require_auth(&headers, &state.auth)?;
    let upload = Upload::find_by_id(&state.db, upload_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Upload not found".to_string()))?;
    if upload.user_id != user_id {
        return Err(AppError::Forbidden("Access denied".to_string()));
    }
    Ok(Json(upload))
}

#[derive(Deserialize)]
pub struct Paging {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn upload_history(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(paging): Query<Paging>,
) -> Result<Json<Vec<Upload>>, AppError> {
    let user_id = require_auth(&headers, &state.auth)?;
    let rows = Upload::history_by_user(
        &state.db,
        user_id,
        paging.limit.unwrap_or(20),
        paging.offset.unwrap_or(0),
    )
    .await?;
    Ok(Json(rows))
}
