use axum::{extract::{Query, State}, http::HeaderMap, Json};
use serde::{Deserialize, Serialize};
use crate::{
    middleware::auth::require_auth,
    models::{transaction::{TokenTransfer, Transaction}, user::{Balance, User}},
    state::SharedState,
    utils::error::AppError,
};

// ---------------------------------------------------------------------------
// Token economics constants
// ---------------------------------------------------------------------------

// No transfer fee — sender pays exactly what they send, recipient gets full amount.
const TRANSFER_FEE: i64 = 0;

// ---------------------------------------------------------------------------
// GET /api/token/balance
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct BalanceResponse {
    pub balance: i64,
}

pub async fn token_balance(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<BalanceResponse>, AppError> {
    let user_id = require_auth(&headers, &state.auth)?;

    // Try to read on-chain balance and sync to DB if Solana is configured.
    //
    // Sync rule:
    //   - If on-chain > DB: someone received a mint we didn't record → sync UP
    //   - If on-chain < DB: user spent COIN (DB deduction) that isn't reflected
    //     on-chain (because burns require user signature) → keep DB value
    //   - If equal: no sync needed
    //
    // This means DB is always the authoritative lower bound for spends,
    // and on-chain is authoritative for credits.
    if let Some(ref token_svc) = state.solana {
        use solana_sdk::pubkey::Pubkey;
        use std::str::FromStr;

        if let Ok(Some(user)) = crate::models::user::User::find_by_id(&state.db, user_id).await {
            if let Ok(pubkey) = Pubkey::from_str(&user.wallet_address) {
                match token_svc.get_coin_balance(&pubkey).await {
                    Ok(on_chain_bal) => {
                        let db_bal = Balance::get_by_user_id(&state.db, user_id)
                            .await
                            .ok()
                            .flatten()
                            .map(|b| b.token_balance)
                            .unwrap_or(0);

                        if on_chain_bal > db_bal {
                            // On-chain has MORE than DB — a mint happened that
                            // wasn't recorded in DB (e.g. direct wallet transfer).
                            // Sync DB up to match on-chain.
                            tracing::info!(
                                "balance sync UP: user={} on_chain={} db={} → setting db={}",
                                user_id, on_chain_bal, db_bal, on_chain_bal
                            );
                            let _ = Balance::set_balance(&state.db, user_id, on_chain_bal).await;
                            return Ok(Json(BalanceResponse { balance: on_chain_bal }));
                        }

                        // on_chain_bal <= db_bal:
                        // DB has MORE than on-chain — this is expected because
                        // quiz/paper spends deduct from DB but can't burn on-chain.
                        // Return DB value (the authoritative spend-tracking value).
                        return Ok(Json(BalanceResponse { balance: db_bal }));
                    }
                    Err(e) => {
                        tracing::warn!("balance: on-chain read failed ({}), using DB", e);
                    }
                }
            }
        }
    }

    // Fallback: DB balance
    let bal = Balance::get_by_user_id(&state.db, user_id)
        .await?
        .map(|b| b.token_balance)
        .unwrap_or(0);
    Ok(Json(BalanceResponse { balance: bal }))
}

// ---------------------------------------------------------------------------
// POST /api/token/send
//
// Rules:
//   - Sender must have `amount` COIN
//   - No fee — sender pays exactly what they send
//   - On-chain: mint `amount` to recipient's ATA (server is mint authority)
//   - DB: deduct `amount` from sender, credit `amount` to recipient
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct SendTokenRequest {
    pub recipient_wallet: String,
    pub amount: i64,
}

#[derive(Serialize)]
pub struct SendTokenResponse {
    pub transfer: TokenTransfer,
    pub fee_charged: i64,
    pub solana_tx: Option<String>,
    pub on_chain_status: String,
}

pub async fn token_send(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<SendTokenRequest>,
) -> Result<Json<SendTokenResponse>, AppError> {
    let sender_id = require_auth(&headers, &state.auth)?;

    if req.amount <= 0 {
        return Err(AppError::BadRequest("Amount must be > 0".to_string()));
    }

    // Validate recipient is registered
    let recipient = User::find_by_wallet(&state.db, &req.recipient_wallet)
        .await?
        .ok_or_else(|| {
            AppError::NotFound("Recipient wallet is not registered on this platform".to_string())
        })?;

    // Prevent sending to yourself
    if recipient.id == sender_id {
        return Err(AppError::BadRequest("Cannot send COIN to yourself".to_string()));
    }

    // Check sender DB balance — must cover exact amount (no fee)
    let sender_bal = Balance::get_by_user_id(&state.db, sender_id)
        .await?
        .map(|b| b.token_balance)
        .unwrap_or(0);

    if sender_bal < req.amount {
        return Err(AppError::Forbidden(format!(
            "Insufficient COIN. You have {} COIN but need {}.",
            sender_bal, req.amount
        )));
    }

    // -----------------------------------------------------------------------
    // On-chain: mint `amount` to recipient's ATA
    // -----------------------------------------------------------------------
    let mut solana_tx: Option<String> = None;
    let mut on_chain_status = "DB-only (Solana not configured)".to_string();

    if let Some(ref token_svc) = state.solana {
        use solana_sdk::pubkey::Pubkey;
        use std::str::FromStr;

        match Pubkey::from_str(&req.recipient_wallet) {
            Ok(recipient_pubkey) => {
                // 1 COIN = 100 raw units (2 decimals)
                let raw_amount = (req.amount as u64) * 100;

                tracing::info!(
                    "token_send: minting {} COIN ({} raw) to {}",
                    req.amount, raw_amount, req.recipient_wallet
                );

                match token_svc.mint_tokens_to_user(&recipient_pubkey, raw_amount).await {
                    Ok(sig) => {
                        tracing::info!("token_send: on-chain mint tx={}", sig);
                        on_chain_status = format!("On-chain mint tx: {}", sig);
                        solana_tx = Some(sig);
                    }
                    Err(e) => {
                        tracing::error!(
                            "token_send: on-chain mint failed — {}. DB balances still updated.",
                            e
                        );
                        on_chain_status = format!("On-chain failed ({}), DB updated", e);
                    }
                }
            }
            Err(e) => {
                tracing::warn!("token_send: invalid recipient pubkey: {}", e);
                on_chain_status = "Invalid recipient pubkey, DB-only".to_string();
            }
        }
    }

    // -----------------------------------------------------------------------
    // DB: deduct `amount` from sender, credit `amount` to recipient
    // -----------------------------------------------------------------------
    let receiver_bal = Balance::get_by_user_id(&state.db, recipient.id)
        .await?
        .map(|b| b.token_balance)
        .unwrap_or(0);

    Balance::set_balance(&state.db, sender_id, sender_bal - req.amount).await?;
    Balance::set_balance(&state.db, recipient.id, receiver_bal + req.amount).await?;

    // Record the transfer
    let tx = Transaction::create(
        &state.db,
        Some(sender_id),
        Some(recipient.id),
        req.amount,
        "send",
        None,
        Some(&on_chain_status),
    )
    .await?;

    let transfer = TokenTransfer::create(
        &state.db,
        sender_id,
        recipient.id,
        req.amount,
        Some(tx.id),
    )
    .await?;

    Ok(Json(SendTokenResponse {
        transfer,
        fee_charged: TRANSFER_FEE,
        solana_tx,
        on_chain_status,
    }))
}

// ---------------------------------------------------------------------------
// GET /api/token/history
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct Paging {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Serialize)]
pub struct TokenHistoryResponse {
    pub transactions: Vec<Transaction>,
    pub sends_and_receives: Vec<TokenTransfer>,
}

pub async fn token_history(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(paging): Query<Paging>,
) -> Result<Json<TokenHistoryResponse>, AppError> {
    let user_id = require_auth(&headers, &state.auth)?;
    let limit = paging.limit.unwrap_or(20);
    let offset = paging.offset.unwrap_or(0);

    let txs = Transaction::history_by_user(&state.db, user_id, limit, offset).await?;
    let moves =
        TokenTransfer::send_receive_history_by_user(&state.db, user_id, limit, offset).await?;

    Ok(Json(TokenHistoryResponse {
        transactions: txs,
        sends_and_receives: moves,
    }))
}

// ---------------------------------------------------------------------------
// POST /api/token/buy  (PayPal placeholder + on-chain mint)
//
// Mints `tokens` COIN to user's wallet on-chain and credits DB.
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct BuyTokenRequest {
    pub usd_amount: i64,
}

#[derive(Serialize)]
pub struct BuyTokenResponse {
    pub checkout_url: String,
    pub credited_tokens: i64,
    pub note: String,
    pub solana_tx: Option<String>,
}

pub async fn token_buy(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<BuyTokenRequest>,
) -> Result<Json<BuyTokenResponse>, AppError> {
    let user_id = require_auth(&headers, &state.auth)?;

    if req.usd_amount <= 0 {
        return Err(AppError::BadRequest("usd_amount must be > 0".to_string()));
    }

    let tokens = req.usd_amount * 5; // 5 COIN per $1 USD

    // Get user wallet for on-chain mint
    let user = User::find_by_id(&state.db, user_id)
        .await?
        .ok_or(AppError::NotFound("User not found".to_string()))?;

    // On-chain mint
    let mut solana_tx: Option<String> = None;
    if let Some(ref token_svc) = state.solana {
        use solana_sdk::pubkey::Pubkey;
        use std::str::FromStr;

        if let Ok(pubkey) = Pubkey::from_str(&user.wallet_address) {
            let raw_amount = (tokens as u64) * 100; // 2 decimals
            match token_svc.mint_tokens_to_user(&pubkey, raw_amount).await {
                Ok(sig) => {
                    tracing::info!("token_buy: on-chain mint {} COIN tx={}", tokens, sig);
                    solana_tx = Some(sig);
                }
                Err(e) => {
                    tracing::error!("token_buy: on-chain mint failed — {}", e);
                }
            }
        }
    }

    // DB update — always runs regardless of on-chain result
    let bal = Balance::get_by_user_id(&state.db, user_id)
        .await?
        .map(|b| b.token_balance)
        .unwrap_or(0);
    Balance::set_balance(&state.db, user_id, bal + tokens).await?;

    let note = solana_tx
        .as_deref()
        .map(|sig| format!("PayPal placeholder + on-chain mint tx: {}", sig))
        .unwrap_or_else(|| "PayPal placeholder credit (DB-only)".to_string());

    let _ = Transaction::create(&state.db, None, Some(user_id), tokens, "buy", None, Some(&note))
        .await;

    Ok(Json(BuyTokenResponse {
        checkout_url: format!("https://www.sandbox.paypal.com/checkoutnow?user={}", user_id),
        credited_tokens: tokens,
        note,
        solana_tx,
    }))
}
