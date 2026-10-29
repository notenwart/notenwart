// SPDX-License-Identifier: AGPL-3.0-or-later

//! The data of `notenwart_app::guests`: its queries, one function each, checked at compile time against
//! `.sqlx/`.

use time::{Date, OffsetDateTime};
use uuid::Uuid;

#[derive(Debug)]
pub struct LinksRow {
    pub id: Uuid,
    pub name: String,
    pub part_ids: Vec<Uuid>,
    pub expires_on: String,
    pub last_used_at: Option<OffsetDateTime>,
    pub revoked: bool,
    pub created_by: Option<String>,
}

#[derive(Debug)]
pub struct ValidRow {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub ensemble: String,
    pub expires_on: String,
    pub part_ids: Vec<Uuid>,
}

#[derive(Debug)]
pub struct GuestpartRow {
    pub id: Uuid,
    pub label: String,
    pub work_title: String,
    pub composer: Option<String>,
    pub page_count: i32,
}

#[derive(Debug)]
pub struct FileRow {
    pub label: String,
    pub title: String,
}

#[derive(Debug)]
pub struct NotifyExpiringRow {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub created_by: Uuid,
    pub expires_on: String,
}

pub async fn links(db: impl sqlx::PgExecutor<'_>, ensemble_id: Uuid) -> sqlx::Result<Vec<LinksRow>> {
    sqlx::query_as!(
        LinksRow,
        r#"SELECT g.id, g.name, g.part_ids, to_char(g.expires_on, 'YYYY-MM-DD') AS "expires_on!", g.last_used_at,
                  g.revoked_at IS NOT NULL AS "revoked!", u.display_name AS "created_by?"
           FROM guest_links g LEFT JOIN users u ON u.id = g.created_by
           WHERE g.ensemble_id = $1 ORDER BY g.revoked_at IS NOT NULL, g.expires_on DESC"#,
        ensemble_id
    )
    .fetch_all(db)
    .await
}

pub async fn create_select_parts(
    db: impl sqlx::PgExecutor<'_>,
    ensemble_id: Uuid,
    parts: &[Uuid],
) -> sqlx::Result<i64> {
    sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM parts p JOIN ensemble_works ew ON ew.work_id = p.work_id
           WHERE ew.ensemble_id = $1 AND p.id = ANY($2)"#,
        ensemble_id,
        parts
    )
    .fetch_one(db)
    .await
}

#[allow(clippy::too_many_arguments)] // one parameter per column
pub async fn create_insert_guest_links(
    db: impl sqlx::PgExecutor<'_>,
    organization_id: Uuid,
    ensemble_id: Uuid,
    name: &str,
    hash: &[u8],
    parts: &[Uuid],
    expires: Date,
    actor: Option<Uuid>,
) -> sqlx::Result<Uuid> {
    sqlx::query_scalar!(
        "INSERT INTO guest_links (organization_id, ensemble_id, name, token_hash, part_ids, expires_on, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
        organization_id,
        ensemble_id,
        name,
        hash,
        parts,
        expires,
        actor
    )
    .fetch_one(db)
    .await
}

pub async fn revoke_select_guest_links(
    db: impl sqlx::PgExecutor<'_>,
    link_id: Uuid,
    organization_id: Uuid,
) -> sqlx::Result<Option<Uuid>> {
    sqlx::query_scalar!(
        "SELECT ensemble_id FROM guest_links WHERE id = $1 AND organization_id = $2",
        link_id,
        organization_id
    )
    .fetch_optional(db)
    .await
}

pub async fn revoke_update_guest_links(
    db: impl sqlx::PgExecutor<'_>,
    link_id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("UPDATE guest_links SET revoked_at = coalesce(revoked_at, now()) WHERE id = $1", link_id)
        .execute(db)
        .await
}

pub async fn valid_select_guest_links(db: impl sqlx::PgExecutor<'_>, p1: &[u8]) -> sqlx::Result<Option<ValidRow>> {
    sqlx::query_as!(ValidRow, r#"SELECT g.id, g.organization_id, g.name, e.name AS ensemble, to_char(g.expires_on, 'YYYY-MM-DD') AS "expires_on!",
                  g.part_ids
           FROM guest_links g JOIN ensembles e ON e.id = g.ensemble_id
           WHERE g.token_hash = $1 AND g.revoked_at IS NULL AND g.expires_on >= current_date"#, p1).fetch_optional(db).await
}

pub async fn valid_update_guest_links(
    db: impl sqlx::PgExecutor<'_>,
    id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("UPDATE guest_links SET last_used_at = now() WHERE id = $1", id).execute(db).await
}

pub async fn open(
    db: impl sqlx::PgExecutor<'_>,
    part_ids: &[Uuid],
    organization_id: Uuid,
) -> sqlx::Result<Vec<GuestpartRow>> {
    sqlx::query_as!(
        GuestpartRow,
        r#"SELECT p.id, p.label, w.title AS work_title, w.composer, cardinality(p.pages) AS "page_count!"
           FROM parts p JOIN works w ON w.id = p.work_id
           WHERE p.id = ANY($1) AND w.organization_id = $2
           ORDER BY lower(w.title), lower(p.label)"#,
        part_ids,
        organization_id
    )
    .fetch_all(db)
    .await
}

pub async fn file(
    db: impl sqlx::PgExecutor<'_>,
    part_id: Uuid,
    organization_id: Uuid,
) -> sqlx::Result<Option<FileRow>> {
    sqlx::query_as!(
        FileRow,
        "SELECT p.label, w.title FROM parts p JOIN works w ON w.id = p.work_id
         WHERE p.id = $1 AND w.organization_id = $2",
        part_id,
        organization_id
    )
    .fetch_optional(db)
    .await
}

pub async fn notify_expiring_select_guest_links(db: impl sqlx::PgExecutor<'_>) -> sqlx::Result<Vec<NotifyExpiringRow>> {
    sqlx::query_as!(NotifyExpiringRow, r#"SELECT id, organization_id, name, created_by AS "created_by!", to_char(expires_on, 'YYYY-MM-DD') AS "expires_on!"
           FROM guest_links
           WHERE revoked_at IS NULL AND expiring_notified_at IS NULL AND created_by IS NOT NULL
             AND expires_on <= current_date + 1 AND expires_on >= current_date"#).fetch_all(db).await
}

pub async fn notify_expiring_update_guest_links(
    db: impl sqlx::PgExecutor<'_>,
    id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("UPDATE guest_links SET expiring_notified_at = now() WHERE id = $1", id).execute(db).await
}
