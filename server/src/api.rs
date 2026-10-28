// SPDX-License-Identifier: AGPL-3.0-or-later

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use crate::app::AppState;

pub fn router(max_upload: usize) -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .merge(crate::auth::router())
        .merge(crate::people::router())
        .merge(crate::catalogue::router())
        .merge(crate::loans::router())
        .merge(crate::privacy::router())
        .merge(crate::sso::router())
        .merge(crate::notify::router())
        .merge(crate::guests::router())
        .merge(crate::images::router())
        .merge(crate::system::router())
        .merge(crate::recordings::router(max_upload))
        .merge(crate::import::router())
        .merge(crate::ensembles::router())
        .merge(crate::programs::router())
        .merge(crate::annotations::router())
        .merge(crate::library::router(max_upload))
        .fallback(|| async { StatusCode::NOT_FOUND })
}

#[derive(Serialize)]
pub struct Health {
    pub status: &'static str,
    pub version: &'static str,
    pub database: &'static str,
}

async fn health(State(state): State<AppState>) -> (StatusCode, Json<Health>) {
    let database_ok = sqlx::query("SELECT 1").execute(&state.pool).await.is_ok();
    let code = if database_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    let health = Health {
        status: if database_ok { "ok" } else { "degraded" },
        version: env!("CARGO_PKG_VERSION"),
        database: if database_ok { "ok" } else { "unavailable" },
    };
    (code, Json(health))
}
