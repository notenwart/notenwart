// SPDX-License-Identifier: AGPL-3.0-or-later

//! The data of `notenwart_app::loans`: its queries, one function each, checked at compile time against
//! `.sqlx/`.

use uuid::Uuid;

#[derive(Debug)]
pub struct LoanRow {
    pub id: Uuid,
    pub work_id: Uuid,
    pub work_title: String,
    pub item: String,
    pub quantity: i32,
    pub borrower_id: Option<Uuid>,
    pub borrower_name: String,
    pub lent_on: String,
    pub due_on: Option<String>,
    pub returned_on: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug)]
pub struct BorrowerRow {
    pub id: Uuid,
    pub name: String,
}

pub async fn check(db: impl sqlx::PgExecutor<'_>, id: Uuid, organization_id: Uuid) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar!(
        "SELECT u.display_name FROM users u
             JOIN memberships m ON m.user_id = u.id WHERE u.id = $1 AND m.organization_id = $2",
        id,
        organization_id
    )
    .fetch_optional(db)
    .await
}

pub async fn loans(
    db: impl sqlx::PgExecutor<'_>,
    organization_id: Uuid,
    work: Option<Uuid>,
    borrower: Option<Uuid>,
    open_only: bool,
) -> sqlx::Result<Vec<LoanRow>> {
    sqlx::query_as!(
        LoanRow,
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
    .fetch_all(db)
    .await
}

pub async fn create_select_works(
    db: impl sqlx::PgExecutor<'_>,
    work_id: Uuid,
    organization_id: Uuid,
) -> sqlx::Result<bool> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM works WHERE id = $1 AND organization_id = $2) AS "known!""#,
        work_id,
        organization_id
    )
    .fetch_one(db)
    .await
}

#[allow(clippy::too_many_arguments)] // one parameter per column
pub async fn create_insert_loans(
    db: impl sqlx::PgExecutor<'_>,
    organization_id: Uuid,
    work_id: Uuid,
    item: &str,
    quantity: i32,
    borrower_id: Option<Uuid>,
    borrower_name: &str,
    lent_on: Option<&str>,
    due_on: Option<&str>,
    returned_on: Option<&str>,
    note: Option<&str>,
) -> sqlx::Result<Uuid> {
    sqlx::query_scalar!(
        "INSERT INTO loans (organization_id, work_id, item, quantity, borrower_id, borrower_name, lent_on, due_on,
                            returned_on, note)
         VALUES ($1, $2, $3, $4, $5, $6, coalesce($7::text::date, current_date), $8::text::date, $9::text::date, $10)
         RETURNING id",
        organization_id,
        work_id,
        item,
        quantity,
        borrower_id,
        borrower_name,
        lent_on,
        due_on,
        returned_on,
        note
    )
    .fetch_one(db)
    .await
}

#[allow(clippy::too_many_arguments)] // one parameter per column
pub async fn update(
    db: impl sqlx::PgExecutor<'_>,
    loan_id: Uuid,
    item: &str,
    quantity: i32,
    borrower_id: Option<Uuid>,
    borrower_name: &str,
    lent_on: Option<&str>,
    due_on: Option<&str>,
    returned_on: Option<&str>,
    note: Option<&str>,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "UPDATE loans SET item = $2, quantity = $3, borrower_id = $4, borrower_name = $5,
                          lent_on = coalesce($6::text::date, lent_on), due_on = $7::text::date,
                          returned_on = $8::text::date, note = $9
         WHERE id = $1",
        loan_id,
        item,
        quantity,
        borrower_id,
        borrower_name,
        lent_on,
        due_on,
        returned_on,
        note
    )
    .execute(db)
    .await
}

pub async fn remove(db: impl sqlx::PgExecutor<'_>, loan_id: Uuid) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("DELETE FROM loans WHERE id = $1", loan_id).execute(db).await
}

pub async fn borrowers(db: impl sqlx::PgExecutor<'_>, organization_id: Uuid) -> sqlx::Result<Vec<BorrowerRow>> {
    sqlx::query_as!(
        BorrowerRow,
        "SELECT u.id, u.display_name AS name FROM users u
             JOIN memberships m ON m.user_id = u.id WHERE m.organization_id = $1
             ORDER BY lower(u.display_name)",
        organization_id
    )
    .fetch_all(db)
    .await
}
