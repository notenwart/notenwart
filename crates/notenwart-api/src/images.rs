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

#[utoipa::path(
    get,
    path = "/organization",
    tag = "images",
    operation_id = "getOrganization",
    summary = "The organisation (anyone signed in)",
    responses(
        (status = 200, description = "Name, logo and settings.", body = Organization),
    ),
)]
async fn organization(State(state): State<App>, user: SignedIn) -> ApiResult<Json<Organization>> {
    Ok(Json(app::organization(&state, &user).await?))
}

#[utoipa::path(
    put,
    path = "/organization",
    tag = "images",
    operation_id = "updateOrganization",
    summary = "Rename the organisation, allow or forbid profile pictures (admin)",
    request_body = inline(OrganizationInput),
    responses(
        (status = 200, description = "Saved.", body = Organization),
        (status = 403, response = crate::openapi::Forbidden),
    ),
)]
async fn update_organization(
    State(state): State<App>,
    user: SignedIn,
    Json(input): Json<OrganizationInput>,
) -> ApiResult<Json<Organization>> {
    Ok(Json(app::update_organization(&state, &user, input).await?))
}

#[utoipa::path(
    delete,
    path = "/organization/logo",
    tag = "images",
    operation_id = "removeOrganizationLogo",
    summary = "Remove the organisation's logo (admin)",
    responses(
        (status = 204, description = "Removed."),
    ),
)]
async fn remove_organization_logo(State(state): State<App>, user: SignedIn) -> ApiResult<StatusCode> {
    app::remove_organization_logo(&state, &user).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/ensembles/{ensembleId}/logo",
    tag = "images",
    operation_id = "removeEnsembleLogo",
    summary = "Remove an ensemble's logo (admin, librarian, leaders of the ensemble)",
    params(
        ("ensembleId" = uuid::Uuid, Path),
    ),
    responses(
        (status = 204, description = "Removed."),
    ),
)]
async fn remove_ensemble_logo(
    State(state): State<App>,
    user: SignedIn,
    Path(ensemble_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    app::remove_ensemble_logo(&state, &user, ensemble_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    put,
    path = "/ensembles/{ensembleId}/symbol",
    tag = "images",
    operation_id = "setEnsembleSymbol",
    summary = "Set an ensemble's symbol and colour (admin, librarian, leaders of the ensemble)",
    params(
        ("ensembleId" = uuid::Uuid, Path),
    ),
    request_body = inline(SymbolInput),
    responses(
        (status = 204, description = "Saved."),
        (status = 422, response = crate::openapi::Invalid),
    ),
)]
async fn ensemble_symbol(
    State(state): State<App>,
    user: SignedIn,
    Path(ensemble_id): Path<Uuid>,
    Json(input): Json<SymbolInput>,
) -> ApiResult<StatusCode> {
    app::ensemble_symbol(&state, &user, ensemble_id, input).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/me/avatar",
    tag = "images",
    operation_id = "removeMyAvatar",
    summary = "Remove one's own picture (anyone)",
    responses(
        (status = 204, description = "Removed."),
    ),
)]
async fn remove_my_avatar(State(state): State<App>, user: SignedIn) -> ApiResult<StatusCode> {
    app::remove_my_avatar(&state, &user).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/images/{imageId}",
    tag = "images",
    operation_id = "getImage",
    summary = "An avatar image (the organisation's logo for anyone; others for signed-in people allowed to see them)",
    params(
        ("imageId" = uuid::Uuid, Path),
    ),
    responses(
        (status = 200, description = "The image.", body = inline(crate::openapi::Binary), content_type = "image/*"),
        (status = 404, response = crate::openapi::NotFound),
    ),
    security(),
)]
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

#[utoipa::path(
    put,
    path = "/organization/logo",
    tag = "images",
    operation_id = "setOrganizationLogo",
    summary = "Set the organisation's logo (admin)",
    request_body(content = inline(crate::openapi::FileForm), content_type = "multipart/form-data"),
    responses(
        (status = 200, description = "Saved.", body = Organization),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 422, response = crate::openapi::Invalid),
    ),
)]
async fn organization_logo(
    State(state): State<App>,
    user: SignedIn,
    multipart: Multipart,
) -> ApiResult<Json<Organization>> {
    let upload = file_field(multipart, upload_error).await;
    Ok(Json(app::organization_logo(&state, &user, upload).await?))
}

#[utoipa::path(
    put,
    path = "/ensembles/{ensembleId}/logo",
    tag = "images",
    operation_id = "setEnsembleLogo",
    summary = "Set an ensemble's logo (admin, librarian, leaders of the ensemble)",
    params(
        ("ensembleId" = uuid::Uuid, Path),
    ),
    request_body(content = inline(crate::openapi::FileForm), content_type = "multipart/form-data"),
    responses(
        (status = 204, description = "Saved."),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 422, response = crate::openapi::Invalid),
    ),
)]
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

#[utoipa::path(
    put,
    path = "/me/avatar",
    tag = "images",
    operation_id = "setMyAvatar",
    summary = "Set one's own picture (anyone, if the organisation allows pictures)",
    request_body(content = inline(crate::openapi::FileForm), content_type = "multipart/form-data"),
    responses(
        (status = 204, description = "Saved."),
        (status = 409, description = "The organisation has switched profile pictures off."),
        (status = 422, response = crate::openapi::Invalid),
    ),
)]
async fn my_avatar(State(state): State<App>, user: SignedIn, multipart: Multipart) -> ApiResult<StatusCode> {
    let upload = file_field(multipart, upload_error).await;
    app::my_avatar(&state, &user, upload).await?;
    Ok(StatusCode::NO_CONTENT)
}
