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

async fn system(State(state): State<App>, user: SignedIn) -> ApiResult<Json<System>> {
    Ok(Json(app::system(&state, &user).await?))
}

async fn test_mail(State(state): State<App>, user: SignedIn) -> ApiResult<Json<TestMail>> {
    Ok(Json(app::test_mail(&state, &user).await?))
}

async fn tick(
    State(state): State<App>,
    user: SignedIn,
    Path(key): Path<String>,
    Json(input): Json<TickInput>,
) -> ApiResult<StatusCode> {
    app::tick(&state, &user, key, input).await?;
    Ok(StatusCode::NO_CONTENT)
}
