// SPDX-License-Identifier: AGPL-3.0-or-later

//! Annotations on a part (pen and highlighter strokes by page): each person's own, and the markings a leader shares
//! with everyone of their ensemble who plays the part. The last write wins.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::App;
use crate::auth::CurrentUser;
use crate::ensembles::may_see_part;
use crate::error::{AppError, AppResult};
use notenwart_store::annotations as db;

const MAX_POINTS: usize = 2000;
const MAX_STROKES: usize = 5000;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Tool {
    Pen,
    Highlighter,
}

#[derive(Serialize, Deserialize, Clone, Debug, utoipa::ToSchema)]
pub struct Stroke {
    #[schema(inline)]
    tool: Tool,
    color: String,
    /// Line width as a fraction of the page width.
    width: f32,
    /// Points as [x, y], each 0 to 1 – 0,0 is the page's top left, 1,1 its bottom right.
    points: Vec<[f32; 2]>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct AnnotationsInput {
    /// Strokes by page number ("1", "2", …), in drawing order.
    pages: BTreeMap<u32, Vec<Stroke>>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Annotations {
    pages: BTreeMap<u32, Vec<Stroke>>,
    #[serde(with = "time::serde::rfc3339::option")]
    #[schema(required = true)]
    updated_at: Option<OffsetDateTime>,
}

/// The part's page count, or 404 when the user may not see it.
async fn page_count(state: &App, user: &CurrentUser, part_id: Uuid) -> AppResult<i32> {
    if !may_see_part(state, user, part_id).await? {
        return Err(AppError::NotFound);
    }
    db::page_count(&state.pool, part_id, user.organization_id).await?.ok_or(AppError::NotFound)
}

fn validate(pages: &BTreeMap<u32, Vec<Stroke>>, page_count: i32) -> Result<(), String> {
    let mut strokes = 0;
    for (&page, list) in pages {
        if page < 1 || page as i32 > page_count {
            return Err(format!("page {page} is outside 1–{page_count}"));
        }
        for stroke in list {
            strokes += 1;
            let hex = stroke.color.strip_prefix('#').unwrap_or_default();
            if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err("a stroke colour must be #rrggbb".into());
            }
            if !(stroke.width > 0.0 && stroke.width <= 0.05) {
                return Err("a stroke width must be above 0 and at most 0.05".into());
            }
            if stroke.points.is_empty() || stroke.points.len() > MAX_POINTS {
                return Err(format!("a stroke needs 1 to {MAX_POINTS} points"));
            }
            if stroke.points.iter().flatten().any(|v| !(0.0..=1.0).contains(v)) {
                return Err("points must lie on the page (0 to 1)".into());
            }
        }
    }
    if strokes > MAX_STROKES {
        return Err(format!("at most {MAX_STROKES} strokes per part"));
    }
    Ok(())
}

pub async fn load(state: &App, user: &CurrentUser, part_id: Uuid) -> AppResult<Annotations> {
    page_count(state, user, part_id).await?;
    let row = db::load(&state.pool, user.id, part_id).await?;
    Ok(match row {
        Some(row) => Annotations { pages: serde_json::from_value(row.pages)?, updated_at: Some(row.updated_at) },
        None => Annotations { pages: BTreeMap::new(), updated_at: None },
    })
}

pub async fn save(
    state: &App,
    user: &CurrentUser,
    part_id: Uuid,
    mut input: AnnotationsInput,
) -> AppResult<Annotations> {
    let pages = page_count(state, user, part_id).await?;
    validate(&input.pages, pages).map_err(AppError::Invalid)?;
    input.pages.retain(|_, strokes| !strokes.is_empty());
    let updated_at = db::save(&state.pool, user.id, part_id, &serde_json::to_value(&input.pages)?).await?;
    Ok(Annotations { pages: input.pages, updated_at: Some(updated_at) })
}

// ---- shared by leaders ----

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = SharedAnnotations)]
pub struct SharedLayer {
    ensemble_id: Uuid,
    ensemble: String,
    pages: BTreeMap<u32, Vec<Stroke>>,
    #[serde(with = "time::serde::rfc3339::option")]
    updated_at: Option<OffsetDateTime>,
    updated_by: Option<String>,
    /// The user may change this layer (leaders of the ensemble, staff).
    can_edit: bool,
}

/// The shared layers on a part the user sees: one per ensemble of theirs that has the work (staff: every such
/// ensemble), empty ones included where the user may start one.
async fn layers(state: &App, user: &CurrentUser, part_id: Uuid) -> AppResult<Vec<SharedLayer>> {
    let rows = db::layers(&state.pool, part_id, user.id, user.organization_id).await?;
    let mut result = vec![];
    for r in rows {
        let can_edit = r.leader || user.is_staff();
        if !(r.member || user.is_staff()) || (r.pages.is_none() && !can_edit) {
            continue;
        }
        result.push(SharedLayer {
            ensemble_id: r.id,
            ensemble: r.name,
            pages: match r.pages {
                Some(pages) => serde_json::from_value(pages)?,
                None => BTreeMap::new(),
            },
            updated_at: r.updated_at,
            updated_by: r.updated_by,
            can_edit,
        });
    }
    Ok(result)
}

pub async fn load_shared(state: &App, user: &CurrentUser, part_id: Uuid) -> AppResult<Vec<SharedLayer>> {
    page_count(state, user, part_id).await?;
    layers(state, user, part_id).await
}

pub async fn save_shared(
    state: &App,
    user: &CurrentUser,
    part_id: Uuid,
    ensemble_id: Uuid,
    mut input: AnnotationsInput,
) -> AppResult<SharedLayer> {
    let pages = page_count(state, user, part_id).await?;
    let Some(layer) =
        layers(state, user, part_id).await?.into_iter().find(|l| l.ensemble_id == ensemble_id && l.can_edit)
    else {
        // Players of the ensemble may look, not change; to everyone else it does not exist.
        let member = db::save_shared_select_ensemble_members(&state.pool, ensemble_id, user.id).await?;
        return Err(if member { AppError::Forbidden } else { AppError::NotFound });
    };
    validate(&input.pages, pages).map_err(AppError::Invalid)?;
    input.pages.retain(|_, strokes| !strokes.is_empty());
    let saved = db::save_shared_insert_shared_annotations(
        &state.pool,
        ensemble_id,
        part_id,
        &serde_json::to_value(&input.pages)?,
        user.actor(),
    )
    .await?;
    if saved.notify && !input.pages.is_empty() {
        db::save_shared_update_shared_annotations(&state.pool, ensemble_id, part_id).await?;
        let players = db::save_shared_select_visible_parts(&state.pool, part_id, ensemble_id, user.id).await?;
        if let Some((part, work)) = crate::notify::part_names(state, part_id).await {
            crate::notify::later(
                state,
                user.organization_id,
                crate::notify::Kind::AnnotationsShared,
                players,
                serde_json::json!({ "by": user.name, "part": part, "work": work }),
                "/".into(),
            );
        }
    }
    Ok(SharedLayer {
        pages: input.pages,
        updated_at: Some(saved.updated_at),
        updated_by: Some(user.name.clone()),
        ..layer
    })
}

// ---- printing ----

#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct AnnotatedQuery {
    /// The user's own annotations (default yes).
    own: Option<bool>,
    /// The leaders' shared markings (default yes).
    shared: Option<bool>,
}

fn to_marks(pages: &BTreeMap<u32, Vec<Stroke>>, into: &mut BTreeMap<u32, Vec<crate::pdf::Mark>>) {
    for (&page, strokes) in pages {
        for stroke in strokes {
            let hex = stroke.color.trim_start_matches('#');
            let channel =
                |i: usize| u8::from_str_radix(hex.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0) as f32 / 255.0;
            into.entry(page).or_default().push(crate::pdf::Mark {
                rgb: [channel(0), channel(2), channel(4)],
                width: stroke.width,
                highlighter: stroke.tool == Tool::Highlighter,
                points: stroke.points.clone(),
            });
        }
    }
}

/// The part's PDF with annotations drawn in, for printing or keeping: the leaders' markings beneath, one's own on
/// top – as in the viewer.
/// A part's PDF with annotations drawn in, for printing.
#[derive(Debug)]
pub struct AnnotatedPdf {
    pub file_name: String,
    pub bytes: Vec<u8>,
}

pub async fn annotated(
    state: &App,
    user: &CurrentUser,
    part_id: Uuid,
    query: AnnotatedQuery,
) -> AppResult<AnnotatedPdf> {
    use object_store::ObjectStoreExt;

    page_count(state, user, part_id).await?;
    let mut marks = BTreeMap::new();
    if query.shared.unwrap_or(true) {
        for layer in layers(state, user, part_id).await? {
            to_marks(&layer.pages, &mut marks);
        }
    }
    if query.own.unwrap_or(true)
        && let Some(pages) = db::annotated_select_annotations(&state.pool, user.id, part_id).await?
    {
        to_marks(&serde_json::from_value(pages)?, &mut marks);
    }
    let names = db::annotated_select_parts(&state.pool, part_id).await?;
    let bytes = state.store.get(&crate::storage::part_key(user.organization_id, part_id)).await?.bytes().await?;
    let pdf = tokio::task::spawn_blocking(move || crate::pdf::stamp(&bytes, &marks))
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .map_err(|e| AppError::Internal(e.into()))?;
    let file_name = crate::library::clean_file_name(&format!("{} – {} (annotated).pdf", names.title, names.label));
    Ok(AnnotatedPdf { file_name, bytes: pdf })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stroke(points: Vec<[f32; 2]>) -> Stroke {
        Stroke { tool: Tool::Pen, color: "#1d2b45".into(), width: 0.004, points }
    }

    #[test]
    fn validates_pages_colours_widths_and_points() {
        let ok = BTreeMap::from([(1, vec![stroke(vec![[0.1, 0.2], [0.3, 0.4]])])]);
        assert!(validate(&ok, 2).is_ok());
        assert!(validate(&BTreeMap::from([(3, vec![stroke(vec![[0.1, 0.1]])])]), 2).is_err());
        assert!(validate(&BTreeMap::from([(1, vec![stroke(vec![[1.2, 0.1]])])]), 2).is_err());
        assert!(validate(&BTreeMap::from([(1, vec![stroke(vec![])])]), 2).is_err());
        let mut red = stroke(vec![[0.5, 0.5]]);
        red.color = "red".into();
        assert!(validate(&BTreeMap::from([(1, vec![red])]), 2).is_err());
        let mut thick = stroke(vec![[0.5, 0.5]]);
        thick.width = 0.2;
        assert!(validate(&BTreeMap::from([(1, vec![thick])]), 2).is_err());
    }
}
