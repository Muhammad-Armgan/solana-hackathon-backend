/// Integration tests for /api/token/* endpoints.
///
/// Run with:
///   cargo test --test token_tests
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

mod common;
use common::build_test_app;

// ---------------------------------------------------------------------------
// GET /api/token/balance
// ---------------------------------------------------------------------------

#[tokio::test]
async fn balance_without_auth_returns_401() {
    let app = build_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/token/balance")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn balance_returns_non_negative_integer() {
    let app = build_test_app().await;
    let token = common::register_and_get_token(&app).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/token/balance")
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap();

    let balance = json["balance"].as_i64().expect("balance must be an integer");
    assert!(balance >= 0, "balance must be non-negative");
}

// ---------------------------------------------------------------------------
// POST /api/token/send
// ---------------------------------------------------------------------------

#[tokio::test]
async fn send_tokens_without_auth_returns_401() {
    let app = build_test_app().await;

    let body = json!({ "recipient_wallet": "SomeWallet", "amount": 5 });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/token/send")
                .header("Content-Type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn send_tokens_zero_amount_returns_400() {
    let app = build_test_app().await;
    let token = common::register_and_get_token(&app).await;

    let body = json!({ "recipient_wallet": "SomeWallet", "amount": 0 });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/token/send")
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn send_tokens_to_unregistered_wallet_returns_404() {
    let app = build_test_app().await;
    let token = common::register_and_get_token(&app).await;

    let body = json!({
        "recipient_wallet": "UnknownWallet111111111111111111111111111111",
        "amount": 1
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/token/send")
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn send_tokens_insufficient_balance_returns_403() {
    let app = build_test_app().await;
    let token = common::register_zero_balance_user(&app).await;

    let body = json!({
        "recipient_wallet": "AnyWallet111111111111111111111111111111111",
        "amount": 100
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/token/send")
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

// ---------------------------------------------------------------------------
// GET /api/token/history
// ---------------------------------------------------------------------------

#[tokio::test]
async fn token_history_returns_transactions_and_transfers() {
    let app = build_test_app().await;
    let token = common::register_and_get_token(&app).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/token/history?limit=20&offset=0")
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap();

    assert!(json.get("transactions").is_some(), "must have transactions array");
    assert!(json.get("sends_and_receives").is_some(), "must have sends_and_receives array");
}

// ---------------------------------------------------------------------------
// POST /api/token/buy
// ---------------------------------------------------------------------------

#[tokio::test]
async fn buy_tokens_zero_usd_returns_400() {
    let app = build_test_app().await;
    let token = common::register_and_get_token(&app).await;

    let body = json!({ "usd_amount": 0 });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/token/buy")
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn buy_tokens_returns_checkout_url_and_credited_amount() {
    let app = build_test_app().await;
    let token = common::register_and_get_token(&app).await;

    let body = json!({ "usd_amount": 2 });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/token/buy")
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", token))
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

    assert!(json["checkout_url"].as_str().is_some());
    // 2 USD × 5 COIN/USD = 10 COIN
    assert_eq!(json["credited_tokens"].as_i64(), Some(10));
}
