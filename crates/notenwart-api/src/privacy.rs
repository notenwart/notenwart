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

async fn export_mine(State(state): State<App>, user: SignedIn) -> ApiResult<Json<Value>> {
    Ok(Json(app::export_mine(&state, &user).await?))
}

async fn export_user(State(state): State<App>, admin: SignedIn, Path(user_id): Path<Uuid>) -> ApiResult<Json<Value>> {
    Ok(Json(app::export_user(&state, &admin, user_id).await?))
}

async fn deletions(State(state): State<App>, admin: SignedIn) -> ApiResult<Json<Vec<Deletion>>> {
    Ok(Json(app::deletions(&state, &admin).await?))
}
