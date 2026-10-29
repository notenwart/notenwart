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

#[utoipa::path(
    get,
    path = "/parts/{partId}/annotations",
    tag = "library",
    operation_id = "getAnnotations",
    summary = "The signed-in user's annotations on a part (anyone who may see the part)",
    params(
        ("partId" = uuid::Uuid, Path),
    ),
    responses(
        (
            status = 200,
            description = "Annotations; empty pages and a null `updatedAt` when there are none yet.",
            body = Annotations,
        ),
        (status = 404, response = crate::openapi::NotFound),
    ),
)]
async fn load(State(state): State<App>, user: SignedIn, Path(part_id): Path<Uuid>) -> ApiResult<Json<Annotations>> {
    Ok(Json(app::load(&state, &user, part_id).await?))
}

#[utoipa::path(
    put,
    path = "/parts/{partId}/annotations",
    tag = "library",
    operation_id = "putAnnotations",
    summary = "Replace the signed-in user's annotations on a part",
    description = "The last write wins – a device that edited offline sends its whole set when it is back online.",
    params(
        ("partId" = uuid::Uuid, Path),
    ),
    request_body = AnnotationsInput,
    responses(
        (status = 200, description = "Saved.", body = Annotations),
        (status = 404, response = crate::openapi::NotFound),
        (status = 422, response = crate::openapi::Invalid),
    ),
)]
async fn save(
    State(state): State<App>,
    user: SignedIn,
    Path(part_id): Path<Uuid>,
    Json(input): Json<AnnotationsInput>,
) -> ApiResult<Json<Annotations>> {
    Ok(Json(app::save(&state, &user, part_id, input).await?))
}

#[utoipa::path(
    get,
    path = "/parts/{partId}/shared-annotations",
    tag = "library",
    operation_id = "getSharedAnnotations",
    summary = "Markings leaders share on a part, one layer per ensemble (whoever sees the part)",
    description = "The layers of the user's ensembles that have the work (staff: all such ensembles); empty layers \
    only where the user may start one.",
    params(
        ("partId" = uuid::Uuid, Path),
    ),
    responses(
        (status = 200, description = "By ensemble name.", body = Vec<SharedLayer>),
        (status = 404, response = crate::openapi::NotFound),
    ),
)]
async fn load_shared(
    State(state): State<App>,
    user: SignedIn,
    Path(part_id): Path<Uuid>,
) -> ApiResult<Json<Vec<SharedLayer>>> {
    Ok(Json(app::load_shared(&state, &user, part_id).await?))
}

#[utoipa::path(
    put,
    path = "/parts/{partId}/shared-annotations/{ensembleId}",
    tag = "library",
    operation_id = "saveSharedAnnotations",
    summary = "Replace an ensemble's shared markings on a part (leaders of the ensemble, admin, librarian)",
    description = "Players are notified of changes at most twice an hour.",
    params(
        ("partId" = uuid::Uuid, Path),
        ("ensembleId" = uuid::Uuid, Path),
    ),
    request_body = AnnotationsInput,
    responses(
        (status = 200, description = "Saved.", body = SharedLayer),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 404, response = crate::openapi::NotFound),
        (status = 422, response = crate::openapi::Invalid),
    ),
)]
async fn save_shared(
    State(state): State<App>,
    user: SignedIn,
    Path((part_id, ensemble_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<AnnotationsInput>,
) -> ApiResult<Json<SharedLayer>> {
    Ok(Json(app::save_shared(&state, &user, part_id, ensemble_id, input).await?))
}

#[utoipa::path(
    get,
    path = "/parts/{partId}/annotated",
    tag = "library",
    operation_id = "annotatedPartFile",
    summary = "The part's PDF with annotations drawn in, for printing or keeping (whoever sees the part)",
    params(
        ("partId" = uuid::Uuid, Path),
        AnnotatedQuery,
    ),
    responses(
        (
            status = 200,
            description = "A copy with the strokes as vector paths; the stored PDF is not changed.",
            body = inline(crate::openapi::Binary),
            content_type = "application/pdf",
        ),
        (status = 404, response = crate::openapi::NotFound),
    ),
)]
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
