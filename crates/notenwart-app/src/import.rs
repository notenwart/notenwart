// SPDX-License-Identifier: AGPL-3.0-or-later

//! Importing people from a table (CSV in the web app): users, their ensembles and what they play there.
//! Validates every row first; changes nothing unless all rows are fine.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::App;
use crate::auth::{CurrentUser, INVITATION_DAYS, Role, check_name, create_link_token, normalize_email};
use crate::error::{AppError, AppResult};
use notenwart_store::import as db;

const MAX_ROWS: usize = 2000;

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImportRow {
    name: String,
    email: String,
    /// admin, librarian, leader or member (also German – Admin, Notenverwaltung, Leitung, Mitglied); empty means
    /// member. Ignored for existing people.
    role: Option<String>,
    /// Ensemble name; created when missing.
    ensemble: Option<String>,
    /// Instrument name or alias (English or German, e.g. "Trompete"); needs an ensemble.
    instrument: Option<String>,
    /// Part number; empty plays every part of the instrument.
    voice: Option<i16>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImportInput {
    rows: Vec<ImportRow>,
    dry_run: bool,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = ImportRowResult)]
pub struct RowResult {
    /// Position in `rows`, starting at 1.
    line: usize,
    #[schema(schema_with = crate::schema::strings::<Outcome>)]
    outcome: &'static str,
    /// Why the row cannot be imported (English).
    message: Option<String>,
    /// For new people after a real import.
    invitation_url: Option<String>,
}

/// The outcomes of a row, as the API names them.
#[derive(Debug)]
pub struct Outcome;

impl crate::schema::Strings for Outcome {
    const VALUES: &[&str] = &["new", "existing", "error"];
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    applied: bool,
    rows: Vec<RowResult>,
    new_people: usize,
    ensembles_created: Vec<String>,
    /// Rows with errors.
    pub errors: usize,
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

/// Imports people; with errors in any row (see [`ImportResult::errors`]) nothing changes.
pub async fn import(state: &App, admin: &CurrentUser, input: ImportInput) -> AppResult<ImportResult> {
    admin.require(admin.is_admin())?;
    if input.rows.is_empty() || input.rows.len() > MAX_ROWS {
        return Err(AppError::Invalid(format!("an import needs 1 to {MAX_ROWS} rows")));
    }
    let org = admin.organization_id;

    // Everything rows can refer to, loaded once.
    let users: HashMap<String, (Uuid, bool)> = db::import_select_memberships(&state.pool, org)
        .await?
        .into_iter()
        .map(|r| (r.email, (r.id, r.member)))
        .collect();
    let ensembles: HashMap<String, Uuid> =
        db::import_select_ensembles(&state.pool, org).await?.into_iter().map(|r| (r.name, r.id)).collect();
    let mut instruments: HashMap<String, Uuid> = HashMap::new();
    for row in db::import_select_instruments(&state.pool, org).await? {
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
                    Some(*instruments.get(&instrument.to_lowercase()).ok_or("unknown instrument")?)
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
                    && !ensembles_created.iter().any(|c| c.to_lowercase() == name.to_lowercase())
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
            Err(message) => {
                results.push(RowResult { line: i + 1, outcome: "error", message: Some(message), invitation_url: None })
            }
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
        return Ok(summary(results, false));
    }
    if input.dry_run {
        return Ok(summary(results, false));
    }

    // Apply everything in one transaction.
    let mut tx = state.pool.begin().await?;
    let mut user_ids: HashMap<String, Uuid> = users.iter().map(|(e, (id, _))| (e.clone(), *id)).collect();
    let mut ensemble_ids = ensembles.clone();
    for name in &ensembles_created {
        let id = db::import_insert_ensembles(&mut *tx, org, name).await?;
        ensemble_ids.insert(name.to_lowercase(), id);
    }
    for plan in &planned {
        let user_id = match user_ids.get(&plan.email) {
            Some(id) => *id,
            None => {
                let id = db::import_insert_users(&mut *tx, &plan.email, &plan.name).await?;
                db::import_insert_memberships(&mut *tx, org, id, plan.role as Role).await?;
                user_ids.insert(plan.email.clone(), id);
                id
            }
        };
        let Some(ensemble) = plan.ensemble.as_ref().and_then(|e| ensemble_ids.get(e)) else {
            continue;
        };
        db::import_insert_ensemble_members(&mut *tx, ensemble, user_id).await?;
        if let Some(instrument) = plan.instrument {
            db::import_insert_ensemble_member_parts(&mut *tx, ensemble, user_id, instrument, plan.voice).await?;
        }
    }
    tx.commit().await?;

    // Invitations for the new people, after the import is safely stored.
    let mut invited = HashSet::new();
    for (result, plan) in results.iter_mut().zip(&planned) {
        if result.outcome != "new" || !invited.insert(plan.email.clone()) {
            continue;
        }
        let (url, _) = create_link_token(state, user_ids[&plan.email], true).await?;
        let mail = crate::mail::invitation(state.config.language, &plan.name, &url, INVITATION_DAYS);
        crate::mail::send_or_log(state, &plan.email, &mail.subject, &mail.body).await;
        result.invitation_url = Some(url);
    }
    Ok(summary(results, true))
}

fn message(error: AppError) -> String {
    match error {
        AppError::Invalid(message) => message,
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
