// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::Arc;

use axum::Router;
use axum::extract::Request;
use axum::http::{Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use sqlx::PgPool;
use sqlx::migrate::Migrator;
use tower_http::compression::CompressionLayer;
use tower_http::compression::predicate::{NotForContentType, Predicate, SizeAbove};
use tower_http::trace::TraceLayer;

use crate::auth::RateLimiter;
use crate::mail::Mailer;
use crate::storage::Store;
use crate::{api, web};

pub static MIGRATOR: Migrator = sqlx::migrate!();

pub struct Config {
    /// Where users reach the server, without a trailing slash; used in emailed links.
    pub public_url: String,
    /// Mark the session cookie `Secure` (the public URL uses https).
    pub secure_cookies: bool,
    /// Largest accepted upload in bytes.
    pub max_upload: usize,
    /// Language of emails (sign-in links, invitations).
    pub language: crate::mail::Language,
    /// Where files are kept, for the system page: "s3" or "fs".
    pub storage: &'static str,
}

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub store: Store,
    pub config: Arc<Config>,
    pub mailer: Option<Arc<Mailer>>,
    pub limiter: Arc<RateLimiter>,
    pub recognizer: Arc<crate::recognize::Recognizer>,
    /// Sign-in through the organisation's identity provider, if configured.
    pub sso: Option<Arc<crate::sso::Sso>>,
}

/// `/api/*` is the JSON API (see `api/openapi.yaml`); everything else is the web app.
pub fn router(state: AppState) -> Router {
    Router::new()
        .nest("/api", api::router(state.config.max_upload))
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

/// Refuses cross-site requests that change something. The session cookie is SameSite=Lax already; this also
/// covers browsers that send an Origin header for same-site but cross-origin requests.
async fn same_origin(request: Request, next: Next) -> Response {
    let safe = matches!(*request.method(), Method::GET | Method::HEAD | Method::OPTIONS);
    if !safe && let Some(origin) = request.headers().get(header::ORIGIN) {
        let origin_host = origin
            .to_str()
            .ok()
            .and_then(|o| o.split_once("://"))
            .map(|(_, host)| host.to_string());
        let host = request.headers().get(header::HOST).and_then(|h| h.to_str().ok());
        if origin_host.is_none() || origin_host.as_deref() != host {
            return (StatusCode::FORBIDDEN, "cross-origin request refused").into_response();
        }
    }
    next.run(request).await
}
