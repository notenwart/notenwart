// SPDX-License-Identifier: AGPL-3.0-or-later

//! Instruments, works, their uploaded source PDFs and the parts split from them (`library` in api/openapi.yaml).

use std::collections::HashSet;

use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use object_store::{ObjectStoreExt, PutPayload};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::app::AppState;
use crate::auth::CurrentUser;
use crate::ensembles::may_see_part;
use crate::error::{ApiError, ApiResult};
use crate::pdf::{self, PdfError};
use crate::storage::{part_key, source_key};

pub fn router(max_upload: usize) -> Router<AppState> {
    Router::new()
        .route("/tags", get(list_tags))
        .route("/works/facets", get(work_facets))
        .route("/works", get(list_works).post(create_work))
        .route("/works/{work_id}", get(get_work).put(update_work).delete(delete_work))
        .route(
            "/works/{work_id}/sources",
            post(upload_source).layer(DefaultBodyLimit::max(max_upload)),
        )
        .route("/sources/{source_id}", axum::routing::delete(delete_source))
        .route("/sources/{source_id}/file", get(source_file))
        .route("/works/{work_id}/parts", post(create_part))
        .route("/parts/{part_id}", put(update_part).delete(delete_part))
        .route("/parts/{part_id}/file", get(part_file))
        .route("/recognize", post(recognize).layer(DefaultBodyLimit::max(max_upload)))
        .route("/sources/{source_id}/recognize", post(recognize_source))
        .route("/sources/{source_id}/split", post(split_source))
}

/// The organisation's instruments, for recognising parts.
async fn known_instruments(state: &AppState, user: &CurrentUser) -> ApiResult<Vec<crate::recognize::Known>> {
    Ok(sqlx::query!(
        "SELECT id, name, aliases FROM instruments WHERE organization_id = $1",
        user.organization_id
    )
    .fetch_all(&state.pool)
    .await?
    .into_iter()
    .map(|r| crate::recognize::Known {
        id: r.id,
        name: r.name,
        aliases: r.aliases,
    })
    .collect())
}

/// A stored source PDF of the user's organisation.
async fn source_bytes(state: &AppState, user: &CurrentUser, source_id: Uuid) -> ApiResult<bytes::Bytes> {
    sqlx::query!(
        "SELECT s.id FROM source_files s JOIN works w ON w.id = s.work_id
         WHERE s.id = $1 AND w.organization_id = $2",
        source_id,
        user.organization_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::NotFound)?;
    Ok(state
        .store
        .get(&source_key(user.organization_id, source_id))
        .await?
        .bytes()
        .await?)
}

/// Reads the first page of a stored source file (to fill in a work's details).
async fn recognize_source(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(source_id): Path<Uuid>,
) -> ApiResult<Json<crate::recognize::Recognition>> {
    user.require(user.is_staff())?;
    let bytes = source_bytes(&state, &user, source_id).await?;
    let instruments = known_instruments(&state, &user).await?;
    Ok(Json(state.recognizer.recognize(&bytes, &instruments).await?))
}

/// Proposes the parts in a source file with several, from the header of every page.
async fn split_source(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(source_id): Path<Uuid>,
) -> ApiResult<Json<Vec<crate::recognize::PartProposal>>> {
    user.require(user.is_staff())?;
    if !state.recognizer.abilities.text {
        return Err(ApiError::Conflict(
            "this server cannot read PDFs (pdftotext is missing)".into(),
        ));
    }
    let bytes = source_bytes(&state, &user, source_id).await?;
    let instruments = known_instruments(&state, &user).await?;
    Ok(Json(state.recognizer.split(&bytes, &instruments).await?))
}

/// Reads the first page of a PDF that is not stored (yet): title, composer, part … as suggestions.
async fn recognize(
    State(state): State<AppState>,
    user: CurrentUser,
    mut multipart: Multipart,
) -> ApiResult<Json<crate::recognize::Recognition>> {
    user.require(user.is_staff())?;
    let mut bytes = None;
    while let Some(field) = multipart.next_field().await.map_err(multipart_error)? {
        if field.name() == Some("file") {
            bytes = Some(field.bytes().await.map_err(multipart_error)?);
            break;
        }
    }
    let bytes = bytes.ok_or_else(|| ApiError::Invalid("missing form field `file`".into()))?;
    if !bytes.starts_with(b"%PDF") {
        return Err(ApiError::Invalid("not a PDF file".into()));
    }
    let instruments = known_instruments(&state, &user).await?;
    Ok(Json(state.recognizer.recognize(&bytes, &instruments).await?))
}

// ---- tags ----

async fn list_tags(State(state): State<AppState>, user: CurrentUser) -> ApiResult<Json<Vec<String>>> {
    user.require(user.reads_library())?;
    let tags = sqlx::query_scalar!(
        r#"SELECT DISTINCT tag AS "tag!" FROM works, unnest(tags) AS tag WHERE organization_id = $1 ORDER BY 1"#,
        user.organization_id
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(tags))
}

/// Values already used on works, most used first, for suggestions while typing.
#[derive(Serialize)]
pub struct Facets {
    composers: Vec<String>,
    arrangers: Vec<String>,
    publishers: Vec<String>,
    instrumentations: Vec<String>,
    tags: Vec<String>,
}

async fn work_facets(State(state): State<AppState>, user: CurrentUser) -> ApiResult<Json<Facets>> {
    user.require(user.reads_library())?;
    let rows = sqlx::query!(
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
        user.organization_id
    )
    .fetch_all(&state.pool)
    .await?;
    let mut facets = Facets {
        composers: vec![],
        arrangers: vec![],
        publishers: vec![],
        instrumentations: vec![],
        tags: vec![],
    };
    for row in rows {
        let list = match row.field.as_str() {
            "composer" => &mut facets.composers,
            "arranger" => &mut facets.arrangers,
            "publisher" => &mut facets.publishers,
            "instrumentation" => &mut facets.instrumentations,
            _ => &mut facets.tags,
        };
        list.push(row.value);
    }
    Ok(Json(facets))
}

// ---- works ----

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkInput {
    title: String,
    composer: Option<String>,
    arranger: Option<String>,
    publisher: Option<String>,
    #[serde(default)]
    lyricist: Option<String>,
    /// The performing rights society's number of the work (e.g. GEMA-Werknummer).
    #[serde(default)]
    society_number: Option<String>,
    /// Where the printed copies are kept, e.g. "Schrank 3, Fach B".
    #[serde(default)]
    archive_location: Option<String>,
    instrumentation: Option<String>,
    difficulty: Option<i16>,
    duration_seconds: Option<i32>,
    source: Option<String>,
    notes: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
}

impl WorkInput {
    /// Trims everything, turns empty strings into null, de-duplicates tags.
    fn normalize(mut self) -> ApiResult<Self> {
        fn clean(value: Option<String>) -> Option<String> {
            value.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
        }
        self.title = self.title.trim().to_string();
        let length = self.title.chars().count();
        if !(1..=300).contains(&length) {
            return Err(ApiError::Invalid("title must have 1 to 300 characters".into()));
        }
        if self.difficulty.is_some_and(|d| !(1..=6).contains(&d)) {
            return Err(ApiError::Invalid("difficulty must be between 1 and 6".into()));
        }
        if self.duration_seconds.is_some_and(|d| d < 0) {
            return Err(ApiError::Invalid("durationSeconds must not be negative".into()));
        }
        self.composer = clean(self.composer);
        self.arranger = clean(self.arranger);
        self.publisher = clean(self.publisher);
        self.lyricist = clean(self.lyricist);
        self.society_number = clean(self.society_number);
        self.archive_location = clean(self.archive_location);
        self.instrumentation = clean(self.instrumentation);
        self.source = clean(self.source);
        self.notes = clean(self.notes);
        let mut seen = HashSet::new();
        self.tags = self
            .tags
            .into_iter()
            .map(|tag| tag.trim().to_string())
            .filter(|tag| !tag.is_empty() && seen.insert(tag.to_lowercase()))
            .collect();
        Ok(self)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Work {
    id: Uuid,
    title: String,
    composer: Option<String>,
    arranger: Option<String>,
    publisher: Option<String>,
    lyricist: Option<String>,
    society_number: Option<String>,
    archive_location: Option<String>,
    instrumentation: Option<String>,
    difficulty: Option<i16>,
    duration_seconds: Option<i32>,
    source: Option<String>,
    notes: Option<String>,
    tags: Vec<String>,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    updated_at: OffsetDateTime,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkSummary {
    #[serde(flatten)]
    work: Work,
    source_count: i64,
    part_count: i64,
}

#[derive(Serialize)]
pub struct WorkDetail {
    #[serde(flatten)]
    work: Work,
    sources: Vec<SourceFile>,
    parts: Vec<Part>,
}

#[derive(Deserialize)]
pub struct WorkFilter {
    q: Option<String>,
    tag: Option<String>,
}

async fn list_works(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(filter): Query<WorkFilter>,
) -> ApiResult<Json<Vec<WorkSummary>>> {
    user.require(user.reads_library())?;
    // Substring match: escape LIKE's wildcards in the user's text.
    let pattern = filter
        .q
        .map(|q| q.trim().to_string())
        .filter(|q| !q.is_empty())
        .map(|q| format!("%{}%", q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")));
    let tag = filter.tag.filter(|t| !t.is_empty());
    let rows = sqlx::query!(
        r#"SELECT w.id, w.title, w.composer, w.arranger, w.publisher, w.lyricist, w.society_number, w.archive_location, w.instrumentation, w.difficulty,
                  w.duration_seconds, w.source, w.notes, w.tags, w.created_at, w.updated_at,
                  (SELECT count(*) FROM source_files s WHERE s.work_id = w.id) AS "source_count!",
                  (SELECT count(*) FROM parts p WHERE p.work_id = w.id) AS "part_count!"
           FROM works w
           WHERE w.organization_id = $1
             AND ($2::text IS NULL OR w.title ILIKE $2 OR w.composer ILIKE $2 OR w.arranger ILIKE $2
                  OR w.publisher ILIKE $2 OR w.lyricist ILIKE $2 OR w.instrumentation ILIKE $2
                  OR w.society_number ILIKE $2 OR array_to_string(w.tags, ' ') ILIKE $2)
             AND ($3::text IS NULL OR $3 = ANY (w.tags))
           ORDER BY lower(w.title), w.created_at"#,
        user.organization_id,
        pattern,
        tag
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|r| WorkSummary {
                work: Work {
                    id: r.id,
                    title: r.title,
                    composer: r.composer,
                    arranger: r.arranger,
                    publisher: r.publisher,
                    lyricist: r.lyricist,
                    society_number: r.society_number,
                    archive_location: r.archive_location,
                    instrumentation: r.instrumentation,
                    difficulty: r.difficulty,
                    duration_seconds: r.duration_seconds,
                    source: r.source,
                    notes: r.notes,
                    tags: r.tags,
                    created_at: r.created_at,
                    updated_at: r.updated_at,
                },
                source_count: r.source_count,
                part_count: r.part_count,
            })
            .collect(),
    ))
}

async fn create_work(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(input): Json<WorkInput>,
) -> ApiResult<(StatusCode, Json<Work>)> {
    user.require(user.is_staff())?;
    let w = input.normalize()?;
    let work = sqlx::query_as!(
        Work,
        r#"INSERT INTO works (organization_id, title, composer, arranger, publisher, instrumentation, difficulty,
                              duration_seconds, source, notes, tags, lyricist, society_number, archive_location)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
           RETURNING id, title, composer, arranger, publisher, lyricist, society_number, archive_location, instrumentation, difficulty, duration_seconds,
                     source, notes, tags, created_at, updated_at"#,
        user.organization_id,
        w.title,
        w.composer,
        w.arranger,
        w.publisher,
        w.instrumentation,
        w.difficulty,
        w.duration_seconds,
        w.source,
        w.notes,
        &w.tags,
        w.lyricist,
        w.society_number,
        w.archive_location
    )
    .fetch_one(&state.pool)
    .await?;
    Ok((StatusCode::CREATED, Json(work)))
}

async fn find_work(state: &AppState, user: &CurrentUser, id: Uuid) -> ApiResult<Work> {
    sqlx::query_as!(
        Work,
        r#"SELECT id, title, composer, arranger, publisher, lyricist, society_number, archive_location, instrumentation, difficulty, duration_seconds,
                  source, notes, tags, created_at, updated_at
           FROM works WHERE id = $1 AND organization_id = $2"#,
        id,
        user.organization_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::NotFound)
}

async fn get_work(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(work_id): Path<Uuid>,
) -> ApiResult<Json<WorkDetail>> {
    user.require(user.reads_library())?;
    let work = find_work(&state, &user, work_id).await?;
    let sources = sqlx::query_as!(
        SourceFile,
        r#"SELECT id, work_id, file_name, size, sha256, page_count, created_at
           FROM source_files WHERE work_id = $1 ORDER BY created_at"#,
        work_id
    )
    .fetch_all(&state.pool)
    .await?;
    let parts = sqlx::query_as!(
        Part,
        r#"SELECT p.id, p.work_id, p.source_id, p.label, p.instrument_id, p.voice, p.pages,
                  cardinality(p.pages) AS "page_count!", p.created_at, p.updated_at
           FROM parts p LEFT JOIN instruments i ON i.id = p.instrument_id
           WHERE p.work_id = $1
           ORDER BY i.sort NULLS FIRST, p.voice NULLS FIRST, lower(p.label)"#,
        work_id
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(WorkDetail { work, sources, parts }))
}

async fn update_work(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(work_id): Path<Uuid>,
    Json(input): Json<WorkInput>,
) -> ApiResult<Json<Work>> {
    user.require(user.is_staff())?;
    let w = input.normalize()?;
    sqlx::query_as!(
        Work,
        r#"UPDATE works SET title = $3, composer = $4, arranger = $5, publisher = $6, instrumentation = $7,
                            difficulty = $8, duration_seconds = $9, source = $10, notes = $11, tags = $12,
                            lyricist = $13, society_number = $14, archive_location = $15, updated_at = now()
           WHERE id = $1 AND organization_id = $2
           RETURNING id, title, composer, arranger, publisher, lyricist, society_number, archive_location, instrumentation, difficulty, duration_seconds,
                     source, notes, tags, created_at, updated_at"#,
        work_id,
        user.organization_id,
        w.title,
        w.composer,
        w.arranger,
        w.publisher,
        w.instrumentation,
        w.difficulty,
        w.duration_seconds,
        w.source,
        w.notes,
        &w.tags,
        w.lyricist,
        w.society_number,
        w.archive_location
    )
    .fetch_optional(&state.pool)
    .await?
    .map(Json)
    .ok_or(ApiError::NotFound)
}

async fn delete_work(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(work_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    user.require(user.is_staff())?;
    let mut tx = state.pool.begin().await?;
    let sources = sqlx::query_scalar!(
        "SELECT s.id FROM source_files s JOIN works w ON w.id = s.work_id WHERE w.id = $1 AND w.organization_id = $2",
        work_id,
        user.organization_id
    )
    .fetch_all(&mut *tx)
    .await?;
    let parts = sqlx::query_scalar!("SELECT id FROM parts WHERE work_id = $1", work_id)
        .fetch_all(&mut *tx)
        .await?;
    let recordings = sqlx::query_scalar!("SELECT id FROM recordings WHERE work_id = $1", work_id)
        .fetch_all(&mut *tx)
        .await?;
    let deleted = sqlx::query!(
        "DELETE FROM works WHERE id = $1 AND organization_id = $2",
        work_id,
        user.organization_id
    )
    .execute(&mut *tx)
    .await?;
    if deleted.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    tx.commit().await?;
    let org = user.organization_id;
    remove_objects(
        &state,
        sources
            .into_iter()
            .map(|id| source_key(org, id))
            .chain(parts.into_iter().map(|id| part_key(org, id)))
            .chain(recordings.into_iter().map(|id| crate::storage::recording_key(org, id))),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

// ---- source files ----

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceFile {
    id: Uuid,
    work_id: Uuid,
    file_name: String,
    size: i64,
    sha256: String,
    page_count: i32,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
}

async fn upload_source(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(work_id): Path<Uuid>,
    mut multipart: Multipart,
) -> ApiResult<(StatusCode, Json<SourceFile>)> {
    user.require(user.is_staff())?;
    find_work(&state, &user, work_id).await?;

    let mut upload = None;
    while let Some(field) = multipart.next_field().await.map_err(multipart_error)? {
        if field.name() == Some("file") {
            let file_name = clean_file_name(field.file_name().unwrap_or("upload.pdf"));
            let bytes = field.bytes().await.map_err(multipart_error)?;
            upload = Some((file_name, bytes));
            break;
        }
    }
    let (file_name, bytes) = upload.ok_or_else(|| ApiError::Invalid("missing form field `file`".into()))?;

    let sha256 = hex::encode(Sha256::digest(&bytes));
    if let Some(existing) = sqlx::query_scalar!(
        "SELECT file_name FROM source_files WHERE work_id = $1 AND sha256 = $2",
        work_id,
        sha256
    )
    .fetch_optional(&state.pool)
    .await?
    {
        return Err(ApiError::Conflict(format!(
            "this file is already uploaded as “{existing}”"
        )));
    }

    let page_count = {
        let bytes = bytes.clone();
        tokio::task::spawn_blocking(move || pdf::page_count(&bytes)).await?
    }
    .map_err(pdf_error)?;

    let id = Uuid::new_v4();
    let key = source_key(user.organization_id, id);
    state.store.put(&key, PutPayload::from_bytes(bytes.clone())).await?;

    let inserted = sqlx::query_as!(
        SourceFile,
        r#"INSERT INTO source_files (id, work_id, file_name, size, sha256, page_count, storage_key)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           RETURNING id, work_id, file_name, size, sha256, page_count, created_at"#,
        id,
        work_id,
        file_name,
        bytes.len() as i64,
        sha256,
        page_count as i32,
        key.as_ref()
    )
    .fetch_one(&state.pool)
    .await;
    match inserted {
        Ok(source) => Ok((StatusCode::CREATED, Json(source))),
        Err(error) => {
            remove_objects(&state, [key]).await;
            Err(error.into())
        }
    }
}

async fn delete_source(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(source_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    user.require(user.is_staff())?;
    let mut tx = state.pool.begin().await?;
    let parts = sqlx::query_scalar!("SELECT id FROM parts WHERE source_id = $1", source_id)
        .fetch_all(&mut *tx)
        .await?;
    let deleted = sqlx::query!(
        "DELETE FROM source_files s USING works w WHERE s.id = $1 AND w.id = s.work_id AND w.organization_id = $2",
        source_id,
        user.organization_id
    )
    .execute(&mut *tx)
    .await?;
    if deleted.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    tx.commit().await?;
    let org = user.organization_id;
    remove_objects(
        &state,
        std::iter::once(source_key(org, source_id)).chain(parts.into_iter().map(|id| part_key(org, id))),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

async fn source_file(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(source_id): Path<Uuid>,
) -> ApiResult<Response> {
    user.require(user.reads_library())?;
    let row = sqlx::query!(
        "SELECT s.file_name FROM source_files s JOIN works w ON w.id = s.work_id
         WHERE s.id = $1 AND w.organization_id = $2",
        source_id,
        user.organization_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::NotFound)?;
    serve_pdf(&state, &source_key(user.organization_id, source_id), &row.file_name).await
}

// ---- parts ----

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PartInput {
    label: String,
    instrument_id: Option<Uuid>,
    voice: Option<i16>,
    source_id: Uuid,
    pages: Vec<i32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    id: Uuid,
    work_id: Uuid,
    source_id: Uuid,
    label: String,
    instrument_id: Option<Uuid>,
    voice: Option<i16>,
    pages: Vec<i32>,
    page_count: i32,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    updated_at: OffsetDateTime,
}

/// Checks a part against its work and renders its PDF into storage.
async fn render_part(
    state: &AppState,
    user: &CurrentUser,
    work_id: Uuid,
    part_id: Uuid,
    input: &mut PartInput,
) -> ApiResult<()> {
    input.label = input.label.trim().to_string();
    if !(1..=120).contains(&input.label.chars().count()) {
        return Err(ApiError::Invalid("label must have 1 to 120 characters".into()));
    }
    if input.voice.is_some_and(|v| v < 1) {
        return Err(ApiError::Invalid("voice must be at least 1".into()));
    }
    if let Some(instrument) = input.instrument_id {
        let known = sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM instruments
                              WHERE id = $1 AND organization_id = $2) AS "known!""#,
            instrument,
            user.organization_id
        )
        .fetch_one(&state.pool)
        .await?;
        if !known {
            return Err(ApiError::Invalid("unknown instrument".into()));
        }
    }
    let page_count = sqlx::query_scalar!(
        "SELECT page_count FROM source_files WHERE id = $1 AND work_id = $2",
        input.source_id,
        work_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::Invalid("sourceId is not a source file of this work".into()))?;
    if input.pages.is_empty() {
        return Err(ApiError::Invalid("pages must not be empty".into()));
    }
    let mut seen = HashSet::new();
    for &page in &input.pages {
        if !(1..=page_count).contains(&page) {
            return Err(ApiError::Invalid(format!("page {page} is outside 1–{page_count}")));
        }
        if !seen.insert(page) {
            return Err(ApiError::Invalid(format!("page {page} appears twice")));
        }
    }

    let source = state
        .store
        .get(&source_key(user.organization_id, input.source_id))
        .await?
        .bytes()
        .await?;
    let pages: Vec<u32> = input.pages.iter().map(|&p| p as u32).collect();
    let rendered = tokio::task::spawn_blocking(move || pdf::extract(&source, &pages))
        .await?
        .map_err(pdf_error)?;
    state
        .store
        .put(&part_key(user.organization_id, part_id), PutPayload::from(rendered))
        .await?;
    Ok(())
}

async fn create_part(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(work_id): Path<Uuid>,
    Json(mut input): Json<PartInput>,
) -> ApiResult<(StatusCode, Json<Part>)> {
    user.require(user.is_staff())?;
    find_work(&state, &user, work_id).await?;
    let id = Uuid::new_v4();
    render_part(&state, &user, work_id, id, &mut input).await?;
    let key = part_key(user.organization_id, id);
    let inserted = sqlx::query_as!(
        Part,
        r#"INSERT INTO parts (id, work_id, source_id, label, instrument_id, voice, pages, storage_key)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
           RETURNING id, work_id, source_id, label, instrument_id, voice, pages,
                     cardinality(pages) AS "page_count!", created_at, updated_at"#,
        id,
        work_id,
        input.source_id,
        input.label,
        input.instrument_id,
        input.voice,
        &input.pages,
        key.as_ref()
    )
    .fetch_one(&state.pool)
    .await;
    match inserted {
        Ok(part) => {
            let recipients = crate::notify::who_sees_part(&state, part.id, user.id).await;
            let title = crate::notify::part_names(&state, part.id)
                .await
                .map(|n| n.1)
                .unwrap_or_default();
            crate::notify::later(
                &state,
                user.organization_id,
                crate::notify::Kind::PartsNew,
                recipients,
                serde_json::json!({ "part": part.label, "work": title }),
                "/".into(),
            );
            Ok((StatusCode::CREATED, Json(part)))
        }
        Err(error) => {
            remove_objects(&state, [key]).await;
            Err(error.into())
        }
    }
}

async fn find_part_work(state: &AppState, user: &CurrentUser, part_id: Uuid) -> ApiResult<Uuid> {
    sqlx::query_scalar!(
        "SELECT p.work_id FROM parts p JOIN works w ON w.id = p.work_id WHERE p.id = $1 AND w.organization_id = $2",
        part_id,
        user.organization_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::NotFound)
}

async fn update_part(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(part_id): Path<Uuid>,
    Json(mut input): Json<PartInput>,
) -> ApiResult<Json<Part>> {
    user.require(user.is_staff())?;
    let work_id = find_part_work(&state, &user, part_id).await?;
    render_part(&state, &user, work_id, part_id, &mut input).await?;
    let part = sqlx::query_as!(
        Part,
        r#"UPDATE parts SET source_id = $2, label = $3, instrument_id = $4, voice = $5, pages = $6, updated_at = now()
           WHERE id = $1
           RETURNING id, work_id, source_id, label, instrument_id, voice, pages,
                     cardinality(pages) AS "page_count!", created_at, updated_at"#,
        part_id,
        input.source_id,
        input.label,
        input.instrument_id,
        input.voice,
        &input.pages
    )
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(part))
}

async fn delete_part(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(part_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    user.require(user.is_staff())?;
    find_part_work(&state, &user, part_id).await?;
    sqlx::query!("DELETE FROM parts WHERE id = $1", part_id)
        .execute(&state.pool)
        .await?;
    remove_objects(&state, [part_key(user.organization_id, part_id)]).await;
    Ok(StatusCode::NO_CONTENT)
}

async fn part_file(State(state): State<AppState>, user: CurrentUser, Path(part_id): Path<Uuid>) -> ApiResult<Response> {
    if !may_see_part(&state, &user, part_id).await? {
        return Err(ApiError::NotFound);
    }
    let row = sqlx::query!(
        "SELECT p.label, w.title FROM parts p JOIN works w ON w.id = p.work_id
         WHERE p.id = $1 AND w.organization_id = $2",
        part_id,
        user.organization_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::NotFound)?;
    let file_name = clean_file_name(&format!("{} – {}.pdf", row.title, row.label));
    serve_pdf(&state, &part_key(user.organization_id, part_id), &file_name).await
}

// ---- helpers ----

pub(crate) async fn serve_pdf(
    state: &AppState,
    key: &object_store::path::Path,
    file_name: &str,
) -> ApiResult<Response> {
    let object = match state.store.get(key).await {
        Ok(object) => object,
        Err(object_store::Error::NotFound { .. }) => {
            tracing::error!("{key} is in the database but not in storage");
            return Err(ApiError::NotFound);
        }
        Err(error) => return Err(error.into()),
    };
    let size = object.meta.size;
    Ok((
        [
            (header::CONTENT_TYPE, "application/pdf".to_string()),
            (header::CONTENT_LENGTH, size.to_string()),
            (header::CONTENT_DISPOSITION, content_disposition(file_name)),
            (header::CACHE_CONTROL, "private, no-cache".to_string()),
        ],
        Body::from_stream(object.into_stream()),
    )
        .into_response())
}

/// `inline` with an ASCII fallback and the full UTF-8 name (RFC 6266).
fn content_disposition(file_name: &str) -> String {
    let ascii: String = file_name
        .chars()
        .map(|c| {
            if c.is_ascii_graphic() && c != '"' && c != '\\' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let encoded = percent_encoding::utf8_percent_encode(file_name, percent_encoding::NON_ALPHANUMERIC);
    format!("inline; filename=\"{ascii}\"; filename*=UTF-8''{encoded}")
}

/// The last path component, without control characters, at most 255 characters.
pub(crate) fn clean_file_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or_default();
    let cleaned: String = base.chars().filter(|c| !c.is_control()).take(255).collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        "upload.pdf".into()
    } else {
        cleaned.into()
    }
}

async fn remove_objects(state: &AppState, keys: impl IntoIterator<Item = object_store::path::Path>) {
    for key in keys {
        if let Err(error) = state.store.delete(&key).await
            && !matches!(error, object_store::Error::NotFound { .. })
        {
            tracing::warn!("cannot delete {key}: {error}");
        }
    }
}

fn multipart_error(error: axum::extract::multipart::MultipartError) -> ApiError {
    if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
        ApiError::TooLarge("the file is larger than this server allows".into())
    } else {
        ApiError::Invalid(error.body_text())
    }
}

fn pdf_error(error: PdfError) -> ApiError {
    match error {
        PdfError::Write(_) => ApiError::Internal(error.into()),
        _ => ApiError::Invalid(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_are_cleaned() {
        assert_eq!(clean_file_name("C:\\scans\\Trompete 2.pdf"), "Trompete 2.pdf");
        assert_eq!(clean_file_name("../../etc/passwd"), "passwd");
        assert_eq!(clean_file_name("  "), "upload.pdf");
    }

    #[test]
    fn content_disposition_keeps_unicode() {
        assert_eq!(
            content_disposition("Flöte 1.pdf"),
            "inline; filename=\"Fl_te 1.pdf\"; filename*=UTF-8''Fl%C3%B6te%201%2Epdf"
        );
    }
}
