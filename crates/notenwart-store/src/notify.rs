// SPDX-License-Identifier: AGPL-3.0-or-later

//! The data of `notenwart_app::notify`: its queries, one function each, checked at compile time against
//! `.sqlx/`.

use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug)]
pub struct RecordRow {
    pub user_id: Uuid,
    pub in_app: bool,
    pub email: bool,
    pub push: bool,
}

#[derive(Debug)]
pub struct PartNamesRow {
    pub label: String,
    pub title: String,
}

#[derive(Debug)]
pub struct NotificationRow {
    pub id: Uuid,
    pub kind: String,
    pub params: Value,
    pub link: Option<String>,
    pub created_at: OffsetDateTime,
    pub read: bool,
}

#[derive(Debug)]
pub struct SettingsRow {
    pub kind: String,
    pub in_app: bool,
    pub email: bool,
    pub push: bool,
}

#[derive(Debug)]
pub struct RemindLoansRow {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub borrower_id: Uuid,
    pub item: String,
    pub title: String,
    pub due_on: String,
    pub overdue: bool,
}

#[derive(Debug)]
pub struct SendDigestsRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub kind: String,
    pub params: Value,
    pub link: Option<String>,
    pub email: Option<String>,
    pub display_name: String,
}

pub async fn record_select_notification_settings(
    db: impl sqlx::PgExecutor<'_>,
    key: &str,
    recipients: &[Uuid],
) -> sqlx::Result<Vec<RecordRow>> {
    sqlx::query_as!(
        RecordRow,
        "SELECT user_id, in_app, email, push FROM notification_settings WHERE kind = $1 AND user_id = ANY($2)",
        key,
        recipients
    )
    .fetch_all(db)
    .await
}

#[allow(clippy::too_many_arguments)] // one parameter per column
pub async fn record_insert_notifications(
    db: impl sqlx::PgExecutor<'_>,
    organization_id: Uuid,
    key: &str,
    params: &Value,
    link: &str,
    users: &[Uuid],
    in_app: &[bool],
    email: &[bool],
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "INSERT INTO notifications (organization_id, user_id, kind, params, link, in_app, email_due)
         SELECT $1, u, $2, $3, $4, a, e FROM unnest($5::uuid[], $6::bool[], $7::bool[]) AS t(u, a, e)",
        organization_id,
        key,
        params,
        link,
        users,
        in_app,
        email
    )
    .execute(db)
    .await
}

pub async fn part_names(db: impl sqlx::PgExecutor<'_>, part_id: Uuid) -> sqlx::Result<Option<PartNamesRow>> {
    sqlx::query_as!(
        PartNamesRow,
        "SELECT p.label, w.title FROM parts p JOIN works w ON w.id = p.work_id WHERE p.id = $1",
        part_id
    )
    .fetch_optional(db)
    .await
}

pub async fn who_sees_part(db: impl sqlx::PgExecutor<'_>, part_id: Uuid, but: Uuid) -> sqlx::Result<Vec<Uuid>> {
    sqlx::query_scalar!(
        r#"SELECT DISTINCT user_id AS "user_id!" FROM visible_parts WHERE part_id = $1 AND user_id <> $2"#,
        part_id,
        but
    )
    .fetch_all(db)
    .await
}

pub async fn ensemble_members(db: impl sqlx::PgExecutor<'_>, ensemble_id: Uuid, but: Uuid) -> sqlx::Result<Vec<Uuid>> {
    sqlx::query_scalar!(
        "SELECT user_id FROM ensemble_members WHERE ensemble_id = $1 AND user_id <> $2",
        ensemble_id,
        but
    )
    .fetch_all(db)
    .await
}

pub async fn inbox_set_transaction(db: impl sqlx::PgExecutor<'_>) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY").execute(db).await
}

pub async fn inbox_select_notifications(db: impl sqlx::PgExecutor<'_>, id: Uuid) -> sqlx::Result<i64> {
    sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM notifications WHERE user_id = $1 AND in_app AND read_at IS NULL"#,
        id
    )
    .fetch_one(db)
    .await
}

pub async fn inbox_select_notifications_2(
    db: impl sqlx::PgExecutor<'_>,
    id: Uuid,
) -> sqlx::Result<Vec<NotificationRow>> {
    sqlx::query_as!(
        NotificationRow,
        r#"SELECT id, kind, params, link, created_at, read_at IS NOT NULL AS "read!"
               FROM notifications WHERE user_id = $1 AND in_app
               ORDER BY created_at DESC LIMIT 100"#,
        id
    )
    .fetch_all(db)
    .await
}

pub async fn mark_read(
    db: impl sqlx::PgExecutor<'_>,
    id: Uuid,
    as_deref: Option<&[Uuid]>,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "UPDATE notifications SET read_at = now()
         WHERE user_id = $1 AND read_at IS NULL AND ($2::uuid[] IS NULL OR id = ANY($2))",
        id,
        as_deref
    )
    .execute(db)
    .await
}

pub async fn settings(db: impl sqlx::PgExecutor<'_>, id: Uuid) -> sqlx::Result<Vec<SettingsRow>> {
    sqlx::query_as!(SettingsRow, "SELECT kind, in_app, email, push FROM notification_settings WHERE user_id = $1", id)
        .fetch_all(db)
        .await
}

pub async fn save_settings(
    db: impl sqlx::PgExecutor<'_>,
    id: Uuid,
    key: &str,
    in_app: bool,
    email: bool,
    push: bool,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "INSERT INTO notification_settings (user_id, kind, in_app, email, push) VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (user_id, kind) DO UPDATE SET in_app = $3, email = $4, push = $5",
        id,
        key,
        in_app,
        email,
        push
    )
    .execute(db)
    .await
}

pub async fn remind_loans_select_loans(db: impl sqlx::PgExecutor<'_>) -> sqlx::Result<Vec<RemindLoansRow>> {
    sqlx::query_as!(
        RemindLoansRow,
        r#"SELECT l.id, l.organization_id, l.borrower_id AS "borrower_id!", l.item, w.title,
                  to_char(l.due_on, 'YYYY-MM-DD') AS "due_on!", l.due_on < current_date AS "overdue!"
           FROM loans l JOIN works w ON w.id = l.work_id
           WHERE l.returned_on IS NULL AND l.borrower_id IS NOT NULL AND l.due_on IS NOT NULL
             AND ((l.reminded_at IS NULL AND l.due_on <= current_date + 1)
                  OR (l.overdue_reminded_at IS NULL AND l.due_on < current_date))"#
    )
    .fetch_all(db)
    .await
}

pub async fn remind_loans_update_loans(
    db: impl sqlx::PgExecutor<'_>,
    id: Uuid,
    overdue: bool,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "UPDATE loans SET reminded_at = coalesce(reminded_at, now()),
                              overdue_reminded_at = CASE WHEN $2 THEN now() ELSE overdue_reminded_at END
             WHERE id = $1",
        id,
        overdue
    )
    .execute(db)
    .await
}

pub async fn send_digests_select_notifications(
    db: impl sqlx::PgExecutor<'_>,
    as_secs_f64: f64,
) -> sqlx::Result<Vec<SendDigestsRow>> {
    sqlx::query_as!(
        SendDigestsRow,
        r#"SELECT n.id, n.user_id, n.kind, n.params, n.link, u.email AS "email?", u.display_name
           FROM notifications n JOIN users u ON u.id = n.user_id
           WHERE n.email_due AND n.emailed_at IS NULL AND n.created_at < now() - make_interval(secs => $1)
           ORDER BY n.user_id, n.created_at"#,
        as_secs_f64
    )
    .fetch_all(db)
    .await
}

pub async fn send_digests_update_notifications(
    db: impl sqlx::PgExecutor<'_>,
    ids: &[Uuid],
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("UPDATE notifications SET emailed_at = now() WHERE id = ANY($1)", ids).execute(db).await
}
