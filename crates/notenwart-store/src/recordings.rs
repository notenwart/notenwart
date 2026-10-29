// SPDX-License-Identifier: AGPL-3.0-or-later

//! The data of `notenwart_app::recordings`: its queries, one function each, checked at compile time against
//! `.sqlx/`.

use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug)]
pub struct RecordingRow {
    pub id: Uuid,
    pub work_id: Uuid,
    pub kind: String,
    pub origin: String,
    pub title: String,
    pub part_id: Option<Uuid>,
    pub instrument_id: Option<Uuid>,
    pub voice: Option<i16>,
    pub file_name: String,
    pub content_type: String,
    pub size: i64,
    pub duration_seconds: Option<f32>,
    pub created_at: OffsetDateTime,
}

#[derive(Debug)]
pub struct FileRow {
    pub file_name: String,
    pub content_type: String,
    pub size: i64,
}

pub async fn may_hear_select_works(
    db: impl sqlx::PgExecutor<'_>,
    work_id: Uuid,
    organization_id: Uuid,
) -> sqlx::Result<bool> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM works WHERE id = $1 AND organization_id = $2) AS "x!""#,
        work_id,
        organization_id
    )
    .fetch_one(db)
    .await
}

pub async fn may_hear_select_visible_parts(
    db: impl sqlx::PgExecutor<'_>,
    id: Uuid,
    work_id: Uuid,
) -> sqlx::Result<bool> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM visible_parts v JOIN parts p ON p.id = v.part_id
                          WHERE v.user_id = $1 AND p.work_id = $2) AS "x!""#,
        id,
        work_id
    )
    .fetch_one(db)
    .await
}

pub async fn may_manage(
    db: impl sqlx::PgExecutor<'_>,
    id: Uuid,
    work_id: Uuid,
    organization_id: Uuid,
) -> sqlx::Result<bool> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM ensemble_members m JOIN ensemble_works ew ON ew.ensemble_id = m.ensemble_id
                          JOIN works w ON w.id = ew.work_id
                          WHERE m.user_id = $1 AND m.leader AND ew.work_id = $2 AND w.organization_id = $3) AS "x!""#,
        id,
        work_id,
        organization_id
    )
    .fetch_one(db)
    .await
}

pub async fn recordings(db: impl sqlx::PgExecutor<'_>, work_id: Uuid) -> sqlx::Result<Vec<RecordingRow>> {
    sqlx::query_as!(
        RecordingRow,
        r#"SELECT id, work_id, kind, origin, title, part_id, instrument_id, voice, file_name, content_type, size,
                  duration_seconds, created_at
           FROM recordings WHERE work_id = $1
           ORDER BY kind = 'practice', lower(title)"#,
        work_id
    )
    .fetch_all(db)
    .await
}

pub async fn check_select_parts(db: impl sqlx::PgExecutor<'_>, part: Uuid, work_id: Uuid) -> sqlx::Result<bool> {
    sqlx::query_scalar!(r#"SELECT EXISTS (SELECT 1 FROM parts WHERE id = $1 AND work_id = $2) AS "x!""#, part, work_id)
        .fetch_one(db)
        .await
}

pub async fn check_select_instruments(
    db: impl sqlx::PgExecutor<'_>,
    instrument: Uuid,
    organization_id: Uuid,
) -> sqlx::Result<bool> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM instruments WHERE id = $1 AND organization_id = $2) AS "x!""#,
        instrument,
        organization_id
    )
    .fetch_one(db)
    .await
}

pub async fn upload_select_recordings(
    db: impl sqlx::PgExecutor<'_>,
    work_id: Uuid,
    sha256: &str,
) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar!("SELECT title FROM recordings WHERE work_id = $1 AND sha256 = $2", work_id, sha256)
        .fetch_optional(db)
        .await
}

#[allow(clippy::too_many_arguments)] // one parameter per column
pub async fn upload_insert_recordings(
    db: impl sqlx::PgExecutor<'_>,
    id: Uuid,
    organization_id: Uuid,
    work_id: Uuid,
    kind: &str,
    origin: &str,
    title: &str,
    part_id: Option<Uuid>,
    instrument_id: Option<Uuid>,
    voice: Option<i16>,
    file_name: &str,
    content_type: &str,
    size: i64,
    sha256: &str,
    p14: Option<f32>,
    as_ref: &str,
    actor: Option<Uuid>,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "INSERT INTO recordings (id, organization_id, work_id, kind, origin, title, part_id, instrument_id, voice,
                                 file_name, content_type, size, sha256, duration_seconds, storage_key, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)",
        id,
        organization_id,
        work_id,
        kind,
        origin,
        title,
        part_id,
        instrument_id,
        voice,
        file_name,
        content_type,
        size,
        sha256,
        p14,
        as_ref,
        actor
    )
    .execute(db)
    .await
}

pub async fn work_of(
    db: impl sqlx::PgExecutor<'_>,
    recording_id: Uuid,
    organization_id: Uuid,
) -> sqlx::Result<Option<Uuid>> {
    sqlx::query_scalar!(
        "SELECT work_id FROM recordings WHERE id = $1 AND organization_id = $2",
        recording_id,
        organization_id
    )
    .fetch_optional(db)
    .await
}

#[allow(clippy::too_many_arguments)] // one parameter per column
pub async fn update(
    db: impl sqlx::PgExecutor<'_>,
    recording_id: Uuid,
    kind: &str,
    origin: &str,
    title: &str,
    part_id: Option<Uuid>,
    instrument_id: Option<Uuid>,
    voice: Option<i16>,
    p8: Option<f32>,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "UPDATE recordings SET kind = $2, origin = $3, title = $4, part_id = $5, instrument_id = $6, voice = $7,
                               duration_seconds = coalesce($8, duration_seconds)
         WHERE id = $1",
        recording_id,
        kind,
        origin,
        title,
        part_id,
        instrument_id,
        voice,
        p8
    )
    .execute(db)
    .await
}

pub async fn remove(db: impl sqlx::PgExecutor<'_>, recording_id: Uuid) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("DELETE FROM recordings WHERE id = $1", recording_id).execute(db).await
}

pub async fn file(db: impl sqlx::PgExecutor<'_>, recording_id: Uuid) -> sqlx::Result<FileRow> {
    sqlx::query_as!(FileRow, "SELECT file_name, content_type, size FROM recordings WHERE id = $1", recording_id)
        .fetch_one(db)
        .await
}
