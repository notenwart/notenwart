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

async fn create(
    State(state): State<App>,
    user: SignedIn,
    Path(ensemble_id): Path<Uuid>,
    Json(input): Json<ProgramInput>,
) -> ApiResult<(StatusCode, Json<Program>)> {
    Ok((StatusCode::CREATED, Json(app::create(&state, &user, ensemble_id, input).await?)))
}

async fn update(
    State(state): State<App>,
    user: SignedIn,
    Path(program_id): Path<Uuid>,
    Json(input): Json<ProgramInput>,
) -> ApiResult<Json<Program>> {
    Ok(Json(app::update(&state, &user, program_id, input).await?))
}

async fn remove(State(state): State<App>, user: SignedIn, Path(program_id): Path<Uuid>) -> ApiResult<StatusCode> {
    app::remove(&state, &user, program_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn set_items(
    State(state): State<App>,
    user: SignedIn,
    Path(program_id): Path<Uuid>,
    Json(input): Json<ItemsInput>,
) -> ApiResult<Json<Program>> {
    Ok(Json(app::set_items(&state, &user, program_id, input).await?))
}

async fn ensemble_parts(
    State(state): State<App>,
    user: SignedIn,
    Path(ensemble_id): Path<Uuid>,
) -> ApiResult<Json<Vec<EnsemblePart>>> {
    Ok(Json(app::ensemble_parts(&state, &user, ensemble_id).await?))
}

async fn grants(
    State(state): State<App>,
    user: SignedIn,
    Path(ensemble_id): Path<Uuid>,
) -> ApiResult<Json<Vec<Grant>>> {
    Ok(Json(app::grants(&state, &user, ensemble_id).await?))
}

async fn grant(
    State(state): State<App>,
    user: SignedIn,
    Path((ensemble_id, part_id, member_id)): Path<(Uuid, Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    app::grant(&state, &user, ensemble_id, part_id, member_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn revoke(
    State(state): State<App>,
    user: SignedIn,
    Path((ensemble_id, part_id, member_id)): Path<(Uuid, Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    app::revoke(&state, &user, ensemble_id, part_id, member_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
