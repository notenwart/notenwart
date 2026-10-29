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

#[utoipa::path(
    get,
    path = "/ensembles",
    tag = "ensembles",
    operation_id = "listEnsembles",
    summary = "Ensembles – all for admins and librarians, otherwise the user's own",
    responses(
        (status = 200, description = "Ensembles ordered by name.", body = Vec<Ensemble>),
    ),
)]
async fn list(State(state): State<App>, user: SignedIn) -> ApiResult<Json<Vec<Ensemble>>> {
    Ok(Json(app::list(&state, &user).await?))
}

#[utoipa::path(
    post,
    path = "/ensembles",
    tag = "ensembles",
    operation_id = "createEnsemble",
    summary = "Create an ensemble (admin, librarian)",
    request_body = EnsembleInput,
    responses(
        (status = 201, description = "Created.", body = Ensemble),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 409, response = crate::openapi::Conflict),
        (status = 422, response = crate::openapi::Invalid),
    ),
)]
async fn create(
    State(state): State<App>,
    user: SignedIn,
    Json(input): Json<EnsembleInput>,
) -> ApiResult<(StatusCode, Json<Ensemble>)> {
    Ok((StatusCode::CREATED, Json(app::create(&state, &user, input).await?)))
}

#[utoipa::path(
    put,
    path = "/ensembles/{ensembleId}",
    tag = "ensembles",
    operation_id = "updateEnsemble",
    summary = "Rename an ensemble (admin, librarian, its leaders)",
    params(
        ("ensembleId" = uuid::Uuid, Path),
    ),
    request_body = EnsembleInput,
    responses(
        (status = 200, description = "Updated.", body = Ensemble),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 404, response = crate::openapi::NotFound),
        (status = 409, response = crate::openapi::Conflict),
        (status = 422, response = crate::openapi::Invalid),
    ),
)]
async fn update(
    State(state): State<App>,
    user: SignedIn,
    Path(ensemble_id): Path<Uuid>,
    Json(input): Json<EnsembleInput>,
) -> ApiResult<Json<Ensemble>> {
    Ok(Json(app::update(&state, &user, ensemble_id, input).await?))
}

#[utoipa::path(
    delete,
    path = "/ensembles/{ensembleId}",
    tag = "ensembles",
    operation_id = "deleteEnsemble",
    summary = "Delete an ensemble (admin, librarian); works and users stay",
    params(
        ("ensembleId" = uuid::Uuid, Path),
    ),
    responses(
        (status = 204, description = "Deleted."),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 404, response = crate::openapi::NotFound),
    ),
)]
async fn remove(State(state): State<App>, user: SignedIn, Path(ensemble_id): Path<Uuid>) -> ApiResult<StatusCode> {
    app::remove(&state, &user, ensemble_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/ensembles/{ensembleId}",
    tag = "ensembles",
    operation_id = "getEnsemble",
    summary = "An ensemble with members and works",
    description = "Members other than the user themself are only listed for those who manage the ensemble.",
    params(
        ("ensembleId" = uuid::Uuid, Path),
    ),
    responses(
        (status = 200, description = "The ensemble.", body = EnsembleDetail),
        (status = 404, response = crate::openapi::NotFound),
    ),
)]
async fn detail(
    State(state): State<App>,
    user: SignedIn,
    Path(ensemble_id): Path<Uuid>,
) -> ApiResult<Json<EnsembleDetail>> {
    Ok(Json(app::detail(&state, &user, ensemble_id).await?))
}

#[utoipa::path(
    put,
    path = "/ensembles/{ensembleId}/members/{userId}",
    tag = "ensembles",
    operation_id = "putEnsembleMember",
    summary = "Add or change a member (admin, librarian, leaders of the ensemble)",
    description = "Only admins and librarians may make someone a leader, and only users with the role leader or higher.",
    params(
        ("ensembleId" = uuid::Uuid, Path),
        ("userId" = uuid::Uuid, Path),
    ),
    request_body = MemberInput,
    responses(
        (status = 200, description = "Saved.", body = EnsembleMember),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 404, response = crate::openapi::NotFound),
        (status = 422, response = crate::openapi::Invalid),
    ),
)]
async fn put_member(
    State(state): State<App>,
    user: SignedIn,
    Path((ensemble_id, member_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<MemberInput>,
) -> ApiResult<Json<EnsembleMember>> {
    Ok(Json(app::put_member(&state, &user, ensemble_id, member_id, input).await?))
}

#[utoipa::path(
    delete,
    path = "/ensembles/{ensembleId}/members/{userId}",
    tag = "ensembles",
    operation_id = "deleteEnsembleMember",
    summary = "Remove a member (admin, librarian, leaders of the ensemble)",
    params(
        ("ensembleId" = uuid::Uuid, Path),
        ("userId" = uuid::Uuid, Path),
    ),
    responses(
        (status = 204, description = "Removed."),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 404, response = crate::openapi::NotFound),
    ),
)]
async fn remove_member(
    State(state): State<App>,
    user: SignedIn,
    Path((ensemble_id, member_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    app::remove_member(&state, &user, ensemble_id, member_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    put,
    path = "/ensembles/{ensembleId}/works/{workId}",
    tag = "ensembles",
    operation_id = "addEnsembleWork",
    summary = "Give a work to an ensemble (admin, librarian, leaders of the ensemble)",
    params(
        ("ensembleId" = uuid::Uuid, Path),
        ("workId" = uuid::Uuid, Path),
    ),
    responses(
        (status = 204, description = "The ensemble has the work."),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 404, response = crate::openapi::NotFound),
    ),
)]
async fn add_work(
    State(state): State<App>,
    user: SignedIn,
    Path((ensemble_id, work_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    app::add_work(&state, &user, ensemble_id, work_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/ensembles/{ensembleId}/works/{workId}",
    tag = "ensembles",
    operation_id = "removeEnsembleWork",
    summary = "Take a work from an ensemble (admin, librarian, leaders of the ensemble)",
    params(
        ("ensembleId" = uuid::Uuid, Path),
        ("workId" = uuid::Uuid, Path),
    ),
    responses(
        (status = 204, description = "Removed."),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 404, response = crate::openapi::NotFound),
    ),
)]
async fn remove_work(
    State(state): State<App>,
    user: SignedIn,
    Path((ensemble_id, work_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    app::remove_work(&state, &user, ensemble_id, work_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/my/music",
    tag = "ensembles",
    operation_id = "getMyMusic",
    summary = "The parts the signed-in user may see, by ensemble and work",
    responses(
        (status = 200, description = "One entry per ensemble the user belongs to.", body = Vec<MyEnsemble>),
    ),
)]
async fn my_music(State(state): State<App>, user: SignedIn) -> ApiResult<Json<Vec<MyEnsemble>>> {
    Ok(Json(app::my_music(&state, &user).await?))
}

#[utoipa::path(
    get,
    path = "/parts/{partId}/info",
    tag = "library",
    operation_id = "getPartInfo",
    summary = "What the viewer shows about a part (anyone who may see the part)",
    params(
        ("partId" = uuid::Uuid, Path),
    ),
    responses(
        (status = 200, description = "The part.", body = PartInfo),
        (status = 404, response = crate::openapi::NotFound),
    ),
)]
async fn part_info(State(state): State<App>, user: SignedIn, Path(part_id): Path<Uuid>) -> ApiResult<Json<PartInfo>> {
    Ok(Json(app::part_info(&state, &user, part_id).await?))
}
