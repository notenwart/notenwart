// SPDX-License-Identifier: AGPL-3.0-or-later

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use axum::{Json, Router};

use notenwart_app::App;
use notenwart_app::catalogue as app;
use notenwart_app::catalogue::*;
use uuid::Uuid;

use crate::auth::SignedIn;
use crate::error::ApiResult;

pub fn router() -> Router<App> {
    Router::new()
        .route("/instruments", get(list_instruments).post(create_instrument))
        .route("/instruments/{instrument_id}", put(update_instrument).delete(delete_instrument))
        .route("/instruments/{instrument_id}/merge", post(merge_instrument))
        .route("/instrument-families", get(list_families).post(create_family))
        .route("/instrument-families/{family_id}", put(update_family).delete(delete_family))
}

async fn list_families(State(state): State<App>, user: SignedIn) -> ApiResult<Json<Vec<Family>>> {
    Ok(Json(app::list_families(&state, &user).await?))
}

async fn create_family(
    State(state): State<App>,
    user: SignedIn,
    Json(input): Json<FamilyInput>,
) -> ApiResult<(StatusCode, Json<Family>)> {
    Ok((StatusCode::CREATED, Json(app::create_family(&state, &user, input).await?)))
}

async fn update_family(
    State(state): State<App>,
    user: SignedIn,
    Path(family_id): Path<Uuid>,
    Json(input): Json<FamilyInput>,
) -> ApiResult<Json<Family>> {
    Ok(Json(app::update_family(&state, &user, family_id, input).await?))
}

async fn delete_family(State(state): State<App>, user: SignedIn, Path(family_id): Path<Uuid>) -> ApiResult<StatusCode> {
    app::delete_family(&state, &user, family_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_instruments(State(state): State<App>, user: SignedIn) -> ApiResult<Json<Vec<Instrument>>> {
    Ok(Json(app::list_instruments(&state, &user).await?))
}

async fn update_instrument(
    State(state): State<App>,
    user: SignedIn,
    Path(instrument_id): Path<Uuid>,
    Json(input): Json<InstrumentInput>,
) -> ApiResult<Json<Instrument>> {
    Ok(Json(app::update_instrument(&state, &user, instrument_id, input).await?))
}

async fn delete_instrument(
    State(state): State<App>,
    user: SignedIn,
    Path(instrument_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    app::delete_instrument(&state, &user, instrument_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn merge_instrument(
    State(state): State<App>,
    user: SignedIn,
    Path(instrument_id): Path<Uuid>,
    Json(input): Json<MergeInput>,
) -> ApiResult<Json<Instrument>> {
    Ok(Json(app::merge_instrument(&state, &user, instrument_id, input).await?))
}

async fn create_instrument(
    State(state): State<App>,
    user: SignedIn,
    Json(input): Json<InstrumentInput>,
) -> ApiResult<(StatusCode, Json<Instrument>)> {
    Ok((StatusCode::CREATED, Json(app::create_instrument(&state, &user, input).await?)))
}
