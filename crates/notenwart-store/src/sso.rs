// SPDX-License-Identifier: AGPL-3.0-or-later

//! The data of `notenwart_app::sso`: its queries, one function each, checked at compile time against
//! `.sqlx/`.

use uuid::Uuid;

pub async fn sign_in(db: impl sqlx::PgExecutor<'_>, email: &str) -> sqlx::Result<Option<Uuid>> {
    sqlx::query_scalar!(
        "SELECT u.id FROM users u JOIN memberships m ON m.user_id = u.id WHERE lower(u.email) = $1",
        email
    )
    .fetch_optional(db)
    .await
}
