// SPDX-License-Identifier: AGPL-3.0-or-later

//! The workspace of notenwart: every command and query. The API (`notenwart-api`), the command line and – later –
//! the terminal interface work through it only, never with the store or the domain on their own.
//!
//! A command is a function of a feature module that takes the [`App`], the caller ([`auth::CurrentUser`], found by
//! the interface – the API by the session cookie) and its input, checks the caller's role, and answers with plain
//! data or an [`error::AppError`]. Nothing here knows HTTP: status codes, cookies, headers and multipart forms are
//! the API's part. Inputs and results derive serde, as the API sends them as JSON, and `utoipa::ToSchema`, which
//! describes them in the API's description (`api/openapi.json`) under the names given there.

pub mod ai;
pub mod annotations;
pub mod auth;
pub mod catalogue;
pub mod ensembles;
pub mod error;
pub mod files;
pub mod guests;
pub mod images;
pub mod import;
pub mod library;
pub mod loans;
pub mod mail;
pub mod notify;
pub mod people;
pub mod privacy;
pub mod programs;
pub mod recognize;
pub mod recordings;
pub mod schema;
pub mod sso;
pub mod system;

use std::sync::Arc;

use sqlx::PgPool;

pub use notenwart_core::pdf;
pub use notenwart_store::objects as storage;

use crate::auth::RateLimiter;
use crate::mail::Mailer;
use crate::storage::Store;

#[derive(Debug)]
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

/// One server: the database, the files, the configuration and the services the commands use.
#[derive(Clone, Debug)]
pub struct App {
    pub pool: PgPool,
    pub store: Store,
    pub config: Arc<Config>,
    pub mailer: Option<Arc<Mailer>>,
    pub limiter: Arc<RateLimiter>,
    pub recognizer: Arc<crate::recognize::Recognizer>,
    /// Sign-in through the organisation's identity provider, if configured.
    pub sso: Option<Arc<crate::sso::Sso>>,
}

/// Whether the database answers (the API's health check).
pub async fn database_ok(app: &App) -> bool {
    sqlx::query("SELECT 1").execute(&app.pool).await.is_ok()
}
