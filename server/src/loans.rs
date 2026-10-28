// SPDX-License-Identifier: AGPL-3.0-or-later

//! The paper archive's loans (`loans` in api/openapi.yaml): which printed parts of a work someone has borrowed.
//! Admins and librarians keep them; everyone sees their own.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::app::AppState;
use crate::auth::CurrentUser;
use crate::error::{ApiError, ApiResult};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/loans", get(list))
        .route("/loans/{loan_id}", put(update).delete(remove))
        .route("/works/{work_id}/loans", get(for_work).post(create))
        .route("/me/loans", get(mine))
        .route("/borrowers", get(borrowers))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Loan {
    id: Uuid,
    work_id: Uuid,
    work_title: String,
    item: String,
    quantity: i32,
    borrower_id: Option<Uuid>,
    borrower_name: String,
    lent_on: String,
    due_on: Option<String>,
    returned_on: Option<String>,
    note: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LoanInput {
    item: String,
    #[serde(default)]
    quantity: Option<i32>,
    /// A person of the organisation; their name is taken from the account.
    borrower_id: Option<Uuid>,
    /// Someone without an account.
    borrower_name: Option<String>,
    lent_on: Option<String>,
    due_on: Option<String>,
    returned_on: Option<String>,
    note: Option<String>,
}

fn date(value: Option<String>, field: &str) -> ApiResult<Option<String>> {
    match value.map(|d| d.trim().to_string()).filter(|d| !d.is_empty()) {
        Some(date) => {
            let format = time::format_description::parse_borrowed::<2>("[year]-[month]-[day]")
                .map_err(|e| ApiError::Internal(anyhow::anyhow!("date format: {e}")))?;
            time::Date::parse(&date, &format).map_err(|_| ApiError::Invalid(format!("{field} must be YYYY-MM-DD")))?;
            Ok(Some(date))
        }
        None => Ok(None),
    }
}

struct Checked {
    item: String,
    quantity: i32,
    borrower_id: Option<Uuid>,
    borrower_name: String,
    lent_on: Option<String>,
    due_on: Option<String>,
    returned_on: Option<String>,
    note: Option<String>,
}

async fn check(state: &AppState, user: &CurrentUser, input: LoanInput) -> ApiResult<Checked> {
    let item = input.item.trim().to_string();
    if !(1..=120).contains(&item.chars().count()) {
        return Err(ApiError::Invalid("item must have 1 to 120 characters".into()));
    }
    let quantity = input.quantity.unwrap_or(1);
    if !(1..=999).contains(&quantity) {
        return Err(ApiError::Invalid("quantity must be 1 to 999".into()));
    }
    let borrower_name = match input.borrower_id {
        Some(id) => sqlx::query_scalar!(
            "SELECT u.display_name FROM users u
             JOIN memberships m ON m.user_id = u.id WHERE u.id = $1 AND m.organization_id = $2",
            id,
            user.organization_id
        )
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::Invalid("unknown borrower".into()))?,
        None => input
            .borrower_name
            .map(|n| n.trim().to_string())
            .filter(|n| (1..=120).contains(&n.chars().count()))
            .ok_or_else(|| ApiError::Invalid("a loan needs a borrower".into()))?,
    };
    let note = input.note.map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
    Ok(Checked {
        item,
        quantity,
        borrower_id: input.borrower_id,
        borrower_name,
        lent_on: date(input.lent_on, "lentOn")?,
        due_on: date(input.due_on, "dueOn")?,
        returned_on: date(input.returned_on, "returnedOn")?,
        note,
    })
}

/// Loans of the organisation, filtered; open ones first, the longest out on top.
async fn loans(
    state: &AppState,
    organization_id: Uuid,
    work: Option<Uuid>,
    borrower: Option<Uuid>,
    open_only: bool,
) -> ApiResult<Vec<Loan>> {
    Ok(sqlx::query_as!(
        Loan,
        r#"SELECT l.id, l.work_id, w.title AS work_title, l.item, l.quantity, l.borrower_id, l.borrower_name,
                  to_char(l.lent_on, 'YYYY-MM-DD') AS "lent_on!", to_char(l.due_on, 'YYYY-MM-DD') AS due_on,
                  to_char(l.returned_on, 'YYYY-MM-DD') AS returned_on, l.note
           FROM loans l JOIN works w ON w.id = l.work_id
           WHERE l.organization_id = $1
             AND ($2::uuid IS NULL OR l.work_id = $2)
             AND ($3::uuid IS NULL OR l.borrower_id = $3)
             AND (NOT $4 OR l.returned_on IS NULL)
           ORDER BY l.returned_on IS NOT NULL, l.lent_on, lower(w.title)"#,
        organization_id,
        work,
        borrower,
        open_only
    )
    .fetch_all(&state.pool)
    .await?)
}

#[derive(Deserialize)]
pub struct Filter {
    /// `all` includes returned loans.
    show: Option<String>,
}

async fn list(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(filter): Query<Filter>,
) -> ApiResult<Json<Vec<Loan>>> {
    user.require(user.is_staff())?;
    let open_only = filter.show.as_deref() != Some("all");
    Ok(Json(loans(&state, user.organization_id, None, None, open_only).await?))
}

async fn for_work(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(work_id): Path<Uuid>,
) -> ApiResult<Json<Vec<Loan>>> {
    user.require(user.is_staff())?;
    Ok(Json(
        loans(&state, user.organization_id, Some(work_id), None, false).await?,
    ))
}

async fn mine(State(state): State<AppState>, user: CurrentUser) -> ApiResult<Json<Vec<Loan>>> {
    Ok(Json(
        loans(&state, user.organization_id, None, Some(user.id), true).await?,
    ))
}

async fn find(state: &AppState, user: &CurrentUser, loan_id: Uuid) -> ApiResult<Loan> {
    loans(state, user.organization_id, None, None, false)
        .await?
        .into_iter()
        .find(|l| l.id == loan_id)
        .ok_or(ApiError::NotFound)
}

async fn create(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(work_id): Path<Uuid>,
    Json(input): Json<LoanInput>,
) -> ApiResult<(StatusCode, Json<Loan>)> {
    user.require(user.is_staff())?;
    let known = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM works WHERE id = $1 AND organization_id = $2) AS "known!""#,
        work_id,
        user.organization_id
    )
    .fetch_one(&state.pool)
    .await?;
    if !known {
        return Err(ApiError::NotFound);
    }
    let l = check(&state, &user, input).await?;
    let id = sqlx::query_scalar!(
        "INSERT INTO loans (organization_id, work_id, item, quantity, borrower_id, borrower_name, lent_on, due_on,
                            returned_on, note)
         VALUES ($1, $2, $3, $4, $5, $6, coalesce($7::text::date, current_date), $8::text::date, $9::text::date, $10)
         RETURNING id",
        user.organization_id,
        work_id,
        l.item,
        l.quantity,
        l.borrower_id,
        l.borrower_name,
        l.lent_on,
        l.due_on,
        l.returned_on,
        l.note
    )
    .fetch_one(&state.pool)
    .await?;
    Ok((StatusCode::CREATED, Json(find(&state, &user, id).await?)))
}

async fn update(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(loan_id): Path<Uuid>,
    Json(input): Json<LoanInput>,
) -> ApiResult<Json<Loan>> {
    user.require(user.is_staff())?;
    find(&state, &user, loan_id).await?;
    let l = check(&state, &user, input).await?;
    sqlx::query!(
        "UPDATE loans SET item = $2, quantity = $3, borrower_id = $4, borrower_name = $5,
                          lent_on = coalesce($6::text::date, lent_on), due_on = $7::text::date,
                          returned_on = $8::text::date, note = $9
         WHERE id = $1",
        loan_id,
        l.item,
        l.quantity,
        l.borrower_id,
        l.borrower_name,
        l.lent_on,
        l.due_on,
        l.returned_on,
        l.note
    )
    .execute(&state.pool)
    .await?;
    Ok(Json(find(&state, &user, loan_id).await?))
}

async fn remove(State(state): State<AppState>, user: CurrentUser, Path(loan_id): Path<Uuid>) -> ApiResult<StatusCode> {
    user.require(user.is_staff())?;
    find(&state, &user, loan_id).await?;
    sqlx::query!("DELETE FROM loans WHERE id = $1", loan_id)
        .execute(&state.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize)]
pub struct Borrower {
    id: Uuid,
    name: String,
}

/// People who can borrow: names only, for choosing one (admins and librarians).
async fn borrowers(State(state): State<AppState>, user: CurrentUser) -> ApiResult<Json<Vec<Borrower>>> {
    user.require(user.is_staff())?;
    Ok(Json(
        sqlx::query_as!(
            Borrower,
            "SELECT u.id, u.display_name AS name FROM users u
             JOIN memberships m ON m.user_id = u.id WHERE m.organization_id = $1
             ORDER BY lower(u.display_name)",
            user.organization_id
        )
        .fetch_all(&state.pool)
        .await?,
    ))
}
