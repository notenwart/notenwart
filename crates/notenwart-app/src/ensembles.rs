// SPDX-License-Identifier: AGPL-3.0-or-later

//! Ensembles, who plays what in them, the works they play, and "my music": the parts a user may see.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::App;
use crate::auth::{CurrentUser, Role};
use crate::error::{AppError, AppResult, unique_violation};
use crate::programs::{Program, programs_for};

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct EnsembleInput {
    name: String,
    description: Option<String>,
}

impl EnsembleInput {
    fn normalize(self) -> AppResult<(String, Option<String>)> {
        let name = crate::auth::check_name(&self.name)?;
        let description = self.description.map(|d| d.trim().to_string()).filter(|d| !d.is_empty());
        Ok((name, description))
    }
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Ensemble {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    /// Path of the logo image, if any.
    pub logo_url: Option<String>,
    /// An emoji shown when there is no logo.
    pub symbol: Option<String>,
    pub color: Option<String>,
    pub member_count: i64,
    pub work_count: i64,
    /// The user may change members and works.
    pub can_manage: bool,
}

/// Whether the user may change the ensemble's members and works.
async fn can_manage(state: &App, user: &CurrentUser, ensemble_id: Uuid) -> AppResult<bool> {
    if user.is_staff() {
        return Ok(true);
    }
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM ensemble_members WHERE ensemble_id = $1 AND user_id = $2 AND leader) AS "x!""#,
        ensemble_id,
        user.id
    )
    .fetch_one(&state.pool)
    .await?)
}

/// 404 unless the ensemble is in the user's organisation and the user may see it (staff, or a member).
pub(crate) async fn check_visible(state: &App, user: &CurrentUser, ensemble_id: Uuid) -> AppResult<()> {
    let visible = sqlx::query_scalar!(
        r#"SELECT EXISTS (
               SELECT 1 FROM ensembles e WHERE e.id = $1 AND e.organization_id = $2
                 AND ($4 OR EXISTS (SELECT 1 FROM ensemble_members m WHERE m.ensemble_id = e.id AND m.user_id = $3))
           ) AS "x!""#,
        ensemble_id,
        user.organization_id,
        user.id,
        user.is_staff()
    )
    .fetch_one(&state.pool)
    .await?;
    if visible { Ok(()) } else { Err(AppError::NotFound) }
}

pub(crate) async fn require_manage(state: &App, user: &CurrentUser, ensemble_id: Uuid) -> AppResult<()> {
    check_visible(state, user, ensemble_id).await?;
    user.require(can_manage(state, user, ensemble_id).await?)
}

async fn summaries(state: &App, user: &CurrentUser, only: Option<Uuid>) -> AppResult<Vec<Ensemble>> {
    let rows = sqlx::query!(
        r#"SELECT e.id, e.name, e.description, '/api/images/' || e.logo_id AS logo_url, e.symbol, e.color,
                  (SELECT count(*) FROM ensemble_members m WHERE m.ensemble_id = e.id) AS "member_count!",
                  (SELECT count(*) FROM ensemble_works w WHERE w.ensemble_id = e.id) AS "work_count!",
                  coalesce((SELECT leader FROM ensemble_members m WHERE m.ensemble_id = e.id AND m.user_id = $2),
                           false) AS "leader!",
                  EXISTS (SELECT 1 FROM ensemble_members m WHERE m.ensemble_id = e.id AND m.user_id = $2)
                      AS "member!"
           FROM ensembles e
           WHERE e.organization_id = $1 AND ($3::uuid IS NULL OR e.id = $3)
           ORDER BY lower(e.name)"#,
        user.organization_id,
        user.id,
        only
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(rows
        .into_iter()
        .filter(|r| user.is_staff() || r.member)
        .map(|r| Ensemble {
            id: r.id,
            name: r.name,
            description: r.description,
            logo_url: r.logo_url,
            symbol: r.symbol,
            color: r.color,
            member_count: r.member_count,
            work_count: r.work_count,
            can_manage: user.is_staff() || r.leader,
        })
        .collect())
}

pub async fn list(state: &App, user: &CurrentUser) -> AppResult<Vec<Ensemble>> {
    summaries(state, user, None).await
}

pub async fn create(state: &App, user: &CurrentUser, input: EnsembleInput) -> AppResult<Ensemble> {
    user.require(user.is_staff())?;
    let (name, description) = input.normalize()?;
    let id = sqlx::query_scalar!(
        "INSERT INTO ensembles (organization_id, name, description) VALUES ($1, $2, $3) RETURNING id",
        user.organization_id,
        name,
        description
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| unique_violation(e, "an ensemble with this name exists already"))?;
    let ensemble = summaries(state, user, Some(id)).await?.pop().ok_or(AppError::NotFound)?;
    Ok(ensemble)
}

pub async fn update(state: &App, user: &CurrentUser, ensemble_id: Uuid, input: EnsembleInput) -> AppResult<Ensemble> {
    require_manage(state, user, ensemble_id).await?;
    let (name, description) = input.normalize()?;
    sqlx::query!("UPDATE ensembles SET name = $2, description = $3 WHERE id = $1", ensemble_id, name, description)
        .execute(&state.pool)
        .await
        .map_err(|e| unique_violation(e, "an ensemble with this name exists already"))?;
    let ensemble = summaries(state, user, Some(ensemble_id)).await?.pop().ok_or(AppError::NotFound)?;
    Ok(ensemble)
}

pub async fn remove(state: &App, user: &CurrentUser, ensemble_id: Uuid) -> AppResult<()> {
    user.require(user.is_staff())?;
    let deleted =
        sqlx::query!("DELETE FROM ensembles WHERE id = $1 AND organization_id = $2", ensemble_id, user.organization_id)
            .execute(&state.pool)
            .await?;
    if deleted.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

// ---- detail: members and works ----

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlayedPart {
    pub instrument_id: Uuid,
    /// Null plays every part number of the instrument.
    pub voice: Option<i16>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EnsembleMember {
    pub user_id: Uuid,
    pub name: String,
    pub avatar_url: Option<String>,
    pub leader: bool,
    pub plays: Vec<PlayedPart>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EnsembleWork {
    pub id: Uuid,
    pub title: String,
    pub composer: Option<String>,
    pub arranger: Option<String>,
    pub publisher: Option<String>,
    pub lyricist: Option<String>,
    pub society_number: Option<String>,
    pub duration_seconds: Option<i32>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct EnsembleDetail {
    #[serde(flatten)]
    pub ensemble: Ensemble,
    pub members: Vec<EnsembleMember>,
    pub works: Vec<EnsembleWork>,
    /// Programmes by date, undated last.
    pub programs: Vec<Program>,
}

async fn members(state: &App, ensemble_id: Uuid, only: Option<Uuid>) -> AppResult<Vec<EnsembleMember>> {
    let rows = sqlx::query!(
        r#"SELECT m.user_id, u.display_name, m.leader,
                  CASE WHEN o.profile_pictures THEN '/api/images/' || u.avatar_id END AS avatar_url,
                  coalesce(array_agg(p.instrument_id) FILTER (WHERE p.instrument_id IS NOT NULL), '{}')
                      AS "instruments!: Vec<Uuid>",
                  coalesce(array_agg(coalesce(p.voice, 0)) FILTER (WHERE p.instrument_id IS NOT NULL), '{}')
                      AS "voices!: Vec<i16>"
           FROM ensemble_members m
           JOIN users u ON u.id = m.user_id
           JOIN ensembles e2 ON e2.id = m.ensemble_id JOIN organizations o ON o.id = e2.organization_id
           LEFT JOIN ensemble_member_parts p ON p.ensemble_id = m.ensemble_id AND p.user_id = m.user_id
           WHERE m.ensemble_id = $1 AND ($2::uuid IS NULL OR m.user_id = $2)
           GROUP BY m.user_id, u.display_name, m.leader, o.profile_pictures, u.avatar_id
           ORDER BY m.leader DESC, lower(u.display_name)"#,
        ensemble_id,
        only
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| EnsembleMember {
            user_id: r.user_id,
            name: r.display_name,
            avatar_url: r.avatar_url,
            leader: r.leader,
            plays: r
                .instruments
                .into_iter()
                .zip(r.voices)
                .map(|(instrument_id, voice)| PlayedPart { instrument_id, voice: (voice > 0).then_some(voice) })
                .collect(),
        })
        .collect())
}

pub async fn detail(state: &App, user: &CurrentUser, ensemble_id: Uuid) -> AppResult<EnsembleDetail> {
    check_visible(state, user, ensemble_id).await?;
    let ensemble = summaries(state, user, Some(ensemble_id)).await?.pop().ok_or(AppError::NotFound)?;
    // Members only see themselves (pupils are often minors); those who manage see everyone.
    let only = (!ensemble.can_manage).then_some(user.id);
    let members = members(state, ensemble_id, only).await?;
    let works = sqlx::query_as!(
        EnsembleWork,
        "SELECT w.id, w.title, w.composer, w.arranger, w.publisher, w.lyricist, w.society_number, w.duration_seconds
         FROM ensemble_works ew JOIN works w ON w.id = ew.work_id
         WHERE ew.ensemble_id = $1 ORDER BY lower(w.title)",
        ensemble_id
    )
    .fetch_all(&state.pool)
    .await?;
    let programs = programs_for(state, &[ensemble_id]).await?.remove(&ensemble_id).unwrap_or_default();
    Ok(EnsembleDetail { ensemble, members, works, programs })
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[schema(as = EnsembleMemberInput)]
pub struct MemberInput {
    leader: bool,
    plays: Vec<PlayedPart>,
}

pub async fn put_member(
    state: &App,
    user: &CurrentUser,
    ensemble_id: Uuid,
    member_id: Uuid,
    input: MemberInput,
) -> AppResult<EnsembleMember> {
    require_manage(state, user, ensemble_id).await?;
    let role = sqlx::query_scalar!(
        r#"SELECT role AS "role: Role" FROM memberships WHERE user_id = $1 AND organization_id = $2"#,
        member_id,
        user.organization_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;
    let was_leader = sqlx::query_scalar!(
        "SELECT leader FROM ensemble_members WHERE ensemble_id = $1 AND user_id = $2",
        ensemble_id,
        member_id
    )
    .fetch_optional(&state.pool)
    .await?
    .unwrap_or(false);
    if input.leader != was_leader {
        // Leaders manage players; who leads is decided by admins and librarians.
        user.require(user.is_staff())?;
    }
    if input.leader && role == Role::Member {
        return Err(AppError::Invalid("only users with the role leader, librarian or admin can lead".into()));
    }
    let mut seen = HashSet::new();
    for part in &input.plays {
        if part.voice.is_some_and(|v| v < 1) {
            return Err(AppError::Invalid("part numbers start at 1".into()));
        }
        if !seen.insert(part.clone()) {
            return Err(AppError::Invalid("an instrument and part number appear twice".into()));
        }
    }
    let instruments: Vec<Uuid> = input.plays.iter().map(|p| p.instrument_id).collect();
    let voices: Vec<Option<i16>> = input.plays.iter().map(|p| p.voice).collect();
    let known = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM instruments
           WHERE id = ANY($1) AND organization_id = $2"#,
        &instruments,
        user.organization_id
    )
    .fetch_one(&state.pool)
    .await?;
    if known as usize != instruments.iter().collect::<HashSet<_>>().len() {
        return Err(AppError::Invalid("unknown instrument".into()));
    }

    let mut tx = state.pool.begin().await?;
    sqlx::query!(
        "INSERT INTO ensemble_members (ensemble_id, user_id, leader) VALUES ($1, $2, $3)
         ON CONFLICT (ensemble_id, user_id) DO UPDATE SET leader = excluded.leader",
        ensemble_id,
        member_id,
        input.leader
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!("DELETE FROM ensemble_member_parts WHERE ensemble_id = $1 AND user_id = $2", ensemble_id, member_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query!(
        "INSERT INTO ensemble_member_parts (ensemble_id, user_id, instrument_id, voice)
         SELECT $1, $2, i, v FROM unnest($3::uuid[], $4::smallint[]) AS t(i, v)",
        ensemble_id,
        member_id,
        &instruments,
        &voices as &[Option<i16>]
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    members(state, ensemble_id, Some(member_id)).await?.pop().ok_or(AppError::NotFound)
}

pub async fn remove_member(state: &App, user: &CurrentUser, ensemble_id: Uuid, member_id: Uuid) -> AppResult<()> {
    require_manage(state, user, ensemble_id).await?;
    let leader = sqlx::query_scalar!(
        "SELECT leader FROM ensemble_members WHERE ensemble_id = $1 AND user_id = $2",
        ensemble_id,
        member_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;
    if leader {
        user.require(user.is_staff())?;
    }
    sqlx::query!("DELETE FROM ensemble_members WHERE ensemble_id = $1 AND user_id = $2", ensemble_id, member_id)
        .execute(&state.pool)
        .await?;
    Ok(())
}

/// Tells the members of an ensemble who now see parts of these works (who, decided now; sent in the background).
pub async fn notify_work_given(state: &App, user: &CurrentUser, ensemble_id: Uuid, works: &[Uuid]) {
    let (by, org, works) = (user.id, user.organization_id, works.to_vec());
    {
        let rows = sqlx::query!(
            r#"SELECT DISTINCT v.user_id AS "user_id!", w.id AS work_id, w.title
               FROM visible_parts v JOIN parts p ON p.id = v.part_id JOIN works w ON w.id = p.work_id
               WHERE v.ensemble_id = $1 AND p.work_id = ANY($2) AND v.user_id <> $3"#,
            ensemble_id,
            &works,
            by
        )
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default();
        let mut by_work: std::collections::HashMap<(Uuid, String), Vec<Uuid>> = Default::default();
        for r in rows {
            by_work.entry((r.work_id, r.title)).or_default().push(r.user_id);
        }
        for ((_, title), users) in by_work {
            crate::notify::later(
                state,
                org,
                crate::notify::Kind::PartsNew,
                users,
                serde_json::json!({ "work": title }),
                "/".into(),
            );
        }
    }
}

pub async fn add_work(state: &App, user: &CurrentUser, ensemble_id: Uuid, work_id: Uuid) -> AppResult<()> {
    require_manage(state, user, ensemble_id).await?;
    let exists = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM works WHERE id = $1 AND organization_id = $2) AS "x!""#,
        work_id,
        user.organization_id
    )
    .fetch_one(&state.pool)
    .await?;
    if !exists {
        return Err(AppError::NotFound);
    }
    let added = sqlx::query!(
        "INSERT INTO ensemble_works (ensemble_id, work_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        ensemble_id,
        work_id
    )
    .execute(&state.pool)
    .await?;
    if added.rows_affected() > 0 {
        notify_work_given(state, user, ensemble_id, &[work_id]).await;
    }
    Ok(())
}

pub async fn remove_work(state: &App, user: &CurrentUser, ensemble_id: Uuid, work_id: Uuid) -> AppResult<()> {
    require_manage(state, user, ensemble_id).await?;
    let mut tx = state.pool.begin().await?;
    let deleted =
        sqlx::query!("DELETE FROM ensemble_works WHERE ensemble_id = $1 AND work_id = $2", ensemble_id, work_id)
            .execute(&mut *tx)
            .await?;
    if deleted.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    // The work leaves the ensemble's programmes, and grants of its parts end.
    sqlx::query!(
        "DELETE FROM program_works pw USING programs p
         WHERE pw.program_id = p.id AND p.ensemble_id = $1 AND pw.work_id = $2",
        ensemble_id,
        work_id
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "DELETE FROM part_grants g USING parts p WHERE g.part_id = p.id AND g.ensemble_id = $1 AND p.work_id = $2",
        ensemble_id,
        work_id
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

// ---- my music ----

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MyPart {
    id: Uuid,
    label: String,
    page_count: i32,
    /// Changes whenever the part's PDF is rendered again; offline copies compare it.
    #[serde(with = "time::serde::rfc3339")]
    updated_at: time::OffsetDateTime,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct MyWork {
    id: Uuid,
    title: String,
    composer: Option<String>,
    parts: Vec<MyPart>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MyEnsemble {
    id: Uuid,
    name: String,
    /// Path of the logo image, if any.
    logo_url: Option<String>,
    /// An emoji shown when there is no logo.
    symbol: Option<String>,
    color: Option<String>,
    leader: bool,
    /// Works of the ensemble with at least one part the user may see.
    works: Vec<MyWork>,
    /// Programmes by date, undated last; their works refer to `works`.
    programs: Vec<Program>,
}

pub async fn my_music(state: &App, user: &CurrentUser) -> AppResult<Vec<MyEnsemble>> {
    let mut result: Vec<MyEnsemble> = sqlx::query!(
        "SELECT e.id, e.name, m.leader, '/api/images/' || e.logo_id AS logo_url, e.symbol, e.color
         FROM ensemble_members m JOIN ensembles e ON e.id = m.ensemble_id
         WHERE m.user_id = $1 AND e.organization_id = $2 ORDER BY lower(e.name)",
        user.id,
        user.organization_id
    )
    .fetch_all(&state.pool)
    .await?
    .into_iter()
    .map(|r| MyEnsemble {
        id: r.id,
        name: r.name,
        logo_url: r.logo_url,
        symbol: r.symbol,
        color: r.color,
        leader: r.leader,
        works: Vec::new(),
        programs: Vec::new(),
    })
    .collect();
    let ids: Vec<Uuid> = result.iter().map(|e| e.id).collect();
    let mut programs = programs_for(state, &ids).await?;
    for ensemble in &mut result {
        ensemble.programs = programs.remove(&ensemble.id).unwrap_or_default();
    }

    let parts = sqlx::query!(
        r#"SELECT v.ensemble_id AS "ensemble_id!", w.id AS work_id, w.title, w.composer, p.id AS part_id, p.label,
                  cardinality(p.pages) AS "page_count!", p.updated_at
           FROM visible_parts v
           JOIN parts p ON p.id = v.part_id
           JOIN works w ON w.id = p.work_id
           LEFT JOIN instruments i ON i.id = p.instrument_id
           WHERE v.user_id = $1
           ORDER BY lower(w.title), w.id, i.sort NULLS FIRST, p.voice NULLS FIRST, lower(p.label)"#,
        user.id
    )
    .fetch_all(&state.pool)
    .await?;
    for row in parts {
        let Some(ensemble) = result.iter_mut().find(|e| e.id == row.ensemble_id) else {
            continue;
        };
        if ensemble.works.last().is_none_or(|w| w.id != row.work_id) {
            ensemble.works.push(MyWork { id: row.work_id, title: row.title, composer: row.composer, parts: vec![] });
        }
        if let Some(work) = ensemble.works.last_mut() {
            work.parts.push(MyPart {
                id: row.part_id,
                label: row.label,
                page_count: row.page_count,
                updated_at: row.updated_at,
            });
        }
    }
    Ok(result)
}

/// Whether the user may download the part: library readers always, members when it is visible to them.
pub async fn may_see_part(state: &App, user: &CurrentUser, part_id: Uuid) -> AppResult<bool> {
    if user.reads_library() {
        return Ok(true);
    }
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM visible_parts WHERE user_id = $1 AND part_id = $2) AS "x!""#,
        user.id,
        part_id
    )
    .fetch_one(&state.pool)
    .await?)
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PartInfo {
    #[serde(flatten)]
    part: MyPart,
    work_id: Uuid,
    work_title: String,
    composer: Option<String>,
}

pub async fn part_info(state: &App, user: &CurrentUser, part_id: Uuid) -> AppResult<PartInfo> {
    if !may_see_part(state, user, part_id).await? {
        return Err(AppError::NotFound);
    }
    let row = sqlx::query!(
        r#"SELECT p.id, p.label, cardinality(p.pages) AS "page_count!", p.updated_at, w.id AS work_id,
                  w.title, w.composer
           FROM parts p JOIN works w ON w.id = p.work_id
           WHERE p.id = $1 AND w.organization_id = $2"#,
        part_id,
        user.organization_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(PartInfo {
        part: MyPart { id: row.id, label: row.label, page_count: row.page_count, updated_at: row.updated_at },
        work_id: row.work_id,
        work_title: row.title,
        composer: row.composer,
    })
}
