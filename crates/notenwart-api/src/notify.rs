// SPDX-License-Identifier: AGPL-3.0-or-later

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};

use notenwart_app::App;
use notenwart_app::notify as app;
use notenwart_app::notify::*;

use crate::auth::SignedIn;
use crate::error::ApiResult;

pub fn router() -> Router<App> {
    Router::new()
        .route("/notifications", get(inbox))
        .route("/notifications/read", post(mark_read))
        .route("/notifications/settings", get(settings).put(save_settings))
}

#[utoipa::path(
    get,
    path = "/notifications",
    tag = "notifications",
    operation_id = "listNotifications",
    summary = "The signed-in user's notifications in the app (anyone)",
    params(
        InboxQuery,
    ),
    responses(
        (status = 200, description = "Newest first, at most 100.", body = Inbox),
    ),
)]
async fn inbox(State(state): State<App>, user: SignedIn, Query(query): Query<InboxQuery>) -> ApiResult<Json<Inbox>> {
    Ok(Json(app::inbox(&state, &user, query).await?))
}

#[utoipa::path(
    post,
    path = "/notifications/read",
    tag = "notifications",
    operation_id = "markNotificationsRead",
    summary = "Mark notifications read (anyone)",
    request_body = inline(ReadInput),
    responses(
        (status = 204, description = "Done."),
    ),
)]
async fn mark_read(State(state): State<App>, user: SignedIn, Json(input): Json<ReadInput>) -> ApiResult<StatusCode> {
    app::mark_read(&state, &user, input).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/notifications/settings",
    tag = "notifications",
    operation_id = "getNotificationSettings",
    summary = "How the signed-in user hears about each kind (anyone)",
    responses(
        (status = 200, description = "One setting per kind that concerns the user.", body = Vec<Setting>),
    ),
)]
async fn settings(State(state): State<App>, user: SignedIn) -> ApiResult<Json<Vec<Setting>>> {
    Ok(Json(app::settings(&state, &user).await?))
}

#[utoipa::path(
    put,
    path = "/notifications/settings",
    tag = "notifications",
    operation_id = "saveNotificationSettings",
    summary = "Change how the signed-in user hears about some kinds (anyone)",
    request_body = Vec<Setting>,
    responses(
        (status = 200, description = "All settings after the change.", body = Vec<Setting>),
    ),
)]
async fn save_settings(
    State(state): State<App>,
    user: SignedIn,
    Json(input): Json<Vec<Setting>>,
) -> ApiResult<Json<Vec<Setting>>> {
    Ok(Json(app::save_settings(&state, &user, input).await?))
}
