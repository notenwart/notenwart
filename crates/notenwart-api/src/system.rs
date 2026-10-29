// SPDX-License-Identifier: AGPL-3.0-or-later

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use axum::{Json, Router};

use notenwart_app::App;
use notenwart_app::system as app;
use notenwart_app::system::*;

use crate::auth::SignedIn;
use crate::error::ApiResult;

pub fn router() -> Router<App> {
    Router::new()
        .route("/system", get(system))
        .route("/system/test-mail", post(test_mail))
        .route("/system/checklist/{key}", put(tick))
}

#[utoipa::path(
    get,
    path = "/system",
    tag = "system",
    operation_id = "getSystem",
    summary = "The server's version, settings, abilities, size and the setup checklist (admin)",
    responses(
        (status = 200, description = "The system.", body = System),
        (status = 403, response = crate::openapi::Forbidden),
    ),
)]
async fn system(State(state): State<App>, user: SignedIn) -> ApiResult<Json<System>> {
    Ok(Json(app::system(&state, &user).await?))
}

#[utoipa::path(
    post,
    path = "/system/test-mail",
    tag = "system",
    operation_id = "sendTestMail",
    summary = "Send a test email to oneself (admin)",
    responses(
        (status = 200, description = "Whether the SMTP server took it.", body = inline(TestMail)),
        (status = 409, description = "The server sends no email."),
    ),
)]
async fn test_mail(State(state): State<App>, user: SignedIn) -> ApiResult<Json<TestMail>> {
    Ok(Json(app::test_mail(&state, &user).await?))
}

#[utoipa::path(
    put,
    path = "/system/checklist/{key}",
    tag = "system",
    operation_id = "tickSetupStep",
    summary = "Tick or untick a setup step that is not recognised by itself (admin)",
    params(
        ("key" = inline(ManualStep), Path),
    ),
    request_body = inline(TickInput),
    responses(
        (status = 204, description = "Saved."),
        (status = 422, response = crate::openapi::Invalid),
    ),
)]
async fn tick(
    State(state): State<App>,
    user: SignedIn,
    Path(key): Path<String>,
    Json(input): Json<TickInput>,
) -> ApiResult<StatusCode> {
    app::tick(&state, &user, key, input).await?;
    Ok(StatusCode::NO_CONTENT)
}
