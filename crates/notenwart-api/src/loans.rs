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

async fn list(State(state): State<App>, user: SignedIn, Query(filter): Query<Filter>) -> ApiResult<Json<Vec<Loan>>> {
    Ok(Json(app::list(&state, &user, filter).await?))
}

async fn for_work(State(state): State<App>, user: SignedIn, Path(work_id): Path<Uuid>) -> ApiResult<Json<Vec<Loan>>> {
    Ok(Json(app::for_work(&state, &user, work_id).await?))
}

async fn mine(State(state): State<App>, user: SignedIn) -> ApiResult<Json<Vec<Loan>>> {
    Ok(Json(app::mine(&state, &user).await?))
}

async fn create(
    State(state): State<App>,
    user: SignedIn,
    Path(work_id): Path<Uuid>,
    Json(input): Json<LoanInput>,
) -> ApiResult<(StatusCode, Json<Loan>)> {
    Ok((StatusCode::CREATED, Json(app::create(&state, &user, work_id, input).await?)))
}

async fn update(
    State(state): State<App>,
    user: SignedIn,
    Path(loan_id): Path<Uuid>,
    Json(input): Json<LoanInput>,
) -> ApiResult<Json<Loan>> {
    Ok(Json(app::update(&state, &user, loan_id, input).await?))
}

async fn remove(State(state): State<App>, user: SignedIn, Path(loan_id): Path<Uuid>) -> ApiResult<StatusCode> {
    app::remove(&state, &user, loan_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn borrowers(State(state): State<App>, user: SignedIn) -> ApiResult<Json<Vec<Borrower>>> {
    Ok(Json(app::borrowers(&state, &user).await?))
}
