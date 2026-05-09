use axum::{extract::State, http::HeaderMap, Json};
use base64::Engine;
use serde::{Deserialize, Serialize};
use solana_sdk::{pubkey::Pubkey, signature::Signer, transaction::Transaction};
use std::str::FromStr;

use crate::{
    middleware::auth::require_auth,
    models::{transaction::Transaction as DbTransaction, user::{Balance, User}},
    state::SharedState,
    utils::error::AppError,
};

// ---------------------------------------------------------------------------
// GET /api/solana/blockhash
// Returns a fresh blockhash for the frontend to use when building transactions.
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct BlockhashResponse {
    pub blockhash: String,
}

pub async fn get_blockhash(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<BlockhashResponse>, AppError> {
    // Auth required — only logged-in users need blockhashes
    let _ = require_auth(&headers, &state.auth)?;

    let token_svc = state
        .solana
        .as_ref()
        .ok_or_else(|| AppError::BadRequest("Solana is not configured on this server".to_string()))?;

    let blockhash = token_svc
        .client()
        .get_latest_blockhash()
        .await
        .map_err(|e| {
            tracing::error!("get_blockhash: RPC failed: {}", e);
            AppError::InternalWithMessage(e.to_string())
        })?;

    Ok(Json(BlockhashResponse {
        blockhash: blockhash.to_string(),
    }))
}

// ---------------------------------------------------------------------------
// POST /api/solana/prepare-transfer
//
// Called by the frontend BEFORE building a transfer transaction.
// Ensures the recipient's Associated Token Account (ATA) exists on-chain.
// If it doesn't, the platform wallet creates it (pays ~0.002 SOL rent).
//
// Returns a fresh blockhash for the frontend to use when building the tx.
// This prevents the "not enough SOL" Phantom simulation error that occurs
// when the recipient ATA doesn't exist and the user would need to pay rent.
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct PrepareTransferRequest {
    pub recipient_wallet: String,
}

#[derive(Serialize)]
pub struct PrepareTransferResponse {
    pub blockhash: String,
    pub recipient_ata: String,
    pub ata_created: bool,
}

pub async fn prepare_transfer(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<PrepareTransferRequest>,
) -> Result<Json<PrepareTransferResponse>, AppError> {
    let _ = require_auth(&headers, &state.auth)?;

    let token_svc = state
        .solana
        .as_ref()
        .ok_or_else(|| AppError::BadRequest("Solana is not configured on this server".to_string()))?;

    let recipient_pubkey = Pubkey::from_str(&req.recipient_wallet)
        .map_err(|_| AppError::BadRequest("Invalid recipient wallet address".to_string()))?;

    let recipient_ata = token_svc.associated_token_address(&recipient_pubkey);

    // Check if the recipient ATA already exists
    let ata_exists = token_svc
        .client()
        .account_exists(&recipient_ata)
        .await
        .map_err(|e| {
            tracing::error!("prepare_transfer: account_exists check failed: {}", e);
            AppError::InternalWithMessage(e.to_string())
        })?;

    let mut ata_created = false;

    if !ata_exists {
        // Create the recipient's ATA — platform wallet pays the rent (~0.002 SOL)
        tracing::info!(
            "prepare_transfer: creating ATA {} for recipient {}",
            recipient_ata,
            req.recipient_wallet
        );

        let create_ata_ix =
            spl_associated_token_account::instruction::create_associated_token_account(
                &token_svc.client().payer.pubkey(),
                &recipient_pubkey,
                &token_svc.client().mint,
                &spl_token::id(),
            );

        let blockhash = token_svc.client().get_latest_blockhash().await.map_err(|e| {
            AppError::InternalWithMessage(format!("Failed to get blockhash: {}", e))
        })?;

        let tx = solana_sdk::transaction::Transaction::new_signed_with_payer(
            &[create_ata_ix],
            Some(&token_svc.client().payer.pubkey()),
            &[token_svc.client().payer.as_ref()],
            blockhash,
        );

        match token_svc.client().send_transaction(&tx).await {
            Ok(sig) => {
                tracing::info!("prepare_transfer: ATA created tx={}", sig);
                ata_created = true;
            }
            Err(e) => {
                tracing::error!("prepare_transfer: ATA creation failed: {}", e);
                return Err(AppError::InternalWithMessage(format!(
                    "Failed to create recipient token account: {}",
                    e
                )));
            }
        }
    }

    // Return a fresh blockhash for the frontend to use in the transfer tx
    let blockhash = token_svc
        .client()
        .get_latest_blockhash()
        .await
        .map_err(|e| AppError::InternalWithMessage(format!("Failed to get blockhash: {}", e)))?;

    Ok(Json(PrepareTransferResponse {
        blockhash: blockhash.to_string(),
        recipient_ata: recipient_ata.to_string(),
        ata_created,
    }))
}
//
// Receives a pre-signed Solana transaction from the frontend (Phantom wallet),
// submits it to the Solana RPC, then updates the DB accordingly.
//
// tx_type: "burn"     — user burned COIN for quiz/paper
// tx_type: "transfer" — user transferred COIN to another wallet
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct SubmitSignedTxRequest {
    /// Base64-encoded signed transaction bytes
    pub signed_tx: String,
    /// "burn" or "transfer"
    pub tx_type: String,
    /// Amount in COIN (not raw units)
    pub amount: i64,
    /// Human-readable purpose (e.g. "quiz_spend", "paper_spend")
    pub purpose: Option<String>,
    /// Recipient wallet address (required for transfer)
    pub recipient_wallet: Option<String>,
}

#[derive(Serialize)]
pub struct SubmitSignedTxResponse {
    pub solana_tx: String,
    pub new_balance: i64,
    pub note: String,
    pub fee_charged: Option<i64>,
}

pub async fn submit_signed_tx(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<SubmitSignedTxRequest>,
) -> Result<Json<SubmitSignedTxResponse>, AppError> {
    let user_id = require_auth(&headers, &state.auth)?;

    if req.amount <= 0 {
        return Err(AppError::BadRequest("amount must be > 0".to_string()));
    }

    let token_svc = state
        .solana
        .as_ref()
        .ok_or_else(|| AppError::BadRequest("Solana is not configured on this server".to_string()))?;

    // Decode the base64 transaction
    let tx_bytes = base64::engine::general_purpose::STANDARD
        .decode(&req.signed_tx)
        .map_err(|_| AppError::BadRequest("Invalid base64 transaction".to_string()))?;

    // Deserialize and verify the transaction
    let tx: Transaction = bincode::deserialize(&tx_bytes)
        .map_err(|_| AppError::BadRequest("Invalid transaction bytes".to_string()))?;

    // Verify the transaction is signed (has at least one signature)
    if tx.signatures.is_empty() || tx.signatures[0] == solana_sdk::signature::Signature::default() {
        return Err(AppError::BadRequest("Transaction is not signed".to_string()));
    }

    // Get the user's wallet address for verification
    let user = User::find_by_id(&state.db, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    let user_pubkey = Pubkey::from_str(&user.wallet_address)
        .map_err(|_| AppError::BadRequest("Invalid user wallet address".to_string()))?;

    // Verify the fee payer is the user's wallet (prevents spoofing)
    if tx.message.account_keys.is_empty() || tx.message.account_keys[0] != user_pubkey {
        return Err(AppError::Forbidden(
            "Transaction fee payer does not match your wallet".to_string(),
        ));
    }

    // Check DB balance before submitting
    let current_bal = Balance::get_by_user_id(&state.db, user_id)
        .await?
        .map(|b| b.token_balance)
        .unwrap_or(0);

    // No fee — sender pays exactly what they send
    let total_cost = req.amount;

    if current_bal < total_cost {
        return Err(AppError::Forbidden(format!(
            "Insufficient COIN. You have {} but need {}.",
            current_bal, total_cost
        )));
    }

    // Submit the signed transaction to Solana RPC
    let sig = token_svc
        .client()
        .send_transaction(&tx)
        .await
        .map_err(|e| {
            tracing::error!("submit_signed_tx: RPC submission failed: {}", e);
            AppError::InternalWithMessage(format!("Solana RPC error: {}", e))
        })?;

    tracing::info!(
        "submit_signed_tx: {} {} COIN for user={} tx={}",
        req.tx_type, req.amount, user_id, sig
    );

    // Update DB based on tx_type
    let new_balance = match req.tx_type.as_str() {
        "burn" => {
            // Deduct from sender
            let new_bal = current_bal - req.amount;
            Balance::set_balance(&state.db, user_id, new_bal).await?;

            let purpose = req.purpose.as_deref().unwrap_or("spend");
            let _ = DbTransaction::create(
                &state.db,
                Some(user_id),
                None,
                req.amount,
                purpose,
                None,
                Some(&format!("On-chain burn tx: {}", sig)),
            )
            .await;

            new_bal
        }
        "transfer" => {
            // Deduct exact amount from sender (no fee)
            let new_sender_bal = current_bal - req.amount;
            Balance::set_balance(&state.db, user_id, new_sender_bal).await?;

            // Credit recipient in DB and record transaction with both user IDs
            // so it appears in both sender's and recipient's history
            if let Some(ref recipient_wallet) = req.recipient_wallet {
                if let Ok(Some(recipient)) =
                    User::find_by_wallet(&state.db, recipient_wallet).await
                {
                    let recipient_bal = Balance::get_by_user_id(&state.db, recipient.id)
                        .await?
                        .map(|b| b.token_balance)
                        .unwrap_or(0);
                    Balance::set_balance(&state.db, recipient.id, recipient_bal + req.amount)
                        .await?;

                    // Record with both from/to so both users see it in history
                    let _ = DbTransaction::create(
                        &state.db,
                        Some(user_id),
                        Some(recipient.id),
                        req.amount,
                        "send",
                        None,
                        Some(&format!("On-chain transfer tx: {}", sig)),
                    )
                    .await;
                } else {
                    // Recipient not found in DB — record sender-only
                    let _ = DbTransaction::create(
                        &state.db,
                        Some(user_id),
                        None,
                        req.amount,
                        "send",
                        None,
                        Some(&format!("On-chain transfer tx: {}", sig)),
                    )
                    .await;
                }
            } else {
                let _ = DbTransaction::create(
                    &state.db,
                    Some(user_id),
                    None,
                    req.amount,
                    "send",
                    None,
                    Some(&format!("On-chain transfer tx: {}", sig)),
                )
                .await;
            }

            new_sender_bal
        }
        _ => {
            return Err(AppError::BadRequest(format!(
                "Unknown tx_type: {}",
                req.tx_type
            )));
        }
    };

    let note = format!(
        "On-chain {} of {} COIN. Solana tx: {}",
        req.tx_type, req.amount, sig
    );

    Ok(Json(SubmitSignedTxResponse {
        solana_tx: sig,
        new_balance,
        note,
        fee_charged: None,
    }))
}
