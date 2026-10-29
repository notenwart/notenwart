// SPDX-License-Identifier: AGPL-3.0-or-later

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use axum::{Json, Router};

use notenwart_app::App;
use notenwart_app::people as app;
use notenwart_app::people::*;
use uuid::Uuid;

use crate::auth::SignedIn;
use crate::error::ApiResult;

pub fn router() -> Router<App> {
    Router::new()
        .route("/users", get(list).post(create))
        .route("/users/{user_id}", put(update).delete(remove))
        .route("/users/{user_id}/invitation", post(invite))
        .route("/users/bulk", post(bulk))
}

#[utoipa::path(
    get,
    path = "/users",
    tag = "people",
    operation_id = "listUsers",
    summary = "Users of the organisation (admin, librarian, leader; email only for admins)",
    responses(
        (status = 200, description = "Users ordered by name.", body = Vec<User>),
        (status = 403, response = crate::openapi::Forbidden),
    ),
)]
async fn list(State(state): State<App>, user: SignedIn) -> ApiResult<Json<Vec<User>>> {
    Ok(Json(app::list(&state, &user).await?))
}

#[utoipa::path(
    post,
    path = "/users",
    tag = "people",
    operation_id = "createUser",
    summary = "Add a user and create an invitation link (admin)",
    request_body = UserInput,
    responses(
        (
            status = 201,
            description = "Created; the link is also emailed when the server can send mail.",
            body = Invitation,
        ),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 409, response = crate::openapi::Conflict),
        (status = 422, response = crate::openapi::Invalid),
    ),
)]
async fn create(
    State(state): State<App>,
    admin: SignedIn,
    Json(input): Json<UserInput>,
) -> ApiResult<(StatusCode, Json<Invitation>)> {
    Ok((StatusCode::CREATED, Json(app::create(&state, &admin, input).await?)))
}

#[utoipa::path(
    put,
    path = "/users/{userId}",
    tag = "people",
    operation_id = "updateUser",
    summary = "Change a user's name, email or role (admin)",
    params(
        ("userId" = uuid::Uuid, Path),
    ),
    request_body = UserInput,
    responses(
        (status = 200, description = "Updated.", body = User),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 404, response = crate::openapi::NotFound),
        (status = 409, response = crate::openapi::Conflict),
        (status = 422, response = crate::openapi::Invalid),
    ),
)]
async fn update(
    State(state): State<App>,
    admin: SignedIn,
    Path(user_id): Path<Uuid>,
    Json(input): Json<UserInput>,
) -> ApiResult<Json<User>> {
    Ok(Json(app::update(&state, &admin, user_id, input).await?))
}

#[utoipa::path(
    delete,
    path = "/users/{userId}",
    tag = "people",
    operation_id = "deleteUser",
    summary = "Delete a user (admin; not yourself)",
    params(
        ("userId" = uuid::Uuid, Path),
    ),
    responses(
        (status = 204, description = "Deleted."),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 404, response = crate::openapi::NotFound),
        (
            status = 409,
            description = "The person has borrowed printed parts that are not returned.",
            body = crate::error::Problem,
        ),
    ),
)]
async fn remove(State(state): State<App>, admin: SignedIn, Path(user_id): Path<Uuid>) -> ApiResult<StatusCode> {
    app::remove(&state, &admin, user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/users/{userId}/invitation",
    tag = "people",
    operation_id = "inviteUser",
    summary = "Create a new invitation link for a user (admin); older links stop working",
    params(
        ("userId" = uuid::Uuid, Path),
    ),
    responses(
        (status = 201, description = "Created.", body = Invitation),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 404, response = crate::openapi::NotFound),
    ),
)]
async fn invite(
    State(state): State<App>,
    admin: SignedIn,
    Path(user_id): Path<Uuid>,
) -> ApiResult<(StatusCode, Json<Invitation>)> {
    Ok((StatusCode::CREATED, Json(app::invite(&state, &admin, user_id).await?)))
}

#[utoipa::path(
    post,
    path = "/users/bulk",
    tag = "people",
    operation_id = "bulkUsers",
    summary = "Do one thing to many people at once (admin)",
    description = "One transaction. Changing the role of or deleting yourself is skipped and reported, as are ids \
    that are\nnot people of the organisation.",
    request_body = BulkInput,
    responses(
        (status = 200, description = "Done.", body = BulkResult),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 404, response = crate::openapi::NotFound),
        (status = 422, response = crate::openapi::Invalid),
    ),
)]
async fn bulk(State(state): State<App>, admin: SignedIn, Json(input): Json<BulkInput>) -> ApiResult<Json<BulkResult>> {
    Ok(Json(app::bulk(&state, &admin, input).await?))
}
