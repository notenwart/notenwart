// SPDX-License-Identifier: AGPL-3.0-or-later

//! The data of `notenwart_app::annotations`: its queries, one function each, checked at compile time against
//! `.sqlx/`.

use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug)]
pub struct LoadRow {
    pub pages: Value,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug)]
pub struct LayersRow {
    pub id: Uuid,
    pub name: String,
    pub leader: bool,
    pub member: bool,
    pub pages: Option<Value>,
    pub updated_at: Option<OffsetDateTime>,
    pub updated_by: Option<String>,
}

#[derive(Debug)]
pub struct SaveSharedRow {
    pub updated_at: OffsetDateTime,
    pub notify: bool,
}

#[derive(Debug)]
pub struct AnnotatedRow {
    pub label: String,
    pub title: String,
}

pub async fn page_count(
    db: impl sqlx::PgExecutor<'_>,
    part_id: Uuid,
    organization_id: Uuid,
) -> sqlx::Result<Option<i32>> {
    sqlx::query_scalar!(
        r#"SELECT cardinality(p.pages) AS "n!" FROM parts p JOIN works w ON w.id = p.work_id
           WHERE p.id = $1 AND w.organization_id = $2"#,
        part_id,
        organization_id
    )
    .fetch_optional(db)
    .await
}

pub async fn load(db: impl sqlx::PgExecutor<'_>, id: Uuid, part_id: Uuid) -> sqlx::Result<Option<LoadRow>> {
    sqlx::query_as!(
        LoadRow,
        "SELECT pages, updated_at FROM annotations WHERE user_id = $1 AND part_id = $2",
        id,
        part_id
    )
    .fetch_optional(db)
    .await
}

pub async fn save(db: impl sqlx::PgExecutor<'_>, id: Uuid, part_id: Uuid, p3: &Value) -> sqlx::Result<OffsetDateTime> {
    sqlx::query_scalar!(
        "INSERT INTO annotations (user_id, part_id, pages) VALUES ($1, $2, $3)
         ON CONFLICT (user_id, part_id) DO UPDATE SET pages = excluded.pages, updated_at = now()
         RETURNING updated_at",
        id,
        part_id,
        p3
    )
    .fetch_one(db)
    .await
}

pub async fn layers(
    db: impl sqlx::PgExecutor<'_>,
    part_id: Uuid,
    id: Uuid,
    organization_id: Uuid,
) -> sqlx::Result<Vec<LayersRow>> {
    sqlx::query_as!(
        LayersRow,
        r#"SELECT e.id, e.name, coalesce(m.leader, false) AS "leader!", m.user_id IS NOT NULL AS "member!",
                  s.pages AS "pages?", s.updated_at AS "updated_at?", u.display_name AS "updated_by?"
           FROM parts p
           JOIN ensemble_works ew ON ew.work_id = p.work_id
           JOIN ensembles e ON e.id = ew.ensemble_id
           LEFT JOIN ensemble_members m ON m.ensemble_id = e.id AND m.user_id = $2
           LEFT JOIN shared_annotations s ON s.ensemble_id = e.id AND s.part_id = p.id
           LEFT JOIN users u ON u.id = s.updated_by
           WHERE p.id = $1 AND e.organization_id = $3
           ORDER BY lower(e.name)"#,
        part_id,
        id,
        organization_id
    )
    .fetch_all(db)
    .await
}

pub async fn save_shared_select_ensemble_members(
    db: impl sqlx::PgExecutor<'_>,
    ensemble_id: Uuid,
    id: Uuid,
) -> sqlx::Result<bool> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM ensemble_members WHERE ensemble_id = $1 AND user_id = $2) AS "x!""#,
        ensemble_id,
        id
    )
    .fetch_one(db)
    .await
}

pub async fn save_shared_insert_shared_annotations(
    db: impl sqlx::PgExecutor<'_>,
    ensemble_id: Uuid,
    part_id: Uuid,
    p3: &Value,
    actor: Option<Uuid>,
) -> sqlx::Result<SaveSharedRow> {
    sqlx::query_as!(
        SaveSharedRow,
        r#"INSERT INTO shared_annotations (ensemble_id, part_id, pages, updated_by) VALUES ($1, $2, $3, $4)
           ON CONFLICT (ensemble_id, part_id)
           DO UPDATE SET pages = excluded.pages, updated_by = excluded.updated_by, updated_at = now()
           RETURNING updated_at, notified_at IS NULL OR notified_at < now() - interval '30 minutes' AS "notify!""#,
        ensemble_id,
        part_id,
        p3,
        actor
    )
    .fetch_one(db)
    .await
}

pub async fn save_shared_update_shared_annotations(
    db: impl sqlx::PgExecutor<'_>,
    ensemble_id: Uuid,
    part_id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "UPDATE shared_annotations SET notified_at = now() WHERE ensemble_id = $1 AND part_id = $2",
        ensemble_id,
        part_id
    )
    .execute(db)
    .await
}

pub async fn save_shared_select_visible_parts(
    db: impl sqlx::PgExecutor<'_>,
    part_id: Uuid,
    ensemble_id: Uuid,
    id: Uuid,
) -> sqlx::Result<Vec<Uuid>> {
    sqlx::query_scalar!(
        r#"SELECT DISTINCT user_id AS "user_id!" FROM visible_parts
               WHERE part_id = $1 AND ensemble_id = $2 AND user_id <> $3"#,
        part_id,
        ensemble_id,
        id
    )
    .fetch_all(db)
    .await
}

pub async fn annotated_select_annotations(
    db: impl sqlx::PgExecutor<'_>,
    id: Uuid,
    part_id: Uuid,
) -> sqlx::Result<Option<Value>> {
    sqlx::query_scalar!("SELECT pages FROM annotations WHERE user_id = $1 AND part_id = $2", id, part_id)
        .fetch_optional(db)
        .await
}

pub async fn annotated_select_parts(db: impl sqlx::PgExecutor<'_>, part_id: Uuid) -> sqlx::Result<AnnotatedRow> {
    sqlx::query_as!(
        AnnotatedRow,
        "SELECT p.label, w.title FROM parts p JOIN works w ON w.id = p.work_id WHERE p.id = $1",
        part_id
    )
    .fetch_one(db)
    .await
}
