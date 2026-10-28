// SPDX-License-Identifier: AGPL-3.0-or-later

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;

use notenwart::app::{AppState, router};

/// A pool that never connects: enough for routes that must work without a database.
fn offline_state() -> AppState {
    let pool = PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_millis(200))
        .connect_lazy("postgres://notenwart@127.0.0.1:1/notenwart")
        .unwrap();
    AppState {
        pool,
        store: std::sync::Arc::new(object_store::memory::InMemory::new()),
        config: std::sync::Arc::new(notenwart::app::Config {
            public_url: "http://notenwart.test".into(),
            secure_cookies: false,
            max_upload: 1024,
            language: notenwart::mail::Language::En,
            storage: "memory",
        }),
        mailer: None,
        limiter: Default::default(),
        recognizer: notenwart::recognize::Recognizer::disabled(),
        sso: None,
    }
}

async fn get(uri: &str) -> (StatusCode, Vec<u8>) {
    let response = router(offline_state())
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes().to_vec();
    (status, body)
}

#[tokio::test]
async fn health_reports_missing_database() {
    let (status, body) = get("/api/health").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["database"], "unavailable");
    assert_eq!(json["version"], env!("CARGO_PKG_VERSION"));
}

#[tokio::test]
async fn unknown_api_route_is_not_found() {
    let (status, _) = get("/api/nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn missing_asset_is_not_found() {
    let (status, _) = get("/no-such-file.js").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
