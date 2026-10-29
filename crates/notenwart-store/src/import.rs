// SPDX-License-Identifier: AGPL-3.0-or-later

//! The data of `notenwart_app::import`: its queries, one function each, checked at compile time against
//! `.sqlx/`.

use notenwart_core::Role;
use uuid::Uuid;

#[derive(Debug)]
pub struct ImportRow {
    pub id: Uuid,
    pub email: String,
    pub member: bool,
}

#[derive(Debug)]
pub struct Import2Row {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug)]
pub struct Import3Row {
    pub id: Uuid,
    pub name: String,
    pub aliases: Vec<String>,
}

pub async fn import_select_memberships(db: impl sqlx::PgExecutor<'_>, org: Uuid) -> sqlx::Result<Vec<ImportRow>> {
    sqlx::query_as!(
        ImportRow,
        r#"SELECT u.id, lower(u.email) AS "email!",
                  EXISTS (SELECT 1 FROM memberships m WHERE m.user_id = u.id AND m.organization_id = $1) AS "member!"
           FROM users u"#,
        org
    )
    .fetch_all(db)
    .await
}

pub async fn import_select_ensembles(db: impl sqlx::PgExecutor<'_>, org: Uuid) -> sqlx::Result<Vec<Import2Row>> {
    sqlx::query_as!(Import2Row, "SELECT id, lower(name) AS \"name!\" FROM ensembles WHERE organization_id = $1", org)
        .fetch_all(db)
        .await
}

pub async fn import_select_instruments(db: impl sqlx::PgExecutor<'_>, org: Uuid) -> sqlx::Result<Vec<Import3Row>> {
    sqlx::query_as!(
        Import3Row,
        "SELECT id, name, aliases FROM instruments WHERE organization_id = $1 ORDER BY sort",
        org
    )
    .fetch_all(db)
    .await
}

pub async fn import_insert_ensembles(db: impl sqlx::PgExecutor<'_>, org: Uuid, name: &str) -> sqlx::Result<Uuid> {
    sqlx::query_scalar!("INSERT INTO ensembles (organization_id, name) VALUES ($1, $2) RETURNING id", org, name)
        .fetch_one(db)
        .await
}

pub async fn import_insert_users(db: impl sqlx::PgExecutor<'_>, email: &str, name: &str) -> sqlx::Result<Uuid> {
    sqlx::query_scalar!("INSERT INTO users (email, display_name) VALUES ($1, $2) RETURNING id", email, name)
        .fetch_one(db)
        .await
}

pub async fn import_insert_memberships(
    db: impl sqlx::PgExecutor<'_>,
    org: Uuid,
    id: Uuid,
    role: Role,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("INSERT INTO memberships (organization_id, user_id, role) VALUES ($1, $2, $3)", org, id, role as Role)
        .execute(db)
        .await
}

pub async fn import_insert_ensemble_members(
    db: impl sqlx::PgExecutor<'_>,
    ensemble: &Uuid,
    user_id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "INSERT INTO ensemble_members (ensemble_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        ensemble,
        user_id
    )
    .execute(db)
    .await
}

pub async fn import_insert_ensemble_member_parts(
    db: impl sqlx::PgExecutor<'_>,
    ensemble: &Uuid,
    user_id: Uuid,
    instrument: Uuid,
    voice: Option<i16>,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "INSERT INTO ensemble_member_parts (ensemble_id, user_id, instrument_id, voice)
                 VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING",
        ensemble,
        user_id,
        instrument,
        voice
    )
    .execute(db)
    .await
}
