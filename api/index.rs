use std::sync::Arc;

use axum::Router;
use backend_rust::{
    config::Settings,
    database::{postgres::create_pool, redis::create_redis_client},
    middleware::{cors::build_cors, logging::build_trace},
    routes::api::build_api_router,
    services::{ai_client::AiClient, auth_service::AuthService, file_storage::StorageProvider},
    solana::{client::SolanaClient, token::TokenService},
    state::{AppState, SharedState},
};
use tokio::sync::OnceCell;
use tower::ServiceExt;
use vercel_runtime::{run_service, service_fn, Error, Request};

static APP_STATE: OnceCell<SharedState> = OnceCell::const_new();
static LOG_INIT: OnceCell<()> = OnceCell::const_new();

#[tokio::main]
async fn main() -> Result<(), Error> {
    run_service(service_fn(handler)).await
}

async fn handler(req: Request) -> Result<axum::response::Response, Error> {
    init_logs().await;
    let state = get_state().await?;

    let app: Router = build_api_router(state.clone())
        .layer(build_cors(&state.settings))
        .layer(build_trace());

    let response = app.oneshot(req).await?;
    Ok(response)
}

async fn init_logs() {
    let _ = LOG_INIT
        .get_or_init(|| async {
            let _ = tracing_subscriber::fmt()
                .with_env_filter("info,backend_rust=debug")
                .try_init();
        })
        .await;
}

async fn get_state() -> Result<SharedState, Error> {
    let state = APP_STATE
        .get_or_try_init(|| async {
            let settings = Settings::from_env()?;

            let db = create_pool(&settings).await?;
            sqlx::migrate!("./migrations").run(&db).await?;

            let redis = match create_redis_client(&settings).await {
                Ok(client) => Some(client),
                Err(e) => {
                    tracing::warn!("Redis unavailable, rate limiting disabled: {}", e);
                    None
                }
            };

            let solana = if is_solana_configured(&settings) {
                match SolanaClient::from_settings(&settings) {
                    Ok(client) => Some(TokenService::new(client)),
                    Err(e) => {
                        tracing::warn!("Solana unavailable, DB-only mode: {}", e);
                        None
                    }
                }
            } else {
                None
            };

            let state = Arc::new(AppState {
                settings: settings.clone(),
                db,
                redis,
                auth: AuthService::new(&settings),
                ai_client: AiClient::new(&settings),
                storage: StorageProvider::from_settings(&settings),
                solana,
            });

            Ok::<SharedState, anyhow::Error>(state)
        })
        .await?;

    Ok(state.clone())
}

fn is_solana_configured(settings: &Settings) -> bool {
    let key = &settings.solana_wallet_private_key;
    let mint = &settings.solana_token_mint_address;

    !key.is_empty()
        && key != "your_private_key_base58"
        && !mint.is_empty()
        && mint != "your_token_mint_address"
}
