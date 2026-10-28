// SPDX-License-Identifier: AGPL-3.0-or-later

//! Annotations on a part (pen and highlighter strokes by page): each person's own, and the markings a leader shares
//! with everyone of their ensemble who plays the part. The last write wins.

use std::collections::BTreeMap;

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::app::AppState;
use crate::auth::CurrentUser;
use crate::ensembles::may_see_part;
use crate::error::{ApiError, ApiResult};

const MAX_POINTS: usize = 2000;
const MAX_STROKES: usize = 5000;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/parts/{part_id}/annotations", get(load).put(save))
        .route("/parts/{part_id}/shared-annotations", get(load_shared))
        .route(
            "/parts/{part_id}/shared-annotations/{ensemble_id}",
            axum::routing::put(save_shared),
        )
        .route("/parts/{part_id}/annotated", get(annotated))
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Tool {
    Pen,
    Highlighter,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Stroke {
    tool: Tool,
    color: String,
    width: f32,
    points: Vec<[f32; 2]>,
}

#[derive(Deserialize)]
pub struct AnnotationsInput {
    pages: BTreeMap<u32, Vec<Stroke>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Annotations {
    pages: BTreeMap<u32, Vec<Stroke>>,
    #[serde(with = "time::serde::rfc3339::option")]
    updated_at: Option<OffsetDateTime>,
}

/// The part's page count, or 404 when the user may not see it.
async fn page_count(state: &AppState, user: &CurrentUser, part_id: Uuid) -> ApiResult<i32> {
    if !may_see_part(state, user, part_id).await? {
        return Err(ApiError::NotFound);
    }
    sqlx::query_scalar!(
        r#"SELECT cardinality(p.pages) AS "n!" FROM parts p JOIN works w ON w.id = p.work_id
           WHERE p.id = $1 AND w.organization_id = $2"#,
        part_id,
        user.organization_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::NotFound)
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

async fn load(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(part_id): Path<Uuid>,
) -> ApiResult<Json<Annotations>> {
    page_count(&state, &user, part_id).await?;
    let row = sqlx::query!(
        "SELECT pages, updated_at FROM annotations WHERE user_id = $1 AND part_id = $2",
        user.id,
        part_id
    )
    .fetch_optional(&state.pool)
    .await?;
    Ok(Json(match row {
        Some(row) => Annotations {
            pages: serde_json::from_value(row.pages)?,
            updated_at: Some(row.updated_at),
        },
        None => Annotations {
            pages: BTreeMap::new(),
            updated_at: None,
        },
    }))
}

async fn save(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(part_id): Path<Uuid>,
    Json(mut input): Json<AnnotationsInput>,
) -> ApiResult<Json<Annotations>> {
    let pages = page_count(&state, &user, part_id).await?;
    validate(&input.pages, pages).map_err(ApiError::Invalid)?;
    input.pages.retain(|_, strokes| !strokes.is_empty());
    let updated_at = sqlx::query_scalar!(
        "INSERT INTO annotations (user_id, part_id, pages) VALUES ($1, $2, $3)
         ON CONFLICT (user_id, part_id) DO UPDATE SET pages = excluded.pages, updated_at = now()
         RETURNING updated_at",
        user.id,
        part_id,
        serde_json::to_value(&input.pages)?
    )
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(Annotations {
        pages: input.pages,
        updated_at: Some(updated_at),
    }))
}

// ---- shared by leaders ----

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
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
async fn layers(state: &AppState, user: &CurrentUser, part_id: Uuid) -> ApiResult<Vec<SharedLayer>> {
    let rows = sqlx::query!(
        r#"SELECT e.id, e.name, coalesce(m.leader, false) AS "leader!", m.user_id IS NOT NULL AS "member!",
                  s.pages AS "pages?", s.updated_at AS "updated_at?", u.display_name AS "updated_by?"
           FROM parts p
           JOIN ensemble_works ew ON ew.work_id = p.work_id
           JOIN ensembles e ON e.id = ew.ensemble_id
           LEFT JOIN ensemble_members m ON m.ensemble_id = e.id AND m.user_id = $2
           LEFT JOIN shared_annotations s ON s.ensemble_id = e.id AND s.part_id = p.id
           LEFT JOIN users u ON u.id = s.updated_by
           WHERE p.id = $1 AND e.organization_id = $3
           ORDER BY lower(e.name)"#,
        part_id,
        user.id,
        user.organization_id
    )
    .fetch_all(&state.pool)
    .await?;
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

async fn load_shared(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(part_id): Path<Uuid>,
) -> ApiResult<Json<Vec<SharedLayer>>> {
    page_count(&state, &user, part_id).await?;
    Ok(Json(layers(&state, &user, part_id).await?))
}

async fn save_shared(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((part_id, ensemble_id)): Path<(Uuid, Uuid)>,
    Json(mut input): Json<AnnotationsInput>,
) -> ApiResult<Json<SharedLayer>> {
    let pages = page_count(&state, &user, part_id).await?;
    let Some(layer) = layers(&state, &user, part_id)
        .await?
        .into_iter()
        .find(|l| l.ensemble_id == ensemble_id && l.can_edit)
    else {
        // Players of the ensemble may look, not change; to everyone else it does not exist.
        let member = sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM ensemble_members WHERE ensemble_id = $1 AND user_id = $2) AS "x!""#,
            ensemble_id,
            user.id
        )
        .fetch_one(&state.pool)
        .await?;
        return Err(if member {
            ApiError::Forbidden
        } else {
            ApiError::NotFound
        });
    };
    validate(&input.pages, pages).map_err(ApiError::Invalid)?;
    input.pages.retain(|_, strokes| !strokes.is_empty());
    let saved = sqlx::query!(
        r#"INSERT INTO shared_annotations (ensemble_id, part_id, pages, updated_by) VALUES ($1, $2, $3, $4)
           ON CONFLICT (ensemble_id, part_id)
           DO UPDATE SET pages = excluded.pages, updated_by = excluded.updated_by, updated_at = now()
           RETURNING updated_at, notified_at IS NULL OR notified_at < now() - interval '30 minutes' AS "notify!""#,
        ensemble_id,
        part_id,
        serde_json::to_value(&input.pages)?,
        user.id
    )
    .fetch_one(&state.pool)
    .await?;
    if saved.notify && !input.pages.is_empty() {
        sqlx::query!(
            "UPDATE shared_annotations SET notified_at = now() WHERE ensemble_id = $1 AND part_id = $2",
            ensemble_id,
            part_id
        )
        .execute(&state.pool)
        .await?;
        let players = sqlx::query_scalar!(
            r#"SELECT DISTINCT user_id AS "user_id!" FROM visible_parts
               WHERE part_id = $1 AND ensemble_id = $2 AND user_id <> $3"#,
            part_id,
            ensemble_id,
            user.id
        )
        .fetch_all(&state.pool)
        .await?;
        if let Some((part, work)) = crate::notify::part_names(&state, part_id).await {
            crate::notify::later(
                &state,
                user.organization_id,
                crate::notify::Kind::AnnotationsShared,
                players,
                serde_json::json!({ "by": user.name, "part": part, "work": work }),
                "/".into(),
            );
        }
    }
    Ok(Json(SharedLayer {
        pages: input.pages,
        updated_at: Some(saved.updated_at),
        updated_by: Some(user.name.clone()),
        ..layer
    }))
}

// ---- printing ----

#[derive(Deserialize)]
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
async fn annotated(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(part_id): Path<Uuid>,
    axum::extract::Query(query): axum::extract::Query<AnnotatedQuery>,
) -> ApiResult<axum::response::Response> {
    use axum::response::IntoResponse;
    use object_store::ObjectStoreExt;

    page_count(&state, &user, part_id).await?;
    let mut marks = BTreeMap::new();
    if query.shared.unwrap_or(true) {
        for layer in layers(&state, &user, part_id).await? {
            to_marks(&layer.pages, &mut marks);
        }
    }
    if query.own.unwrap_or(true)
        && let Some(pages) = sqlx::query_scalar!(
            "SELECT pages FROM annotations WHERE user_id = $1 AND part_id = $2",
            user.id,
            part_id
        )
        .fetch_optional(&state.pool)
        .await?
    {
        to_marks(&serde_json::from_value(pages)?, &mut marks);
    }
    let names = sqlx::query!(
        "SELECT p.label, w.title FROM parts p JOIN works w ON w.id = p.work_id WHERE p.id = $1",
        part_id
    )
    .fetch_one(&state.pool)
    .await?;
    let bytes = state
        .store
        .get(&crate::storage::part_key(user.organization_id, part_id))
        .await?
        .bytes()
        .await?;
    let pdf = tokio::task::spawn_blocking(move || crate::pdf::stamp(&bytes, &marks))
        .await
        .map_err(|e| ApiError::Internal(e.into()))?
        .map_err(|e| ApiError::Internal(e.into()))?;
    let file_name = crate::library::clean_file_name(&format!("{} – {} (annotated).pdf", names.title, names.label));
    let disposition = format!(
        "inline; filename*=UTF-8''{}",
        percent_encoding::utf8_percent_encode(&file_name, percent_encoding::NON_ALPHANUMERIC)
    );
    Ok((
        [
            (axum::http::header::CONTENT_TYPE, "application/pdf".to_string()),
            (axum::http::header::CONTENT_DISPOSITION, disposition),
            (axum::http::header::CACHE_CONTROL, "private, no-store".to_string()),
        ],
        pdf,
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stroke(points: Vec<[f32; 2]>) -> Stroke {
        Stroke {
            tool: Tool::Pen,
            color: "#1d2b45".into(),
            width: 0.004,
            points,
        }
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
