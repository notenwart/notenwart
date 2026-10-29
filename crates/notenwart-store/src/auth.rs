// SPDX-License-Identifier: AGPL-3.0-or-later

//! The data of `notenwart_app::auth`: its queries, one function each, checked at compile time against
//! `.sqlx/`.

use notenwart_core::Role;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug)]
pub struct UserForSessionRow {
    pub id: Uuid,
    pub display_name: String,
    pub email: String,
    pub role: Role,
    pub has_password: bool,
    pub organization_id: Uuid,
    pub organization_name: String,
    pub avatar_id: Option<Uuid>,
    pub logo_id: Option<Uuid>,
    pub profile_pictures: bool,
    pub stale: bool,
}

#[derive(Debug)]
pub struct StatusRow {
    pub name: String,
    pub logo_id: Option<Uuid>,
}

#[derive(Debug)]
pub struct LoginRow {
    pub id: Uuid,
    pub password_hash: Option<String>,
}

#[derive(Debug)]
pub struct MagicLinkRow {
    pub id: Uuid,
    pub display_name: String,
}

pub async fn user_for_session_select_sessions(
    db: impl sqlx::PgExecutor<'_>,
    token_hash: &[u8],
) -> sqlx::Result<Option<UserForSessionRow>> {
    sqlx::query_as!(
        UserForSessionRow,
        r#"SELECT u.id, u.display_name, u.email, m.role AS "role: Role", u.password_hash IS NOT NULL AS "has_password!",
                  o.id AS organization_id, o.name AS organization_name, u.avatar_id, o.logo_id, o.profile_pictures,
                  s.last_seen_at < now() - interval '1 hour' AS "stale!"
           FROM sessions s
           JOIN users u ON u.id = s.user_id
           JOIN memberships m ON m.user_id = s.user_id AND m.organization_id = s.organization_id
           JOIN organizations o ON o.id = s.organization_id
           WHERE s.token_hash = $1 AND s.expires_at > now()"#,
        token_hash
    )
    .fetch_optional(db)
    .await
}

pub async fn user_for_session_update_sessions(
    db: impl sqlx::PgExecutor<'_>,
    token_hash: &[u8],
    p2: i32,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "UPDATE sessions SET last_seen_at = now(), expires_at = now() + make_interval(days => $2)
             WHERE token_hash = $1",
        token_hash,
        p2
    )
    .execute(db)
    .await
}

pub async fn start_session_select_memberships(
    db: impl sqlx::PgExecutor<'_>,
    user_id: Uuid,
) -> sqlx::Result<Option<Uuid>> {
    sqlx::query_scalar!(
        "SELECT organization_id FROM memberships WHERE user_id = $1 ORDER BY created_at LIMIT 1",
        user_id
    )
    .fetch_optional(db)
    .await
}

pub async fn start_session_insert_sessions(
    db: impl sqlx::PgExecutor<'_>,
    hash: &[u8],
    user_id: Uuid,
    organization_id: Uuid,
    p4: i32,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "INSERT INTO sessions (token_hash, user_id, organization_id, expires_at)
         VALUES ($1, $2, $3, now() + make_interval(days => $4))",
        hash,
        user_id,
        organization_id,
        p4
    )
    .execute(db)
    .await
}

pub async fn start_session_update_users(
    db: impl sqlx::PgExecutor<'_>,
    user_id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("UPDATE users SET last_login_at = now() WHERE id = $1", user_id).execute(db).await
}

pub async fn create_link_token_delete_login_tokens(
    db: impl sqlx::PgExecutor<'_>,
    user_id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("DELETE FROM login_tokens WHERE user_id = $1 AND purpose = 'invitation' AND used_at IS NULL", user_id)
        .execute(db)
        .await
}

pub async fn create_link_token_insert_login_tokens(
    db: impl sqlx::PgExecutor<'_>,
    hash: &[u8],
    user_id: Uuid,
    purpose: &str,
    minutes: i32,
) -> sqlx::Result<OffsetDateTime> {
    sqlx::query_scalar!(
        "INSERT INTO login_tokens (token_hash, user_id, purpose, expires_at)
         VALUES ($1, $2, $3::text::token_purpose, now() + make_interval(mins => $4)) RETURNING expires_at",
        hash,
        user_id,
        purpose,
        minutes
    )
    .fetch_one(db)
    .await
}

pub async fn status_select_organizations(db: impl sqlx::PgExecutor<'_>) -> sqlx::Result<Option<StatusRow>> {
    sqlx::query_as!(StatusRow, "SELECT name, logo_id FROM organizations ORDER BY created_at LIMIT 1")
        .fetch_optional(db)
        .await
}

pub async fn status_select_users(db: impl sqlx::PgExecutor<'_>) -> sqlx::Result<bool> {
    sqlx::query_scalar!(r#"SELECT EXISTS (SELECT 1 FROM users) AS "any!""#).fetch_one(db).await
}

pub async fn setup_lock_users(db: impl sqlx::PgExecutor<'_>) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("LOCK TABLE users IN EXCLUSIVE MODE").execute(db).await
}

pub async fn setup_select_users(db: impl sqlx::PgExecutor<'_>) -> sqlx::Result<bool> {
    sqlx::query_scalar!(r#"SELECT EXISTS (SELECT 1 FROM users) AS "any!""#).fetch_one(db).await
}

pub async fn setup_select_organizations(db: impl sqlx::PgExecutor<'_>) -> sqlx::Result<Uuid> {
    sqlx::query_scalar!("SELECT id FROM organizations ORDER BY created_at LIMIT 1").fetch_one(db).await
}

pub async fn setup_update_organizations(
    db: impl sqlx::PgExecutor<'_>,
    organization_id: Uuid,
    organization_name: &str,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("UPDATE organizations SET name = $2 WHERE id = $1", organization_id, organization_name)
        .execute(db)
        .await
}

pub async fn setup_insert_users(
    db: impl sqlx::PgExecutor<'_>,
    email: &str,
    name: &str,
    password_hash: &str,
) -> sqlx::Result<Uuid> {
    sqlx::query_scalar!(
        "INSERT INTO users (email, display_name, password_hash) VALUES ($1, $2, $3) RETURNING id",
        email,
        name,
        password_hash
    )
    .fetch_one(db)
    .await
}

pub async fn setup_insert_memberships(
    db: impl sqlx::PgExecutor<'_>,
    organization_id: Uuid,
    user_id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "INSERT INTO memberships (organization_id, user_id, role) VALUES ($1, $2, 'admin')",
        organization_id,
        user_id
    )
    .execute(db)
    .await
}

pub async fn login(db: impl sqlx::PgExecutor<'_>, email: &str) -> sqlx::Result<Option<LoginRow>> {
    sqlx::query_as!(LoginRow, "SELECT id, password_hash FROM users WHERE lower(email) = $1", email)
        .fetch_optional(db)
        .await
}

pub async fn magic_link(db: impl sqlx::PgExecutor<'_>, email: &str) -> sqlx::Result<Option<MagicLinkRow>> {
    sqlx::query_as!(MagicLinkRow, "SELECT id, display_name FROM users WHERE lower(email) = $1", email)
        .fetch_optional(db)
        .await
}

pub async fn redeem(db: impl sqlx::PgExecutor<'_>, p1: &[u8]) -> sqlx::Result<Option<Uuid>> {
    sqlx::query_scalar!(
        "UPDATE login_tokens SET used_at = now()
         WHERE token_hash = $1 AND used_at IS NULL AND expires_at > now() RETURNING user_id",
        p1
    )
    .fetch_optional(db)
    .await
}

pub async fn logout(db: impl sqlx::PgExecutor<'_>, p1: &[u8]) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("DELETE FROM sessions WHERE token_hash = $1", p1).execute(db).await
}

pub async fn update_me_select_users(db: impl sqlx::PgExecutor<'_>, id: Uuid) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar!("SELECT password_hash FROM users WHERE id = $1", id).fetch_one(db).await
}

pub async fn update_me_update_users(
    db: impl sqlx::PgExecutor<'_>,
    id: Uuid,
    hash: &str,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("UPDATE users SET password_hash = $2 WHERE id = $1", id, hash).execute(db).await
}

pub async fn update_me_update_users_2(
    db: impl sqlx::PgExecutor<'_>,
    id: Uuid,
    name: &str,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("UPDATE users SET display_name = $2 WHERE id = $1", id, name).execute(db).await
}

pub async fn update_me_select_users_2(db: impl sqlx::PgExecutor<'_>, id: Uuid) -> sqlx::Result<bool> {
    sqlx::query_scalar!(r#"SELECT password_hash IS NOT NULL AS "has!" FROM users WHERE id = $1"#, id)
        .fetch_one(db)
        .await
}
