// SPDX-License-Identifier: AGPL-3.0-or-later

use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::StatusCode;
use axum::routing::{get, put};
use axum::{Json, Router};

use notenwart_app::App;
use notenwart_app::recordings as app;
use notenwart_app::recordings::*;
use uuid::Uuid;

use axum::body::Body;
use axum::extract::Multipart;
use axum::http::{HeaderMap, header};
use axum::response::{IntoResponse, Response};
use notenwart_app::error::AppError;
use notenwart_app::files::Upload;

use crate::auth::SignedIn;
use crate::error::ApiResult;
use crate::files::upload_error;

pub fn router(max_upload: usize) -> Router<App> {
    Router::new()
        .route("/works/{work_id}/recordings", get(list).post(upload).layer(DefaultBodyLimit::max(max_upload)))
        .route("/recordings/{recording_id}", put(update).delete(remove))
        .route("/recordings/{recording_id}/file", get(file))
}

async fn list(State(state): State<App>, user: SignedIn, Path(work_id): Path<Uuid>) -> ApiResult<Json<Vec<Recording>>> {
    Ok(Json(app::list(&state, &user, work_id).await?))
}

async fn update(
    State(state): State<App>,
    user: SignedIn,
    Path(recording_id): Path<Uuid>,
    Json(input): Json<RecordingInput>,
) -> ApiResult<Json<Recording>> {
    Ok(Json(app::update(&state, &user, recording_id, input).await?))
}

async fn remove(State(state): State<App>, user: SignedIn, Path(recording_id): Path<Uuid>) -> ApiResult<StatusCode> {
    app::remove(&state, &user, recording_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Multipart: `file` and `meta` (JSON, RecordingInput).
async fn read_upload(mut multipart: Multipart) -> Result<RecordingUpload, AppError> {
    let mut upload = RecordingUpload::default();
    while let Some(field) = multipart.next_field().await.map_err(upload_error)? {
        match field.name() {
            Some("file") => {
                let file_name = field.file_name().map(str::to_string);
                let bytes = field.bytes().await.map_err(upload_error)?;
                upload.file = Some(Upload { file_name, bytes });
            }
            Some("meta") => {
                let text = field.text().await.map_err(upload_error)?;
                upload.meta = Some(
                    serde_json::from_str::<RecordingInput>(&text)
                        .map_err(|e| AppError::Invalid(format!("meta: {e}")))?,
                );
            }
            _ => {}
        }
    }
    Ok(upload)
}

async fn upload(
    State(state): State<App>,
    user: SignedIn,
    Path(work_id): Path<Uuid>,
    multipart: Multipart,
) -> ApiResult<(StatusCode, Json<Recording>)> {
    let upload = read_upload(multipart).await;
    Ok((StatusCode::CREATED, Json(app::upload(&state, &user, work_id, upload).await?)))
}

async fn file(
    State(state): State<App>,
    user: SignedIn,
    Path(recording_id): Path<Uuid>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let requested = headers.get(header::RANGE).and_then(|v| v.to_str().ok());
    let (content_type, file_name, size, range, content) =
        match app::file(&state, &user, recording_id, requested).await? {
            RecordingFile::Unsatisfiable { size } => {
                return Ok((StatusCode::RANGE_NOT_SATISFIABLE, [(header::CONTENT_RANGE, format!("bytes */{size}"))])
                    .into_response());
            }
            RecordingFile::Content { content_type, file_name, size, range, content } => {
                (content_type, file_name, size, range, content)
            }
        };
    let body = Body::from_stream(content);
    let common = [
        (header::CONTENT_TYPE, content_type),
        (header::ACCEPT_RANGES, "bytes".to_string()),
        (header::CONTENT_DISPOSITION, crate::files::utf8_disposition(&file_name)),
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
        None => (StatusCode::OK, common, [(header::CONTENT_LENGTH, size.to_string())], body).into_response(),
    })
}
