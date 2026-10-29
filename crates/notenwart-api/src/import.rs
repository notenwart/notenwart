// SPDX-License-Identifier: AGPL-3.0-or-later

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use notenwart_app::App;
use notenwart_app::import::{self as app, ImportInput, ImportResult};

use crate::auth::SignedIn;
use crate::error::ApiResult;

pub fn router() -> Router<App> {
    Router::new().route("/users/import", post(import))
}

/// 422 with the results when a row has errors (nothing changed), else 200.
async fn import(
    State(state): State<App>,
    admin: SignedIn,
    Json(input): Json<ImportInput>,
) -> ApiResult<(StatusCode, Json<ImportResult>)> {
    let result = app::import(&state, &admin, input).await?;
    let status = if result.errors > 0 { StatusCode::UNPROCESSABLE_ENTITY } else { StatusCode::OK };
    Ok((status, Json(result)))
}
