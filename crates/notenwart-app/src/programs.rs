// SPDX-License-Identifier: AGPL-3.0-or-later

//! Programmes (concerts, set lists) of an ensemble, and parts granted to single members.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::App;
use crate::auth::{CurrentUser, check_name};
use crate::ensembles::{check_visible, require_manage};
use crate::error::{AppError, AppResult};
use notenwart_store::programs as db;

const MAX_WORKS: usize = 200;

// ---- programmes ----

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProgramInput {
    name: String,
    #[schema(format = Date)]
    date: Option<String>,
    /// "19:30".
    start_time: Option<String>,
    venue: Option<String>,
    notes: Option<String>,
}

struct Details {
    name: String,
    date: Option<String>,
    start_time: Option<String>,
    venue: Option<String>,
    notes: Option<String>,
}

fn optional_text(text: Option<String>, max: usize, what: &str) -> AppResult<Option<String>> {
    let text = text.map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
    if text.as_ref().is_some_and(|t| t.chars().count() > max) {
        return Err(AppError::Invalid(format!("{what} has at most {max} characters")));
    }
    Ok(text)
}

impl ProgramInput {
    /// Name checked, texts trimmed, date as YYYY-MM-DD and start as HH:MM (or none).
    fn normalize(self) -> AppResult<Details> {
        let name = check_name(&self.name)?;
        let date = match self.date.map(|d| d.trim().to_string()).filter(|d| !d.is_empty()) {
            Some(date) => {
                let format = time::format_description::parse_borrowed::<2>("[year]-[month]-[day]")
                    .map_err(|e| AppError::Internal(anyhow::anyhow!("date format: {e}")))?;
                time::Date::parse(&date, &format).map_err(|_| AppError::Invalid("date must be YYYY-MM-DD".into()))?;
                Some(date)
            }
            None => None,
        };
        let start_time = match self.start_time.map(|d| d.trim().to_string()).filter(|d| !d.is_empty()) {
            Some(start) => {
                let format = time::format_description::parse_borrowed::<2>("[hour]:[minute]")
                    .map_err(|e| AppError::Internal(anyhow::anyhow!("time format: {e}")))?;
                time::Time::parse(&start, &format).map_err(|_| AppError::Invalid("startTime must be HH:MM".into()))?;
                Some(start)
            }
            None => None,
        };
        Ok(Details {
            name,
            date,
            start_time,
            venue: optional_text(self.venue, 200, "venue")?,
            notes: self.notes.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()),
        })
    }
}

/// One entry of a programme in playing order: a work, or a break (interval, speech).
#[derive(Debug, Serialize, Clone, utoipa::ToSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ProgramItem {
    #[serde(rename_all = "camelCase")]
    Work { work_id: Uuid, note: Option<String> },
    #[serde(rename_all = "camelCase")]
    Break { label: String, duration_seconds: Option<i32>, note: Option<String> },
}

#[derive(Debug, Serialize, Clone, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Program {
    id: Uuid,
    ensemble_id: Uuid,
    name: String,
    #[schema(format = Date)]
    date: Option<String>,
    start_time: Option<String>,
    venue: Option<String>,
    notes: Option<String>,
    /// The works in playing order (also in `items`).
    work_ids: Vec<Uuid>,
    /// Works and breaks in playing order.
    items: Vec<ProgramItem>,
}

/// The programmes of these ensembles, by ensemble; each list by date, undated last.
pub async fn programs_for(state: &App, ensembles: &[Uuid]) -> AppResult<HashMap<Uuid, Vec<Program>>> {
    let rows = db::programs_for_select_programs(&state.pool, ensembles).await?;
    let ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
    let mut items: HashMap<Uuid, Vec<(i32, ProgramItem)>> = HashMap::new();
    for w in db::programs_for_select_program_works(&state.pool, &ids).await? {
        items
            .entry(w.program_id)
            .or_default()
            .push((w.position, ProgramItem::Work { work_id: w.work_id, note: w.note }));
    }
    for b in db::programs_for_select_program_breaks(&state.pool, &ids).await? {
        items.entry(b.program_id).or_default().push((
            b.position,
            ProgramItem::Break { label: b.label, duration_seconds: b.duration_seconds, note: b.note },
        ));
    }
    let mut map: HashMap<Uuid, Vec<Program>> = HashMap::new();
    for r in rows {
        let mut list = items.remove(&r.id).unwrap_or_default();
        list.sort_by_key(|(position, _)| *position);
        let items: Vec<ProgramItem> = list.into_iter().map(|(_, item)| item).collect();
        let work_ids = items
            .iter()
            .filter_map(|i| match i {
                ProgramItem::Work { work_id, .. } => Some(*work_id),
                ProgramItem::Break { .. } => None,
            })
            .collect();
        map.entry(r.ensemble_id).or_default().push(Program {
            id: r.id,
            ensemble_id: r.ensemble_id,
            name: r.name,
            date: r.date,
            start_time: r.start_time,
            venue: r.venue,
            notes: r.notes,
            work_ids,
            items,
        });
    }
    Ok(map)
}

async fn find(state: &App, user: &CurrentUser, program_id: Uuid) -> AppResult<Program> {
    let ensemble_id = db::find(&state.pool, program_id, user.organization_id).await?.ok_or(AppError::NotFound)?;
    check_visible(state, user, ensemble_id).await?;
    programs_for(state, &[ensemble_id])
        .await?
        .remove(&ensemble_id)
        .and_then(|list| list.into_iter().find(|p| p.id == program_id))
        .ok_or(AppError::NotFound)
}

/// Tells the ensemble's members that a programme was created or changed.
fn notify_program(state: &App, user: &CurrentUser, program: &Program) {
    let (state, by, org) = (state.clone(), user.id, user.organization_id);
    let (ensemble_id, name) = (program.ensemble_id, program.name.clone());
    tokio::spawn(async move {
        let ensemble = db::notify_program(&state.pool, ensemble_id).await.ok().flatten().unwrap_or_default();
        let members = crate::notify::ensemble_members(&state, ensemble_id, by).await;
        crate::notify::notify(
            &state,
            org,
            crate::notify::Kind::ProgramChanged,
            &members,
            serde_json::json!({ "program": name, "ensemble": ensemble }),
            "/",
        )
        .await;
    });
}

pub async fn create(state: &App, user: &CurrentUser, ensemble_id: Uuid, input: ProgramInput) -> AppResult<Program> {
    require_manage(state, user, ensemble_id).await?;
    let d = input.normalize()?;
    let id = db::create(
        &state.pool,
        ensemble_id,
        &d.name,
        d.date.as_deref(),
        d.start_time.as_deref(),
        d.venue.as_deref(),
        d.notes.as_deref(),
    )
    .await?;
    let program = find(state, user, id).await?;
    notify_program(state, user, &program);
    Ok(program)
}

pub async fn update(state: &App, user: &CurrentUser, program_id: Uuid, input: ProgramInput) -> AppResult<Program> {
    let program = find(state, user, program_id).await?;
    require_manage(state, user, program.ensemble_id).await?;
    let d = input.normalize()?;
    db::update(
        &state.pool,
        program_id,
        &d.name,
        d.date.as_deref(),
        d.start_time.as_deref(),
        d.venue.as_deref(),
        d.notes.as_deref(),
    )
    .await?;
    let program = find(state, user, program_id).await?;
    notify_program(state, user, &program);
    Ok(program)
}

pub async fn remove(state: &App, user: &CurrentUser, program_id: Uuid) -> AppResult<()> {
    let program = find(state, user, program_id).await?;
    require_manage(state, user, program.ensemble_id).await?;
    db::remove(&state.pool, program_id).await?;
    Ok(())
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ItemInput {
    #[serde(rename_all = "camelCase")]
    Work { work_id: Uuid, note: Option<String> },
    #[serde(rename_all = "camelCase")]
    Break { label: String, duration_seconds: Option<i32>, note: Option<String> },
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct ItemsInput {
    #[schema(value_type = Vec<ProgramItem>)]
    items: Vec<ItemInput>,
}

/// Replaces a programme's items; works not yet the ensemble's become its works.
pub async fn set_items(state: &App, user: &CurrentUser, program_id: Uuid, input: ItemsInput) -> AppResult<Program> {
    let program = find(state, user, program_id).await?;
    require_manage(state, user, program.ensemble_id).await?;
    if input.items.len() > MAX_WORKS {
        return Err(AppError::Invalid(format!("a programme holds at most {MAX_WORKS} works")));
    }
    let (mut works, mut work_positions, mut work_notes) = (vec![], vec![], vec![]);
    let (mut labels, mut durations, mut break_notes, mut break_positions) = (vec![], vec![], vec![], vec![]);
    for (position, item) in input.items.into_iter().enumerate() {
        match item {
            ItemInput::Work { work_id, note } => {
                works.push(work_id);
                work_positions.push(position as i32);
                work_notes.push(optional_text(note, 300, "note")?);
            }
            ItemInput::Break { label, duration_seconds, note } => {
                let label = optional_text(Some(label), 120, "label")?
                    .ok_or_else(|| AppError::Invalid("a break needs a label".into()))?;
                if duration_seconds.is_some_and(|d| !(0..=86_400).contains(&d)) {
                    return Err(AppError::Invalid("durationSeconds must be 0 to 86400".into()));
                }
                labels.push(label);
                durations.push(duration_seconds);
                break_notes.push(optional_text(note, 300, "note")?);
                break_positions.push(position as i32);
            }
        }
    }
    if works.iter().collect::<HashSet<_>>().len() != works.len() {
        return Err(AppError::Invalid("a work appears twice".into()));
    }
    let known = db::set_items_select_works(&state.pool, &works, user.organization_id).await?;
    if known as usize != works.len() {
        return Err(AppError::Invalid("unknown work".into()));
    }
    let mut tx = state.pool.begin().await?;
    // A programme's works are the ensemble's works.
    let given = db::set_items_insert_ensemble_works(&mut *tx, program.ensemble_id, &works).await?;
    db::set_items_delete_program_works(&mut *tx, program_id).await?;
    db::set_items_delete_program_breaks(&mut *tx, program_id).await?;
    db::set_items_insert_program_works(&mut *tx, program_id, &works, &work_positions, &work_notes as &[Option<String>])
        .await?;
    db::set_items_insert_program_breaks(
        &mut *tx,
        program_id,
        &break_positions,
        &labels,
        &durations as &[Option<i32>],
        &break_notes as &[Option<String>],
    )
    .await?;
    tx.commit().await?;
    if !given.is_empty() {
        crate::ensembles::notify_work_given(state, user, program.ensemble_id, &given).await;
    }
    let program = find(state, user, program_id).await?;
    notify_program(state, user, &program);
    Ok(program)
}

// ---- parts and grants ----

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EnsemblePart {
    id: Uuid,
    label: String,
    instrument_id: Option<Uuid>,
    voice: Option<i16>,
    work_id: Uuid,
    work_title: String,
}

pub async fn ensemble_parts(state: &App, user: &CurrentUser, ensemble_id: Uuid) -> AppResult<Vec<EnsemblePart>> {
    require_manage(state, user, ensemble_id).await?;
    let parts = db::ensemble_parts(&state.pool, ensemble_id)
        .await
        .map(|rows| rows.into_iter().map(EnsemblePart::from).collect::<Vec<_>>())?;
    Ok(parts)
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Grant {
    part_id: Uuid,
    user_id: Uuid,
    label: String,
    work_title: String,
    user_name: String,
}

pub async fn grants(state: &App, user: &CurrentUser, ensemble_id: Uuid) -> AppResult<Vec<Grant>> {
    require_manage(state, user, ensemble_id).await?;
    let grants =
        db::grants(&state.pool, ensemble_id).await.map(|rows| rows.into_iter().map(Grant::from).collect::<Vec<_>>())?;
    Ok(grants)
}

pub async fn grant(
    state: &App,
    user: &CurrentUser,
    ensemble_id: Uuid,
    part_id: Uuid,
    member_id: Uuid,
) -> AppResult<()> {
    require_manage(state, user, ensemble_id).await?;
    let part_in_ensemble = db::grant_select_parts(&state.pool, part_id, ensemble_id).await?;
    if !part_in_ensemble {
        return Err(AppError::Invalid("the part's work is not one of the ensemble's works".into()));
    }
    let member = db::grant_select_ensemble_members(&state.pool, ensemble_id, member_id).await?;
    if !member {
        return Err(AppError::Invalid("only members of the ensemble can be granted parts".into()));
    }
    let granted = db::grant_insert_part_grants(&state.pool, ensemble_id, member_id, part_id, user.actor()).await?;
    if granted.rows_affected() > 0
        && let Some((part, work)) = crate::notify::part_names(state, part_id).await
    {
        crate::notify::later(
            state,
            user.organization_id,
            crate::notify::Kind::PartsNew,
            vec![member_id],
            serde_json::json!({ "part": part, "work": work }),
            "/".into(),
        );
    }
    Ok(())
}

pub async fn revoke(
    state: &App,
    user: &CurrentUser,
    ensemble_id: Uuid,
    part_id: Uuid,
    member_id: Uuid,
) -> AppResult<()> {
    require_manage(state, user, ensemble_id).await?;
    let deleted = db::revoke(&state.pool, ensemble_id, part_id, member_id).await?;
    if deleted.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

impl From<db::EnsemblepartRow> for EnsemblePart {
    fn from(row: db::EnsemblepartRow) -> Self {
        EnsemblePart {
            id: row.id,
            label: row.label,
            instrument_id: row.instrument_id,
            voice: row.voice,
            work_id: row.work_id,
            work_title: row.work_title,
        }
    }
}

impl From<db::GrantRow> for Grant {
    fn from(row: db::GrantRow) -> Self {
        Grant {
            part_id: row.part_id,
            user_id: row.user_id,
            label: row.label,
            work_title: row.work_title,
            user_name: row.user_name,
        }
    }
}
