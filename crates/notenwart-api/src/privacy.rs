// SPDX-License-Identifier: AGPL-3.0-or-later

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::Value;

use notenwart_app::App;
use notenwart_app::privacy as app;
use notenwart_app::privacy::*;
use uuid::Uuid;

use crate::auth::SignedIn;
use crate::error::ApiResult;

pub fn router() -> Router<App> {
    Router::new()
        .route("/me/export", get(export_mine))
        .route("/users/{user_id}/export", get(export_user))
        .route("/deletions", get(deletions))
}

#[utoipa::path(
    get,
    path = "/me/export",
    tag = "privacy",
    operation_id = "exportMyData",
    summary = "Everything stored about the signed-in user (anyone; GDPR art. 15 and 20)",
    responses(
        (
            status = 200,
            description = "Account, ensembles, single parts, annotations, loans and sign-ins.",
            body = crate::openapi::PersonalData,
        ),
    ),
)]
async fn export_mine(State(state): State<App>, user: SignedIn) -> ApiResult<Json<Value>> {
    Ok(Json(app::export_mine(&state, &user).await?))
}

#[utoipa::path(
    get,
    path = "/users/{userId}/export",
    tag = "privacy",
    operation_id = "exportUserData",
    summary = "Everything stored about a person (admin)",
    params(
        ("userId" = uuid::Uuid, Path),
    ),
    responses(
        (status = 200, description = "As /me/export.", body = crate::openapi::PersonalData),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 404, response = crate::openapi::NotFound),
    ),
)]
async fn export_user(State(state): State<App>, admin: SignedIn, Path(user_id): Path<Uuid>) -> ApiResult<Json<Value>> {
    Ok(Json(app::export_user(&state, &admin, user_id).await?))
}

#[utoipa::path(
    get,
    path = "/deletions",
    tag = "privacy",
    operation_id = "listDeletions",
    summary = "Record of deleted people, without their data (admin)",
    responses(
        (status = 200, description = "Newest first, at most 1000.", body = Vec<Deletion>),
        (status = 403, response = crate::openapi::Forbidden),
    ),
)]
async fn deletions(State(state): State<App>, admin: SignedIn) -> ApiResult<Json<Vec<Deletion>>> {
    Ok(Json(app::deletions(&state, &admin).await?))
}
