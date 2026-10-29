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

async fn inbox(State(state): State<App>, user: SignedIn, Query(query): Query<InboxQuery>) -> ApiResult<Json<Inbox>> {
    Ok(Json(app::inbox(&state, &user, query).await?))
}

async fn mark_read(State(state): State<App>, user: SignedIn, Json(input): Json<ReadInput>) -> ApiResult<StatusCode> {
    app::mark_read(&state, &user, input).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn settings(State(state): State<App>, user: SignedIn) -> ApiResult<Json<Vec<Setting>>> {
    Ok(Json(app::settings(&state, &user).await?))
}

async fn save_settings(
    State(state): State<App>,
    user: SignedIn,
    Json(input): Json<Vec<Setting>>,
) -> ApiResult<Json<Vec<Setting>>> {
    Ok(Json(app::save_settings(&state, &user, input).await?))
}
