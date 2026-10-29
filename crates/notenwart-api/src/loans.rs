// SPDX-License-Identifier: AGPL-3.0-or-later

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, put};
use axum::{Json, Router};

use notenwart_app::App;
use notenwart_app::loans as app;
use notenwart_app::loans::*;
use uuid::Uuid;

use crate::auth::SignedIn;
use crate::error::ApiResult;

pub fn router() -> Router<App> {
    Router::new()
        .route("/loans", get(list))
        .route("/loans/{loan_id}", put(update).delete(remove))
        .route("/works/{work_id}/loans", get(for_work).post(create))
        .route("/me/loans", get(mine))
        .route("/borrowers", get(borrowers))
}

#[utoipa::path(
    get,
    path = "/loans",
    tag = "loans",
    operation_id = "listLoans",
    summary = "Loans of the organisation (admin, librarian)",
    params(
        Filter,
    ),
    responses(
        (status = 200, description = "Open loans first, the longest out on top.", body = Vec<Loan>),
        (status = 403, response = crate::openapi::Forbidden),
    ),
)]
async fn list(State(state): State<App>, user: SignedIn, Query(filter): Query<Filter>) -> ApiResult<Json<Vec<Loan>>> {
    Ok(Json(app::list(&state, &user, filter).await?))
}

#[utoipa::path(
    get,
    path = "/works/{workId}/loans",
    tag = "loans",
    operation_id = "listWorkLoans",
    summary = "Loans of one work, returned ones included (admin, librarian)",
    params(
        ("workId" = uuid::Uuid, Path),
    ),
    responses(
        (status = 200, description = "Open loans first.", body = Vec<Loan>),
        (status = 403, response = crate::openapi::Forbidden),
    ),
)]
async fn for_work(State(state): State<App>, user: SignedIn, Path(work_id): Path<Uuid>) -> ApiResult<Json<Vec<Loan>>> {
    Ok(Json(app::for_work(&state, &user, work_id).await?))
}

#[utoipa::path(
    get,
    path = "/me/loans",
    tag = "loans",
    operation_id = "myLoans",
    summary = "What the signed-in user has borrowed and not returned (anyone)",
    responses(
        (status = 200, description = "Open loans.", body = Vec<Loan>),
    ),
)]
async fn mine(State(state): State<App>, user: SignedIn) -> ApiResult<Json<Vec<Loan>>> {
    Ok(Json(app::mine(&state, &user).await?))
}

#[utoipa::path(
    post,
    path = "/works/{workId}/loans",
    tag = "loans",
    operation_id = "createLoan",
    summary = "Lend printed parts of a work (admin, librarian)",
    params(
        ("workId" = uuid::Uuid, Path),
    ),
    request_body = LoanInput,
    responses(
        (status = 201, description = "Created.", body = Loan),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 404, response = crate::openapi::NotFound),
        (status = 422, response = crate::openapi::Invalid),
    ),
)]
async fn create(
    State(state): State<App>,
    user: SignedIn,
    Path(work_id): Path<Uuid>,
    Json(input): Json<LoanInput>,
) -> ApiResult<(StatusCode, Json<Loan>)> {
    Ok((StatusCode::CREATED, Json(app::create(&state, &user, work_id, input).await?)))
}

#[utoipa::path(
    put,
    path = "/loans/{loanId}",
    tag = "loans",
    operation_id = "updateLoan",
    summary = "Change a loan, e.g. mark it returned (admin, librarian)",
    params(
        ("loanId" = uuid::Uuid, Path),
    ),
    request_body = LoanInput,
    responses(
        (status = 200, description = "Saved.", body = Loan),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 404, response = crate::openapi::NotFound),
        (status = 422, response = crate::openapi::Invalid),
    ),
)]
async fn update(
    State(state): State<App>,
    user: SignedIn,
    Path(loan_id): Path<Uuid>,
    Json(input): Json<LoanInput>,
) -> ApiResult<Json<Loan>> {
    Ok(Json(app::update(&state, &user, loan_id, input).await?))
}

#[utoipa::path(
    delete,
    path = "/loans/{loanId}",
    tag = "loans",
    operation_id = "deleteLoan",
    summary = "Delete a loan entered by mistake (admin, librarian)",
    params(
        ("loanId" = uuid::Uuid, Path),
    ),
    responses(
        (status = 204, description = "Deleted."),
        (status = 403, response = crate::openapi::Forbidden),
        (status = 404, response = crate::openapi::NotFound),
    ),
)]
async fn remove(State(state): State<App>, user: SignedIn, Path(loan_id): Path<Uuid>) -> ApiResult<StatusCode> {
    app::remove(&state, &user, loan_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/borrowers",
    tag = "loans",
    operation_id = "listBorrowers",
    summary = "Names of the organisation's people, for choosing a borrower (admin, librarian)",
    responses(
        (status = 200, description = "By name.", body = Vec<Borrower>),
        (status = 403, response = crate::openapi::Forbidden),
    ),
)]
async fn borrowers(State(state): State<App>, user: SignedIn) -> ApiResult<Json<Vec<Borrower>>> {
    Ok(Json(app::borrowers(&state, &user).await?))
}
