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

#[utoipa::path(
    get,
    path = "/instrument-families",
    tag = "catalogue",
    operation_id = "listInstrumentFamilies",
    summary = "The organisation's instrument families (any signed-in user)",
    responses(
        (status = 200, description = "All families in their order.", body = Vec<Family>),
    ),
)]
async fn list_families(State(state): State<App>, user: SignedIn) -> ApiResult<Json<Vec<Family>>> {
    Ok(Json(app::list_families(&state, &user).await?))
}

#[utoipa::path(
    post,
    path = "/instrument-families",
    tag = "catalogue",
    operation_id = "createInstrumentFamily",
    summary = "Add a family (admin, librarian)",
    request_body = FamilyInput,
    responses(
        (status = 201, description = "Created.", body = Family),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 409, response = crate::openapi::Conflict),
        (status = 422, response = crate::openapi::Invalid),
    ),
)]
async fn create_family(
    State(state): State<App>,
    user: SignedIn,
    Json(input): Json<FamilyInput>,
) -> ApiResult<(StatusCode, Json<Family>)> {
    Ok((StatusCode::CREATED, Json(app::create_family(&state, &user, input).await?)))
}

#[utoipa::path(
    put,
    path = "/instrument-families/{familyId}",
    tag = "catalogue",
    operation_id = "updateInstrumentFamily",
    summary = "Rename or reorder a family (admin, librarian)",
    params(
        ("familyId" = uuid::Uuid, Path),
    ),
    request_body = FamilyInput,
    responses(
        (status = 200, description = "Updated.", body = Family),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 404, response = crate::openapi::NotFound),
        (status = 409, response = crate::openapi::Conflict),
        (status = 422, response = crate::openapi::Invalid),
    ),
)]
async fn update_family(
    State(state): State<App>,
    user: SignedIn,
    Path(family_id): Path<Uuid>,
    Json(input): Json<FamilyInput>,
) -> ApiResult<Json<Family>> {
    Ok(Json(app::update_family(&state, &user, family_id, input).await?))
}

#[utoipa::path(
    delete,
    path = "/instrument-families/{familyId}",
    tag = "catalogue",
    operation_id = "deleteInstrumentFamily",
    summary = "Remove a family without instruments (admin, librarian)",
    params(
        ("familyId" = uuid::Uuid, Path),
    ),
    responses(
        (status = 204, description = "Removed."),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 404, response = crate::openapi::NotFound),
        (status = 409, response = crate::openapi::Conflict),
    ),
)]
async fn delete_family(State(state): State<App>, user: SignedIn, Path(family_id): Path<Uuid>) -> ApiResult<StatusCode> {
    app::delete_family(&state, &user, family_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/instruments",
    tag = "catalogue",
    operation_id = "listInstruments",
    summary = "The organisation's instruments (any signed-in user)",
    responses(
        (
            status = 200,
            description = "All instruments, ordered by family, then by their order within it.",
            body = Vec<Instrument>,
        ),
    ),
)]
async fn list_instruments(State(state): State<App>, user: SignedIn) -> ApiResult<Json<Vec<Instrument>>> {
    Ok(Json(app::list_instruments(&state, &user).await?))
}

#[utoipa::path(
    put,
    path = "/instruments/{instrumentId}",
    tag = "catalogue",
    operation_id = "updateInstrument",
    summary = "Rename, regroup or reorder an instrument (admin, librarian)",
    params(
        ("instrumentId" = uuid::Uuid, Path),
    ),
    request_body = InstrumentInput,
    responses(
        (status = 200, description = "Updated.", body = Instrument),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 404, response = crate::openapi::NotFound),
        (status = 409, response = crate::openapi::Conflict),
        (status = 422, response = crate::openapi::Invalid),
    ),
)]
async fn update_instrument(
    State(state): State<App>,
    user: SignedIn,
    Path(instrument_id): Path<Uuid>,
    Json(input): Json<InstrumentInput>,
) -> ApiResult<Json<Instrument>> {
    Ok(Json(app::update_instrument(&state, &user, instrument_id, input).await?))
}

#[utoipa::path(
    delete,
    path = "/instruments/{instrumentId}",
    tag = "catalogue",
    operation_id = "deleteInstrument",
    summary = "Remove an instrument that no part or member uses (admin, librarian)",
    params(
        ("instrumentId" = uuid::Uuid, Path),
    ),
    responses(
        (status = 204, description = "Removed."),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 404, response = crate::openapi::NotFound),
        (status = 409, response = crate::openapi::Conflict),
    ),
)]
async fn delete_instrument(
    State(state): State<App>,
    user: SignedIn,
    Path(instrument_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    app::delete_instrument(&state, &user, instrument_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/instruments/{instrumentId}/merge",
    tag = "catalogue",
    operation_id = "mergeInstrument",
    summary = "Fold an instrument into another (admin, librarian)",
    description = "Parts, members' assignments and recordings move to `into`; the instrument's name and other names \
    become other names of `into`; the instrument is deleted.",
    params(
        ("instrumentId" = uuid::Uuid, Path),
    ),
    request_body = inline(MergeInput),
    responses(
        (status = 200, description = "The instrument that stays.", body = Instrument),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 404, response = crate::openapi::NotFound),
        (status = 422, response = crate::openapi::Invalid),
    ),
)]
async fn merge_instrument(
    State(state): State<App>,
    user: SignedIn,
    Path(instrument_id): Path<Uuid>,
    Json(input): Json<MergeInput>,
) -> ApiResult<Json<Instrument>> {
    Ok(Json(app::merge_instrument(&state, &user, instrument_id, input).await?))
}

#[utoipa::path(
    post,
    path = "/instruments",
    tag = "catalogue",
    operation_id = "createInstrument",
    summary = "Add an instrument (admin, librarian)",
    request_body = InstrumentInput,
    responses(
        (status = 201, description = "Created.", body = Instrument),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 409, response = crate::openapi::Conflict),
        (status = 422, response = crate::openapi::Invalid),
    ),
)]
async fn create_instrument(
    State(state): State<App>,
    user: SignedIn,
    Json(input): Json<InstrumentInput>,
) -> ApiResult<(StatusCode, Json<Instrument>)> {
    Ok((StatusCode::CREATED, Json(app::create_instrument(&state, &user, input).await?)))
}
