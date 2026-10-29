// SPDX-License-Identifier: AGPL-3.0-or-later

//! Recordings of works (`recordings` in api/openapi.json): full recordings and practice recordings of a part or
//! voice, recorded or generated. Audio only, in the common formats (MP3, AAC/M4A including voice memos, WAV, FLAC,
//! Ogg/Opus, WebM audio, AIFF). Served with HTTP ranges, so players can seek.
//!
//! Whoever sees a part of a work hears its recordings; staff and the leaders of an ensemble with the work add them.

use futures_util::stream::BoxStream;
use object_store::{GetOptions, GetRange, ObjectStoreExt, PutPayload};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::App;
use crate::auth::CurrentUser;
use crate::error::{AppError, AppResult};
use crate::files::Upload;
use crate::library::clean_file_name;
use crate::storage::recording_key;
use notenwart_store::recordings as db;

/// The audio format of a file, from its first bytes (and the name where containers are ambiguous), or none.
pub fn audio_type(bytes: &[u8], file_name: &str) -> Option<&'static str> {
    let name = file_name.to_lowercase();
    let at = |offset: usize, magic: &[u8]| {
        bytes.len() >= offset + magic.len() && &bytes[offset..offset + magic.len()] == magic
    };
    if at(0, b"ID3") || (bytes.len() > 1 && bytes[0] == 0xFF && bytes[1] & 0xE0 == 0xE0 && !name.ends_with(".aac")) {
        return Some("audio/mpeg");
    }
    if bytes.len() > 1 && bytes[0] == 0xFF && bytes[1] & 0xF6 == 0xF0 {
        return Some("audio/aac");
    }
    if at(0, b"RIFF") && at(8, b"WAVE") {
        return Some("audio/wav");
    }
    if at(0, b"fLaC") {
        return Some("audio/flac");
    }
    if at(0, b"OggS") {
        return Some(if name.ends_with(".opus") { "audio/opus" } else { "audio/ogg" });
    }
    if at(0, b"FORM") && (at(8, b"AIFF") || at(8, b"AIFC")) {
        return Some("audio/aiff");
    }
    // MP4 and WebM can hold video too: only with an audio name (voice memos are .m4a).
    if at(4, b"ftyp") && [".m4a", ".m4b", ".aac", ".mp4a"].iter().any(|e| name.ends_with(e)) {
        return Some("audio/mp4");
    }
    if at(0, &[0x1A, 0x45, 0xDF, 0xA3]) && (name.ends_with(".weba") || name.ends_with(".webm")) {
        return Some("audio/webm");
    }
    None
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Recording {
    id: Uuid,
    work_id: Uuid,
    #[schema(schema_with = crate::schema::strings::<RecordingKind>)]
    kind: String,
    #[schema(schema_with = crate::schema::strings::<RecordingOrigin>)]
    origin: String,
    title: String,
    part_id: Option<Uuid>,
    instrument_id: Option<Uuid>,
    voice: Option<i16>,
    file_name: String,
    content_type: String,
    size: i64,
    duration_seconds: Option<f32>,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
}

/// Staff and readers of the library see every work; members those they have a part of.
async fn may_hear(state: &App, user: &CurrentUser, work_id: Uuid) -> AppResult<bool> {
    if user.reads_library() {
        return Ok(db::may_hear_select_works(&state.pool, work_id, user.organization_id).await?);
    }
    Ok(db::may_hear_select_visible_parts(&state.pool, user.id, work_id).await?)
}

/// Staff, and leaders of an ensemble that has the work.
async fn may_manage(state: &App, user: &CurrentUser, work_id: Uuid) -> AppResult<bool> {
    if user.is_staff() {
        return may_hear(state, user, work_id).await;
    }
    Ok(db::may_manage(&state.pool, user.id, work_id, user.organization_id).await?)
}

async fn recordings(state: &App, work_id: Uuid) -> AppResult<Vec<Recording>> {
    Ok(db::recordings(&state.pool, work_id)
        .await
        .map(|rows| rows.into_iter().map(Recording::from).collect::<Vec<_>>())?)
}

pub async fn list(state: &App, user: &CurrentUser, work_id: Uuid) -> AppResult<Vec<Recording>> {
    if !may_hear(state, user, work_id).await? {
        return Err(AppError::NotFound);
    }
    recordings(state, work_id).await
}

#[derive(Debug, Deserialize, Default, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecordingInput {
    /// A recording of the whole work, or a practice recording of one part or voice.
    #[schema(schema_with = crate::schema::strings::<RecordingKind>)]
    kind: String,
    /// Played and recorded, or generated (e.g. from MIDI or notation software).
    #[schema(schema_with = crate::schema::strings::<RecordingOrigin>)]
    origin: String,
    title: String,
    /// Practice recordings only.
    part_id: Option<Uuid>,
    /// Practice recordings only, e.g. the voice "Sopran" when all voices share one score.
    instrument_id: Option<Uuid>,
    voice: Option<i16>,
    /// Measured by the uploading browser.
    duration_seconds: Option<f32>,
}

/// The kinds of recordings, as the API names them.
#[derive(Debug)]
pub struct RecordingKind;

impl crate::schema::Strings for RecordingKind {
    const VALUES: &[&str] = &["full", "practice"];
}

/// Where recordings come from, as the API names it.
#[derive(Debug)]
pub struct RecordingOrigin;

impl crate::schema::Strings for RecordingOrigin {
    const VALUES: &[&str] = &["recorded", "generated"];
}

struct Checked {
    kind: String,
    origin: String,
    title: String,
    part_id: Option<Uuid>,
    instrument_id: Option<Uuid>,
    voice: Option<i16>,
}

async fn check(state: &App, user: &CurrentUser, work_id: Uuid, input: &RecordingInput) -> AppResult<Checked> {
    if !["full", "practice"].contains(&input.kind.as_str()) {
        return Err(AppError::Invalid("kind must be full or practice".into()));
    }
    if !["recorded", "generated"].contains(&input.origin.as_str()) {
        return Err(AppError::Invalid("origin must be recorded or generated".into()));
    }
    let title = input.title.trim().to_string();
    if !(1..=120).contains(&title.chars().count()) {
        return Err(AppError::Invalid("title must have 1 to 120 characters".into()));
    }
    let practice = input.kind == "practice";
    if let Some(part) = input.part_id.filter(|_| practice) {
        let ok = db::check_select_parts(&state.pool, part, work_id).await?;
        if !ok {
            return Err(AppError::Invalid("the part is not of this work".into()));
        }
    }
    if let Some(instrument) = input.instrument_id.filter(|_| practice) {
        let ok = db::check_select_instruments(&state.pool, instrument, user.organization_id).await?;
        if !ok {
            return Err(AppError::Invalid("unknown instrument".into()));
        }
    }
    Ok(Checked {
        kind: input.kind.clone(),
        origin: input.origin.clone(),
        title,
        part_id: input.part_id.filter(|_| practice),
        instrument_id: input.instrument_id.filter(|_| practice),
        voice: input.voice.filter(|v| practice && *v >= 1),
    })
}

/// A recording as uploaded: the file, and what it is (`meta` in the multipart form).
#[derive(Debug, Default)]
pub struct RecordingUpload {
    pub file: Option<Upload>,
    pub meta: Option<RecordingInput>,
}

/// Adds a recording to a work. `upload` is what the interface read; a failed read counts only after the role.
pub async fn upload(
    state: &App,
    user: &CurrentUser,
    work_id: Uuid,
    upload: AppResult<RecordingUpload>,
) -> AppResult<Recording> {
    if !may_manage(state, user, work_id).await? {
        return Err(AppError::Forbidden);
    }
    let RecordingUpload { file, meta } = upload?;
    let file = file.map(|f| (clean_file_name(f.file_name.as_deref().unwrap_or("recording")), f.bytes));
    let (file_name, bytes) = file.ok_or_else(|| AppError::Invalid("missing form field `file`".into()))?;
    let input = meta.ok_or_else(|| AppError::Invalid("missing form field `meta`".into()))?;
    let content_type = audio_type(&bytes, &file_name)
        .ok_or_else(|| AppError::Invalid("not an audio file (MP3, M4A, WAV, FLAC, Ogg, Opus, WebM, AIFF)".into()))?;
    let c = check(state, user, work_id, &input).await?;
    let sha256 = hex::encode(Sha256::digest(&bytes));
    let duplicate = db::upload_select_recordings(&state.pool, work_id, &sha256).await?;
    if let Some(title) = duplicate {
        return Err(AppError::Conflict(format!("this recording is already here as “{title}”")));
    }
    let id = Uuid::new_v4();
    let key = recording_key(user.organization_id, id);
    let size = bytes.len() as i64;
    state.store.put(&key, PutPayload::from(bytes)).await?;
    let inserted = db::upload_insert_recordings(
        &state.pool,
        id,
        user.organization_id,
        work_id,
        &c.kind,
        &c.origin,
        &c.title,
        c.part_id,
        c.instrument_id,
        c.voice,
        &file_name,
        content_type,
        size,
        &sha256,
        input.duration_seconds.filter(|d| d.is_finite() && *d >= 0.0),
        key.as_ref(),
        user.actor(),
    )
    .await;
    if let Err(error) = inserted {
        let _ = state.store.delete(&key).await;
        return Err(error.into());
    }
    let recording = recordings(state, work_id).await?.into_iter().find(|r| r.id == id).ok_or(AppError::NotFound)?;
    Ok(recording)
}

/// The work of a recording of the user's organisation.
async fn work_of(state: &App, user: &CurrentUser, recording_id: Uuid) -> AppResult<Uuid> {
    db::work_of(&state.pool, recording_id, user.organization_id).await?.ok_or(AppError::NotFound)
}

pub async fn update(
    state: &App,
    user: &CurrentUser,
    recording_id: Uuid,
    input: RecordingInput,
) -> AppResult<Recording> {
    let work_id = work_of(state, user, recording_id).await?;
    if !may_manage(state, user, work_id).await? {
        return Err(AppError::Forbidden);
    }
    let c = check(state, user, work_id, &input).await?;
    db::update(
        &state.pool,
        recording_id,
        &c.kind,
        &c.origin,
        &c.title,
        c.part_id,
        c.instrument_id,
        c.voice,
        input.duration_seconds.filter(|d| d.is_finite() && *d >= 0.0),
    )
    .await?;
    recordings(state, work_id).await?.into_iter().find(|r| r.id == recording_id).ok_or(AppError::NotFound)
}

pub async fn remove(state: &App, user: &CurrentUser, recording_id: Uuid) -> AppResult<()> {
    let work_id = work_of(state, user, recording_id).await?;
    if !may_manage(state, user, work_id).await? {
        return Err(AppError::Forbidden);
    }
    db::remove(&state.pool, recording_id).await?;
    let _ = state.store.delete(&recording_key(user.organization_id, recording_id)).await;
    Ok(())
}

/// `bytes=a-b`, `bytes=a-` or `bytes=-n` (one range) for an object of `size` bytes; none when it cannot be served.
pub fn parse_range(header: &str, size: u64) -> Option<std::ops::Range<u64>> {
    let spec = header.trim().strip_prefix("bytes=")?;
    if spec.contains(',') || size == 0 {
        return None;
    }
    let (start, end) = spec.split_once('-')?;
    let range = match (start.trim(), end.trim()) {
        ("", n) => size.saturating_sub(n.parse().ok()?)..size,
        (a, "") => a.parse().ok()?..size,
        (a, b) => {
            let (a, b): (u64, u64) = (a.parse().ok()?, b.parse().ok()?);
            a..(b + 1).min(size)
        }
    };
    (range.start < range.end && range.start < size).then_some(range)
}

/// A recording to send: all of it or the asked-for range.
pub enum RecordingFile {
    /// The asked-for range lies outside the file of this size.
    Unsatisfiable { size: u64 },
    /// The file, or the part of it in `range`.
    Content {
        content_type: String,
        file_name: String,
        size: u64,
        range: Option<std::ops::Range<u64>>,
        content: BoxStream<'static, object_store::Result<bytes::Bytes>>,
    },
}

impl std::fmt::Debug for RecordingFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RecordingFile::Unsatisfiable { size } => f.debug_struct("Unsatisfiable").field("size", size).finish(),
            RecordingFile::Content { content_type, file_name, size, range, .. } => f
                .debug_struct("Content")
                .field("content_type", content_type)
                .field("file_name", file_name)
                .field("size", size)
                .field("range", range)
                .finish(),
        }
    }
}

/// A recording for whoever may hear it; `range` is an HTTP `Range` header (`bytes=…`), if one was sent.
pub async fn file(
    state: &App,
    user: &CurrentUser,
    recording_id: Uuid,
    range: Option<&str>,
) -> AppResult<RecordingFile> {
    let work_id = work_of(state, user, recording_id).await?;
    if !may_hear(state, user, work_id).await? {
        return Err(AppError::NotFound);
    }
    let row = db::file(&state.pool, recording_id).await?;
    let size = row.size as u64;
    let key = recording_key(user.organization_id, recording_id);
    let range = match range.map(|r| parse_range(r, size)) {
        Some(None) => return Ok(RecordingFile::Unsatisfiable { size }),
        Some(Some(range)) => Some(range),
        None => None,
    };
    let options = GetOptions::default().with_range(range.clone().map(GetRange::Bounded));
    let object = state.store.get_opts(&key, options).await?;
    Ok(RecordingFile::Content {
        content_type: row.content_type,
        file_name: row.file_name,
        size,
        range,
        content: object.into_stream(),
    })
}

impl From<db::RecordingRow> for Recording {
    fn from(row: db::RecordingRow) -> Self {
        Recording {
            id: row.id,
            work_id: row.work_id,
            kind: row.kind,
            origin: row.origin,
            title: row.title,
            part_id: row.part_id,
            instrument_id: row.instrument_id,
            voice: row.voice,
            file_name: row.file_name,
            content_type: row.content_type,
            size: row.size,
            duration_seconds: row.duration_seconds,
            created_at: row.created_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knows_audio_and_refuses_the_rest() {
        assert_eq!(audio_type(b"ID3\x04\0\0", "a.mp3"), Some("audio/mpeg"));
        assert_eq!(audio_type(b"RIFF\0\0\0\0WAVEfmt ", "a.wav"), Some("audio/wav"));
        assert_eq!(audio_type(b"fLaC\0\0", "a.flac"), Some("audio/flac"));
        assert_eq!(audio_type(b"OggS\0\0", "a.opus"), Some("audio/opus"));
        assert_eq!(audio_type(b"\0\0\0\x20ftypM4A ", "Sprachmemo.m4a"), Some("audio/mp4"));
        assert_eq!(audio_type(b"\0\0\0\x20ftypisom", "video.mp4"), None, "no videos");
        assert_eq!(audio_type(b"%PDF-1.7", "a.mp3"), None);
    }

    #[test]
    fn reads_byte_ranges() {
        assert_eq!(parse_range("bytes=0-99", 1000), Some(0..100));
        assert_eq!(parse_range("bytes=900-", 1000), Some(900..1000));
        assert_eq!(parse_range("bytes=-100", 1000), Some(900..1000));
        assert_eq!(parse_range("bytes=990-2000", 1000), Some(990..1000));
        assert_eq!(parse_range("bytes=1000-", 1000), None);
        assert_eq!(parse_range("bytes=0-1,5-6", 1000), None);
    }
}
