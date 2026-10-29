// SPDX-License-Identifier: AGPL-3.0-or-later

//! The data of `notenwart_app::images`: its queries, one function each, checked at compile time against
//! `.sqlx/`.

use uuid::Uuid;

#[derive(Debug)]
pub struct ImageRow {
    pub organization_id: Uuid,
    pub owner_kind: String,
    pub owner_id: Uuid,
    pub content_type: String,
}

#[derive(Debug)]
pub struct FindOrganizationRow {
    pub id: Uuid,
    pub name: String,
    pub logo_id: Option<Uuid>,
    pub profile_pictures: bool,
}

pub async fn store(
    db: impl sqlx::PgExecutor<'_>,
    id: Uuid,
    organization_id: Uuid,
    owner_kind: &str,
    owner_id: Uuid,
    content_type: &str,
    size: i32,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("INSERT INTO images (id, organization_id, owner_kind, owner_id, content_type, size) VALUES ($1, $2, $3, $4, $5, $6)", id, organization_id, owner_kind, owner_id, content_type, size).execute(db).await
}

pub async fn discard(db: impl sqlx::PgExecutor<'_>, id: Uuid) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("DELETE FROM images WHERE id = $1", id).execute(db).await
}

pub async fn image(db: impl sqlx::PgExecutor<'_>, image_id: Uuid) -> sqlx::Result<Option<ImageRow>> {
    sqlx::query_as!(
        ImageRow,
        "SELECT organization_id, owner_kind, owner_id, content_type FROM images WHERE id = $1",
        image_id
    )
    .fetch_optional(db)
    .await
}

pub async fn leads(db: impl sqlx::PgExecutor<'_>, leader: Uuid, member: Uuid) -> sqlx::Result<bool> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM ensemble_members l JOIN ensemble_members m ON m.ensemble_id = l.ensemble_id
                          WHERE l.user_id = $1 AND l.leader AND m.user_id = $2) AS "x!""#,
        leader,
        member
    )
    .fetch_one(db)
    .await
}

pub async fn find_organization(db: impl sqlx::PgExecutor<'_>, id: Uuid) -> sqlx::Result<FindOrganizationRow> {
    sqlx::query_as!(
        FindOrganizationRow,
        "SELECT id, name, logo_id, profile_pictures FROM organizations WHERE id = $1",
        id
    )
    .fetch_one(db)
    .await
}

pub async fn update_organization(
    db: impl sqlx::PgExecutor<'_>,
    organization_id: Uuid,
    name: &str,
    profile_pictures: bool,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!(
        "UPDATE organizations SET name = $2, profile_pictures = $3 WHERE id = $1",
        organization_id,
        name,
        profile_pictures
    )
    .execute(db)
    .await
}

pub async fn organization_logo_select_organizations(
    db: impl sqlx::PgExecutor<'_>,
    organization_id: Uuid,
) -> sqlx::Result<Option<Uuid>> {
    sqlx::query_scalar!("SELECT logo_id FROM organizations WHERE id = $1", organization_id).fetch_one(db).await
}

pub async fn organization_logo_update_organizations(
    db: impl sqlx::PgExecutor<'_>,
    org: Uuid,
    id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("UPDATE organizations SET logo_id = $2 WHERE id = $1", org, id).execute(db).await
}

pub async fn remove_organization_logo_select_organizations(
    db: impl sqlx::PgExecutor<'_>,
    organization_id: Uuid,
) -> sqlx::Result<Option<Uuid>> {
    sqlx::query_scalar!("SELECT logo_id FROM organizations WHERE id = $1", organization_id).fetch_one(db).await
}

pub async fn remove_organization_logo_update_organizations(
    db: impl sqlx::PgExecutor<'_>,
    organization_id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("UPDATE organizations SET logo_id = NULL WHERE id = $1", organization_id).execute(db).await
}

pub async fn ensemble_logo_select_ensembles(
    db: impl sqlx::PgExecutor<'_>,
    ensemble_id: Uuid,
) -> sqlx::Result<Option<Uuid>> {
    sqlx::query_scalar!("SELECT logo_id FROM ensembles WHERE id = $1", ensemble_id).fetch_one(db).await
}

pub async fn ensemble_logo_update_ensembles(
    db: impl sqlx::PgExecutor<'_>,
    ensemble_id: Uuid,
    id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("UPDATE ensembles SET logo_id = $2 WHERE id = $1", ensemble_id, id).execute(db).await
}

pub async fn remove_ensemble_logo_select_ensembles(
    db: impl sqlx::PgExecutor<'_>,
    ensemble_id: Uuid,
) -> sqlx::Result<Option<Uuid>> {
    sqlx::query_scalar!("SELECT logo_id FROM ensembles WHERE id = $1", ensemble_id).fetch_one(db).await
}

pub async fn remove_ensemble_logo_update_ensembles(
    db: impl sqlx::PgExecutor<'_>,
    ensemble_id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("UPDATE ensembles SET logo_id = NULL WHERE id = $1", ensemble_id).execute(db).await
}

pub async fn ensemble_symbol(
    db: impl sqlx::PgExecutor<'_>,
    ensemble_id: Uuid,
    symbol: Option<&str>,
    color: Option<&str>,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("UPDATE ensembles SET symbol = $2, color = $3 WHERE id = $1", ensemble_id, symbol, color)
        .execute(db)
        .await
}

pub async fn my_avatar_select_users(db: impl sqlx::PgExecutor<'_>, id: Uuid) -> sqlx::Result<Option<Uuid>> {
    sqlx::query_scalar!("SELECT avatar_id FROM users WHERE id = $1", id).fetch_one(db).await
}

pub async fn my_avatar_update_users(
    db: impl sqlx::PgExecutor<'_>,
    id: Uuid,
    id_2: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("UPDATE users SET avatar_id = $2 WHERE id = $1", id, id_2).execute(db).await
}

pub async fn remove_my_avatar_select_users(db: impl sqlx::PgExecutor<'_>, id: Uuid) -> sqlx::Result<Option<Uuid>> {
    sqlx::query_scalar!("SELECT avatar_id FROM users WHERE id = $1", id).fetch_one(db).await
}

pub async fn remove_my_avatar_update_users(
    db: impl sqlx::PgExecutor<'_>,
    id: Uuid,
) -> sqlx::Result<sqlx::postgres::PgQueryResult> {
    sqlx::query!("UPDATE users SET avatar_id = NULL WHERE id = $1", id).execute(db).await
}
