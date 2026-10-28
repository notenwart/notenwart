// SPDX-License-Identifier: AGPL-3.0-or-later

//! Importing people from a table (CSV in the web app): users, their ensembles and what they play there.
//! Validates every row first; changes nothing unless all rows are fine.

use std::collections::{HashMap, HashSet};

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::app::AppState;
use crate::auth::{CurrentUser, INVITATION_DAYS, Role, check_name, create_link_token, normalize_email};
use crate::error::{ApiError, ApiResult};

const MAX_ROWS: usize = 2000;

pub fn router() -> Router<AppState> {
    Router::new().route("/users/import", post(import))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRow {
    name: String,
    email: String,
    role: Option<String>,
    ensemble: Option<String>,
    instrument: Option<String>,
    voice: Option<i16>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportInput {
    rows: Vec<ImportRow>,
    dry_run: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RowResult {
    line: usize,
    outcome: &'static str,
    message: Option<String>,
    invitation_url: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    applied: bool,
    rows: Vec<RowResult>,
    new_people: usize,
    ensembles_created: Vec<String>,
    errors: usize,
}

/// A checked row, ready to apply.
struct Planned {
    email: String,
    name: String,
    role: Role,
    /// Lower-case ensemble name.
    ensemble: Option<String>,
    instrument: Option<Uuid>,
    voice: Option<i16>,
}

/// English and German role names; empty means member.
fn parse_role(value: Option<&str>) -> Result<Role, String> {
    let value = value.map(str::trim).unwrap_or_default().to_lowercase();
    Ok(match value.as_str() {
        "" | "member" | "mitglied" => Role::Member,
        "leader" | "leitung" => Role::Leader,
        "librarian" | "notenverwaltung" => Role::Librarian,
        "admin" => Role::Admin,
        _ => return Err("unknown role".into()),
    })
}

fn blank_to_none(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

async fn import(
    State(state): State<AppState>,
    admin: CurrentUser,
    Json(input): Json<ImportInput>,
) -> ApiResult<(StatusCode, Json<ImportResult>)> {
    admin.require(admin.is_admin())?;
    if input.rows.is_empty() || input.rows.len() > MAX_ROWS {
        return Err(ApiError::Invalid(format!("an import needs 1 to {MAX_ROWS} rows")));
    }
    let org = admin.organization_id;

    // Everything rows can refer to, loaded once.
    let users: HashMap<String, (Uuid, bool)> = sqlx::query!(
        r#"SELECT u.id, lower(u.email) AS "email!",
                  EXISTS (SELECT 1 FROM memberships m WHERE m.user_id = u.id AND m.organization_id = $1) AS "member!"
           FROM users u"#,
        org
    )
    .fetch_all(&state.pool)
    .await?
    .into_iter()
    .map(|r| (r.email, (r.id, r.member)))
    .collect();
    let ensembles: HashMap<String, Uuid> = sqlx::query!(
        "SELECT id, lower(name) AS \"name!\" FROM ensembles WHERE organization_id = $1",
        org
    )
    .fetch_all(&state.pool)
    .await?
    .into_iter()
    .map(|r| (r.name, r.id))
    .collect();
    let mut instruments: HashMap<String, Uuid> = HashMap::new();
    for row in sqlx::query!(
        "SELECT id, name, aliases FROM instruments WHERE organization_id = $1 ORDER BY sort",
        org
    )
    .fetch_all(&state.pool)
    .await?
    {
        for key in std::iter::once(row.name).chain(row.aliases) {
            instruments.entry(key.trim().to_lowercase()).or_insert(row.id);
        }
    }

    // Check every row.
    let mut results = Vec::new();
    let mut planned = Vec::new();
    let mut new_emails = HashSet::new();
    let mut ensembles_created: Vec<String> = Vec::new();
    for (i, row) in input.rows.into_iter().enumerate() {
        let checked = (|| -> Result<Planned, String> {
            let name = check_name(&row.name).map_err(message)?;
            let email = normalize_email(&row.email).map_err(message)?;
            let role = parse_role(row.role.as_deref())?;
            let ensemble = blank_to_none(row.ensemble);
            let instrument = match blank_to_none(row.instrument) {
                Some(instrument) => {
                    if ensemble.is_none() {
                        return Err("an instrument needs an ensemble".into());
                    }
                    Some(
                        *instruments
                            .get(&instrument.to_lowercase())
                            .ok_or("unknown instrument")?,
                    )
                }
                None => None,
            };
            if row.voice.is_some() && instrument.is_none() {
                return Err("a part number needs an instrument".into());
            }
            if row.voice.is_some_and(|v| v < 1) {
                return Err("part numbers start at 1".into());
            }
            if let Some(name) = &ensemble
                && name.chars().count() > 120
            {
                return Err("name must have 1 to 120 characters".into());
            }
            if let Some((_, false)) = users.get(&email) {
                return Err("this email belongs to another organisation".into());
            }
            Ok(Planned {
                email,
                name,
                role,
                ensemble: ensemble.as_ref().map(|e| e.to_lowercase()),
                instrument,
                voice: row.voice,
            })
            .inspect(|_| {
                if let Some(name) = ensemble
                    && !ensembles.contains_key(&name.to_lowercase())
                    && !ensembles_created
                        .iter()
                        .any(|c| c.to_lowercase() == name.to_lowercase())
                {
                    ensembles_created.push(name);
                }
            })
        })();
        match checked {
            Ok(plan) => {
                let new = !users.contains_key(&plan.email) && new_emails.insert(plan.email.clone());
                results.push(RowResult {
                    line: i + 1,
                    outcome: if new { "new" } else { "existing" },
                    message: None,
                    invitation_url: None,
                });
                planned.push(plan);
            }
            Err(message) => results.push(RowResult {
                line: i + 1,
                outcome: "error",
                message: Some(message),
                invitation_url: None,
            }),
        }
    }
    let errors = results.iter().filter(|r| r.outcome == "error").count();
    let summary = |rows, applied| ImportResult {
        applied,
        rows,
        new_people: new_emails.len(),
        ensembles_created: ensembles_created.clone(),
        errors,
    };
    if errors > 0 {
        return Ok((StatusCode::UNPROCESSABLE_ENTITY, Json(summary(results, false))));
    }
    if input.dry_run {
        return Ok((StatusCode::OK, Json(summary(results, false))));
    }

    // Apply everything in one transaction.
    let mut tx = state.pool.begin().await?;
    let mut user_ids: HashMap<String, Uuid> = users.iter().map(|(e, (id, _))| (e.clone(), *id)).collect();
    let mut ensemble_ids = ensembles.clone();
    for name in &ensembles_created {
        let id = sqlx::query_scalar!(
            "INSERT INTO ensembles (organization_id, name) VALUES ($1, $2) RETURNING id",
            org,
            name
        )
        .fetch_one(&mut *tx)
        .await?;
        ensemble_ids.insert(name.to_lowercase(), id);
    }
    for plan in &planned {
        let user_id = match user_ids.get(&plan.email) {
            Some(id) => *id,
            None => {
                let id = sqlx::query_scalar!(
                    "INSERT INTO users (email, display_name) VALUES ($1, $2) RETURNING id",
                    plan.email,
                    plan.name
                )
                .fetch_one(&mut *tx)
                .await?;
                sqlx::query!(
                    "INSERT INTO memberships (organization_id, user_id, role) VALUES ($1, $2, $3)",
                    org,
                    id,
                    plan.role as Role
                )
                .execute(&mut *tx)
                .await?;
                user_ids.insert(plan.email.clone(), id);
                id
            }
        };
        let Some(ensemble) = plan.ensemble.as_ref().and_then(|e| ensemble_ids.get(e)) else {
            continue;
        };
        sqlx::query!(
            "INSERT INTO ensemble_members (ensemble_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            ensemble,
            user_id
        )
        .execute(&mut *tx)
        .await?;
        if let Some(instrument) = plan.instrument {
            sqlx::query!(
                "INSERT INTO ensemble_member_parts (ensemble_id, user_id, instrument_id, voice)
                 VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING",
                ensemble,
                user_id,
                instrument,
                plan.voice
            )
            .execute(&mut *tx)
            .await?;
        }
    }
    tx.commit().await?;

    // Invitations for the new people, after the import is safely stored.
    let mut invited = HashSet::new();
    for (result, plan) in results.iter_mut().zip(&planned) {
        if result.outcome != "new" || !invited.insert(plan.email.clone()) {
            continue;
        }
        let (url, _) = create_link_token(&state, user_ids[&plan.email], true).await?;
        let mail = crate::mail::invitation(state.config.language, &plan.name, &url, INVITATION_DAYS);
        crate::mail::send_or_log(&state, &plan.email, &mail.subject, &mail.body).await;
        result.invitation_url = Some(url);
    }
    Ok((StatusCode::OK, Json(summary(results, true))))
}

fn message(error: ApiError) -> String {
    match error {
        ApiError::Invalid(message) => message,
        _ => "invalid".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_in_english_and_german() {
        assert_eq!(parse_role(None).unwrap(), Role::Member);
        assert_eq!(parse_role(Some(" Leitung ")).unwrap(), Role::Leader);
        assert_eq!(parse_role(Some("Notenverwaltung")).unwrap(), Role::Librarian);
        assert_eq!(parse_role(Some("ADMIN")).unwrap(), Role::Admin);
        assert!(parse_role(Some("Dirigent")).is_err());
    }
}
