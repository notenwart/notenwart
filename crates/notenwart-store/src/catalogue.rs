// SPDX-License-Identifier: AGPL-3.0-or-later

//! The data of `notenwart_app::catalogue`: its queries, one function each, checked at compile time against
//! `.sqlx/`.

use uuid::Uuid;

#[derive(Debug)]
pub struct EnsureRow {
    pub id: Uuid,
}

#[derive(Debug)]
pub struct Ensure2Row {
    pub id: Uuid,
    pub name: String,
    pub family: String,
    pub transposition: Option<String>,
    pub aliases: Vec<String>,
    pub sort: i32,
}

#[derive(Debug)]
pub struct FamilyRow {
    pub id: Uuid,
    pub name: String,
    pub sort: i32,
    pub instrument_count: i64,
}

#[derive(Debug)]
pub struct InstrumentRow {
    pub id: Uuid,
    pub name: String,
    pub family_id: Uuid,
    pub family: String,
    pub transposition: Option<String>,
    pub aliases: Vec<String>,
    pub sort: i32,
    pub usage: i64,
}

pub async fn ensure_select_organizations(
    db: impl sqlx::PgExecutor<'_>,
    organization_id: Uuid,
) -> sqlx::Result<EnsureRow> {
    sqlx::query_as!(EnsureRow, "SELECT id FROM organizations WHERE id = $1 FOR UPDATE", organization_id)
        .fetch_one(db)
        .await
}

pub async fn ensure_select_instrument_families(
    db: impl sqlx::PgExecutor<'_>,
    organization_id: Uuid,
) -> sqlx::Result<bool> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM instrument_families WHERE organization_id = $1) AS "has!""#,
        organization_id
    )
    .fetch_one(db)
    .await
}

pub async fn ensure_insert_instrument_families(
    db: impl sqlx::PgExecutor<'_>,
    organization_id: Uuid,
    name: &str,
    sort: i32,
) -> sqlx::Result<Uuid> {
    sqlx::query_scalar!(
        "INSERT INTO instrument_families (organization_id, name, sort) VALUES ($1, $2, $3) RETURNING id",
        organization_id,
        name,
        sort
    )
    .fetch_one(db)
    .await
}

pub async fn ensure_select_instruments(db: impl sqlx::PgExecutor<'_>) -> sqlx::Result<Vec<Ensure2Row>> {
    sqlx::query_as!(
        Ensure2Row,
        r#"SELECT id, name, family::text AS "family!", transposition, aliases, sort
           FROM instruments WHERE organization_id IS NULL ORDER BY sort"#
    )
    .fetch_all(db)
    .await
}

pub async fn ensure_insert_instruments(
    db: impl sqlx::PgExecutor<'_>,
    organization_id: Uuid,
    name: &str,
    p3: Uuid,
    p4: Option<&str>,
    aliases: &[String],
    sort: i32,
) -> sqlx::Result<Uuid> {
    sqlx::query_scalar!(
        "INSERT INTO instruments (organization_id, name, family_id, transposition, aliases, sort)
             VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
        organization_id,
        name,
        p3,
        p4,
        aliases,
        sort
    )
    .fetch_one(db)
    .await
}

pub async fn ensure_update_parts(
    db: impl sqlx::PgExecutor<'_>,
    copy: Uuid,
    id: Uuid,
    organization_id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "UPDATE parts SET instrument_id = $1
             WHERE instrument_id = $2 AND work_id IN (SELECT id FROM works WHERE organization_id = $3)",
        copy,
        id,
        organization_id
    )
    .execute(db)
    .await
}

pub async fn ensure_update_ensemble_member_parts(
    db: impl sqlx::PgExecutor<'_>,
    copy: Uuid,
    id: Uuid,
    organization_id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "UPDATE ensemble_member_parts SET instrument_id = $1
             WHERE instrument_id = $2 AND ensemble_id IN (SELECT id FROM ensembles WHERE organization_id = $3)",
        copy,
        id,
        organization_id
    )
    .execute(db)
    .await
}

pub async fn list_families(db: impl sqlx::PgExecutor<'_>, organization_id: Uuid) -> sqlx::Result<Vec<FamilyRow>> {
    sqlx::query_as!(
        FamilyRow,
        r#"SELECT f.id, f.name, f.sort,
                  (SELECT count(*) FROM instruments i WHERE i.family_id = f.id) AS "instrument_count!"
           FROM instrument_families f WHERE f.organization_id = $1
           ORDER BY f.sort, lower(f.name)"#,
        organization_id
    )
    .fetch_all(db)
    .await
}

pub async fn create_family(
    db: impl sqlx::PgExecutor<'_>,
    organization_id: Uuid,
    name: &str,
    sort: Option<i32>,
) -> sqlx::Result<FamilyRow> {
    sqlx::query_as!(
        FamilyRow,
        r#"INSERT INTO instrument_families (organization_id, name, sort)
           VALUES ($1, $2, coalesce($3, (SELECT coalesce(max(sort), 0) + 10 FROM instrument_families
                                         WHERE organization_id = $1)))
           RETURNING id, name, sort, 0::bigint AS "instrument_count!""#,
        organization_id,
        name,
        sort
    )
    .fetch_one(db)
    .await
}

pub async fn update_family(
    db: impl sqlx::PgExecutor<'_>,
    family_id: Uuid,
    organization_id: Uuid,
    name: &str,
    sort: Option<i32>,
) -> sqlx::Result<Option<FamilyRow>> {
    sqlx::query_as!(
        FamilyRow,
        r#"UPDATE instrument_families f SET name = $3, sort = coalesce($4, f.sort)
           WHERE f.id = $1 AND f.organization_id = $2
           RETURNING f.id, f.name, f.sort,
                     (SELECT count(*) FROM instruments i WHERE i.family_id = f.id) AS "instrument_count!""#,
        family_id,
        organization_id,
        name,
        sort
    )
    .fetch_optional(db)
    .await
}

pub async fn delete_family_select_instruments(db: impl sqlx::PgExecutor<'_>, family_id: Uuid) -> sqlx::Result<bool> {
    sqlx::query_scalar!(r#"SELECT EXISTS (SELECT 1 FROM instruments WHERE family_id = $1) AS "used!""#, family_id)
        .fetch_one(db)
        .await
}

pub async fn delete_family_delete_instrument_families(
    db: impl sqlx::PgExecutor<'_>,
    family_id: Uuid,
    organization_id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("DELETE FROM instrument_families WHERE id = $1 AND organization_id = $2", family_id, organization_id)
        .execute(db)
        .await
}

pub async fn fetch_instrument(
    db: impl sqlx::PgExecutor<'_>,
    id: Uuid,
    organization_id: Uuid,
) -> sqlx::Result<Option<InstrumentRow>> {
    sqlx::query_as!(
        InstrumentRow,
        r#"SELECT i.id, i.name, f.id AS family_id, f.name AS family, i.transposition, i.aliases, i.sort,
                  (SELECT count(*) FROM parts p WHERE p.instrument_id = i.id)
                  + (SELECT count(*) FROM ensemble_member_parts m WHERE m.instrument_id = i.id) AS "usage!"
           FROM instruments i JOIN instrument_families f ON f.id = i.family_id
           WHERE i.id = $1 AND i.organization_id = $2"#,
        id,
        organization_id
    )
    .fetch_optional(db)
    .await
}

pub async fn list_instruments(
    db: impl sqlx::PgExecutor<'_>,
    organization_id: Uuid,
) -> sqlx::Result<Vec<InstrumentRow>> {
    sqlx::query_as!(
        InstrumentRow,
        r#"SELECT i.id, i.name, f.id AS family_id, f.name AS family, i.transposition, i.aliases, i.sort,
                  (SELECT count(*) FROM parts p WHERE p.instrument_id = i.id)
                  + (SELECT count(*) FROM ensemble_member_parts m WHERE m.instrument_id = i.id) AS "usage!"
           FROM instruments i JOIN instrument_families f ON f.id = i.family_id
           WHERE i.organization_id = $1
           ORDER BY f.sort, lower(f.name), i.sort, lower(i.name)"#,
        organization_id
    )
    .fetch_all(db)
    .await
}

pub async fn check_family(db: impl sqlx::PgExecutor<'_>, family_id: Uuid, organization_id: Uuid) -> sqlx::Result<bool> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM instrument_families WHERE id = $1 AND organization_id = $2) AS "known!""#,
        family_id,
        organization_id
    )
    .fetch_one(db)
    .await
}

pub async fn create_instrument(
    db: impl sqlx::PgExecutor<'_>,
    organization_id: Uuid,
    name: &str,
    family_id: Uuid,
    transposition: Option<&str>,
    aliases: &[String],
    sort: Option<i32>,
) -> sqlx::Result<Uuid> {
    sqlx::query_scalar!(
        "INSERT INTO instruments (organization_id, name, family_id, transposition, aliases, sort)
         VALUES ($1, $2, $3, $4, $5, coalesce($6, (SELECT coalesce(max(sort), 0) + 1 FROM instruments
                                                   WHERE family_id = $3)))
         RETURNING id",
        organization_id,
        name,
        family_id,
        transposition,
        aliases,
        sort
    )
    .fetch_one(db)
    .await
}

#[allow(clippy::too_many_arguments)] // one parameter per column
pub async fn update_instrument(
    db: impl sqlx::PgExecutor<'_>,
    instrument_id: Uuid,
    organization_id: Uuid,
    name: &str,
    family_id: Uuid,
    transposition: Option<&str>,
    aliases: &[String],
    sort: Option<i32>,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "UPDATE instruments SET name = $3, family_id = $4, transposition = $5, aliases = $6, sort = coalesce($7, sort)
         WHERE id = $1 AND organization_id = $2",
        instrument_id,
        organization_id,
        name,
        family_id,
        transposition,
        aliases,
        sort
    )
    .execute(db)
    .await
}

pub async fn delete_instrument(
    db: impl sqlx::PgExecutor<'_>,
    instrument_id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("DELETE FROM instruments WHERE id = $1", instrument_id).execute(db).await
}

pub async fn merge_instrument_update_parts(
    db: impl sqlx::PgExecutor<'_>,
    instrument_id: Uuid,
    into_id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("UPDATE parts SET instrument_id = $2 WHERE instrument_id = $1", instrument_id, into_id)
        .execute(db)
        .await
}

pub async fn merge_instrument_delete_ensemble_member_parts(
    db: impl sqlx::PgExecutor<'_>,
    instrument_id: Uuid,
    into_id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "DELETE FROM ensemble_member_parts a USING ensemble_member_parts b
         WHERE a.instrument_id = $1 AND b.instrument_id = $2 AND a.ensemble_id = b.ensemble_id
           AND a.user_id = b.user_id AND coalesce(a.voice, 0) = coalesce(b.voice, 0)",
        instrument_id,
        into_id
    )
    .execute(db)
    .await
}

pub async fn merge_instrument_update_ensemble_member_parts(
    db: impl sqlx::PgExecutor<'_>,
    instrument_id: Uuid,
    into_id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("UPDATE ensemble_member_parts SET instrument_id = $2 WHERE instrument_id = $1", instrument_id, into_id)
        .execute(db)
        .await
}

pub async fn merge_instrument_update_recordings(
    db: impl sqlx::PgExecutor<'_>,
    instrument_id: Uuid,
    into_id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("UPDATE recordings SET instrument_id = $2 WHERE instrument_id = $1", instrument_id, into_id)
        .execute(db)
        .await
}

pub async fn merge_instrument_delete_instruments(
    db: impl sqlx::PgExecutor<'_>,
    instrument_id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("DELETE FROM instruments WHERE id = $1", instrument_id).execute(db).await
}

pub async fn merge_instrument_update_instruments(
    db: impl sqlx::PgExecutor<'_>,
    into_id: Uuid,
    aliases: &[String],
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("UPDATE instruments SET aliases = $2 WHERE id = $1", into_id, aliases).execute(db).await
}
