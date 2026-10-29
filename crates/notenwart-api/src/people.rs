// SPDX-License-Identifier: AGPL-3.0-or-later

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use axum::{Json, Router};

use notenwart_app::App;
use notenwart_app::people as app;
use notenwart_app::people::*;
use uuid::Uuid;

use crate::auth::SignedIn;
use crate::error::ApiResult;

pub fn router() -> Router<App> {
    Router::new()
        .route("/users", get(list).post(create))
        .route("/users/{user_id}", put(update).delete(remove))
        .route("/users/{user_id}/invitation", post(invite))
        .route("/users/bulk", post(bulk))
}

async fn list(State(state): State<App>, user: SignedIn) -> ApiResult<Json<Vec<User>>> {
    Ok(Json(app::list(&state, &user).await?))
}

async fn create(
    State(state): State<App>,
    admin: SignedIn,
    Json(input): Json<UserInput>,
) -> ApiResult<(StatusCode, Json<Invitation>)> {
    Ok((StatusCode::CREATED, Json(app::create(&state, &admin, input).await?)))
}

async fn update(
    State(state): State<App>,
    admin: SignedIn,
    Path(user_id): Path<Uuid>,
    Json(input): Json<UserInput>,
) -> ApiResult<Json<User>> {
    Ok(Json(app::update(&state, &admin, user_id, input).await?))
}

async fn remove(State(state): State<App>, admin: SignedIn, Path(user_id): Path<Uuid>) -> ApiResult<StatusCode> {
    app::remove(&state, &admin, user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn invite(
    State(state): State<App>,
    admin: SignedIn,
    Path(user_id): Path<Uuid>,
) -> ApiResult<(StatusCode, Json<Invitation>)> {
    Ok((StatusCode::CREATED, Json(app::invite(&state, &admin, user_id).await?)))
}

async fn bulk(State(state): State<App>, admin: SignedIn, Json(input): Json<BulkInput>) -> ApiResult<Json<BulkResult>> {
    Ok(Json(app::bulk(&state, &admin, input).await?))
}
