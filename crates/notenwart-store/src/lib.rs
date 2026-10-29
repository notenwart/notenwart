// SPDX-License-Identifier: AGPL-3.0-or-later

//! Where the data of notenwart lives: PostgreSQL (the connection and its migrations in `migrations/`) and the
//! object store for files (S3-compatible or a local directory). Only `notenwart-app` uses it.
//!
//! Every query lives here, in the module of the feature that uses it (`library`, `people` …), one function each,
//! checked by sqlx at compile time against `.sqlx/`. A function takes any executor – the pool or a transaction –
//! so `notenwart-app` decides what happens together; it holds the rules, this crate the SQL.

pub mod annotations;
pub mod auth;
pub mod catalogue;
pub mod ensembles;
pub mod guests;
pub mod images;
pub mod import;
pub mod library;
pub mod loans;
pub mod notify;
pub mod objects;
pub mod organization;
pub mod people;
pub mod privacy;
pub mod programs;
pub mod recordings;
pub mod sso;
pub mod system;

pub use sqlx::PgPool;
use sqlx::migrate::Migrator;
use sqlx::postgres::PgPoolOptions;

/// The database migrations, embedded at compile time; append-only.
pub static MIGRATOR: Migrator = sqlx::migrate!();

/// Connects to PostgreSQL.
pub async fn connect(database_url: &str) -> sqlx::Result<PgPool> {
    PgPoolOptions::new().max_connections(10).connect(database_url).await
}

/// Whether the database answers.
pub async fn ping(db: impl sqlx::PgExecutor<'_>) -> bool {
    sqlx::query("SELECT 1").execute(db).await.is_ok()
}
