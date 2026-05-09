/// Integration tests for /api/quiz/* endpoints.
///
/// Run with:
///   cargo test --test quiz_tests
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

mod common;
use common::build_test_app;

// ---------------------------------------------------------------------------
// POST /api/quiz/generate
// ---------------------------------------------------------------------------

#[tokio::test]
async fn generate_quiz_without_auth_returns_401() {
    let app = build_test_app().await;

    let body = json!({ "subject": "Mathematics" });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/quiz/generate")
                .header("Content-Type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn generate_quiz_empty_subject_returns_400() {
    let app = build_test_app().await;
    let token = common::register_and_get_token(&app).await;

    let body = json!({ "subject": "   " }); // whitespace-only subject

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/quiz/generate")
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
async fn generate_quiz_insufficient_balance_returns_403() {
    let app = build_test_app().await;
    // Register a fresh user with 0 balance (no signup bonus applied in this helper).
    let token = common::register_zero_balance_user(&app).await;

    let body = json!({ "subject": "Physics" });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/quiz/generate")
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
// GET /api/quiz/history
// ---------------------------------------------------------------------------

#[tokio::test]
async fn quiz_history_without_auth_returns_401() {
    let app = build_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/quiz/history")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn quiz_history_returns_paginated_list() {
    let app = build_test_app().await;
    let token = common::register_and_get_token(&app).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/quiz/history?limit=10&offset=0")
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

    assert!(json.get("items").is_some(), "response must contain items array");
    assert!(json.get("limit").is_some());
    assert!(json.get("offset").is_some());
}

// ---------------------------------------------------------------------------
// POST /api/quiz/submit
// ---------------------------------------------------------------------------

#[tokio::test]
async fn submit_quiz_nonexistent_id_returns_404() {
    let app = build_test_app().await;
    let token = common::register_and_get_token(&app).await;

    let body = json!({
        "quiz_id": "00000000-0000-0000-0000-000000000000",
        "answers": [0, 1, 2],
        "score": 80.0
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/quiz/submit")
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
async fn submit_quiz_invalid_score_returns_400() {
    let app = build_test_app().await;
    let token = common::register_and_get_token(&app).await;

    let body = json!({
        "quiz_id": "00000000-0000-0000-0000-000000000000",
        "answers": [],
        "score": 150.0  // out of range
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/quiz/submit")
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
