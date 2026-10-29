// SPDX-License-Identifier: AGPL-3.0-or-later

//! Where the data of notenwart lives: PostgreSQL (the connection and its migrations in `migrations/`) and the
//! object store for files (S3-compatible or a local directory). Only `notenwart-app` uses it.
//!
//! OPEN: the queries themselves are still written next to the commands in `notenwart-app` (checked by sqlx at
//! compile time against `.sqlx/`); they move here feature by feature.

pub mod objects;
pub mod organization;

use sqlx::PgPool;
use sqlx::migrate::Migrator;
use sqlx::postgres::PgPoolOptions;

/// The database migrations, embedded at compile time; append-only.
pub static MIGRATOR: Migrator = sqlx::migrate!();

/// Connects to PostgreSQL.
pub async fn connect(database_url: &str) -> sqlx::Result<PgPool> {
    PgPoolOptions::new().max_connections(10).connect(database_url).await
}
