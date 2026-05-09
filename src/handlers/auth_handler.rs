use axum::{extract::State, http::HeaderMap, Json};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use uuid::Uuid;

use crate::{
    middleware::auth::require_auth,
    models::{transaction::Transaction, user::{Balance, User}},
    state::SharedState,
    utils::error::AppError,
};

#[derive(Deserialize)]
pub struct SignupRequest {
    pub wallet_address: String,
    pub email: Option<String>,
    pub signed_message: String,
    pub signature: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub wallet_address: String,
    pub signed_message: String,
    pub signature: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub user_id: Uuid,
    pub token: String,
    /// Solana transaction signature for the signup bonus mint, if on-chain.
    pub solana_tx: Option<String>,
}

pub async fn signup(
    State(state): State<SharedState>,
    Json(req): Json<SignupRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    tracing::info!("signup: wallet={}", &req.wallet_address[..8.min(req.wallet_address.len())]);

    // Step 1 — Verify wallet signature
    state
        .auth
        .verify_wallet_login(&req.wallet_address, &req.signed_message, &req.signature)
        .map_err(|e| {
            tracing::warn!("signup: signature verification failed: {:?}", e);
            e
        })?;

    tracing::info!("signup: signature verified");

    // Step 2 — Find or create user
    let user = match User::find_by_wallet(&state.db, &req.wallet_address)
        .await
        .map_err(|e| { tracing::error!("signup: find_by_wallet failed: {:?}", e); AppError::from(e) })?
    {
        Some(u) => {
            tracing::info!("signup: existing user id={}", u.id);
            u
        }
        None => {
            tracing::info!("signup: creating new user");
            let created = User::create(&state.db, &req.wallet_address, req.email.as_deref())
                .await
                .map_err(|e| { tracing::error!("signup: User::create failed: {:?}", e); AppError::from(e) })?;
            Balance::create_if_missing(&state.db, created.id)
                .await
                .map_err(|e| { tracing::error!("signup: Balance::create_if_missing failed: {:?}", e); AppError::from(e) })?;
            created
        }
    };

    // Step 3 — Grant signup bonus (once per wallet)
    let mut solana_tx: Option<String> = None;

    let mut balance = Balance::get_by_user_id(&state.db, user.id)
        .await
        .map_err(|e| { tracing::error!("signup: get_by_user_id failed: {:?}", e); AppError::from(e) })?
        .map(|b| b.token_balance)
        .unwrap_or(0);

    if !user.signup_bonus_granted {
        tracing::info!("signup: granting 20 COIN bonus to user={}", user.id);

        // --- Try on-chain mint first ---
        if let Some(ref token_svc) = state.solana {
            match Pubkey::from_str(&req.wallet_address) {
                Ok(pubkey) => {
                    // COIN has 2 decimals → 20 COIN = 2000 raw units
                    match token_svc.mint_tokens_to_user(&pubkey, 2000).await {
                        Ok(sig) => {
                            tracing::info!("signup: on-chain mint tx={}", sig);
                            solana_tx = Some(sig);
                        }
                        Err(e) => {
                            tracing::warn!(
                                "signup: on-chain mint failed ({}), falling back to DB-only",
                                e
                            );
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("signup: invalid pubkey for on-chain mint: {}", e);
                }
            }
        }

        // --- Always update DB balance (source of truth for fast reads) ---
        balance += 20;
        Balance::set_balance(&state.db, user.id, balance)
            .await
            .map_err(|e| { tracing::error!("signup: set_balance failed: {:?}", e); AppError::from(e) })?;
        User::mark_signup_bonus_granted(&state.db, user.id)
            .await
            .map_err(|e| { tracing::error!("signup: mark_signup_bonus_granted failed: {:?}", e); AppError::from(e) })?;
        let _ = Transaction::create(
            &state.db,
            None,
            Some(user.id),
            20,
            "signup_bonus",
            None,
            Some(solana_tx.as_deref().unwrap_or("DB-only: Solana not configured")),
        )
        .await;
    }

    // Step 4 — Issue JWT
    let token = state.auth.issue_jwt(user.id, &user.wallet_address)
        .map_err(|e| { tracing::error!("signup: issue_jwt failed: {:?}", e); e })?;

    tracing::info!("signup: success user={}", user.id);
    Ok(Json(AuthResponse { user_id: user.id, token, solana_tx }))
}

pub async fn login(
    State(state): State<SharedState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    tracing::info!("login: wallet={}", &req.wallet_address[..8.min(req.wallet_address.len())]);

    state
        .auth
        .verify_wallet_login(&req.wallet_address, &req.signed_message, &req.signature)
        .map_err(|e| {
            tracing::warn!("login: signature verification failed: {:?}", e);
            e
        })?;

    let user = User::find_by_wallet(&state.db, &req.wallet_address)
        .await
        .map_err(|e| { tracing::error!("login: find_by_wallet failed: {:?}", e); AppError::from(e) })?
        .ok_or_else(|| AppError::NotFound("User is not registered. Please sign up first.".to_string()))?;

    let token = state.auth.issue_jwt(user.id, &user.wallet_address)
        .map_err(|e| { tracing::error!("login: issue_jwt failed: {:?}", e); e })?;

    tracing::info!("login: success user={}", user.id);
    Ok(Json(AuthResponse { user_id: user.id, token, solana_tx: None }))
}

#[derive(Serialize)]
pub struct MeResponse {
    pub user: User,
    pub balance: i64,
}

pub async fn me(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<MeResponse>, AppError> {
    let user_id = require_auth(&headers, &state.auth)?;
    let user = User::find_by_id(&state.db, user_id)
        .await
        .map_err(|e| { tracing::error!("me: find_by_id failed: {:?}", e); AppError::from(e) })?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;
    let balance = Balance::get_by_user_id(&state.db, user_id)
        .await
        .map_err(|e| { tracing::error!("me: get_by_user_id failed: {:?}", e); AppError::from(e) })?
        .map(|b| b.token_balance)
        .unwrap_or(0);

    Ok(Json(MeResponse { user, balance }))
}
