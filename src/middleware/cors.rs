use axum::http::{HeaderValue, Method};
use tower_http::cors::CorsLayer;

use crate::config::Settings;

pub fn build_cors(settings: &Settings) -> CorsLayer {
    // Always allow the configured FRONTEND_URL.
    // Also always allow localhost:3001 so local dev works without env changes.
    let mut origins: Vec<HeaderValue> = vec![];

    for raw in [
        settings.frontend_url.as_str(),
        "http://localhost:3001",
        "http://127.0.0.1:3001",
    ] {
        if let Ok(v) = raw.parse::<HeaderValue>() {
            if !origins.contains(&v) {
                origins.push(v);
            }
        }
    }

    CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers(tower_http::cors::Any)
}
