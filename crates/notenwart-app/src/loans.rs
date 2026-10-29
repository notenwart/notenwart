// SPDX-License-Identifier: AGPL-3.0-or-later

//! The paper archive's loans (`loans` in api/openapi.json): which printed parts of a work someone has borrowed.
//! Admins and librarians keep them; everyone sees their own.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::App;
use crate::auth::CurrentUser;
use crate::error::{AppError, AppResult};
use notenwart_store::loans as db;

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Loan {
    id: Uuid,
    work_id: Uuid,
    work_title: String,
    item: String,
    quantity: i32,
    borrower_id: Option<Uuid>,
    borrower_name: String,
    #[schema(format = Date)]
    lent_on: String,
    #[schema(format = Date)]
    due_on: Option<String>,
    #[schema(format = Date)]
    returned_on: Option<String>,
    note: Option<String>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LoanInput {
    /// What was lent, e.g. "Trompete 2" or "complete set".
    item: String,
    #[serde(default)]
    quantity: Option<i32>,
    /// A person of the organisation; their name is taken from the account.
    borrower_id: Option<Uuid>,
    /// Someone without an account.
    borrower_name: Option<String>,
    /// Today when omitted.
    #[schema(format = Date)]
    lent_on: Option<String>,
    #[schema(format = Date)]
    due_on: Option<String>,
    #[schema(format = Date)]
    returned_on: Option<String>,
    note: Option<String>,
}

fn date(value: Option<String>, field: &str) -> AppResult<Option<String>> {
    match value.map(|d| d.trim().to_string()).filter(|d| !d.is_empty()) {
        Some(date) => {
            let format = time::format_description::parse_borrowed::<2>("[year]-[month]-[day]")
                .map_err(|e| AppError::Internal(anyhow::anyhow!("date format: {e}")))?;
            time::Date::parse(&date, &format).map_err(|_| AppError::Invalid(format!("{field} must be YYYY-MM-DD")))?;
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

async fn check(state: &App, user: &CurrentUser, input: LoanInput) -> AppResult<Checked> {
    let item = input.item.trim().to_string();
    if !(1..=120).contains(&item.chars().count()) {
        return Err(AppError::Invalid("item must have 1 to 120 characters".into()));
    }
    let quantity = input.quantity.unwrap_or(1);
    if !(1..=999).contains(&quantity) {
        return Err(AppError::Invalid("quantity must be 1 to 999".into()));
    }
    let borrower_name = match input.borrower_id {
        Some(id) => db::check(&state.pool, id, user.organization_id)
            .await?
            .ok_or_else(|| AppError::Invalid("unknown borrower".into()))?,
        None => input
            .borrower_name
            .map(|n| n.trim().to_string())
            .filter(|n| (1..=120).contains(&n.chars().count()))
            .ok_or_else(|| AppError::Invalid("a loan needs a borrower".into()))?,
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
    state: &App,
    organization_id: Uuid,
    work: Option<Uuid>,
    borrower: Option<Uuid>,
    open_only: bool,
) -> AppResult<Vec<Loan>> {
    Ok(db::loans(&state.pool, organization_id, work, borrower, open_only)
        .await
        .map(|rows| rows.into_iter().map(Loan::from).collect::<Vec<_>>())?)
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct Filter {
    /// `open` (default) leaves out returned loans.
    #[param(inline, value_type = Show, required = false)]
    show: Option<String>,
}

/// Which loans to list, as the API names it.
#[derive(Debug)]
pub struct Show;

impl crate::schema::Strings for Show {
    const VALUES: &[&str] = &["open", "all"];
}

impl utoipa::PartialSchema for Show {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::Schema> {
        crate::schema::strings::<Show>().into()
    }
}

impl utoipa::ToSchema for Show {}

pub async fn list(state: &App, user: &CurrentUser, filter: Filter) -> AppResult<Vec<Loan>> {
    user.require(user.is_staff())?;
    let open_only = filter.show.as_deref() != Some("all");
    loans(state, user.organization_id, None, None, open_only).await
}

pub async fn for_work(state: &App, user: &CurrentUser, work_id: Uuid) -> AppResult<Vec<Loan>> {
    user.require(user.is_staff())?;
    loans(state, user.organization_id, Some(work_id), None, false).await
}

pub async fn mine(state: &App, user: &CurrentUser) -> AppResult<Vec<Loan>> {
    loans(state, user.organization_id, None, Some(user.id), true).await
}

async fn find(state: &App, user: &CurrentUser, loan_id: Uuid) -> AppResult<Loan> {
    loans(state, user.organization_id, None, None, false)
        .await?
        .into_iter()
        .find(|l| l.id == loan_id)
        .ok_or(AppError::NotFound)
}

pub async fn create(state: &App, user: &CurrentUser, work_id: Uuid, input: LoanInput) -> AppResult<Loan> {
    user.require(user.is_staff())?;
    let known = db::create_select_works(&state.pool, work_id, user.organization_id).await?;
    if !known {
        return Err(AppError::NotFound);
    }
    let l = check(state, user, input).await?;
    let id = db::create_insert_loans(
        &state.pool,
        user.organization_id,
        work_id,
        &l.item,
        l.quantity,
        l.borrower_id,
        &l.borrower_name,
        l.lent_on.as_deref(),
        l.due_on.as_deref(),
        l.returned_on.as_deref(),
        l.note.as_deref(),
    )
    .await?;
    find(state, user, id).await
}

pub async fn update(state: &App, user: &CurrentUser, loan_id: Uuid, input: LoanInput) -> AppResult<Loan> {
    user.require(user.is_staff())?;
    find(state, user, loan_id).await?;
    let l = check(state, user, input).await?;
    db::update(
        &state.pool,
        loan_id,
        &l.item,
        l.quantity,
        l.borrower_id,
        &l.borrower_name,
        l.lent_on.as_deref(),
        l.due_on.as_deref(),
        l.returned_on.as_deref(),
        l.note.as_deref(),
    )
    .await?;
    find(state, user, loan_id).await
}

pub async fn remove(state: &App, user: &CurrentUser, loan_id: Uuid) -> AppResult<()> {
    user.require(user.is_staff())?;
    find(state, user, loan_id).await?;
    db::remove(&state.pool, loan_id).await?;
    Ok(())
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct Borrower {
    id: Uuid,
    name: String,
}

/// People who can borrow: names only, for choosing one (admins and librarians).
pub async fn borrowers(state: &App, user: &CurrentUser) -> AppResult<Vec<Borrower>> {
    user.require(user.is_staff())?;
    Ok(db::borrowers(&state.pool, user.organization_id)
        .await
        .map(|rows| rows.into_iter().map(Borrower::from).collect::<Vec<_>>())?)
}

impl From<db::LoanRow> for Loan {
    fn from(row: db::LoanRow) -> Self {
        Loan {
            id: row.id,
            work_id: row.work_id,
            work_title: row.work_title,
            item: row.item,
            quantity: row.quantity,
            borrower_id: row.borrower_id,
            borrower_name: row.borrower_name,
            lent_on: row.lent_on,
            due_on: row.due_on,
            returned_on: row.returned_on,
            note: row.note,
        }
    }
}

impl From<db::BorrowerRow> for Borrower {
    fn from(row: db::BorrowerRow) -> Self {
        Borrower { id: row.id, name: row.name }
    }
}
