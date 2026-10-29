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
#[utoipa::path(
    post,
    path = "/users/import",
    tag = "people",
    operation_id = "importUsers",
    summary = "Import people from a table (admin)",
    description = "Each row adds a person (or finds an existing one by email) and optionally puts them in an \
    ensemble, playing an\ninstrument and part number. Missing ensembles are created. Nothing is changed when any row \
    has an error: the\nanswer is then 422 with the results per row. With `dryRun`, nothing is changed either, and the \
    answer is 200.\nNew people get an invitation link (emailed when the server can send mail).",
    request_body = ImportInput,
    responses(
        (status = 200, description = "Checked (dry run) or imported.", body = ImportResult),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 422, description = "At least one row has an error; nothing was imported.", body = ImportResult),
    ),
)]
async fn import(
    State(state): State<App>,
    admin: SignedIn,
    Json(input): Json<ImportInput>,
) -> ApiResult<(StatusCode, Json<ImportResult>)> {
    let result = app::import(&state, &admin, input).await?;
    let status = if result.errors > 0 { StatusCode::UNPROCESSABLE_ENTITY } else { StatusCode::OK };
    Ok((status, Json(result)))
}
