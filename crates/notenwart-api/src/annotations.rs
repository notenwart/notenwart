// SPDX-License-Identifier: AGPL-3.0-or-later

use axum::extract::{Path, Query, State};
use axum::routing::{get, put};
use axum::{Json, Router};

use notenwart_app::App;
use notenwart_app::annotations as app;
use notenwart_app::annotations::*;
use uuid::Uuid;

use crate::auth::SignedIn;
use crate::error::ApiResult;
use axum::http::header;
use axum::response::{IntoResponse, Response};

pub fn router() -> Router<App> {
    Router::new()
        .route("/parts/{part_id}/annotations", get(load).put(save))
        .route("/parts/{part_id}/shared-annotations", get(load_shared))
        .route("/parts/{part_id}/shared-annotations/{ensemble_id}", put(save_shared))
        .route("/parts/{part_id}/annotated", get(annotated))
}

async fn load(State(state): State<App>, user: SignedIn, Path(part_id): Path<Uuid>) -> ApiResult<Json<Annotations>> {
    Ok(Json(app::load(&state, &user, part_id).await?))
}

async fn save(
    State(state): State<App>,
    user: SignedIn,
    Path(part_id): Path<Uuid>,
    Json(input): Json<AnnotationsInput>,
) -> ApiResult<Json<Annotations>> {
    Ok(Json(app::save(&state, &user, part_id, input).await?))
}

async fn load_shared(
    State(state): State<App>,
    user: SignedIn,
    Path(part_id): Path<Uuid>,
) -> ApiResult<Json<Vec<SharedLayer>>> {
    Ok(Json(app::load_shared(&state, &user, part_id).await?))
}

async fn save_shared(
    State(state): State<App>,
    user: SignedIn,
    Path((part_id, ensemble_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<AnnotationsInput>,
) -> ApiResult<Json<SharedLayer>> {
    Ok(Json(app::save_shared(&state, &user, part_id, ensemble_id, input).await?))
}

async fn annotated(
    State(state): State<App>,
    user: SignedIn,
    Path(part_id): Path<Uuid>,
    Query(query): Query<AnnotatedQuery>,
) -> ApiResult<Response> {
    let pdf = app::annotated(&state, &user, part_id, query).await?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/pdf".to_string()),
            (header::CONTENT_DISPOSITION, crate::files::utf8_disposition(&pdf.file_name)),
            (header::CACHE_CONTROL, "private, no-store".to_string()),
        ],
        pdf.bytes,
    )
        .into_response())
}
