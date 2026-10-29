// SPDX-License-Identifier: AGPL-3.0-or-later

//! Instruments, works, their uploaded source PDFs and the parts split from them (`library` in api/openapi.json).

use std::collections::HashSet;

use futures_util::stream::BoxStream;
use object_store::{ObjectStoreExt, PutPayload};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::App;
use crate::auth::CurrentUser;
use crate::ensembles::may_see_part;
use crate::error::{AppError, AppResult};
use crate::files::Upload;
use crate::pdf::{self, PdfError};
use crate::storage::{part_key, source_key};
use notenwart_store::library as db;

/// The organisation's instruments, for recognising parts.
async fn known_instruments(state: &App, user: &CurrentUser) -> AppResult<Vec<crate::recognize::Known>> {
    Ok(db::known_instruments(&state.pool, user.organization_id)
        .await?
        .into_iter()
        .map(|r| crate::recognize::Known { id: r.id, name: r.name, aliases: r.aliases })
        .collect())
}

/// A stored source PDF of the user's organisation.
async fn source_bytes(state: &App, user: &CurrentUser, source_id: Uuid) -> AppResult<bytes::Bytes> {
    db::source_bytes(&state.pool, source_id, user.organization_id).await?.ok_or(AppError::NotFound)?;
    Ok(state.store.get(&source_key(user.organization_id, source_id)).await?.bytes().await?)
}

/// Reads the first page of a stored source file (to fill in a work's details).
pub async fn recognize_source(
    state: &App,
    user: &CurrentUser,
    source_id: Uuid,
) -> AppResult<crate::recognize::Recognition> {
    user.require(user.is_staff())?;
    let bytes = source_bytes(state, user, source_id).await?;
    let instruments = known_instruments(state, user).await?;
    Ok(state.recognizer.recognize(&bytes, &instruments).await?)
}

/// Proposes the parts in a source file with several, from the header of every page.
pub async fn split_source(
    state: &App,
    user: &CurrentUser,
    source_id: Uuid,
) -> AppResult<Vec<crate::recognize::PartProposal>> {
    user.require(user.is_staff())?;
    if !state.recognizer.abilities.text {
        return Err(AppError::Conflict("this server cannot read PDFs (pdftotext is missing)".into()));
    }
    let bytes = source_bytes(state, user, source_id).await?;
    let instruments = known_instruments(state, user).await?;
    Ok(state.recognizer.split(&bytes, &instruments).await?)
}

/// Reads the first page of a PDF that is not stored (yet): title, composer, part … as suggestions.
///
/// `upload` is the file as the interface read it; a failed read counts only after the role is checked.
pub async fn recognize(
    state: &App,
    user: &CurrentUser,
    upload: AppResult<Option<Upload>>,
) -> AppResult<crate::recognize::Recognition> {
    user.require(user.is_staff())?;
    let bytes = upload?.ok_or_else(|| AppError::Invalid("missing form field `file`".into()))?.bytes;
    if !bytes.starts_with(b"%PDF") {
        return Err(AppError::Invalid("not a PDF file".into()));
    }
    let instruments = known_instruments(state, user).await?;
    Ok(state.recognizer.recognize(&bytes, &instruments).await?)
}

// ---- tags ----

pub async fn list_tags(state: &App, user: &CurrentUser) -> AppResult<Vec<String>> {
    user.require(user.reads_library())?;
    let tags = db::list_tags(&state.pool, user.organization_id).await?;
    Ok(tags)
}

/// Values already used on works, most used first, for suggestions while typing.
#[derive(Debug, Serialize, utoipa::ToSchema)]
#[schema(as = WorkFacets)]
pub struct Facets {
    composers: Vec<String>,
    arrangers: Vec<String>,
    publishers: Vec<String>,
    instrumentations: Vec<String>,
    tags: Vec<String>,
}

pub async fn work_facets(state: &App, user: &CurrentUser) -> AppResult<Facets> {
    user.require(user.reads_library())?;
    let rows = db::work_facets(&state.pool, user.organization_id).await?;
    let mut facets =
        Facets { composers: vec![], arrangers: vec![], publishers: vec![], instrumentations: vec![], tags: vec![] };
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
    Ok(facets)
}

// ---- works ----

#[derive(Debug, Deserialize, utoipa::ToSchema)]
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
    /// Free text, e.g. "Symphonic band" or "SATB".
    instrumentation: Option<String>,
    difficulty: Option<i16>,
    duration_seconds: Option<i32>,
    /// Where the music comes from or under which licence it is used (free text, for the operator).
    source: Option<String>,
    notes: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
}

impl WorkInput {
    /// Trims everything, turns empty strings into null, de-duplicates tags.
    fn normalize(mut self) -> AppResult<Self> {
        fn clean(value: Option<String>) -> Option<String> {
            value.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
        }
        self.title = self.title.trim().to_string();
        let length = self.title.chars().count();
        if !(1..=300).contains(&length) {
            return Err(AppError::Invalid("title must have 1 to 300 characters".into()));
        }
        if self.difficulty.is_some_and(|d| !(1..=6).contains(&d)) {
            return Err(AppError::Invalid("difficulty must be between 1 and 6".into()));
        }
        if self.duration_seconds.is_some_and(|d| d < 0) {
            return Err(AppError::Invalid("durationSeconds must not be negative".into()));
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

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Work {
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
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkSummary {
    #[serde(flatten)]
    pub work: Work,
    pub source_count: i64,
    pub part_count: i64,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct WorkDetail {
    #[serde(flatten)]
    pub work: Work,
    pub sources: Vec<SourceFile>,
    pub parts: Vec<Part>,
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct WorkFilter {
    /// Matches title, composer and arranger (case-insensitive, substring).
    pub q: Option<String>,
    /// Only works with this tag.
    pub tag: Option<String>,
}

pub async fn list_works(state: &App, user: &CurrentUser, filter: WorkFilter) -> AppResult<Vec<WorkSummary>> {
    user.require(user.reads_library())?;
    // Substring match: escape LIKE's wildcards in the user's text.
    let pattern = filter
        .q
        .map(|q| q.trim().to_string())
        .filter(|q| !q.is_empty())
        .map(|q| format!("%{}%", q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")));
    let tag = filter.tag.filter(|t| !t.is_empty());
    let rows = db::list_works(&state.pool, user.organization_id, pattern.as_deref(), tag.as_deref()).await?;
    Ok(rows
        .into_iter()
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
        .collect())
}

pub async fn create_work(state: &App, user: &CurrentUser, input: WorkInput) -> AppResult<Work> {
    user.require(user.is_staff())?;
    let w = input.normalize()?;
    let work = db::create_work(
        &state.pool,
        user.organization_id,
        &w.title,
        w.composer.as_deref(),
        w.arranger.as_deref(),
        w.publisher.as_deref(),
        w.instrumentation.as_deref(),
        w.difficulty,
        w.duration_seconds,
        w.source.as_deref(),
        w.notes.as_deref(),
        &w.tags,
        w.lyricist.as_deref(),
        w.society_number.as_deref(),
        w.archive_location.as_deref(),
    )
    .await
    .map(Work::from)?;
    Ok(work)
}

async fn find_work(state: &App, user: &CurrentUser, id: Uuid) -> AppResult<Work> {
    db::find_work(&state.pool, id, user.organization_id).await.map(|row| row.map(Work::from))?.ok_or(AppError::NotFound)
}

pub async fn get_work(state: &App, user: &CurrentUser, work_id: Uuid) -> AppResult<WorkDetail> {
    user.require(user.reads_library())?;
    let work = find_work(state, user, work_id).await?;
    let sources = db::get_work_select_source_files(&state.pool, work_id)
        .await
        .map(|rows| rows.into_iter().map(SourceFile::from).collect::<Vec<_>>())?;
    let parts = db::get_work_select_parts(&state.pool, work_id)
        .await
        .map(|rows| rows.into_iter().map(Part::from).collect::<Vec<_>>())?;
    Ok(WorkDetail { work, sources, parts })
}

pub async fn update_work(state: &App, user: &CurrentUser, work_id: Uuid, input: WorkInput) -> AppResult<Work> {
    user.require(user.is_staff())?;
    let w = input.normalize()?;
    db::update_work(
        &state.pool,
        work_id,
        user.organization_id,
        &w.title,
        w.composer.as_deref(),
        w.arranger.as_deref(),
        w.publisher.as_deref(),
        w.instrumentation.as_deref(),
        w.difficulty,
        w.duration_seconds,
        w.source.as_deref(),
        w.notes.as_deref(),
        &w.tags,
        w.lyricist.as_deref(),
        w.society_number.as_deref(),
        w.archive_location.as_deref(),
    )
    .await
    .map(|row| row.map(Work::from))?
    .ok_or(AppError::NotFound)
}

pub async fn delete_work(state: &App, user: &CurrentUser, work_id: Uuid) -> AppResult<()> {
    user.require(user.is_staff())?;
    let mut tx = state.pool.begin().await?;
    let sources = db::delete_work_select_source_files(&mut *tx, work_id, user.organization_id).await?;
    let parts = db::delete_work_select_parts(&mut *tx, work_id).await?;
    let recordings = db::delete_work_select_recordings(&mut *tx, work_id).await?;
    let deleted = db::delete_work_delete_works(&mut *tx, work_id, user.organization_id).await?;
    if deleted.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    tx.commit().await?;
    let org = user.organization_id;
    remove_objects(
        state,
        sources
            .into_iter()
            .map(|id| source_key(org, id))
            .chain(parts.into_iter().map(|id| part_key(org, id)))
            .chain(recordings.into_iter().map(|id| crate::storage::recording_key(org, id))),
    )
    .await;
    Ok(())
}

// ---- source files ----

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SourceFile {
    pub id: Uuid,
    pub work_id: Uuid,
    pub file_name: String,
    /// Bytes.
    pub size: i64,
    pub sha256: String,
    pub page_count: i32,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

/// Adds a source PDF to a work. `upload` is the file as the interface read it; a failed read counts only after the
/// role and the work are checked.
pub async fn upload_source(
    state: &App,
    user: &CurrentUser,
    work_id: Uuid,
    upload: AppResult<Option<Upload>>,
) -> AppResult<SourceFile> {
    user.require(user.is_staff())?;
    find_work(state, user, work_id).await?;

    let upload = upload?.ok_or_else(|| AppError::Invalid("missing form field `file`".into()))?;
    let file_name = clean_file_name(upload.file_name.as_deref().unwrap_or("upload.pdf"));
    let bytes = upload.bytes;

    let sha256 = hex::encode(Sha256::digest(&bytes));
    if let Some(existing) = db::upload_source_select_source_files(&state.pool, work_id, &sha256).await? {
        return Err(AppError::Conflict(format!("this file is already uploaded as “{existing}”")));
    }

    let page_count = {
        let bytes = bytes.clone();
        tokio::task::spawn_blocking(move || pdf::page_count(&bytes)).await?
    }
    .map_err(pdf_error)?;

    let id = Uuid::new_v4();
    let key = source_key(user.organization_id, id);
    state.store.put(&key, PutPayload::from_bytes(bytes.clone())).await?;

    let inserted = db::upload_source_insert_source_files(
        &state.pool,
        id,
        work_id,
        &file_name,
        bytes.len() as i64,
        &sha256,
        page_count as i32,
        key.as_ref(),
    )
    .await
    .map(SourceFile::from);
    match inserted {
        Ok(source) => Ok(source),
        Err(error) => {
            remove_objects(state, [key]).await;
            Err(error.into())
        }
    }
}

pub async fn delete_source(state: &App, user: &CurrentUser, source_id: Uuid) -> AppResult<()> {
    user.require(user.is_staff())?;
    let mut tx = state.pool.begin().await?;
    let parts = db::delete_source_select_parts(&mut *tx, source_id).await?;
    let deleted = db::delete_source_delete_source_files(&mut *tx, source_id, user.organization_id).await?;
    if deleted.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    tx.commit().await?;
    let org = user.organization_id;
    remove_objects(
        state,
        std::iter::once(source_key(org, source_id)).chain(parts.into_iter().map(|id| part_key(org, id))),
    )
    .await;
    Ok(())
}

/// A source PDF, to download.
pub async fn source_file(state: &App, user: &CurrentUser, source_id: Uuid) -> AppResult<PdfFile> {
    user.require(user.reads_library())?;
    let row = db::source_file(&state.pool, source_id, user.organization_id).await?.ok_or(AppError::NotFound)?;
    open_pdf(state, &source_key(user.organization_id, source_id), &row.file_name).await
}

// ---- parts ----

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PartInput {
    /// Display name, e.g. "Clarinet 2" or "Full score".
    label: String,
    /// Null for parts without a single instrument (full score, choir).
    instrument_id: Option<Uuid>,
    /// Part number for instruments with several parts (Clarinet 1, 2, 3).
    voice: Option<i16>,
    /// A source file of the same work.
    source_id: Uuid,
    /// 1-based page numbers of the source file in the order the part shows them; each page at most once.
    pages: Vec<i32>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    pub id: Uuid,
    pub work_id: Uuid,
    pub source_id: Uuid,
    pub label: String,
    pub instrument_id: Option<Uuid>,
    pub voice: Option<i16>,
    pub pages: Vec<i32>,
    pub page_count: i32,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

/// Checks a part against its work and renders its PDF into storage.
async fn render_part(
    state: &App,
    user: &CurrentUser,
    work_id: Uuid,
    part_id: Uuid,
    input: &mut PartInput,
) -> AppResult<()> {
    input.label = input.label.trim().to_string();
    if !(1..=120).contains(&input.label.chars().count()) {
        return Err(AppError::Invalid("label must have 1 to 120 characters".into()));
    }
    if input.voice.is_some_and(|v| v < 1) {
        return Err(AppError::Invalid("voice must be at least 1".into()));
    }
    if let Some(instrument) = input.instrument_id {
        let known = db::render_part_select_instruments(&state.pool, instrument, user.organization_id).await?;
        if !known {
            return Err(AppError::Invalid("unknown instrument".into()));
        }
    }
    let page_count = db::render_part_select_source_files(&state.pool, input.source_id, work_id)
        .await?
        .ok_or_else(|| AppError::Invalid("sourceId is not a source file of this work".into()))?;
    if input.pages.is_empty() {
        return Err(AppError::Invalid("pages must not be empty".into()));
    }
    let mut seen = HashSet::new();
    for &page in &input.pages {
        if !(1..=page_count).contains(&page) {
            return Err(AppError::Invalid(format!("page {page} is outside 1–{page_count}")));
        }
        if !seen.insert(page) {
            return Err(AppError::Invalid(format!("page {page} appears twice")));
        }
    }

    let source = state.store.get(&source_key(user.organization_id, input.source_id)).await?.bytes().await?;
    let pages: Vec<u32> = input.pages.iter().map(|&p| p as u32).collect();
    let rendered = tokio::task::spawn_blocking(move || pdf::extract(&source, &pages)).await?.map_err(pdf_error)?;
    state.store.put(&part_key(user.organization_id, part_id), PutPayload::from(rendered)).await?;
    Ok(())
}

pub async fn create_part(state: &App, user: &CurrentUser, work_id: Uuid, mut input: PartInput) -> AppResult<Part> {
    user.require(user.is_staff())?;
    find_work(state, user, work_id).await?;
    let id = Uuid::new_v4();
    render_part(state, user, work_id, id, &mut input).await?;
    let key = part_key(user.organization_id, id);
    let inserted = db::create_part(
        &state.pool,
        id,
        work_id,
        input.source_id,
        &input.label,
        input.instrument_id,
        input.voice,
        &input.pages,
        key.as_ref(),
    )
    .await
    .map(Part::from);
    match inserted {
        Ok(part) => {
            let recipients = crate::notify::who_sees_part(state, part.id, user.id).await;
            let title = crate::notify::part_names(state, part.id).await.map(|n| n.1).unwrap_or_default();
            crate::notify::later(
                state,
                user.organization_id,
                crate::notify::Kind::PartsNew,
                recipients,
                serde_json::json!({ "part": part.label, "work": title }),
                "/".into(),
            );
            Ok(part)
        }
        Err(error) => {
            remove_objects(state, [key]).await;
            Err(error.into())
        }
    }
}

async fn find_part_work(state: &App, user: &CurrentUser, part_id: Uuid) -> AppResult<Uuid> {
    db::find_part_work(&state.pool, part_id, user.organization_id).await?.ok_or(AppError::NotFound)
}

pub async fn update_part(state: &App, user: &CurrentUser, part_id: Uuid, mut input: PartInput) -> AppResult<Part> {
    user.require(user.is_staff())?;
    let work_id = find_part_work(state, user, part_id).await?;
    render_part(state, user, work_id, part_id, &mut input).await?;
    let part = db::update_part(
        &state.pool,
        part_id,
        input.source_id,
        &input.label,
        input.instrument_id,
        input.voice,
        &input.pages,
    )
    .await
    .map(Part::from)?;
    Ok(part)
}

pub async fn delete_part(state: &App, user: &CurrentUser, part_id: Uuid) -> AppResult<()> {
    user.require(user.is_staff())?;
    find_part_work(state, user, part_id).await?;
    db::delete_part(&state.pool, part_id).await?;
    remove_objects(state, [part_key(user.organization_id, part_id)]).await;
    Ok(())
}

/// A part's PDF, for whoever may see the part.
pub async fn part_file(state: &App, user: &CurrentUser, part_id: Uuid) -> AppResult<PdfFile> {
    if !may_see_part(state, user, part_id).await? {
        return Err(AppError::NotFound);
    }
    let row = db::part_file(&state.pool, part_id, user.organization_id).await?.ok_or(AppError::NotFound)?;
    let file_name = clean_file_name(&format!("{} – {}.pdf", row.title, row.label));
    open_pdf(state, &part_key(user.organization_id, part_id), &file_name).await
}

// ---- helpers ----

/// A stored PDF to send: its name, size and content.
pub struct PdfFile {
    pub file_name: String,
    pub size: u64,
    pub content: BoxStream<'static, object_store::Result<bytes::Bytes>>,
}

impl std::fmt::Debug for PdfFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PdfFile").field("file_name", &self.file_name).field("size", &self.size).finish()
    }
}

pub(crate) async fn open_pdf(state: &App, key: &object_store::path::Path, file_name: &str) -> AppResult<PdfFile> {
    let object = match state.store.get(key).await {
        Ok(object) => object,
        Err(object_store::Error::NotFound { .. }) => {
            tracing::error!("{key} is in the database but not in storage");
            return Err(AppError::NotFound);
        }
        Err(error) => return Err(error.into()),
    };
    Ok(PdfFile { file_name: file_name.to_string(), size: object.meta.size, content: object.into_stream() })
}

/// The last path component, without control characters, at most 255 characters.
pub(crate) fn clean_file_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or_default();
    let cleaned: String = base.chars().filter(|c| !c.is_control()).take(255).collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() { "upload.pdf".into() } else { cleaned.into() }
}

async fn remove_objects(state: &App, keys: impl IntoIterator<Item = object_store::path::Path>) {
    for key in keys {
        if let Err(error) = state.store.delete(&key).await
            && !matches!(error, object_store::Error::NotFound { .. })
        {
            tracing::warn!("cannot delete {key}: {error}");
        }
    }
}

fn pdf_error(error: PdfError) -> AppError {
    match error {
        PdfError::Write(_) => AppError::Internal(error.into()),
        _ => AppError::Invalid(error.to_string()),
    }
}

impl From<db::WorkRow> for Work {
    fn from(row: db::WorkRow) -> Self {
        Work {
            id: row.id,
            title: row.title,
            composer: row.composer,
            arranger: row.arranger,
            publisher: row.publisher,
            lyricist: row.lyricist,
            society_number: row.society_number,
            archive_location: row.archive_location,
            instrumentation: row.instrumentation,
            difficulty: row.difficulty,
            duration_seconds: row.duration_seconds,
            source: row.source,
            notes: row.notes,
            tags: row.tags,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl From<db::SourcefileRow> for SourceFile {
    fn from(row: db::SourcefileRow) -> Self {
        SourceFile {
            id: row.id,
            work_id: row.work_id,
            file_name: row.file_name,
            size: row.size,
            sha256: row.sha256,
            page_count: row.page_count,
            created_at: row.created_at,
        }
    }
}

impl From<db::PartRow> for Part {
    fn from(row: db::PartRow) -> Self {
        Part {
            id: row.id,
            work_id: row.work_id,
            source_id: row.source_id,
            label: row.label,
            instrument_id: row.instrument_id,
            voice: row.voice,
            pages: row.pages,
            page_count: row.page_count,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
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
}
