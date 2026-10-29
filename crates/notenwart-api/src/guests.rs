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

async fn list(
    State(state): State<App>,
    user: SignedIn,
    Path(ensemble_id): Path<Uuid>,
) -> ApiResult<Json<Vec<GuestLink>>> {
    Ok(Json(app::list(&state, &user, ensemble_id).await?))
}

async fn create(
    State(state): State<App>,
    user: SignedIn,
    Path(ensemble_id): Path<Uuid>,
    Json(input): Json<GuestLinkInput>,
) -> ApiResult<(StatusCode, Json<GuestLink>)> {
    Ok((StatusCode::CREATED, Json(app::create(&state, &user, ensemble_id, input).await?)))
}

async fn revoke(State(state): State<App>, user: SignedIn, Path(link_id): Path<Uuid>) -> ApiResult<StatusCode> {
    app::revoke(&state, &user, link_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn open(State(state): State<App>, Path(token): Path<String>) -> ApiResult<Json<GuestView>> {
    Ok(Json(app::open(&state, token).await?))
}

async fn file(State(state): State<App>, Path((token, part_id)): Path<(String, Uuid)>) -> ApiResult<Response> {
    Ok(crate::files::pdf_response(app::file(&state, &token, part_id).await?))
}
