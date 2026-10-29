// SPDX-License-Identifier: AGPL-3.0-or-later

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use axum::{Json, Router};

use notenwart_app::App;
use notenwart_app::programs as app;
use notenwart_app::programs::*;
use uuid::Uuid;

use crate::auth::SignedIn;
use crate::error::ApiResult;

pub fn router() -> Router<App> {
    Router::new()
        .route("/ensembles/{ensemble_id}/programs", post(create))
        .route("/programs/{program_id}", put(update).delete(remove))
        .route("/programs/{program_id}/items", put(set_items))
        .route("/ensembles/{ensemble_id}/parts", get(ensemble_parts))
        .route("/ensembles/{ensemble_id}/grants", get(grants))
        .route("/ensembles/{ensemble_id}/grants/{part_id}/{user_id}", put(grant).delete(revoke))
}

#[utoipa::path(
    post,
    path = "/ensembles/{ensembleId}/programs",
    tag = "ensembles",
    operation_id = "createProgram",
    summary = "Create a programme, e.g. a concert (admin, librarian, leaders of the ensemble)",
    params(
        ("ensembleId" = uuid::Uuid, Path),
    ),
    request_body = ProgramInput,
    responses(
        (status = 201, description = "Created, without works.", body = Program),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 404, response = crate::openapi::NotFound),
        (status = 422, response = crate::openapi::Invalid),
    ),
)]
async fn create(
    State(state): State<App>,
    user: SignedIn,
    Path(ensemble_id): Path<Uuid>,
    Json(input): Json<ProgramInput>,
) -> ApiResult<(StatusCode, Json<Program>)> {
    Ok((StatusCode::CREATED, Json(app::create(&state, &user, ensemble_id, input).await?)))
}

#[utoipa::path(
    put,
    path = "/programs/{programId}",
    tag = "ensembles",
    operation_id = "updateProgram",
    summary = "Rename or re-date a programme (admin, librarian, leaders of the ensemble)",
    params(
        ("programId" = uuid::Uuid, Path),
    ),
    request_body = ProgramInput,
    responses(
        (status = 200, description = "Updated.", body = Program),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 404, response = crate::openapi::NotFound),
        (status = 422, response = crate::openapi::Invalid),
    ),
)]
async fn update(
    State(state): State<App>,
    user: SignedIn,
    Path(program_id): Path<Uuid>,
    Json(input): Json<ProgramInput>,
) -> ApiResult<Json<Program>> {
    Ok(Json(app::update(&state, &user, program_id, input).await?))
}

#[utoipa::path(
    delete,
    path = "/programs/{programId}",
    tag = "ensembles",
    operation_id = "deleteProgram",
    summary = "Delete a programme (the works stay with the ensemble)",
    params(
        ("programId" = uuid::Uuid, Path),
    ),
    responses(
        (status = 204, description = "Deleted."),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 404, response = crate::openapi::NotFound),
    ),
)]
async fn remove(State(state): State<App>, user: SignedIn, Path(program_id): Path<Uuid>) -> ApiResult<StatusCode> {
    app::remove(&state, &user, program_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    put,
    path = "/programs/{programId}/items",
    tag = "ensembles",
    operation_id = "setProgramItems",
    summary = "Set the works and breaks of a programme in playing order (admin, librarian, leaders of the ensemble)",
    description = "Works the ensemble does not have yet are given to it.",
    params(
        ("programId" = uuid::Uuid, Path),
    ),
    request_body = inline(ItemsInput),
    responses(
        (status = 200, description = "Saved.", body = Program),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 404, response = crate::openapi::NotFound),
        (status = 422, response = crate::openapi::Invalid),
    ),
)]
async fn set_items(
    State(state): State<App>,
    user: SignedIn,
    Path(program_id): Path<Uuid>,
    Json(input): Json<ItemsInput>,
) -> ApiResult<Json<Program>> {
    Ok(Json(app::set_items(&state, &user, program_id, input).await?))
}

#[utoipa::path(
    get,
    path = "/ensembles/{ensembleId}/parts",
    tag = "ensembles",
    operation_id = "listEnsembleParts",
    summary = "The parts of the ensemble's works, for granting (admin, librarian, leaders of the ensemble)",
    params(
        ("ensembleId" = uuid::Uuid, Path),
    ),
    responses(
        (status = 200, description = "Parts by work title.", body = Vec<EnsemblePart>),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 404, response = crate::openapi::NotFound),
    ),
)]
async fn ensemble_parts(
    State(state): State<App>,
    user: SignedIn,
    Path(ensemble_id): Path<Uuid>,
) -> ApiResult<Json<Vec<EnsemblePart>>> {
    Ok(Json(app::ensemble_parts(&state, &user, ensemble_id).await?))
}

#[utoipa::path(
    get,
    path = "/ensembles/{ensembleId}/grants",
    tag = "ensembles",
    operation_id = "listGrants",
    summary = "Parts granted to single members of the ensemble (admin, librarian, leaders of the ensemble)",
    params(
        ("ensembleId" = uuid::Uuid, Path),
    ),
    responses(
        (status = 200, description = "Grants.", body = Vec<Grant>),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 404, response = crate::openapi::NotFound),
    ),
)]
async fn grants(
    State(state): State<App>,
    user: SignedIn,
    Path(ensemble_id): Path<Uuid>,
) -> ApiResult<Json<Vec<Grant>>> {
    Ok(Json(app::grants(&state, &user, ensemble_id).await?))
}

#[utoipa::path(
    put,
    path = "/ensembles/{ensembleId}/grants/{partId}/{userId}",
    tag = "ensembles",
    operation_id = "grantPart",
    summary = "Let one member see one part of an ensemble's work (admin, librarian, leaders of the ensemble)",
    params(
        ("ensembleId" = uuid::Uuid, Path),
        ("partId" = uuid::Uuid, Path),
        ("userId" = uuid::Uuid, Path),
    ),
    responses(
        (status = 204, description = "Granted."),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 404, response = crate::openapi::NotFound),
        (status = 422, response = crate::openapi::Invalid),
    ),
)]
async fn grant(
    State(state): State<App>,
    user: SignedIn,
    Path((ensemble_id, part_id, member_id)): Path<(Uuid, Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    app::grant(&state, &user, ensemble_id, part_id, member_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/ensembles/{ensembleId}/grants/{partId}/{userId}",
    tag = "ensembles",
    operation_id = "revokePart",
    summary = "Take a granted part back",
    params(
        ("ensembleId" = uuid::Uuid, Path),
        ("partId" = uuid::Uuid, Path),
        ("userId" = uuid::Uuid, Path),
    ),
    responses(
        (status = 204, description = "Revoked."),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 404, response = crate::openapi::NotFound),
    ),
)]
async fn revoke(
    State(state): State<App>,
    user: SignedIn,
    Path((ensemble_id, part_id, member_id)): Path<(Uuid, Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    app::revoke(&state, &user, ensemble_id, part_id, member_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
