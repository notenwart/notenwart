// SPDX-License-Identifier: AGPL-3.0-or-later

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, put};
use axum::{Json, Router};

use notenwart_app::App;
use notenwart_app::ensembles as app;
use notenwart_app::ensembles::*;
use uuid::Uuid;

use crate::auth::SignedIn;
use crate::error::ApiResult;

pub fn router() -> Router<App> {
    Router::new()
        .route("/ensembles", get(list).post(create))
        .route("/ensembles/{ensemble_id}", get(detail).put(update).delete(remove))
        .route("/ensembles/{ensemble_id}/members/{user_id}", put(put_member).delete(remove_member))
        .route("/ensembles/{ensemble_id}/works/{work_id}", put(add_work).delete(remove_work))
        .route("/my/music", get(my_music))
        .route("/parts/{part_id}/info", get(part_info))
}

async fn list(State(state): State<App>, user: SignedIn) -> ApiResult<Json<Vec<Ensemble>>> {
    Ok(Json(app::list(&state, &user).await?))
}

async fn create(
    State(state): State<App>,
    user: SignedIn,
    Json(input): Json<EnsembleInput>,
) -> ApiResult<(StatusCode, Json<Ensemble>)> {
    Ok((StatusCode::CREATED, Json(app::create(&state, &user, input).await?)))
}

async fn update(
    State(state): State<App>,
    user: SignedIn,
    Path(ensemble_id): Path<Uuid>,
    Json(input): Json<EnsembleInput>,
) -> ApiResult<Json<Ensemble>> {
    Ok(Json(app::update(&state, &user, ensemble_id, input).await?))
}

async fn remove(State(state): State<App>, user: SignedIn, Path(ensemble_id): Path<Uuid>) -> ApiResult<StatusCode> {
    app::remove(&state, &user, ensemble_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn detail(
    State(state): State<App>,
    user: SignedIn,
    Path(ensemble_id): Path<Uuid>,
) -> ApiResult<Json<EnsembleDetail>> {
    Ok(Json(app::detail(&state, &user, ensemble_id).await?))
}

async fn put_member(
    State(state): State<App>,
    user: SignedIn,
    Path((ensemble_id, member_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<MemberInput>,
) -> ApiResult<Json<EnsembleMember>> {
    Ok(Json(app::put_member(&state, &user, ensemble_id, member_id, input).await?))
}

async fn remove_member(
    State(state): State<App>,
    user: SignedIn,
    Path((ensemble_id, member_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    app::remove_member(&state, &user, ensemble_id, member_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn add_work(
    State(state): State<App>,
    user: SignedIn,
    Path((ensemble_id, work_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    app::add_work(&state, &user, ensemble_id, work_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_work(
    State(state): State<App>,
    user: SignedIn,
    Path((ensemble_id, work_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    app::remove_work(&state, &user, ensemble_id, work_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn my_music(State(state): State<App>, user: SignedIn) -> ApiResult<Json<Vec<MyEnsemble>>> {
    Ok(Json(app::my_music(&state, &user).await?))
}

async fn part_info(State(state): State<App>, user: SignedIn, Path(part_id): Path<Uuid>) -> ApiResult<Json<PartInfo>> {
    Ok(Json(app::part_info(&state, &user, part_id).await?))
}
