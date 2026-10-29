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

#[utoipa::path(
    post,
    path = "/sources/{sourceId}/recognize",
    tag = "library",
    operation_id = "recognizeSource",
    summary = "Read title, composer, part … from the first page of a stored file (admin, librarian)",
    params(
        ("sourceId" = uuid::Uuid, Path),
    ),
    responses(
        (
            status = 200,
            description = "What was found; nothing is changed.",
            body = notenwart_app::recognize::Recognition,
        ),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 404, response = crate::openapi::NotFound),
    ),
)]
async fn recognize_source(
    State(state): State<App>,
    user: SignedIn,
    Path(source_id): Path<Uuid>,
) -> ApiResult<Json<notenwart_app::recognize::Recognition>> {
    Ok(Json(app::recognize_source(&state, &user, source_id).await?))
}

#[utoipa::path(
    post,
    path = "/sources/{sourceId}/split",
    tag = "library",
    operation_id = "splitSource",
    summary = "Propose the parts of a file with several, from each page's header (admin, librarian)",
    description = "Reads the top of every page (text, or OCR for scans; at most 400 pages). A page naming an \
    instrument starts a part unless it repeats the previous one; pages without a header continue the current part. \
    Nothing is created.",
    params(
        ("sourceId" = uuid::Uuid, Path),
    ),
    responses(
        (
            status = 200,
            description = "Proposed parts in page order.",
            body = Vec<notenwart_app::recognize::PartProposal>,
        ),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 404, response = crate::openapi::NotFound),
        (status = 409, response = crate::openapi::Conflict),
    ),
)]
async fn split_source(
    State(state): State<App>,
    user: SignedIn,
    Path(source_id): Path<Uuid>,
) -> ApiResult<Json<Vec<notenwart_app::recognize::PartProposal>>> {
    Ok(Json(app::split_source(&state, &user, source_id).await?))
}

#[utoipa::path(
    get,
    path = "/tags",
    tag = "library",
    operation_id = "listTags",
    summary = "All tags used on works, for filters",
    responses(
        (status = 200, description = "Tags in alphabetical order.", body = Vec<String>),
    ),
)]
async fn list_tags(State(state): State<App>, user: SignedIn) -> ApiResult<Json<Vec<String>>> {
    Ok(Json(app::list_tags(&state, &user).await?))
}

#[utoipa::path(
    get,
    path = "/works/facets",
    tag = "library",
    operation_id = "getWorkFacets",
    summary = "Values already used on works, for suggestions while typing (anyone who reads the library)",
    responses(
        (
            status = 200,
            description = "Each list is ordered by how often the value is used, then alphabetically.",
            body = Facets,
        ),
        (status = 403, response = crate::openapi::Forbidden),
    ),
)]
async fn work_facets(State(state): State<App>, user: SignedIn) -> ApiResult<Json<Facets>> {
    Ok(Json(app::work_facets(&state, &user).await?))
}

#[utoipa::path(
    get,
    path = "/works",
    tag = "library",
    operation_id = "listWorks",
    summary = "List and search works (admin, librarian, leader)",
    params(
        WorkFilter,
    ),
    responses(
        (status = 200, description = "Works ordered by title.", body = Vec<WorkSummary>),
    ),
)]
async fn list_works(
    State(state): State<App>,
    user: SignedIn,
    Query(filter): Query<WorkFilter>,
) -> ApiResult<Json<Vec<WorkSummary>>> {
    Ok(Json(app::list_works(&state, &user, filter).await?))
}

#[utoipa::path(
    post,
    path = "/works",
    tag = "library",
    operation_id = "createWork",
    summary = "Create a work (admin, librarian)",
    request_body = WorkInput,
    responses(
        (status = 201, description = "Created.", body = Work),
        (status = 422, response = crate::openapi::Invalid),
    ),
)]
async fn create_work(
    State(state): State<App>,
    user: SignedIn,
    Json(input): Json<WorkInput>,
) -> ApiResult<(StatusCode, Json<Work>)> {
    Ok((StatusCode::CREATED, Json(app::create_work(&state, &user, input).await?)))
}

#[utoipa::path(
    get,
    path = "/works/{workId}",
    tag = "library",
    operation_id = "getWork",
    summary = "A work with its source files and parts (admin, librarian, leader)",
    params(
        ("workId" = uuid::Uuid, Path),
    ),
    responses(
        (status = 200, description = "The work.", body = WorkDetail),
        (status = 404, response = crate::openapi::NotFound),
    ),
)]
async fn get_work(State(state): State<App>, user: SignedIn, Path(work_id): Path<Uuid>) -> ApiResult<Json<WorkDetail>> {
    Ok(Json(app::get_work(&state, &user, work_id).await?))
}

#[utoipa::path(
    put,
    path = "/works/{workId}",
    tag = "library",
    operation_id = "updateWork",
    summary = "Replace a work's metadata (admin, librarian)",
    params(
        ("workId" = uuid::Uuid, Path),
    ),
    request_body = WorkInput,
    responses(
        (status = 200, description = "Updated.", body = Work),
        (status = 404, response = crate::openapi::NotFound),
        (status = 422, response = crate::openapi::Invalid),
    ),
)]
async fn update_work(
    State(state): State<App>,
    user: SignedIn,
    Path(work_id): Path<Uuid>,
    Json(input): Json<WorkInput>,
) -> ApiResult<Json<Work>> {
    Ok(Json(app::update_work(&state, &user, work_id, input).await?))
}

#[utoipa::path(
    delete,
    path = "/works/{workId}",
    tag = "library",
    operation_id = "deleteWork",
    summary = "Delete a work with all its files and parts (admin, librarian)",
    params(
        ("workId" = uuid::Uuid, Path),
    ),
    responses(
        (status = 204, description = "Deleted."),
        (status = 404, response = crate::openapi::NotFound),
    ),
)]
async fn delete_work(State(state): State<App>, user: SignedIn, Path(work_id): Path<Uuid>) -> ApiResult<StatusCode> {
    app::delete_work(&state, &user, work_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/sources/{sourceId}",
    tag = "library",
    operation_id = "deleteSource",
    summary = "Delete a source file and every part split from it (admin, librarian)",
    params(
        ("sourceId" = uuid::Uuid, Path),
    ),
    responses(
        (status = 204, description = "Deleted."),
        (status = 404, response = crate::openapi::NotFound),
    ),
)]
async fn delete_source(State(state): State<App>, user: SignedIn, Path(source_id): Path<Uuid>) -> ApiResult<StatusCode> {
    app::delete_source(&state, &user, source_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/works/{workId}/parts",
    tag = "library",
    operation_id = "createPart",
    summary = "Split a part from a source file (admin, librarian)",
    description = "The server extracts the given pages into the part's own PDF.",
    params(
        ("workId" = uuid::Uuid, Path),
    ),
    request_body = PartInput,
    responses(
        (status = 201, description = "Created and rendered.", body = Part),
        (status = 404, response = crate::openapi::NotFound),
        (status = 422, response = crate::openapi::Invalid),
    ),
)]
async fn create_part(
    State(state): State<App>,
    user: SignedIn,
    Path(work_id): Path<Uuid>,
    Json(input): Json<PartInput>,
) -> ApiResult<(StatusCode, Json<Part>)> {
    Ok((StatusCode::CREATED, Json(app::create_part(&state, &user, work_id, input).await?)))
}

#[utoipa::path(
    put,
    path = "/parts/{partId}",
    tag = "library",
    operation_id = "updatePart",
    summary = "Change a part, rendering its PDF again (admin, librarian)",
    params(
        ("partId" = uuid::Uuid, Path),
    ),
    request_body = PartInput,
    responses(
        (status = 200, description = "Updated.", body = Part),
        (status = 404, response = crate::openapi::NotFound),
        (status = 422, response = crate::openapi::Invalid),
    ),
)]
async fn update_part(
    State(state): State<App>,
    user: SignedIn,
    Path(part_id): Path<Uuid>,
    Json(input): Json<PartInput>,
) -> ApiResult<Json<Part>> {
    Ok(Json(app::update_part(&state, &user, part_id, input).await?))
}

#[utoipa::path(
    delete,
    path = "/parts/{partId}",
    tag = "library",
    operation_id = "deletePart",
    summary = "Delete a part (admin, librarian)",
    params(
        ("partId" = uuid::Uuid, Path),
    ),
    responses(
        (status = 204, description = "Deleted."),
        (status = 404, response = crate::openapi::NotFound),
    ),
)]
async fn delete_part(State(state): State<App>, user: SignedIn, Path(part_id): Path<Uuid>) -> ApiResult<StatusCode> {
    app::delete_part(&state, &user, part_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/recognize",
    tag = "library",
    operation_id = "recognize",
    summary = "Read title, composer, part … from the first page of a PDF (admin, librarian)",
    description = "Nothing is stored. The PDF's own text is read, scans go through OCR, and an external AI also reads \
    the page if the operator set one up. Every field is a suggestion and may be missing.",
    request_body(content = inline(crate::openapi::FileForm), content_type = "multipart/form-data"),
    responses(
        (status = 200, description = "What was found.", body = notenwart_app::recognize::Recognition),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 422, response = crate::openapi::Invalid),
    ),
)]
async fn recognize(
    State(state): State<App>,
    user: SignedIn,
    multipart: Multipart,
) -> ApiResult<Json<notenwart_app::recognize::Recognition>> {
    let upload = file_field(multipart, large_upload_error).await;
    Ok(Json(app::recognize(&state, &user, upload).await?))
}

#[utoipa::path(
    post,
    path = "/works/{workId}/sources",
    tag = "library",
    operation_id = "uploadSource",
    summary = "Upload a PDF to a work (admin, librarian)",
    params(
        ("workId" = uuid::Uuid, Path),
    ),
    request_body(content = inline(crate::openapi::FileForm), content_type = "multipart/form-data"),
    responses(
        (status = 201, description = "Stored.", body = SourceFile),
        (status = 404, response = crate::openapi::NotFound),
        (
            status = 409,
            description = "The same file (by SHA-256) is already part of this work.",
            body = crate::error::Problem,
        ),
        (status = 413, description = "The file is larger than the server allows."),
        (status = 422, response = crate::openapi::Invalid),
    ),
)]
async fn upload_source(
    State(state): State<App>,
    user: SignedIn,
    Path(work_id): Path<Uuid>,
    multipart: Multipart,
) -> ApiResult<(StatusCode, Json<SourceFile>)> {
    let upload = file_field(multipart, large_upload_error).await;
    Ok((StatusCode::CREATED, Json(app::upload_source(&state, &user, work_id, upload).await?)))
}

#[utoipa::path(
    get,
    path = "/sources/{sourceId}/file",
    tag = "library",
    operation_id = "getSourceFile",
    summary = "The uploaded PDF (admin, librarian, leader)",
    params(
        ("sourceId" = uuid::Uuid, Path),
    ),
    responses(
        (
            status = 200,
            description = "The PDF as uploaded.",
            body = inline(crate::openapi::Binary),
            content_type = "application/pdf",
        ),
        (status = 404, response = crate::openapi::NotFound),
    ),
)]
async fn source_file(State(state): State<App>, user: SignedIn, Path(source_id): Path<Uuid>) -> ApiResult<Response> {
    Ok(pdf_response(app::source_file(&state, &user, source_id).await?))
}

#[utoipa::path(
    get,
    path = "/parts/{partId}/file",
    tag = "library",
    operation_id = "getPartFile",
    summary = "The part's PDF (anyone who may see the part)",
    params(
        ("partId" = uuid::Uuid, Path),
    ),
    responses(
        (
            status = 200,
            description = "The extracted pages.",
            body = inline(crate::openapi::Binary),
            content_type = "application/pdf",
        ),
        (status = 404, response = crate::openapi::NotFound),
    ),
)]
async fn part_file(State(state): State<App>, user: SignedIn, Path(part_id): Path<Uuid>) -> ApiResult<Response> {
    Ok(pdf_response(app::part_file(&state, &user, part_id).await?))
}
