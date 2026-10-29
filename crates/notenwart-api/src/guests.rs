// SPDX-License-Identifier: AGPL-3.0-or-later

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get};
use axum::{Json, Router};

use notenwart_app::App;
use notenwart_app::guests as app;
use notenwart_app::guests::*;
use uuid::Uuid;

use crate::auth::SignedIn;
use crate::error::ApiResult;
use axum::response::Response;

pub fn router() -> Router<App> {
    Router::new()
        .route("/ensembles/{ensemble_id}/guest-links", get(list).post(create))
        .route("/guest-links/{link_id}", delete(revoke))
        .route("/guest/{token}", get(open))
        .route("/guest/{token}/parts/{part_id}/file", get(file))
}

#[utoipa::path(
    get,
    path = "/ensembles/{ensembleId}/guest-links",
    tag = "guests",
    operation_id = "listGuestLinks",
    summary = "Guest links of an ensemble (admin, librarian, leaders of the ensemble)",
    params(
        ("ensembleId" = uuid::Uuid, Path),
    ),
    responses(
        (status = 200, description = "Active first.", body = Vec<GuestLink>),
        (status = 403, response = crate::openapi::Forbidden),
    ),
)]
async fn list(
    State(state): State<App>,
    user: SignedIn,
    Path(ensemble_id): Path<Uuid>,
) -> ApiResult<Json<Vec<GuestLink>>> {
    Ok(Json(app::list(&state, &user, ensemble_id).await?))
}

#[utoipa::path(
    post,
    path = "/ensembles/{ensembleId}/guest-links",
    tag = "guests",
    operation_id = "createGuestLink",
    summary = "Make a guest link (admin, librarian, leaders of the ensemble)",
    params(
        ("ensembleId" = uuid::Uuid, Path),
    ),
    request_body = inline(GuestLinkInput),
    responses(
        (status = 201, description = "Created; `url` is shown this once (only its hash is stored).", body = GuestLink),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 422, response = crate::openapi::Invalid),
    ),
)]
async fn create(
    State(state): State<App>,
    user: SignedIn,
    Path(ensemble_id): Path<Uuid>,
    Json(input): Json<GuestLinkInput>,
) -> ApiResult<(StatusCode, Json<GuestLink>)> {
    Ok((StatusCode::CREATED, Json(app::create(&state, &user, ensemble_id, input).await?)))
}

#[utoipa::path(
    delete,
    path = "/guest-links/{linkId}",
    tag = "guests",
    operation_id = "revokeGuestLink",
    summary = "Stop a guest link (admin, librarian, leaders of the ensemble)",
    params(
        ("linkId" = uuid::Uuid, Path),
    ),
    responses(
        (status = 204, description = "It no longer works."),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 404, response = crate::openapi::NotFound),
    ),
)]
async fn revoke(State(state): State<App>, user: SignedIn, Path(link_id): Path<Uuid>) -> ApiResult<StatusCode> {
    app::revoke(&state, &user, link_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/guest/{token}",
    tag = "guests",
    operation_id = "openGuestLink",
    summary = "What a guest link shows (no account)",
    params(
        ("token" = String, Path),
    ),
    responses(
        (status = 200, description = "The guest's parts.", body = GuestView),
        (status = 404, description = "Unknown, expired or revoked (indistinguishable)."),
    ),
    security(),
)]
async fn open(State(state): State<App>, Path(token): Path<String>) -> ApiResult<Json<GuestView>> {
    Ok(Json(app::open(&state, token).await?))
}

#[utoipa::path(
    get,
    path = "/guest/{token}/parts/{partId}/file",
    tag = "guests",
    operation_id = "guestPartFile",
    summary = "A part's PDF through a guest link (no account)",
    params(
        ("token" = String, Path),
        ("partId" = uuid::Uuid, Path),
    ),
    responses(
        (
            status = 200,
            description = "The PDF.",
            body = inline(crate::openapi::Binary),
            content_type = "application/pdf",
        ),
        (status = 404, response = crate::openapi::NotFound),
    ),
    security(),
)]
async fn file(State(state): State<App>, Path((token, part_id)): Path<(String, Uuid)>) -> ApiResult<Response> {
    Ok(crate::files::pdf_response(app::file(&state, &token, part_id).await?))
}
