// SPDX-License-Identifier: AGPL-3.0-or-later

//! Recordings of works (`recordings` in api/openapi.yaml): full recordings and practice recordings of a part or
//! voice, recorded or generated. Audio only, in the common formats (MP3, AAC/M4A including voice memos, WAV, FLAC,
//! Ogg/Opus, WebM audio, AIFF). Served with HTTP ranges, so players can seek.
//!
//! Whoever sees a part of a work hears its recordings; staff and the leaders of an ensemble with the work add them.

use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Multipart, Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, put};
use axum::{Json, Router};
use object_store::{GetOptions, GetRange, ObjectStoreExt, PutPayload};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::app::AppState;
use crate::auth::CurrentUser;
use crate::error::{ApiError, ApiResult};
use crate::library::clean_file_name;
use crate::storage::recording_key;

pub fn router(max_upload: usize) -> Router<AppState> {
    Router::new()
        .route(
            "/works/{work_id}/recordings",
            get(list).post(upload).layer(DefaultBodyLimit::max(max_upload)),
        )
        .route("/recordings/{recording_id}", put(update).delete(remove))
        .route("/recordings/{recording_id}/file", get(file))
}

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
        return Some(if name.ends_with(".opus") {
            "audio/opus"
        } else {
            "audio/ogg"
        });
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recording {
    id: Uuid,
    work_id: Uuid,
    kind: String,
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
async fn may_hear(state: &AppState, user: &CurrentUser, work_id: Uuid) -> ApiResult<bool> {
    if user.reads_library() {
        return Ok(sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM works WHERE id = $1 AND organization_id = $2) AS "x!""#,
            work_id,
            user.organization_id
        )
        .fetch_one(&state.pool)
        .await?);
    }
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM visible_parts v JOIN parts p ON p.id = v.part_id
                          WHERE v.user_id = $1 AND p.work_id = $2) AS "x!""#,
        user.id,
        work_id
    )
    .fetch_one(&state.pool)
    .await?)
}

/// Staff, and leaders of an ensemble that has the work.
async fn may_manage(state: &AppState, user: &CurrentUser, work_id: Uuid) -> ApiResult<bool> {
    if user.is_staff() {
        return may_hear(state, user, work_id).await;
    }
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM ensemble_members m JOIN ensemble_works ew ON ew.ensemble_id = m.ensemble_id
                          JOIN works w ON w.id = ew.work_id
                          WHERE m.user_id = $1 AND m.leader AND ew.work_id = $2 AND w.organization_id = $3) AS "x!""#,
        user.id,
        work_id,
        user.organization_id
    )
    .fetch_one(&state.pool)
    .await?)
}

async fn recordings(state: &AppState, work_id: Uuid) -> ApiResult<Vec<Recording>> {
    Ok(sqlx::query_as!(
        Recording,
        r#"SELECT id, work_id, kind, origin, title, part_id, instrument_id, voice, file_name, content_type, size,
                  duration_seconds, created_at
           FROM recordings WHERE work_id = $1
           ORDER BY kind = 'practice', lower(title)"#,
        work_id
    )
    .fetch_all(&state.pool)
    .await?)
}

async fn list(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(work_id): Path<Uuid>,
) -> ApiResult<Json<Vec<Recording>>> {
    if !may_hear(&state, &user, work_id).await? {
        return Err(ApiError::NotFound);
    }
    Ok(Json(recordings(&state, work_id).await?))
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecordingInput {
    kind: String,
    origin: String,
    title: String,
    part_id: Option<Uuid>,
    instrument_id: Option<Uuid>,
    voice: Option<i16>,
    duration_seconds: Option<f32>,
}

struct Checked {
    kind: String,
    origin: String,
    title: String,
    part_id: Option<Uuid>,
    instrument_id: Option<Uuid>,
    voice: Option<i16>,
}

async fn check(state: &AppState, user: &CurrentUser, work_id: Uuid, input: &RecordingInput) -> ApiResult<Checked> {
    if !["full", "practice"].contains(&input.kind.as_str()) {
        return Err(ApiError::Invalid("kind must be full or practice".into()));
    }
    if !["recorded", "generated"].contains(&input.origin.as_str()) {
        return Err(ApiError::Invalid("origin must be recorded or generated".into()));
    }
    let title = input.title.trim().to_string();
    if !(1..=120).contains(&title.chars().count()) {
        return Err(ApiError::Invalid("title must have 1 to 120 characters".into()));
    }
    let practice = input.kind == "practice";
    if let Some(part) = input.part_id.filter(|_| practice) {
        let ok = sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM parts WHERE id = $1 AND work_id = $2) AS "x!""#,
            part,
            work_id
        )
        .fetch_one(&state.pool)
        .await?;
        if !ok {
            return Err(ApiError::Invalid("the part is not of this work".into()));
        }
    }
    if let Some(instrument) = input.instrument_id.filter(|_| practice) {
        let ok = sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM instruments WHERE id = $1 AND organization_id = $2) AS "x!""#,
            instrument,
            user.organization_id
        )
        .fetch_one(&state.pool)
        .await?;
        if !ok {
            return Err(ApiError::Invalid("unknown instrument".into()));
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

/// Multipart: `file` and `meta` (JSON, RecordingInput).
async fn upload(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(work_id): Path<Uuid>,
    mut multipart: Multipart,
) -> ApiResult<(StatusCode, Json<Recording>)> {
    if !may_manage(&state, &user, work_id).await? {
        return Err(ApiError::Forbidden);
    }
    let (mut file, mut meta) = (None, None);
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::Invalid(format!("upload: {e}")))?
    {
        match field.name() {
            Some("file") => {
                let name = clean_file_name(field.file_name().unwrap_or("recording"));
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|e| ApiError::Invalid(format!("upload: {e}")))?;
                file = Some((name, bytes));
            }
            Some("meta") => {
                let text = field
                    .text()
                    .await
                    .map_err(|e| ApiError::Invalid(format!("upload: {e}")))?;
                meta = Some(
                    serde_json::from_str::<RecordingInput>(&text)
                        .map_err(|e| ApiError::Invalid(format!("meta: {e}")))?,
                );
            }
            _ => {}
        }
    }
    let (file_name, bytes) = file.ok_or_else(|| ApiError::Invalid("missing form field `file`".into()))?;
    let input = meta.ok_or_else(|| ApiError::Invalid("missing form field `meta`".into()))?;
    let content_type = audio_type(&bytes, &file_name)
        .ok_or_else(|| ApiError::Invalid("not an audio file (MP3, M4A, WAV, FLAC, Ogg, Opus, WebM, AIFF)".into()))?;
    let c = check(&state, &user, work_id, &input).await?;
    let sha256 = hex::encode(Sha256::digest(&bytes));
    let duplicate = sqlx::query_scalar!(
        "SELECT title FROM recordings WHERE work_id = $1 AND sha256 = $2",
        work_id,
        sha256
    )
    .fetch_optional(&state.pool)
    .await?;
    if let Some(title) = duplicate {
        return Err(ApiError::Conflict(format!(
            "this recording is already here as “{title}”"
        )));
    }
    let id = Uuid::new_v4();
    let key = recording_key(user.organization_id, id);
    let size = bytes.len() as i64;
    state.store.put(&key, PutPayload::from(bytes)).await?;
    let inserted = sqlx::query!(
        "INSERT INTO recordings (id, organization_id, work_id, kind, origin, title, part_id, instrument_id, voice,
                                 file_name, content_type, size, sha256, duration_seconds, storage_key, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)",
        id,
        user.organization_id,
        work_id,
        c.kind,
        c.origin,
        c.title,
        c.part_id,
        c.instrument_id,
        c.voice,
        file_name,
        content_type,
        size,
        sha256,
        input.duration_seconds.filter(|d| d.is_finite() && *d >= 0.0),
        key.as_ref(),
        user.id
    )
    .execute(&state.pool)
    .await;
    if let Err(error) = inserted {
        let _ = state.store.delete(&key).await;
        return Err(error.into());
    }
    let recording = recordings(&state, work_id)
        .await?
        .into_iter()
        .find(|r| r.id == id)
        .ok_or(ApiError::NotFound)?;
    Ok((StatusCode::CREATED, Json(recording)))
}

/// The work of a recording of the user's organisation.
async fn work_of(state: &AppState, user: &CurrentUser, recording_id: Uuid) -> ApiResult<Uuid> {
    sqlx::query_scalar!(
        "SELECT work_id FROM recordings WHERE id = $1 AND organization_id = $2",
        recording_id,
        user.organization_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::NotFound)
}

async fn update(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(recording_id): Path<Uuid>,
    Json(input): Json<RecordingInput>,
) -> ApiResult<Json<Recording>> {
    let work_id = work_of(&state, &user, recording_id).await?;
    if !may_manage(&state, &user, work_id).await? {
        return Err(ApiError::Forbidden);
    }
    let c = check(&state, &user, work_id, &input).await?;
    sqlx::query!(
        "UPDATE recordings SET kind = $2, origin = $3, title = $4, part_id = $5, instrument_id = $6, voice = $7,
                               duration_seconds = coalesce($8, duration_seconds)
         WHERE id = $1",
        recording_id,
        c.kind,
        c.origin,
        c.title,
        c.part_id,
        c.instrument_id,
        c.voice,
        input.duration_seconds.filter(|d| d.is_finite() && *d >= 0.0)
    )
    .execute(&state.pool)
    .await?;
    recordings(&state, work_id)
        .await?
        .into_iter()
        .find(|r| r.id == recording_id)
        .map(Json)
        .ok_or(ApiError::NotFound)
}

async fn remove(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(recording_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let work_id = work_of(&state, &user, recording_id).await?;
    if !may_manage(&state, &user, work_id).await? {
        return Err(ApiError::Forbidden);
    }
    sqlx::query!("DELETE FROM recordings WHERE id = $1", recording_id)
        .execute(&state.pool)
        .await?;
    let _ = state
        .store
        .delete(&recording_key(user.organization_id, recording_id))
        .await;
    Ok(StatusCode::NO_CONTENT)
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

async fn file(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(recording_id): Path<Uuid>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let work_id = work_of(&state, &user, recording_id).await?;
    if !may_hear(&state, &user, work_id).await? {
        return Err(ApiError::NotFound);
    }
    let row = sqlx::query!(
        "SELECT file_name, content_type, size FROM recordings WHERE id = $1",
        recording_id
    )
    .fetch_one(&state.pool)
    .await?;
    let size = row.size as u64;
    let key = recording_key(user.organization_id, recording_id);
    let disposition = format!(
        "inline; filename*=UTF-8''{}",
        percent_encoding::utf8_percent_encode(&row.file_name, percent_encoding::NON_ALPHANUMERIC)
    );
    let requested = headers.get(header::RANGE).and_then(|v| v.to_str().ok());
    let range = match requested.map(|r| parse_range(r, size)) {
        Some(None) => {
            return Ok((
                StatusCode::RANGE_NOT_SATISFIABLE,
                [(header::CONTENT_RANGE, format!("bytes */{size}"))],
            )
                .into_response());
        }
        Some(Some(range)) => Some(range),
        None => None,
    };
    let options = GetOptions::default().with_range(range.clone().map(GetRange::Bounded));
    let object = state.store.get_opts(&key, options).await?;
    let body = Body::from_stream(object.into_stream());
    let common = [
        (header::CONTENT_TYPE, row.content_type),
        (header::ACCEPT_RANGES, "bytes".to_string()),
        (header::CONTENT_DISPOSITION, disposition),
        (header::CACHE_CONTROL, "private, max-age=3600".to_string()),
    ];
    Ok(match range {
        Some(r) => (
            StatusCode::PARTIAL_CONTENT,
            common,
            [
                (header::CONTENT_RANGE, format!("bytes {}-{}/{size}", r.start, r.end - 1)),
                (header::CONTENT_LENGTH, (r.end - r.start).to_string()),
            ],
            body,
        )
            .into_response(),
        None => (
            StatusCode::OK,
            common,
            [(header::CONTENT_LENGTH, size.to_string())],
            body,
        )
            .into_response(),
    })
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
