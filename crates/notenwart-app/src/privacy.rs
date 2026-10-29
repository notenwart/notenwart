// SPDX-License-Identifier: AGPL-3.0-or-later

//! Data protection (`privacy` in api/openapi.json): everything stored about a person as one download (GDPR art. 15
//! and 20), and deleting people with a record that it happened but without their data.

use serde::Serialize;
use serde_json::{Value, json};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::App;
use crate::auth::CurrentUser;
use crate::error::{AppError, AppResult};
use notenwart_store::privacy as db;

/// Everything stored about one person of the organisation.
async fn export(state: &App, organization_id: Uuid, user_id: Uuid) -> AppResult<Value> {
    let account = db::export_select_users(&state.pool, user_id, organization_id).await?.ok_or(AppError::NotFound)?;
    let ensembles = db::export_select_ensemble_members(&state.pool, user_id, organization_id).await?;
    let grants = db::export_select_part_grants(&state.pool, user_id, organization_id).await?;
    let annotations = db::export_select_annotations(&state.pool, user_id, organization_id).await?;
    let loans = db::export_select_loans(&state.pool, user_id, organization_id).await?;
    let sessions = db::export_select_sessions(&state.pool, user_id).await?;
    let time = |t: OffsetDateTime| t.format(&time::format_description::well_known::Rfc3339).unwrap_or_default();
    Ok(json!({
        "exportedAt": time(OffsetDateTime::now_utc()),
        "notice": "Everything notenwart stores about this person. Passwords are stored only as a hash and are not part of the export.",
        "account": {
            "name": account.display_name,
            "email": account.email,
            "role": account.role,
            "organization": account.organization,
            "createdAt": time(account.created_at),
            "lastSignInAt": account.last_login_at.map(time),
            "hasPassword": account.has_password,
        },
        "ensembles": ensembles.iter().map(|e| json!({
            "name": e.name, "leader": e.leader, "since": time(e.created_at), "plays": e.plays,
        })).collect::<Vec<_>>(),
        "singleParts": grants.iter().map(|g| json!({
            "work": g.title, "part": g.label, "since": time(g.created_at),
        })).collect::<Vec<_>>(),
        "annotations": annotations.iter().map(|a| json!({
            "work": a.title, "part": a.label, "pages": a.pages, "updatedAt": time(a.updated_at),
        })).collect::<Vec<_>>(),
        "loans": loans.iter().map(|l| json!({
            "work": l.title, "item": l.item, "quantity": l.quantity, "lentOn": l.lent_on,
            "dueOn": l.due_on, "returnedOn": l.returned_on, "note": l.note,
        })).collect::<Vec<_>>(),
        "signIns": sessions.iter().map(|s| json!({
            "since": time(s.created_at), "expires": time(s.expires_at),
        })).collect::<Vec<_>>(),
    }))
}

pub async fn export_mine(state: &App, user: &CurrentUser) -> AppResult<Value> {
    export(state, user.organization_id, user.id).await
}

pub async fn export_user(state: &App, admin: &CurrentUser, user_id: Uuid) -> AppResult<Value> {
    admin.require(admin.is_admin())?;
    export(state, admin.organization_id, user_id).await
}

/// Of these people, those who still have printed parts that are not returned.
pub async fn with_open_loans(state: &App, organization_id: Uuid, ids: &[Uuid]) -> AppResult<Vec<Uuid>> {
    Ok(db::with_open_loans(&state.pool, organization_id, ids).await?)
}

/// Deletes people for good: their account with memberships, sessions, annotations and single parts; their name on
/// returned loans is erased. A log entry per person keeps only the account's id, who deleted it and what went.
pub async fn delete_users(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    organization_id: Uuid,
    deleted_by: Option<Uuid>,
    ids: &[Uuid],
) -> AppResult<()> {
    for &id in ids {
        let records = db::delete_users_select_ensemble_members(&mut **tx, id).await?;
        db::delete_users_insert_deletion_log(
            &mut **tx,
            organization_id,
            id,
            deleted_by,
            &json!({
                "ensembles": records.ensembles,
                "annotations": records.annotations,
                "singleParts": records.single_parts,
                "loans": records.loans,
                "sessions": records.sessions,
            }),
        )
        .await?;
    }
    db::delete_users_update_loans(&mut **tx, ids).await?;
    db::delete_users_delete_users(&mut **tx, ids).await?;
    Ok(())
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Deletion {
    /// Id of the deleted account (nothing else about the person is kept).
    user_id: Uuid,
    /// Name of the admin who deleted it, while that account exists.
    deleted_by: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    deleted_at: OffsetDateTime,
    /// How many records went with the account, by kind.
    #[schema(value_type = std::collections::HashMap<String, i64>)]
    records: Value,
}

/// The record of deleted people (admin).
pub async fn deletions(state: &App, admin: &CurrentUser) -> AppResult<Vec<Deletion>> {
    admin.require(admin.is_admin())?;
    Ok(db::deletions(&state.pool, admin.organization_id)
        .await
        .map(|rows| rows.into_iter().map(Deletion::from).collect::<Vec<_>>())?)
}

impl From<db::DeletionRow> for Deletion {
    fn from(row: db::DeletionRow) -> Self {
        Deletion { user_id: row.user_id, deleted_by: row.deleted_by, deleted_at: row.deleted_at, records: row.records }
    }
}
