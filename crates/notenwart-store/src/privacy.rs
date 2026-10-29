// SPDX-License-Identifier: AGPL-3.0-or-later

//! The data of `notenwart_app::privacy`: its queries, one function each, checked at compile time against
//! `.sqlx/`.

use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug)]
pub struct ExportRow {
    pub display_name: String,
    pub email: String,
    pub created_at: OffsetDateTime,
    pub last_login_at: Option<OffsetDateTime>,
    pub has_password: bool,
    pub role: String,
    pub organization: String,
}

#[derive(Debug)]
pub struct Export2Row {
    pub name: String,
    pub leader: bool,
    pub created_at: OffsetDateTime,
    pub plays: Vec<String>,
}

#[derive(Debug)]
pub struct Export3Row {
    pub title: String,
    pub label: String,
    pub created_at: OffsetDateTime,
}

#[derive(Debug)]
pub struct Export4Row {
    pub title: String,
    pub label: String,
    pub pages: Value,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug)]
pub struct Export5Row {
    pub title: String,
    pub item: String,
    pub quantity: i32,
    pub lent_on: Option<String>,
    pub due_on: Option<String>,
    pub returned_on: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug)]
pub struct Export6Row {
    pub created_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
}

#[derive(Debug)]
pub struct DeleteUsersRow {
    pub ensembles: i64,
    pub annotations: i64,
    pub single_parts: i64,
    pub loans: i64,
    pub sessions: i64,
}

#[derive(Debug)]
pub struct DeletionRow {
    pub user_id: Uuid,
    pub deleted_by: Option<String>,
    pub deleted_at: OffsetDateTime,
    pub records: Value,
}

pub async fn export_select_users(
    db: impl sqlx::PgExecutor<'_>,
    user_id: Uuid,
    organization_id: Uuid,
) -> sqlx::Result<Option<ExportRow>> {
    sqlx::query_as!(ExportRow, r#"SELECT u.display_name, u.email, u.created_at, u.last_login_at, u.password_hash IS NOT NULL AS "has_password!",
                  m.role::text AS "role!", o.name AS organization
           FROM users u JOIN memberships m ON m.user_id = u.id JOIN organizations o ON o.id = m.organization_id
           WHERE u.id = $1 AND m.organization_id = $2"#, user_id, organization_id).fetch_optional(db).await
}

pub async fn export_select_ensemble_members(
    db: impl sqlx::PgExecutor<'_>,
    user_id: Uuid,
    organization_id: Uuid,
) -> sqlx::Result<Vec<Export2Row>> {
    sqlx::query_as!(
        Export2Row,
        r#"SELECT e.name, m.leader, m.created_at,
                  coalesce(array_agg(i.name || coalesce(' ' || p.voice, '')) FILTER (WHERE i.id IS NOT NULL), '{}')
                      AS "plays!: Vec<String>"
           FROM ensemble_members m JOIN ensembles e ON e.id = m.ensemble_id
           LEFT JOIN ensemble_member_parts p ON p.ensemble_id = m.ensemble_id AND p.user_id = m.user_id
           LEFT JOIN instruments i ON i.id = p.instrument_id
           WHERE m.user_id = $1 AND e.organization_id = $2
           GROUP BY e.name, m.leader, m.created_at ORDER BY e.name"#,
        user_id,
        organization_id
    )
    .fetch_all(db)
    .await
}

pub async fn export_select_part_grants(
    db: impl sqlx::PgExecutor<'_>,
    user_id: Uuid,
    organization_id: Uuid,
) -> sqlx::Result<Vec<Export3Row>> {
    sqlx::query_as!(
        Export3Row,
        "SELECT w.title, p.label, g.created_at FROM part_grants g
         JOIN parts p ON p.id = g.part_id JOIN works w ON w.id = p.work_id
         WHERE g.user_id = $1 AND w.organization_id = $2 ORDER BY w.title",
        user_id,
        organization_id
    )
    .fetch_all(db)
    .await
}

pub async fn export_select_annotations(
    db: impl sqlx::PgExecutor<'_>,
    user_id: Uuid,
    organization_id: Uuid,
) -> sqlx::Result<Vec<Export4Row>> {
    sqlx::query_as!(
        Export4Row,
        "SELECT w.title, p.label, a.pages, a.updated_at FROM annotations a
         JOIN parts p ON p.id = a.part_id JOIN works w ON w.id = p.work_id
         WHERE a.user_id = $1 AND w.organization_id = $2 ORDER BY w.title",
        user_id,
        organization_id
    )
    .fetch_all(db)
    .await
}

pub async fn export_select_loans(
    db: impl sqlx::PgExecutor<'_>,
    user_id: Uuid,
    organization_id: Uuid,
) -> sqlx::Result<Vec<Export5Row>> {
    sqlx::query_as!(
        Export5Row,
        "SELECT w.title, l.item, l.quantity, to_char(l.lent_on, 'YYYY-MM-DD') AS lent_on,
                to_char(l.due_on, 'YYYY-MM-DD') AS due_on, to_char(l.returned_on, 'YYYY-MM-DD') AS returned_on, l.note
         FROM loans l JOIN works w ON w.id = l.work_id
         WHERE l.borrower_id = $1 AND l.organization_id = $2 ORDER BY l.lent_on",
        user_id,
        organization_id
    )
    .fetch_all(db)
    .await
}

pub async fn export_select_sessions(db: impl sqlx::PgExecutor<'_>, user_id: Uuid) -> sqlx::Result<Vec<Export6Row>> {
    sqlx::query_as!(
        Export6Row,
        "SELECT created_at, expires_at FROM sessions WHERE user_id = $1 ORDER BY created_at",
        user_id
    )
    .fetch_all(db)
    .await
}

pub async fn with_open_loans(
    db: impl sqlx::PgExecutor<'_>,
    organization_id: Uuid,
    ids: &[Uuid],
) -> sqlx::Result<Vec<Uuid>> {
    sqlx::query_scalar!(
        r#"SELECT DISTINCT borrower_id AS "id!" FROM loans
           WHERE organization_id = $1 AND borrower_id = ANY($2) AND returned_on IS NULL"#,
        organization_id,
        ids
    )
    .fetch_all(db)
    .await
}

pub async fn delete_users_select_ensemble_members(
    db: impl sqlx::PgExecutor<'_>,
    id: Uuid,
) -> sqlx::Result<DeleteUsersRow> {
    sqlx::query_as!(
        DeleteUsersRow,
        r#"SELECT (SELECT count(*) FROM ensemble_members WHERE user_id = $1) AS "ensembles!",
                      (SELECT count(*) FROM annotations WHERE user_id = $1) AS "annotations!",
                      (SELECT count(*) FROM part_grants WHERE user_id = $1) AS "single_parts!",
                      (SELECT count(*) FROM loans WHERE borrower_id = $1) AS "loans!",
                      (SELECT count(*) FROM sessions WHERE user_id = $1) AS "sessions!""#,
        id
    )
    .fetch_one(db)
    .await
}

pub async fn delete_users_insert_deletion_log(
    db: impl sqlx::PgExecutor<'_>,
    organization_id: Uuid,
    id: Uuid,
    deleted_by: Option<Uuid>,
    p4: &Value,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "INSERT INTO deletion_log (organization_id, user_id, deleted_by, records) VALUES ($1, $2, $3, $4)",
        organization_id,
        id,
        deleted_by,
        p4
    )
    .execute(db)
    .await
}

pub async fn delete_users_update_loans(
    db: impl sqlx::PgExecutor<'_>,
    ids: &[Uuid],
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("UPDATE loans SET borrower_name = '–', note = NULL WHERE borrower_id = ANY($1)", ids).execute(db).await
}

pub async fn delete_users_delete_users(
    db: impl sqlx::PgExecutor<'_>,
    ids: &[Uuid],
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("DELETE FROM users WHERE id = ANY($1)", ids).execute(db).await
}

pub async fn deletions(db: impl sqlx::PgExecutor<'_>, organization_id: Uuid) -> sqlx::Result<Vec<DeletionRow>> {
    sqlx::query_as!(
        DeletionRow,
        r#"SELECT d.user_id, u.display_name AS "deleted_by?", d.deleted_at, d.records
               FROM deletion_log d LEFT JOIN users u ON u.id = d.deleted_by
               WHERE d.organization_id = $1 ORDER BY d.deleted_at DESC LIMIT 1000"#,
        organization_id
    )
    .fetch_all(db)
    .await
}
