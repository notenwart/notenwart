// SPDX-License-Identifier: AGPL-3.0-or-later

//! The data of `notenwart_app::people`: its queries, one function each, checked at compile time against
//! `.sqlx/`.

use notenwart_core::Role;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug)]
pub struct UserrowRow {
    pub id: Uuid,
    pub name: String,
    pub email: Option<String>,
    pub role: Role,
    pub has_password: bool,
    pub last_login_at: Option<OffsetDateTime>,
    pub avatar_url: Option<String>,
}

#[derive(Debug)]
pub struct ListRow {
    pub user_id: Uuid,
    pub id: Uuid,
    pub name: String,
}

pub async fn find(
    db: impl sqlx::PgExecutor<'_>,
    user_id: Uuid,
    organization_id: Uuid,
) -> sqlx::Result<Option<UserrowRow>> {
    sqlx::query_as!(
        UserrowRow,
        r#"SELECT u.id, u.display_name AS name, u.email AS "email?", m.role AS "role: Role",
                  u.password_hash IS NOT NULL AS "has_password!", u.last_login_at,
                  CASE WHEN o.profile_pictures THEN '/api/images/' || u.avatar_id END AS avatar_url
           FROM users u JOIN memberships m ON m.user_id = u.id JOIN organizations o ON o.id = m.organization_id
           WHERE u.id = $1 AND m.organization_id = $2"#,
        user_id,
        organization_id
    )
    .fetch_optional(db)
    .await
}

pub async fn list_select_users(db: impl sqlx::PgExecutor<'_>, organization_id: Uuid) -> sqlx::Result<Vec<UserrowRow>> {
    sqlx::query_as!(
        UserrowRow,
        r#"SELECT u.id, u.display_name AS name, u.email AS "email?", m.role AS "role: Role",
                  u.password_hash IS NOT NULL AS "has_password!", u.last_login_at,
                  CASE WHEN o.profile_pictures THEN '/api/images/' || u.avatar_id END AS avatar_url
           FROM users u JOIN memberships m ON m.user_id = u.id JOIN organizations o ON o.id = m.organization_id
           WHERE m.organization_id = $1
           ORDER BY lower(u.display_name), u.id"#,
        organization_id
    )
    .fetch_all(db)
    .await
}

pub async fn list_select_ensemble_members(
    db: impl sqlx::PgExecutor<'_>,
    organization_id: Uuid,
) -> sqlx::Result<Vec<ListRow>> {
    sqlx::query_as!(
        ListRow,
        "SELECT m.user_id, e.id, e.name FROM ensemble_members m JOIN ensembles e ON e.id = m.ensemble_id
         WHERE e.organization_id = $1 ORDER BY lower(e.name)",
        organization_id
    )
    .fetch_all(db)
    .await
}

pub async fn create_insert_users(db: impl sqlx::PgExecutor<'_>, email: &str, name: &str) -> sqlx::Result<Uuid> {
    sqlx::query_scalar!("INSERT INTO users (email, display_name) VALUES ($1, $2) RETURNING id", email, name)
        .fetch_one(db)
        .await
}

pub async fn create_insert_memberships(
    db: impl sqlx::PgExecutor<'_>,
    organization_id: Uuid,
    user_id: Uuid,
    role: Role,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "INSERT INTO memberships (organization_id, user_id, role) VALUES ($1, $2, $3)",
        organization_id,
        user_id,
        role as Role
    )
    .execute(db)
    .await
}

pub async fn update_update_users(
    db: impl sqlx::PgExecutor<'_>,
    user_id: Uuid,
    name: &str,
    email: &str,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("UPDATE users SET display_name = $2, email = $3 WHERE id = $1", user_id, name, email).execute(db).await
}

pub async fn update_update_memberships(
    db: impl sqlx::PgExecutor<'_>,
    user_id: Uuid,
    organization_id: Uuid,
    role: Role,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "UPDATE memberships SET role = $3 WHERE user_id = $1 AND organization_id = $2",
        user_id,
        organization_id,
        role as Role
    )
    .execute(db)
    .await
}

pub async fn update_update_ensemble_members(
    db: impl sqlx::PgExecutor<'_>,
    user_id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("UPDATE ensemble_members SET leader = false WHERE user_id = $1", user_id).execute(db).await
}

pub async fn bulk_select_memberships(
    db: impl sqlx::PgExecutor<'_>,
    org: Uuid,
    user_ids: &[Uuid],
) -> sqlx::Result<Vec<Uuid>> {
    sqlx::query_scalar!(
        "SELECT user_id FROM memberships WHERE organization_id = $1 AND user_id = ANY($2)",
        org,
        user_ids
    )
    .fetch_all(db)
    .await
}

pub async fn bulk_select_ensembles(db: impl sqlx::PgExecutor<'_>, id: Uuid, org: Uuid) -> sqlx::Result<bool> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM ensembles WHERE id = $1 AND organization_id = $2) AS "x!""#,
        id,
        org
    )
    .fetch_one(db)
    .await
}

pub async fn bulk_select_instruments(db: impl sqlx::PgExecutor<'_>, instrument: Uuid, org: Uuid) -> sqlx::Result<bool> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM instruments
                              WHERE id = $1 AND organization_id = $2) AS "x!""#,
        instrument,
        org
    )
    .fetch_one(db)
    .await
}

pub async fn bulk_update_memberships(
    db: impl sqlx::PgExecutor<'_>,
    org: Uuid,
    ids: &[Uuid],
    role: Role,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "UPDATE memberships SET role = $3 WHERE organization_id = $1 AND user_id = ANY($2)",
        org,
        ids,
        role as Role
    )
    .execute(db)
    .await
}

pub async fn bulk_update_ensemble_members(
    db: impl sqlx::PgExecutor<'_>,
    ids: &[Uuid],
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("UPDATE ensemble_members SET leader = false WHERE user_id = ANY($1)", ids).execute(db).await
}

pub async fn bulk_insert_ensemble_members(
    db: impl sqlx::PgExecutor<'_>,
    ensemble: Uuid,
    ids: &[Uuid],
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "INSERT INTO ensemble_members (ensemble_id, user_id) SELECT $1, u FROM unnest($2::uuid[]) AS u
                 ON CONFLICT DO NOTHING",
        ensemble,
        ids
    )
    .execute(db)
    .await
}

pub async fn bulk_insert_ensemble_member_parts(
    db: impl sqlx::PgExecutor<'_>,
    ensemble: Uuid,
    ids: &[Uuid],
    instrument: Uuid,
    voice: Option<i16>,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "INSERT INTO ensemble_member_parts (ensemble_id, user_id, instrument_id, voice)
                     SELECT $1, u, $3, $4 FROM unnest($2::uuid[]) AS u ON CONFLICT DO NOTHING",
        ensemble,
        ids,
        instrument,
        voice
    )
    .execute(db)
    .await
}

pub async fn bulk_delete_ensemble_members(
    db: impl sqlx::PgExecutor<'_>,
    p1: Uuid,
    ids: &[Uuid],
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("DELETE FROM ensemble_members WHERE ensemble_id = $1 AND user_id = ANY($2)", p1, ids).execute(db).await
}
