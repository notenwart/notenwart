// SPDX-License-Identifier: AGPL-3.0-or-later

use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::StatusCode;
use axum::routing::{get, put};
use axum::{Json, Router};

use notenwart_app::App;
use notenwart_app::images as app;
use notenwart_app::images::*;
use uuid::Uuid;

use axum::extract::Multipart;
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum_extra::extract::CookieJar;

use crate::auth::SignedIn;
use crate::error::ApiResult;
use crate::files::{file_field, upload_error};

pub fn router() -> Router<App> {
    let upload = DefaultBodyLimit::max(MAX_BYTES + 64 * 1024);
    Router::new()
        .route("/images/{image_id}", get(image))
        .route("/organization", get(organization).put(update_organization))
        .route("/organization/logo", put(organization_logo).delete(remove_organization_logo).layer(upload))
        .route("/ensembles/{ensemble_id}/logo", put(ensemble_logo).delete(remove_ensemble_logo).layer(upload))
        .route("/ensembles/{ensemble_id}/symbol", put(ensemble_symbol))
        .route("/me/avatar", put(my_avatar).delete(remove_my_avatar).layer(upload))
}

async fn organization(State(state): State<App>, user: SignedIn) -> ApiResult<Json<Organization>> {
    Ok(Json(app::organization(&state, &user).await?))
}

async fn update_organization(
    State(state): State<App>,
    user: SignedIn,
    Json(input): Json<OrganizationInput>,
) -> ApiResult<Json<Organization>> {
    Ok(Json(app::update_organization(&state, &user, input).await?))
}

async fn remove_organization_logo(State(state): State<App>, user: SignedIn) -> ApiResult<StatusCode> {
    app::remove_organization_logo(&state, &user).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_ensemble_logo(
    State(state): State<App>,
    user: SignedIn,
    Path(ensemble_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    app::remove_ensemble_logo(&state, &user, ensemble_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn ensemble_symbol(
    State(state): State<App>,
    user: SignedIn,
    Path(ensemble_id): Path<Uuid>,
    Json(input): Json<SymbolInput>,
) -> ApiResult<StatusCode> {
    app::ensemble_symbol(&state, &user, ensemble_id, input).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_my_avatar(State(state): State<App>, user: SignedIn) -> ApiResult<StatusCode> {
    app::remove_my_avatar(&state, &user).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn image(State(state): State<App>, jar: CookieJar, Path(image_id): Path<Uuid>) -> ApiResult<Response> {
    let user = crate::auth::from_cookie(&state, &jar).await?;
    let image = app::image(&state, user.as_ref(), image_id).await?;
    // Ids change with every new image, so they can be kept for long.
    Ok((
        [
            (header::CONTENT_TYPE, image.content_type),
            (header::CACHE_CONTROL, "private, max-age=31536000, immutable".to_string()),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
        ],
        image.bytes,
    )
        .into_response())
}

async fn organization_logo(
    State(state): State<App>,
    user: SignedIn,
    multipart: Multipart,
) -> ApiResult<Json<Organization>> {
    let upload = file_field(multipart, upload_error).await;
    Ok(Json(app::organization_logo(&state, &user, upload).await?))
}

async fn ensemble_logo(
    State(state): State<App>,
    user: SignedIn,
    Path(ensemble_id): Path<Uuid>,
    multipart: Multipart,
) -> ApiResult<StatusCode> {
    let upload = file_field(multipart, upload_error).await;
    app::ensemble_logo(&state, &user, ensemble_id, upload).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn my_avatar(State(state): State<App>, user: SignedIn, multipart: Multipart) -> ApiResult<StatusCode> {
    let upload = file_field(multipart, upload_error).await;
    app::my_avatar(&state, &user, upload).await?;
    Ok(StatusCode::NO_CONTENT)
}
