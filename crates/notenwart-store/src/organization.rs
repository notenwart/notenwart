// SPDX-License-Identifier: AGPL-3.0-or-later

use uuid::Uuid;

/// A server has one organisation for now, created on the first start and named during setup.
pub async fn ensure_default(pool: &sqlx::PgPool, name: &str) -> sqlx::Result<Uuid> {
    if let Some(id) =
        sqlx::query_scalar!("SELECT id FROM organizations ORDER BY created_at LIMIT 1").fetch_optional(pool).await?
    {
        return Ok(id);
    }
    sqlx::query_scalar!("INSERT INTO organizations (slug, name) VALUES ('default', $1) RETURNING id", name)
        .fetch_one(pool)
        .await
}
