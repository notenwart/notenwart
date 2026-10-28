// SPDX-License-Identifier: AGPL-3.0-or-later

//! Data protection (`privacy` in api/openapi.yaml): everything stored about a person as one download (GDPR art. 15
//! and 20), and deleting people with a record that it happened but without their data.

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use serde_json::{Value, json};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::app::AppState;
use crate::auth::CurrentUser;
use crate::error::{ApiError, ApiResult};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/me/export", get(export_mine))
        .route("/users/{user_id}/export", get(export_user))
        .route("/deletions", get(deletions))
}

/// Everything stored about one person of the organisation.
async fn export(state: &AppState, organization_id: Uuid, user_id: Uuid) -> ApiResult<Value> {
    let account = sqlx::query!(
        r#"SELECT u.display_name, u.email, u.created_at, u.last_login_at, u.password_hash IS NOT NULL AS "has_password!",
                  m.role::text AS "role!", o.name AS organization
           FROM users u JOIN memberships m ON m.user_id = u.id JOIN organizations o ON o.id = m.organization_id
           WHERE u.id = $1 AND m.organization_id = $2"#,
        user_id,
        organization_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::NotFound)?;
    let ensembles = sqlx::query!(
        r#"SELECT e.name, m.leader, m.created_at,
                  coalesce(array_agg(i.name || coalesce(' ' || p.voice, '')) FILTER (WHERE i.id IS NOT NULL), '{}')
                      AS "plays!: Vec<String>"
           FROM ensemble_members m JOIN ensembles e ON e.id = m.ensemble_id
           LEFT JOIN ensemble_member_parts p ON p.ensemble_id = m.ensemble_id AND p.user_id = m.user_id
           LEFT JOIN instruments i ON i.id = p.instrument_id
           WHERE m.user_id = $1 AND e.organization_id = $2
           GROUP BY e.name, m.leader, m.created_at ORDER BY e.name"#,
        user_id,
        organization_id
    )
    .fetch_all(&state.pool)
    .await?;
    let grants = sqlx::query!(
        "SELECT w.title, p.label, g.created_at FROM part_grants g
         JOIN parts p ON p.id = g.part_id JOIN works w ON w.id = p.work_id
         WHERE g.user_id = $1 AND w.organization_id = $2 ORDER BY w.title",
        user_id,
        organization_id
    )
    .fetch_all(&state.pool)
    .await?;
    let annotations = sqlx::query!(
        "SELECT w.title, p.label, a.pages, a.updated_at FROM annotations a
         JOIN parts p ON p.id = a.part_id JOIN works w ON w.id = p.work_id
         WHERE a.user_id = $1 AND w.organization_id = $2 ORDER BY w.title",
        user_id,
        organization_id
    )
    .fetch_all(&state.pool)
    .await?;
    let loans = sqlx::query!(
        "SELECT w.title, l.item, l.quantity, to_char(l.lent_on, 'YYYY-MM-DD') AS lent_on,
                to_char(l.due_on, 'YYYY-MM-DD') AS due_on, to_char(l.returned_on, 'YYYY-MM-DD') AS returned_on, l.note
         FROM loans l JOIN works w ON w.id = l.work_id
         WHERE l.borrower_id = $1 AND l.organization_id = $2 ORDER BY l.lent_on",
        user_id,
        organization_id
    )
    .fetch_all(&state.pool)
    .await?;
    let sessions = sqlx::query!(
        "SELECT created_at, expires_at FROM sessions WHERE user_id = $1 ORDER BY created_at",
        user_id
    )
    .fetch_all(&state.pool)
    .await?;
    let time = |t: OffsetDateTime| {
        t.format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default()
    };
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

async fn export_mine(State(state): State<AppState>, user: CurrentUser) -> ApiResult<Json<Value>> {
    Ok(Json(export(&state, user.organization_id, user.id).await?))
}

async fn export_user(
    State(state): State<AppState>,
    admin: CurrentUser,
    Path(user_id): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    admin.require(admin.is_admin())?;
    Ok(Json(export(&state, admin.organization_id, user_id).await?))
}

/// Of these people, those who still have printed parts that are not returned.
pub async fn with_open_loans(state: &AppState, organization_id: Uuid, ids: &[Uuid]) -> ApiResult<Vec<Uuid>> {
    Ok(sqlx::query_scalar!(
        r#"SELECT DISTINCT borrower_id AS "id!" FROM loans
           WHERE organization_id = $1 AND borrower_id = ANY($2) AND returned_on IS NULL"#,
        organization_id,
        ids
    )
    .fetch_all(&state.pool)
    .await?)
}

/// Deletes people for good: their account with memberships, sessions, annotations and single parts; their name on
/// returned loans is erased. A log entry per person keeps only the account's id, who deleted it and what went.
pub async fn delete_users(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    organization_id: Uuid,
    deleted_by: Uuid,
    ids: &[Uuid],
) -> ApiResult<()> {
    for &id in ids {
        let records = sqlx::query!(
            r#"SELECT (SELECT count(*) FROM ensemble_members WHERE user_id = $1) AS "ensembles!",
                      (SELECT count(*) FROM annotations WHERE user_id = $1) AS "annotations!",
                      (SELECT count(*) FROM part_grants WHERE user_id = $1) AS "single_parts!",
                      (SELECT count(*) FROM loans WHERE borrower_id = $1) AS "loans!",
                      (SELECT count(*) FROM sessions WHERE user_id = $1) AS "sessions!""#,
            id
        )
        .fetch_one(&mut **tx)
        .await?;
        sqlx::query!(
            "INSERT INTO deletion_log (organization_id, user_id, deleted_by, records) VALUES ($1, $2, $3, $4)",
            organization_id,
            id,
            deleted_by,
            json!({
                "ensembles": records.ensembles,
                "annotations": records.annotations,
                "singleParts": records.single_parts,
                "loans": records.loans,
                "sessions": records.sessions,
            })
        )
        .execute(&mut **tx)
        .await?;
    }
    sqlx::query!(
        "UPDATE loans SET borrower_name = '–', note = NULL WHERE borrower_id = ANY($1)",
        ids
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query!("DELETE FROM users WHERE id = ANY($1)", ids)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Deletion {
    user_id: Uuid,
    deleted_by: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    deleted_at: OffsetDateTime,
    records: Value,
}

/// The record of deleted people (admin).
async fn deletions(State(state): State<AppState>, admin: CurrentUser) -> ApiResult<Json<Vec<Deletion>>> {
    admin.require(admin.is_admin())?;
    Ok(Json(
        sqlx::query_as!(
            Deletion,
            r#"SELECT d.user_id, u.display_name AS "deleted_by?", d.deleted_at, d.records
               FROM deletion_log d LEFT JOIN users u ON u.id = d.deleted_by
               WHERE d.organization_id = $1 ORDER BY d.deleted_at DESC LIMIT 1000"#,
            admin.organization_id
        )
        .fetch_all(&state.pool)
        .await?,
    ))
}
