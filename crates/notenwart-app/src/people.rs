// SPDX-License-Identifier: AGPL-3.0-or-later

//! The organisation's users: listing for those who assign people, management and invitations for admins.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::App;
use crate::auth::{CurrentUser, Role, check_name, create_link_token, normalize_email};
use crate::error::{AppError, AppResult, unique_violation};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserRow {
    id: Uuid,
    name: String,
    email: Option<String>,
    role: Role,
    has_password: bool,
    #[serde(with = "time::serde::rfc3339::option")]
    last_login_at: Option<OffsetDateTime>,
    avatar_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct User {
    #[serde(flatten)]
    row: UserRow,
    /// Only for admins.
    ensembles: Option<Vec<EnsembleRef>>,
}

#[derive(Debug, Serialize, Clone)]
pub struct EnsembleRef {
    id: Uuid,
    name: String,
}

#[derive(Debug, Deserialize)]
pub struct UserInput {
    name: String,
    email: String,
    role: Role,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Invitation {
    user: User,
    url: String,
    #[serde(with = "time::serde::rfc3339")]
    expires_at: OffsetDateTime,
    emailed: bool,
}

async fn find(state: &App, organization_id: Uuid, user_id: Uuid) -> AppResult<User> {
    let row = sqlx::query_as!(
        UserRow,
        r#"SELECT u.id, u.display_name AS name, u.email AS "email?", m.role AS "role: Role",
                  u.password_hash IS NOT NULL AS "has_password!", u.last_login_at,
                  CASE WHEN o.profile_pictures THEN '/api/images/' || u.avatar_id END AS avatar_url
           FROM users u JOIN memberships m ON m.user_id = u.id JOIN organizations o ON o.id = m.organization_id
           WHERE u.id = $1 AND m.organization_id = $2"#,
        user_id,
        organization_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(User { row, ensembles: None })
}

pub async fn list(state: &App, user: &CurrentUser) -> AppResult<Vec<User>> {
    user.require(user.reads_library())?;
    let mut rows = sqlx::query_as!(
        UserRow,
        r#"SELECT u.id, u.display_name AS name, u.email AS "email?", m.role AS "role: Role",
                  u.password_hash IS NOT NULL AS "has_password!", u.last_login_at,
                  CASE WHEN o.profile_pictures THEN '/api/images/' || u.avatar_id END AS avatar_url
           FROM users u JOIN memberships m ON m.user_id = u.id JOIN organizations o ON o.id = m.organization_id
           WHERE m.organization_id = $1
           ORDER BY lower(u.display_name), u.id"#,
        user.organization_id
    )
    .fetch_all(&state.pool)
    .await?;
    if !user.is_admin() {
        for other in &mut rows {
            other.email = None;
            other.last_login_at = None;
        }
        return Ok(rows.into_iter().map(|row| User { row, ensembles: None }).collect());
    }
    let mut ensembles: HashMap<Uuid, Vec<EnsembleRef>> = HashMap::new();
    for r in sqlx::query!(
        "SELECT m.user_id, e.id, e.name FROM ensemble_members m JOIN ensembles e ON e.id = m.ensemble_id
         WHERE e.organization_id = $1 ORDER BY lower(e.name)",
        user.organization_id
    )
    .fetch_all(&state.pool)
    .await?
    {
        ensembles.entry(r.user_id).or_default().push(EnsembleRef { id: r.id, name: r.name });
    }
    Ok(rows
        .into_iter()
        .map(|row| User { ensembles: Some(ensembles.remove(&row.id).unwrap_or_default()), row })
        .collect())
}

async fn invitation(state: &App, user: User) -> AppResult<Invitation> {
    let (url, expires_at) = create_link_token(state, user.row.id, true).await?;
    let email = user.row.email.clone().unwrap_or_default();
    let mail = crate::mail::invitation(state.config.language, &user.row.name, &url, crate::auth::INVITATION_DAYS);
    let emailed = crate::mail::send_or_log(state, &email, &mail.subject, &mail.body).await;
    Ok(Invitation { user, url, expires_at, emailed })
}

pub async fn create(state: &App, admin: &CurrentUser, input: UserInput) -> AppResult<Invitation> {
    admin.require(admin.is_admin())?;
    let name = check_name(&input.name)?;
    let email = normalize_email(&input.email)?;
    let mut tx = state.pool.begin().await?;
    let user_id =
        sqlx::query_scalar!("INSERT INTO users (email, display_name) VALUES ($1, $2) RETURNING id", email, name)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| unique_violation(e, "a user with this email exists already"))?;
    sqlx::query!(
        "INSERT INTO memberships (organization_id, user_id, role) VALUES ($1, $2, $3)",
        admin.organization_id,
        user_id,
        input.role as Role
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    let user = find(state, admin.organization_id, user_id).await?;
    invitation(state, user).await
}

pub async fn update(state: &App, admin: &CurrentUser, user_id: Uuid, input: UserInput) -> AppResult<User> {
    admin.require(admin.is_admin())?;
    find(state, admin.organization_id, user_id).await?;
    if user_id == admin.id && input.role != Role::Admin {
        return Err(AppError::Invalid("you cannot take away your own admin role".into()));
    }
    let name = check_name(&input.name)?;
    let email = normalize_email(&input.email)?;
    let mut tx = state.pool.begin().await?;
    sqlx::query!("UPDATE users SET display_name = $2, email = $3 WHERE id = $1", user_id, name, email)
        .execute(&mut *tx)
        .await
        .map_err(|e| unique_violation(e, "a user with this email exists already"))?;
    sqlx::query!(
        "UPDATE memberships SET role = $3 WHERE user_id = $1 AND organization_id = $2",
        user_id,
        admin.organization_id,
        input.role as Role
    )
    .execute(&mut *tx)
    .await?;
    // Leading an ensemble needs the role leader or higher.
    if input.role == Role::Member {
        sqlx::query!("UPDATE ensemble_members SET leader = false WHERE user_id = $1", user_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    find(state, admin.organization_id, user_id).await
}

pub async fn remove(state: &App, admin: &CurrentUser, user_id: Uuid) -> AppResult<()> {
    admin.require(admin.is_admin())?;
    if user_id == admin.id {
        return Err(AppError::Invalid("you cannot delete yourself".into()));
    }
    find(state, admin.organization_id, user_id).await?;
    if !crate::privacy::with_open_loans(state, admin.organization_id, &[user_id]).await?.is_empty() {
        return Err(AppError::Conflict("this person has borrowed parts that are not returned".into()));
    }
    // Users belong to one organisation for now, so the user goes with the membership.
    let mut tx = state.pool.begin().await?;
    crate::privacy::delete_users(&mut tx, admin.organization_id, admin.id, &[user_id]).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn invite(state: &App, admin: &CurrentUser, user_id: Uuid) -> AppResult<Invitation> {
    admin.require(admin.is_admin())?;
    let user = find(state, admin.organization_id, user_id).await?;
    invitation(state, user).await
}

// ---- bulk actions ----

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum BulkAction {
    SetRole,
    AddToEnsemble,
    RemoveFromEnsemble,
    Invite,
    Delete,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkInput {
    user_ids: Vec<Uuid>,
    action: BulkAction,
    role: Option<Role>,
    ensemble_id: Option<Uuid>,
    instrument_id: Option<Uuid>,
    voice: Option<i16>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Skipped {
    user_id: Uuid,
    reason: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkInvitation {
    user_id: Uuid,
    name: String,
    email: String,
    url: String,
}

#[derive(Debug, Serialize)]
pub struct BulkResult {
    done: usize,
    skipped: Vec<Skipped>,
    invitations: Vec<BulkInvitation>,
}

pub async fn bulk(state: &App, admin: &CurrentUser, input: BulkInput) -> AppResult<BulkResult> {
    admin.require(admin.is_admin())?;
    if input.user_ids.is_empty() || input.user_ids.len() > 2000 {
        return Err(AppError::Invalid("select 1 to 2000 people".into()));
    }
    let org = admin.organization_id;
    let known: HashSet<Uuid> = sqlx::query_scalar!(
        "SELECT user_id FROM memberships WHERE organization_id = $1 AND user_id = ANY($2)",
        org,
        &input.user_ids
    )
    .fetch_all(&state.pool)
    .await?
    .into_iter()
    .collect();
    // Nobody is deleted while they still have printed parts of the archive.
    let borrowing: HashSet<Uuid> = if input.action == BulkAction::Delete {
        crate::privacy::with_open_loans(state, org, &input.user_ids).await?.into_iter().collect()
    } else {
        HashSet::new()
    };
    let mut skipped = Vec::new();
    let mut ids = Vec::new();
    for id in input.user_ids.iter().copied().collect::<HashSet<_>>() {
        if !known.contains(&id) {
            skipped.push(Skipped { user_id: id, reason: "not a person of this organisation" });
        } else if id == admin.id && matches!(input.action, BulkAction::SetRole | BulkAction::Delete) {
            skipped.push(Skipped { user_id: id, reason: "not yourself" });
        } else if borrowing.contains(&id) {
            skipped.push(Skipped { user_id: id, reason: "has borrowed parts" });
        } else {
            ids.push(id);
        }
    }

    let ensemble = match input.action {
        BulkAction::AddToEnsemble | BulkAction::RemoveFromEnsemble => {
            let id = input.ensemble_id.ok_or_else(|| AppError::Invalid("choose an ensemble".into()))?;
            let exists = sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT 1 FROM ensembles WHERE id = $1 AND organization_id = $2) AS "x!""#,
                id,
                org
            )
            .fetch_one(&state.pool)
            .await?;
            if !exists {
                return Err(AppError::NotFound);
            }
            Some(id)
        }
        _ => None,
    };
    if let Some(instrument) = input.instrument_id {
        let known = sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM instruments
                              WHERE id = $1 AND organization_id = $2) AS "x!""#,
            instrument,
            org
        )
        .fetch_one(&state.pool)
        .await?;
        if !known {
            return Err(AppError::Invalid("unknown instrument".into()));
        }
    }
    if input.voice.is_some_and(|v| v < 1) {
        return Err(AppError::Invalid("part numbers start at 1".into()));
    }

    let mut invitations = Vec::new();
    let mut tx = state.pool.begin().await?;
    match input.action {
        BulkAction::SetRole => {
            let role = input.role.ok_or_else(|| AppError::Invalid("choose a role".into()))?;
            sqlx::query!(
                "UPDATE memberships SET role = $3 WHERE organization_id = $1 AND user_id = ANY($2)",
                org,
                &ids,
                role as Role
            )
            .execute(&mut *tx)
            .await?;
            if role == Role::Member {
                sqlx::query!("UPDATE ensemble_members SET leader = false WHERE user_id = ANY($1)", &ids)
                    .execute(&mut *tx)
                    .await?;
            }
        }
        BulkAction::AddToEnsemble => {
            let ensemble = ensemble.expect("checked above");
            sqlx::query!(
                "INSERT INTO ensemble_members (ensemble_id, user_id) SELECT $1, u FROM unnest($2::uuid[]) AS u
                 ON CONFLICT DO NOTHING",
                ensemble,
                &ids
            )
            .execute(&mut *tx)
            .await?;
            if let Some(instrument) = input.instrument_id {
                sqlx::query!(
                    "INSERT INTO ensemble_member_parts (ensemble_id, user_id, instrument_id, voice)
                     SELECT $1, u, $3, $4 FROM unnest($2::uuid[]) AS u ON CONFLICT DO NOTHING",
                    ensemble,
                    &ids,
                    instrument,
                    input.voice
                )
                .execute(&mut *tx)
                .await?;
            }
        }
        BulkAction::RemoveFromEnsemble => {
            sqlx::query!(
                "DELETE FROM ensemble_members WHERE ensemble_id = $1 AND user_id = ANY($2)",
                ensemble.expect("checked above"),
                &ids
            )
            .execute(&mut *tx)
            .await?;
        }
        BulkAction::Delete => {
            crate::privacy::delete_users(&mut tx, org, admin.id, &ids).await?;
        }
        BulkAction::Invite => {}
    }
    tx.commit().await?;

    if input.action == BulkAction::Invite {
        for id in &ids {
            let user = find(state, org, *id).await?;
            let invitation = invitation(state, user).await?;
            invitations.push(BulkInvitation {
                user_id: invitation.user.row.id,
                name: invitation.user.row.name,
                email: invitation.user.row.email.unwrap_or_default(),
                url: invitation.url,
            });
        }
    }
    Ok(BulkResult { done: ids.len(), skipped, invitations })
}
