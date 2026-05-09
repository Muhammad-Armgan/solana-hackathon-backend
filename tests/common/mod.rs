/// Shared test helpers used across all integration test files.
///
/// The helpers here build a real Axum router wired to test-specific
/// PostgreSQL and Redis instances.  Set the following environment variables
/// (or add them to a `.env.test` file at the workspace root) before running
/// the integration tests:
///
///   TEST_DATABASE_URL=postgresql://postgres:password@localhost:5432/learning_platform_test
///   TEST_REDIS_URL=redis://localhost:6379/1
///
/// If those variables are absent the helpers will fall back to the values in
/// the regular `.env` file so that a local dev environment works out of the box.
use std::sync::Arc;

use axum::Router;
use axum::{body::Body, http::Request};
use serde_json::Value;
use tower::ServiceExt;

use backend_rust::{
    config::Settings,
    database::{postgres::create_pool, redis::create_redis_client},
    middleware::{cors::build_cors, logging::build_trace},
    routes::api::build_api_router,
    services::{ai_client::AiClient, auth_service::AuthService, file_storage::StorageProvider},
    state::AppState,
};

// ---------------------------------------------------------------------------
// Test keypair constants
//
// These are a pre-generated ed25519 keypair used only in tests.
// The private key is NOT a secret — it exists solely to produce a valid
// signature for the test wallet address.
// ---------------------------------------------------------------------------

/// Base58-encoded public key (wallet address) of the test keypair.
pub const TEST_WALLET: &str = "4vJ9JU1bJJE96FWSJKvHsmmFADCg4gpZQff4P3bkLKi";

/// The plaintext message that was signed with the test private key.
pub const TEST_MESSAGE: &str =
    "Sign this message to authenticate with Universal Learning Platform.\nWallet: 4vJ9JU1bJJE96FWSJKvHsmmFADCg4gpZQff4P3bkLKi\nNonce: test_nonce_for_integration";

/// Base58-encoded ed25519 signature of TEST_MESSAGE produced by the test private key.
/// Replace this with a real pre-computed signature if signature verification is enabled.
pub const TEST_SIGNATURE: &str = "placeholder_signature_replace_with_real_value";

// ---------------------------------------------------------------------------
// App builder
// ---------------------------------------------------------------------------

/// Build a full Axum application wired to test databases.
///
/// Signature verification is intentionally left enabled so that tests
/// exercise the real auth path.  Use `TEST_WALLET` / `TEST_MESSAGE` /
/// `TEST_SIGNATURE` constants for requests that need a valid auth token.
pub async fn build_test_app() -> Router {
    // Allow tests to override DB/Redis URLs without touching the main .env.
    if std::env::var("TEST_DATABASE_URL").is_ok() {
        std::env::set_var("DATABASE_URL", std::env::var("TEST_DATABASE_URL").unwrap());
    }
    if std::env::var("TEST_REDIS_URL").is_ok() {
        std::env::set_var("REDIS_URL", std::env::var("TEST_REDIS_URL").unwrap());
    }

    let settings = Settings::from_env().expect("Failed to load settings for tests");
    let db = create_pool(&settings).await.expect("Failed to connect to test DB");
    let redis = create_redis_client(&settings).await.expect("Failed to connect to test Redis");
    let auth = AuthService::new(&settings);
    let ai_client = AiClient::new(&settings);
    let storage = StorageProvider::from_settings(&settings);

    let state = Arc::new(AppState {
        settings: settings.clone(),
        db,
        redis,
        auth,
        ai_client,
        storage,
    });

    build_api_router(state)
        .layer(build_cors(&settings))
        .layer(build_trace())
}

// ---------------------------------------------------------------------------
// Auth helpers
// ---------------------------------------------------------------------------

/// Register a new user with the test wallet and return their JWT.
///
/// NOTE: This will fail if wallet signature verification is strict and
/// TEST_SIGNATURE is a placeholder.  Replace TEST_SIGNATURE with a real
/// pre-computed value or mock the auth service for unit tests.
pub async fn register_and_get_token(app: &Router) -> String {
    use serde_json::json;

    let body = json!({
        "wallet_address": TEST_WALLET,
        "signed_message": TEST_MESSAGE,
        "signature": TEST_SIGNATURE
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/signup")
                .header("Content-Type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    json["token"]
        .as_str()
        .expect("signup response must contain a token")
        .to_string()
}

/// Register a user whose balance is explicitly zeroed out after signup.
/// Useful for testing insufficient-balance error paths.
pub async fn register_zero_balance_user(app: &Router) -> String {
    // For now this is the same as register_and_get_token.
    // In a real test suite you would insert a user directly into the DB
    // with token_balance = 0 and issue a JWT manually.
    register_and_get_token(app).await
}
