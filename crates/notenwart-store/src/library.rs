// SPDX-License-Identifier: AGPL-3.0-or-later

//! The data of `notenwart_app::library`: its queries, one function each, checked at compile time against
//! `.sqlx/`.

use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug)]
pub struct KnownInstrumentsRow {
    pub id: Uuid,
    pub name: String,
    pub aliases: Vec<String>,
}

#[derive(Debug)]
pub struct SourceBytesRow {
    pub id: Uuid,
}

#[derive(Debug)]
pub struct WorkFacetsRow {
    pub field: String,
    pub value: String,
}

#[derive(Debug)]
pub struct ListWorksRow {
    pub id: Uuid,
    pub title: String,
    pub composer: Option<String>,
    pub arranger: Option<String>,
    pub publisher: Option<String>,
    pub lyricist: Option<String>,
    pub society_number: Option<String>,
    pub archive_location: Option<String>,
    pub instrumentation: Option<String>,
    pub difficulty: Option<i16>,
    pub duration_seconds: Option<i32>,
    pub source: Option<String>,
    pub notes: Option<String>,
    pub tags: Vec<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub source_count: i64,
    pub part_count: i64,
}

#[derive(Debug)]
pub struct WorkRow {
    pub id: Uuid,
    pub title: String,
    pub composer: Option<String>,
    pub arranger: Option<String>,
    pub publisher: Option<String>,
    pub lyricist: Option<String>,
    pub society_number: Option<String>,
    pub archive_location: Option<String>,
    pub instrumentation: Option<String>,
    pub difficulty: Option<i16>,
    pub duration_seconds: Option<i32>,
    pub source: Option<String>,
    pub notes: Option<String>,
    pub tags: Vec<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug)]
pub struct SourcefileRow {
    pub id: Uuid,
    pub work_id: Uuid,
    pub file_name: String,
    pub size: i64,
    pub sha256: String,
    pub page_count: i32,
    pub created_at: OffsetDateTime,
}

#[derive(Debug)]
pub struct PartRow {
    pub id: Uuid,
    pub work_id: Uuid,
    pub source_id: Uuid,
    pub label: String,
    pub instrument_id: Option<Uuid>,
    pub voice: Option<i16>,
    pub pages: Vec<i32>,
    pub page_count: i32,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug)]
pub struct SourceFileRow {
    pub file_name: String,
}

#[derive(Debug)]
pub struct PartFileRow {
    pub label: String,
    pub title: String,
}

pub async fn known_instruments(
    db: impl sqlx::PgExecutor<'_>,
    organization_id: Uuid,
) -> sqlx::Result<Vec<KnownInstrumentsRow>> {
    sqlx::query_as!(
        KnownInstrumentsRow,
        "SELECT id, name, aliases FROM instruments WHERE organization_id = $1",
        organization_id
    )
    .fetch_all(db)
    .await
}

pub async fn source_bytes(
    db: impl sqlx::PgExecutor<'_>,
    source_id: Uuid,
    organization_id: Uuid,
) -> sqlx::Result<Option<SourceBytesRow>> {
    sqlx::query_as!(
        SourceBytesRow,
        "SELECT s.id FROM source_files s JOIN works w ON w.id = s.work_id
         WHERE s.id = $1 AND w.organization_id = $2",
        source_id,
        organization_id
    )
    .fetch_optional(db)
    .await
}

pub async fn list_tags(db: impl sqlx::PgExecutor<'_>, organization_id: Uuid) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar!(
        r#"SELECT DISTINCT tag AS "tag!" FROM works, unnest(tags) AS tag WHERE organization_id = $1 ORDER BY 1"#,
        organization_id
    )
    .fetch_all(db)
    .await
}

pub async fn work_facets(db: impl sqlx::PgExecutor<'_>, organization_id: Uuid) -> sqlx::Result<Vec<WorkFacetsRow>> {
    sqlx::query_as!(
        WorkFacetsRow,
        r#"SELECT field AS "field!", value AS "value!" FROM (
               SELECT 'composer' AS field, composer AS value FROM works WHERE organization_id = $1
               UNION ALL SELECT 'arranger', arranger FROM works WHERE organization_id = $1
               UNION ALL SELECT 'publisher', publisher FROM works WHERE organization_id = $1
               UNION ALL SELECT 'instrumentation', instrumentation FROM works WHERE organization_id = $1
               UNION ALL SELECT 'tag', tag FROM works, unnest(tags) AS tag WHERE organization_id = $1
           ) used
           WHERE value IS NOT NULL
           GROUP BY field, value
           ORDER BY field, count(*) DESC, lower(value)"#,
        organization_id
    )
    .fetch_all(db)
    .await
}

pub async fn list_works(
    db: impl sqlx::PgExecutor<'_>,
    organization_id: Uuid,
    pattern: Option<&str>,
    tag: Option<&str>,
) -> sqlx::Result<Vec<ListWorksRow>> {
    sqlx::query_as!(ListWorksRow, r#"SELECT w.id, w.title, w.composer, w.arranger, w.publisher, w.lyricist, w.society_number, w.archive_location, w.instrumentation, w.difficulty,
                  w.duration_seconds, w.source, w.notes, w.tags, w.created_at, w.updated_at,
                  (SELECT count(*) FROM source_files s WHERE s.work_id = w.id) AS "source_count!",
                  (SELECT count(*) FROM parts p WHERE p.work_id = w.id) AS "part_count!"
           FROM works w
           WHERE w.organization_id = $1
             AND ($2::text IS NULL OR w.title ILIKE $2 OR w.composer ILIKE $2 OR w.arranger ILIKE $2
                  OR w.publisher ILIKE $2 OR w.lyricist ILIKE $2 OR w.instrumentation ILIKE $2
                  OR w.society_number ILIKE $2 OR array_to_string(w.tags, ' ') ILIKE $2)
             AND ($3::text IS NULL OR $3 = ANY (w.tags))
           ORDER BY lower(w.title), w.created_at"#, organization_id, pattern, tag).fetch_all(db).await
}

#[allow(clippy::too_many_arguments)] // one parameter per column
pub async fn create_work(
    db: impl sqlx::PgExecutor<'_>,
    organization_id: Uuid,
    title: &str,
    composer: Option<&str>,
    arranger: Option<&str>,
    publisher: Option<&str>,
    instrumentation: Option<&str>,
    difficulty: Option<i16>,
    duration_seconds: Option<i32>,
    source: Option<&str>,
    notes: Option<&str>,
    tags: &[String],
    lyricist: Option<&str>,
    society_number: Option<&str>,
    archive_location: Option<&str>,
) -> sqlx::Result<WorkRow> {
    sqlx::query_as!(WorkRow, r#"INSERT INTO works (organization_id, title, composer, arranger, publisher, instrumentation, difficulty,
                              duration_seconds, source, notes, tags, lyricist, society_number, archive_location)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
           RETURNING id, title, composer, arranger, publisher, lyricist, society_number, archive_location, instrumentation, difficulty, duration_seconds,
                     source, notes, tags, created_at, updated_at"#, organization_id, title, composer, arranger, publisher, instrumentation, difficulty, duration_seconds, source, notes, tags, lyricist, society_number, archive_location).fetch_one(db).await
}

pub async fn find_work(
    db: impl sqlx::PgExecutor<'_>,
    id: Uuid,
    organization_id: Uuid,
) -> sqlx::Result<Option<WorkRow>> {
    sqlx::query_as!(WorkRow, r#"SELECT id, title, composer, arranger, publisher, lyricist, society_number, archive_location, instrumentation, difficulty, duration_seconds,
                  source, notes, tags, created_at, updated_at
           FROM works WHERE id = $1 AND organization_id = $2"#, id, organization_id).fetch_optional(db).await
}

pub async fn get_work_select_source_files(
    db: impl sqlx::PgExecutor<'_>,
    work_id: Uuid,
) -> sqlx::Result<Vec<SourcefileRow>> {
    sqlx::query_as!(
        SourcefileRow,
        r#"SELECT id, work_id, file_name, size, sha256, page_count, created_at
           FROM source_files WHERE work_id = $1 ORDER BY created_at"#,
        work_id
    )
    .fetch_all(db)
    .await
}

pub async fn get_work_select_parts(db: impl sqlx::PgExecutor<'_>, work_id: Uuid) -> sqlx::Result<Vec<PartRow>> {
    sqlx::query_as!(
        PartRow,
        r#"SELECT p.id, p.work_id, p.source_id, p.label, p.instrument_id, p.voice, p.pages,
                  cardinality(p.pages) AS "page_count!", p.created_at, p.updated_at
           FROM parts p LEFT JOIN instruments i ON i.id = p.instrument_id
           WHERE p.work_id = $1
           ORDER BY i.sort NULLS FIRST, p.voice NULLS FIRST, lower(p.label)"#,
        work_id
    )
    .fetch_all(db)
    .await
}

#[allow(clippy::too_many_arguments)] // one parameter per column
pub async fn update_work(
    db: impl sqlx::PgExecutor<'_>,
    work_id: Uuid,
    organization_id: Uuid,
    title: &str,
    composer: Option<&str>,
    arranger: Option<&str>,
    publisher: Option<&str>,
    instrumentation: Option<&str>,
    difficulty: Option<i16>,
    duration_seconds: Option<i32>,
    source: Option<&str>,
    notes: Option<&str>,
    tags: &[String],
    lyricist: Option<&str>,
    society_number: Option<&str>,
    archive_location: Option<&str>,
) -> sqlx::Result<Option<WorkRow>> {
    sqlx::query_as!(WorkRow, r#"UPDATE works SET title = $3, composer = $4, arranger = $5, publisher = $6, instrumentation = $7,
                            difficulty = $8, duration_seconds = $9, source = $10, notes = $11, tags = $12,
                            lyricist = $13, society_number = $14, archive_location = $15, updated_at = now()
           WHERE id = $1 AND organization_id = $2
           RETURNING id, title, composer, arranger, publisher, lyricist, society_number, archive_location, instrumentation, difficulty, duration_seconds,
                     source, notes, tags, created_at, updated_at"#, work_id, organization_id, title, composer, arranger, publisher, instrumentation, difficulty, duration_seconds, source, notes, tags, lyricist, society_number, archive_location).fetch_optional(db).await
}

pub async fn delete_work_select_source_files(
    db: impl sqlx::PgExecutor<'_>,
    work_id: Uuid,
    organization_id: Uuid,
) -> sqlx::Result<Vec<Uuid>> {
    sqlx::query_scalar!(
        "SELECT s.id FROM source_files s JOIN works w ON w.id = s.work_id WHERE w.id = $1 AND w.organization_id = $2",
        work_id,
        organization_id
    )
    .fetch_all(db)
    .await
}

pub async fn delete_work_select_parts(db: impl sqlx::PgExecutor<'_>, work_id: Uuid) -> sqlx::Result<Vec<Uuid>> {
    sqlx::query_scalar!("SELECT id FROM parts WHERE work_id = $1", work_id).fetch_all(db).await
}

pub async fn delete_work_select_recordings(db: impl sqlx::PgExecutor<'_>, work_id: Uuid) -> sqlx::Result<Vec<Uuid>> {
    sqlx::query_scalar!("SELECT id FROM recordings WHERE work_id = $1", work_id).fetch_all(db).await
}

pub async fn delete_work_delete_works(
    db: impl sqlx::PgExecutor<'_>,
    work_id: Uuid,
    organization_id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("DELETE FROM works WHERE id = $1 AND organization_id = $2", work_id, organization_id).execute(db).await
}

pub async fn upload_source_select_source_files(
    db: impl sqlx::PgExecutor<'_>,
    work_id: Uuid,
    sha256: &str,
) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar!("SELECT file_name FROM source_files WHERE work_id = $1 AND sha256 = $2", work_id, sha256)
        .fetch_optional(db)
        .await
}

#[allow(clippy::too_many_arguments)] // one parameter per column
pub async fn upload_source_insert_source_files(
    db: impl sqlx::PgExecutor<'_>,
    id: Uuid,
    work_id: Uuid,
    file_name: &str,
    len: i64,
    sha256: &str,
    page_count: i32,
    as_ref: &str,
) -> sqlx::Result<SourcefileRow> {
    sqlx::query_as!(
        SourcefileRow,
        r#"INSERT INTO source_files (id, work_id, file_name, size, sha256, page_count, storage_key)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           RETURNING id, work_id, file_name, size, sha256, page_count, created_at"#,
        id,
        work_id,
        file_name,
        len,
        sha256,
        page_count,
        as_ref
    )
    .fetch_one(db)
    .await
}

pub async fn delete_source_select_parts(db: impl sqlx::PgExecutor<'_>, source_id: Uuid) -> sqlx::Result<Vec<Uuid>> {
    sqlx::query_scalar!("SELECT id FROM parts WHERE source_id = $1", source_id).fetch_all(db).await
}

pub async fn delete_source_delete_source_files(
    db: impl sqlx::PgExecutor<'_>,
    source_id: Uuid,
    organization_id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "DELETE FROM source_files s USING works w WHERE s.id = $1 AND w.id = s.work_id AND w.organization_id = $2",
        source_id,
        organization_id
    )
    .execute(db)
    .await
}

pub async fn source_file(
    db: impl sqlx::PgExecutor<'_>,
    source_id: Uuid,
    organization_id: Uuid,
) -> sqlx::Result<Option<SourceFileRow>> {
    sqlx::query_as!(
        SourceFileRow,
        "SELECT s.file_name FROM source_files s JOIN works w ON w.id = s.work_id
         WHERE s.id = $1 AND w.organization_id = $2",
        source_id,
        organization_id
    )
    .fetch_optional(db)
    .await
}

pub async fn render_part_select_instruments(
    db: impl sqlx::PgExecutor<'_>,
    instrument: Uuid,
    organization_id: Uuid,
) -> sqlx::Result<bool> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM instruments
                              WHERE id = $1 AND organization_id = $2) AS "known!""#,
        instrument,
        organization_id
    )
    .fetch_one(db)
    .await
}

pub async fn render_part_select_source_files(
    db: impl sqlx::PgExecutor<'_>,
    source_id: Uuid,
    work_id: Uuid,
) -> sqlx::Result<Option<i32>> {
    sqlx::query_scalar!("SELECT page_count FROM source_files WHERE id = $1 AND work_id = $2", source_id, work_id)
        .fetch_optional(db)
        .await
}

#[allow(clippy::too_many_arguments)] // one parameter per column
pub async fn create_part(
    db: impl sqlx::PgExecutor<'_>,
    id: Uuid,
    work_id: Uuid,
    source_id: Uuid,
    label: &str,
    instrument_id: Option<Uuid>,
    voice: Option<i16>,
    pages: &[i32],
    as_ref: &str,
) -> sqlx::Result<PartRow> {
    sqlx::query_as!(
        PartRow,
        r#"INSERT INTO parts (id, work_id, source_id, label, instrument_id, voice, pages, storage_key)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
           RETURNING id, work_id, source_id, label, instrument_id, voice, pages,
                     cardinality(pages) AS "page_count!", created_at, updated_at"#,
        id,
        work_id,
        source_id,
        label,
        instrument_id,
        voice,
        pages,
        as_ref
    )
    .fetch_one(db)
    .await
}

pub async fn find_part_work(
    db: impl sqlx::PgExecutor<'_>,
    part_id: Uuid,
    organization_id: Uuid,
) -> sqlx::Result<Option<Uuid>> {
    sqlx::query_scalar!(
        "SELECT p.work_id FROM parts p JOIN works w ON w.id = p.work_id WHERE p.id = $1 AND w.organization_id = $2",
        part_id,
        organization_id
    )
    .fetch_optional(db)
    .await
}

pub async fn update_part(
    db: impl sqlx::PgExecutor<'_>,
    part_id: Uuid,
    source_id: Uuid,
    label: &str,
    instrument_id: Option<Uuid>,
    voice: Option<i16>,
    pages: &[i32],
) -> sqlx::Result<PartRow> {
    sqlx::query_as!(
        PartRow,
        r#"UPDATE parts SET source_id = $2, label = $3, instrument_id = $4, voice = $5, pages = $6, updated_at = now()
           WHERE id = $1
           RETURNING id, work_id, source_id, label, instrument_id, voice, pages,
                     cardinality(pages) AS "page_count!", created_at, updated_at"#,
        part_id,
        source_id,
        label,
        instrument_id,
        voice,
        pages
    )
    .fetch_one(db)
    .await
}

pub async fn delete_part(db: impl sqlx::PgExecutor<'_>, part_id: Uuid) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("DELETE FROM parts WHERE id = $1", part_id).execute(db).await
}

pub async fn part_file(
    db: impl sqlx::PgExecutor<'_>,
    part_id: Uuid,
    organization_id: Uuid,
) -> sqlx::Result<Option<PartFileRow>> {
    sqlx::query_as!(
        PartFileRow,
        "SELECT p.label, w.title FROM parts p JOIN works w ON w.id = p.work_id
         WHERE p.id = $1 AND w.organization_id = $2",
        part_id,
        organization_id
    )
    .fetch_optional(db)
    .await
}
