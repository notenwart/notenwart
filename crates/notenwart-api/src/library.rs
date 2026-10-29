// SPDX-License-Identifier: AGPL-3.0-or-later

use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};

use notenwart_app::App;
use notenwart_app::library as app;
use notenwart_app::library::*;
use uuid::Uuid;

use axum::extract::Multipart;
use axum::response::Response;

use crate::auth::SignedIn;
use crate::error::ApiResult;
use crate::files::{file_field, large_upload_error, pdf_response};

pub fn router(max_upload: usize) -> Router<App> {
    Router::new()
        .route("/tags", get(list_tags))
        .route("/works/facets", get(work_facets))
        .route("/works", get(list_works).post(create_work))
        .route("/works/{work_id}", get(get_work).put(update_work).delete(delete_work))
        .route("/works/{work_id}/sources", post(upload_source).layer(DefaultBodyLimit::max(max_upload)))
        .route("/sources/{source_id}", delete(delete_source))
        .route("/sources/{source_id}/file", get(source_file))
        .route("/works/{work_id}/parts", post(create_part))
        .route("/parts/{part_id}", put(update_part).delete(delete_part))
        .route("/parts/{part_id}/file", get(part_file))
        .route("/recognize", post(recognize).layer(DefaultBodyLimit::max(max_upload)))
        .route("/sources/{source_id}/recognize", post(recognize_source))
        .route("/sources/{source_id}/split", post(split_source))
}

async fn recognize_source(
    State(state): State<App>,
    user: SignedIn,
    Path(source_id): Path<Uuid>,
) -> ApiResult<Json<notenwart_app::recognize::Recognition>> {
    Ok(Json(app::recognize_source(&state, &user, source_id).await?))
}

async fn split_source(
    State(state): State<App>,
    user: SignedIn,
    Path(source_id): Path<Uuid>,
) -> ApiResult<Json<Vec<notenwart_app::recognize::PartProposal>>> {
    Ok(Json(app::split_source(&state, &user, source_id).await?))
}

async fn list_tags(State(state): State<App>, user: SignedIn) -> ApiResult<Json<Vec<String>>> {
    Ok(Json(app::list_tags(&state, &user).await?))
}

async fn work_facets(State(state): State<App>, user: SignedIn) -> ApiResult<Json<Facets>> {
    Ok(Json(app::work_facets(&state, &user).await?))
}

async fn list_works(
    State(state): State<App>,
    user: SignedIn,
    Query(filter): Query<WorkFilter>,
) -> ApiResult<Json<Vec<WorkSummary>>> {
    Ok(Json(app::list_works(&state, &user, filter).await?))
}

async fn create_work(
    State(state): State<App>,
    user: SignedIn,
    Json(input): Json<WorkInput>,
) -> ApiResult<(StatusCode, Json<Work>)> {
    Ok((StatusCode::CREATED, Json(app::create_work(&state, &user, input).await?)))
}

async fn get_work(State(state): State<App>, user: SignedIn, Path(work_id): Path<Uuid>) -> ApiResult<Json<WorkDetail>> {
    Ok(Json(app::get_work(&state, &user, work_id).await?))
}

async fn update_work(
    State(state): State<App>,
    user: SignedIn,
    Path(work_id): Path<Uuid>,
    Json(input): Json<WorkInput>,
) -> ApiResult<Json<Work>> {
    Ok(Json(app::update_work(&state, &user, work_id, input).await?))
}

async fn delete_work(State(state): State<App>, user: SignedIn, Path(work_id): Path<Uuid>) -> ApiResult<StatusCode> {
    app::delete_work(&state, &user, work_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_source(State(state): State<App>, user: SignedIn, Path(source_id): Path<Uuid>) -> ApiResult<StatusCode> {
    app::delete_source(&state, &user, source_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn create_part(
    State(state): State<App>,
    user: SignedIn,
    Path(work_id): Path<Uuid>,
    Json(input): Json<PartInput>,
) -> ApiResult<(StatusCode, Json<Part>)> {
    Ok((StatusCode::CREATED, Json(app::create_part(&state, &user, work_id, input).await?)))
}

async fn update_part(
    State(state): State<App>,
    user: SignedIn,
    Path(part_id): Path<Uuid>,
    Json(input): Json<PartInput>,
) -> ApiResult<Json<Part>> {
    Ok(Json(app::update_part(&state, &user, part_id, input).await?))
}

async fn delete_part(State(state): State<App>, user: SignedIn, Path(part_id): Path<Uuid>) -> ApiResult<StatusCode> {
    app::delete_part(&state, &user, part_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn recognize(
    State(state): State<App>,
    user: SignedIn,
    multipart: Multipart,
) -> ApiResult<Json<notenwart_app::recognize::Recognition>> {
    let upload = file_field(multipart, large_upload_error).await;
    Ok(Json(app::recognize(&state, &user, upload).await?))
}

async fn upload_source(
    State(state): State<App>,
    user: SignedIn,
    Path(work_id): Path<Uuid>,
    multipart: Multipart,
) -> ApiResult<(StatusCode, Json<SourceFile>)> {
    let upload = file_field(multipart, large_upload_error).await;
    Ok((StatusCode::CREATED, Json(app::upload_source(&state, &user, work_id, upload).await?)))
}

async fn source_file(State(state): State<App>, user: SignedIn, Path(source_id): Path<Uuid>) -> ApiResult<Response> {
    Ok(pdf_response(app::source_file(&state, &user, source_id).await?))
}

async fn part_file(State(state): State<App>, user: SignedIn, Path(part_id): Path<Uuid>) -> ApiResult<Response> {
    Ok(pdf_response(app::part_file(&state, &user, part_id).await?))
}
