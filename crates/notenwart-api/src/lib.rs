// SPDX-License-Identifier: AGPL-3.0-or-later

//! The open API of notenwart: HTTP and JSON under `/api`, described in `api/openapi.yaml` (the contract, MIT), and
//! the web app around it, embedded into the binary. Like every interface, it works through `notenwart-app` only: a
//! handler takes the request apart (path, query, JSON, multipart, cookies), finds the caller ([`auth::SignedIn`])
//! and calls a command, then turns the answer into a status, headers and JSON. No queries and no rules here.

mod annotations;
pub mod auth;
mod catalogue;
mod ensembles;
pub mod error;
mod files;
mod guests;
mod images;
mod import;
mod library;
mod loans;
mod notify;
mod people;
mod privacy;
mod programs;
mod recordings;
mod sso;
mod system;
mod web;

use axum::extract::{Request, State};
use axum::http::{Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use notenwart_app::App;
use serde::Serialize;
use tower_http::compression::CompressionLayer;
use tower_http::compression::predicate::{NotForContentType, Predicate, SizeAbove};
use tower_http::trace::TraceLayer;

/// `/api/*` is the JSON API (see `api/openapi.yaml`); everything else is the web app.
pub fn router(state: App) -> Router {
    Router::new()
        .nest("/api", api(state.config.max_upload))
        .fallback(web::serve)
        .layer(middleware::from_fn(same_origin))
        // JSON and the web app shrink a lot; PDFs, audio and images are compressed already.
        .layer(
            CompressionLayer::new().compress_when(
                SizeAbove::new(1024)
                    .and(NotForContentType::new("application/pdf"))
                    .and(NotForContentType::new("audio/"))
                    .and(NotForContentType::new("image/")),
            ),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Serves the API and the web app on `listener` until `shutdown` completes.
pub async fn serve(
    listener: tokio::net::TcpListener,
    state: App,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    axum::serve(listener, router(state)).with_graceful_shutdown(shutdown).await
}

fn api(max_upload: usize) -> Router<App> {
    Router::new()
        .route("/health", get(health))
        .merge(auth::router())
        .merge(people::router())
        .merge(catalogue::router())
        .merge(loans::router())
        .merge(privacy::router())
        .merge(sso::router())
        .merge(notify::router())
        .merge(guests::router())
        .merge(images::router())
        .merge(system::router())
        .merge(recordings::router(max_upload))
        .merge(import::router())
        .merge(ensembles::router())
        .merge(programs::router())
        .merge(annotations::router())
        .merge(library::router(max_upload))
        .fallback(|| async { StatusCode::NOT_FOUND })
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
    version: &'static str,
    database: &'static str,
}

async fn health(State(state): State<App>) -> (StatusCode, Json<Health>) {
    let database_ok = notenwart_app::database_ok(&state).await;
    let code = if database_ok { StatusCode::OK } else { StatusCode::SERVICE_UNAVAILABLE };
    let health = Health {
        status: if database_ok { "ok" } else { "degraded" },
        version: env!("CARGO_PKG_VERSION"),
        database: if database_ok { "ok" } else { "unavailable" },
    };
    (code, Json(health))
}

/// Refuses cross-site requests that change something. The session cookie is SameSite=Lax already; this also
/// covers browsers that send an Origin header for same-site but cross-origin requests.
async fn same_origin(request: Request, next: Next) -> Response {
    let safe = matches!(*request.method(), Method::GET | Method::HEAD | Method::OPTIONS);
    if !safe && let Some(origin) = request.headers().get(header::ORIGIN) {
        let origin_host = origin.to_str().ok().and_then(|o| o.split_once("://")).map(|(_, host)| host.to_string());
        let host = request.headers().get(header::HOST).and_then(|h| h.to_str().ok());
        if origin_host.is_none() || origin_host.as_deref() != host {
            return (StatusCode::FORBIDDEN, "cross-origin request refused").into_response();
        }
    }
    next.run(request).await
}
