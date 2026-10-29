// SPDX-License-Identifier: AGPL-3.0-or-later

//! The organisation's users: listing for those who assign people, management and invitations for admins.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::App;
use crate::auth::{CurrentUser, Role, check_name, create_link_token, normalize_email};
use crate::error::{AppError, AppResult, unique_violation};
use notenwart_store::people as db;

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserRow {
    pub id: Uuid,
    pub name: String,
    pub email: Option<String>,
    pub role: Role,
    pub has_password: bool,
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_login_at: Option<OffsetDateTime>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct User {
    #[serde(flatten)]
    #[schema(inline)]
    pub row: UserRow,
    /// Only for admins.
    pub ensembles: Option<Vec<EnsembleRef>>,
}

#[derive(Debug, Serialize, Clone, utoipa::ToSchema)]
pub struct EnsembleRef {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UserInput {
    pub name: String,
    pub email: String,
    pub role: Role,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Invitation {
    pub user: User,
    /// One-time sign-in link to hand over.
    pub url: String,
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
    pub emailed: bool,
}

async fn find(state: &App, organization_id: Uuid, user_id: Uuid) -> AppResult<User> {
    let row = db::find(&state.pool, user_id, organization_id)
        .await
        .map(|row| row.map(UserRow::from))?
        .ok_or(AppError::NotFound)?;
    Ok(User { row, ensembles: None })
}

pub async fn list(state: &App, user: &CurrentUser) -> AppResult<Vec<User>> {
    user.require(user.reads_library())?;
    let mut rows = db::list_select_users(&state.pool, user.organization_id)
        .await
        .map(|rows| rows.into_iter().map(UserRow::from).collect::<Vec<_>>())?;
    if !user.is_admin() {
        for other in &mut rows {
            other.email = None;
            other.last_login_at = None;
        }
        return Ok(rows.into_iter().map(|row| User { row, ensembles: None }).collect());
    }
    let mut ensembles: HashMap<Uuid, Vec<EnsembleRef>> = HashMap::new();
    for r in db::list_select_ensemble_members(&state.pool, user.organization_id).await? {
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
    let user_id = db::create_insert_users(&mut *tx, &email, &name)
        .await
        .map_err(|e| unique_violation(e, "a user with this email exists already"))?;
    db::create_insert_memberships(&mut *tx, admin.organization_id, user_id, input.role as Role).await?;
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
    db::update_update_users(&mut *tx, user_id, &name, &email)
        .await
        .map_err(|e| unique_violation(e, "a user with this email exists already"))?;
    db::update_update_memberships(&mut *tx, user_id, admin.organization_id, input.role as Role).await?;
    // Leading an ensemble needs the role leader or higher.
    if input.role == Role::Member {
        db::update_update_ensemble_members(&mut *tx, user_id).await?;
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
    crate::privacy::delete_users(&mut tx, admin.organization_id, admin.actor(), &[user_id]).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn invite(state: &App, admin: &CurrentUser, user_id: Uuid) -> AppResult<Invitation> {
    admin.require(admin.is_admin())?;
    let user = find(state, admin.organization_id, user_id).await?;
    invitation(state, user).await
}

// ---- bulk actions ----

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum BulkAction {
    SetRole,
    AddToEnsemble,
    RemoveFromEnsemble,
    Invite,
    Delete,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BulkInput {
    user_ids: Vec<Uuid>,
    #[schema(inline)]
    action: BulkAction,
    role: Option<Role>,
    /// For addToEnsemble and removeFromEnsemble.
    ensemble_id: Option<Uuid>,
    /// For addToEnsemble – also play this instrument there.
    instrument_id: Option<Uuid>,
    voice: Option<i16>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Skipped {
    user_id: Uuid,
    reason: &'static str,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BulkInvitation {
    user_id: Uuid,
    name: String,
    email: String,
    url: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct BulkResult {
    /// People the action was applied to.
    done: usize,
    #[schema(inline)]
    skipped: Vec<Skipped>,
    /// New invitation links (action invite).
    #[schema(inline)]
    invitations: Vec<BulkInvitation>,
}

pub async fn bulk(state: &App, admin: &CurrentUser, input: BulkInput) -> AppResult<BulkResult> {
    admin.require(admin.is_admin())?;
    if input.user_ids.is_empty() || input.user_ids.len() > 2000 {
        return Err(AppError::Invalid("select 1 to 2000 people".into()));
    }
    let org = admin.organization_id;
    let known: HashSet<Uuid> =
        db::bulk_select_memberships(&state.pool, org, &input.user_ids).await?.into_iter().collect();
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
            let exists = db::bulk_select_ensembles(&state.pool, id, org).await?;
            if !exists {
                return Err(AppError::NotFound);
            }
            Some(id)
        }
        _ => None,
    };
    if let Some(instrument) = input.instrument_id {
        let known = db::bulk_select_instruments(&state.pool, instrument, org).await?;
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
            db::bulk_update_memberships(&mut *tx, org, &ids, role as Role).await?;
            if role == Role::Member {
                db::bulk_update_ensemble_members(&mut *tx, &ids).await?;
            }
        }
        BulkAction::AddToEnsemble => {
            let ensemble = ensemble.expect("checked above");
            db::bulk_insert_ensemble_members(&mut *tx, ensemble, &ids).await?;
            if let Some(instrument) = input.instrument_id {
                db::bulk_insert_ensemble_member_parts(&mut *tx, ensemble, &ids, instrument, input.voice).await?;
            }
        }
        BulkAction::RemoveFromEnsemble => {
            db::bulk_delete_ensemble_members(&mut *tx, ensemble.expect("checked above"), &ids).await?;
        }
        BulkAction::Delete => {
            crate::privacy::delete_users(&mut tx, org, admin.actor(), &ids).await?;
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

impl From<db::UserrowRow> for UserRow {
    fn from(row: db::UserrowRow) -> Self {
        UserRow {
            id: row.id,
            name: row.name,
            email: row.email,
            role: row.role,
            has_password: row.has_password,
            last_login_at: row.last_login_at,
            avatar_url: row.avatar_url,
        }
    }
}
