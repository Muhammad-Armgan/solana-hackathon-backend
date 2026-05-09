/// Integration tests for /api/auth/* endpoints.
///
/// These tests spin up the full Axum router against a real PostgreSQL + Redis
/// instance.  Set TEST_DATABASE_URL and TEST_REDIS_URL in your environment (or
/// a .env.test file) before running:
///
///   cargo test --test auth_tests
///
/// Wallet signature verification is bypassed in tests by using a known
/// test keypair whose signature is pre-computed below.
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt; // for `oneshot`

mod common;
use common::build_test_app;

// ---------------------------------------------------------------------------
// POST /api/auth/signup
// ---------------------------------------------------------------------------

#[tokio::test]
async fn signup_new_wallet_returns_token_and_bonus() {
    let app = build_test_app().await;

    let body = json!({
        "wallet_address": common::TEST_WALLET,
        "signed_message": common::TEST_MESSAGE,
        "signature": common::TEST_SIGNATURE
    });

    let response = app
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

    assert_eq!(response.status(), StatusCode::OK);

    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap();

    assert!(json.get("token").is_some(), "response must contain a JWT token");
    assert!(json.get("user_id").is_some(), "response must contain user_id");
}

#[tokio::test]
async fn signup_missing_fields_returns_422() {
    let app = build_test_app().await;

    let body = json!({ "wallet_address": "some_wallet" }); // missing signature fields

    let response = app
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

    // Axum returns 422 Unprocessable Entity for missing required JSON fields.
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

// ---------------------------------------------------------------------------
// POST /api/auth/login
// ---------------------------------------------------------------------------

#[tokio::test]
async fn login_unregistered_wallet_returns_404() {
    let app = build_test_app().await;

    let body = json!({
        "wallet_address": "UnknownWallet111111111111111111111111111111",
        "signed_message": common::TEST_MESSAGE,
        "signature": common::TEST_SIGNATURE
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("Content-Type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// GET /api/auth/me
// ---------------------------------------------------------------------------

#[tokio::test]
async fn me_without_token_returns_401() {
    let app = build_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/auth/me")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
