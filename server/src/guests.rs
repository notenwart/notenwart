// SPDX-License-Identifier: AGPL-3.0-or-later

//! Guest links (`guests` in api/openapi.yaml): a substitute player without an account sees chosen parts of an
//! ensemble until a date. Leaders and staff make them; the link holds a random token, stored only as a hash.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::{delete, get};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::app::AppState;
use crate::auth::{CurrentUser, check_name, hash_token, new_token};
use crate::ensembles::require_manage;
use crate::error::{ApiError, ApiResult};
use crate::library::{clean_file_name, serve_pdf};
use crate::storage::part_key;

/// At most this many parts per link, and this far ahead.
const MAX_PARTS: usize = 200;
const MAX_DAYS: i64 = 366;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/ensembles/{ensemble_id}/guest-links", get(list).post(create))
        .route("/guest-links/{link_id}", delete(revoke))
        .route("/guest/{token}", get(open))
        .route("/guest/{token}/parts/{part_id}/file", get(file))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestLink {
    id: Uuid,
    name: String,
    part_ids: Vec<Uuid>,
    expires_on: String,
    #[serde(with = "time::serde::rfc3339::option")]
    last_used_at: Option<OffsetDateTime>,
    revoked: bool,
    created_by: Option<String>,
    /// Only when just created: the link itself (the server keeps only its hash).
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
}

async fn links(state: &AppState, ensemble_id: Uuid) -> ApiResult<Vec<GuestLink>> {
    Ok(sqlx::query!(
        r#"SELECT g.id, g.name, g.part_ids, to_char(g.expires_on, 'YYYY-MM-DD') AS "expires_on!", g.last_used_at,
                  g.revoked_at IS NOT NULL AS "revoked!", u.display_name AS "created_by?"
           FROM guest_links g LEFT JOIN users u ON u.id = g.created_by
           WHERE g.ensemble_id = $1 ORDER BY g.revoked_at IS NOT NULL, g.expires_on DESC"#,
        ensemble_id
    )
    .fetch_all(&state.pool)
    .await?
    .into_iter()
    .map(|r| GuestLink {
        id: r.id,
        name: r.name,
        part_ids: r.part_ids,
        expires_on: r.expires_on,
        last_used_at: r.last_used_at,
        revoked: r.revoked,
        created_by: r.created_by,
        url: None,
    })
    .collect())
}

async fn list(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(ensemble_id): Path<Uuid>,
) -> ApiResult<Json<Vec<GuestLink>>> {
    require_manage(&state, &user, ensemble_id).await?;
    Ok(Json(links(&state, ensemble_id).await?))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GuestLinkInput {
    name: String,
    part_ids: Vec<Uuid>,
    expires_on: String,
}

async fn create(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(ensemble_id): Path<Uuid>,
    Json(input): Json<GuestLinkInput>,
) -> ApiResult<(StatusCode, Json<GuestLink>)> {
    require_manage(&state, &user, ensemble_id).await?;
    let name = check_name(&input.name)?;
    let mut parts = input.part_ids;
    parts.sort();
    parts.dedup();
    if parts.is_empty() || parts.len() > MAX_PARTS {
        return Err(ApiError::Invalid(format!("choose 1 to {MAX_PARTS} parts")));
    }
    let format = time::format_description::parse_borrowed::<2>("[year]-[month]-[day]")
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("date format: {e}")))?;
    let expires = time::Date::parse(input.expires_on.trim(), &format)
        .map_err(|_| ApiError::Invalid("expiresOn must be YYYY-MM-DD".into()))?;
    let today = OffsetDateTime::now_utc().date();
    if expires < today || expires > today + time::Duration::days(MAX_DAYS) {
        return Err(ApiError::Invalid(
            "the link must expire within a year from today".into(),
        ));
    }
    // Only parts of the ensemble's works.
    let known = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM parts p JOIN ensemble_works ew ON ew.work_id = p.work_id
           WHERE ew.ensemble_id = $1 AND p.id = ANY($2)"#,
        ensemble_id,
        &parts
    )
    .fetch_one(&state.pool)
    .await?;
    if known as usize != parts.len() {
        return Err(ApiError::Invalid("only parts of the ensemble's works".into()));
    }
    let (token, hash) = new_token();
    let id = sqlx::query_scalar!(
        "INSERT INTO guest_links (organization_id, ensemble_id, name, token_hash, part_ids, expires_on, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
        user.organization_id,
        ensemble_id,
        name,
        hash,
        &parts,
        expires,
        user.id
    )
    .fetch_one(&state.pool)
    .await?;
    let mut link = links(&state, ensemble_id)
        .await?
        .into_iter()
        .find(|l| l.id == id)
        .ok_or(ApiError::NotFound)?;
    link.url = Some(format!("{}/guest/{token}", state.config.public_url));
    Ok((StatusCode::CREATED, Json(link)))
}

async fn revoke(State(state): State<AppState>, user: CurrentUser, Path(link_id): Path<Uuid>) -> ApiResult<StatusCode> {
    let ensemble_id = sqlx::query_scalar!(
        "SELECT ensemble_id FROM guest_links WHERE id = $1 AND organization_id = $2",
        link_id,
        user.organization_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::NotFound)?;
    require_manage(&state, &user, ensemble_id).await?;
    sqlx::query!(
        "UPDATE guest_links SET revoked_at = coalesce(revoked_at, now()) WHERE id = $1",
        link_id
    )
    .execute(&state.pool)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---- what the guest sees ----

struct Valid {
    organization_id: Uuid,
    name: String,
    ensemble: String,
    expires_on: String,
    part_ids: Vec<Uuid>,
}

/// The link behind a token, if it still works. Unknown, expired and revoked links look the same.
async fn valid(state: &AppState, token: &str) -> ApiResult<Valid> {
    let link = sqlx::query!(
        r#"SELECT g.id, g.organization_id, g.name, e.name AS ensemble, to_char(g.expires_on, 'YYYY-MM-DD') AS "expires_on!",
                  g.part_ids
           FROM guest_links g JOIN ensembles e ON e.id = g.ensemble_id
           WHERE g.token_hash = $1 AND g.revoked_at IS NULL AND g.expires_on >= current_date"#,
        hash_token(token)
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::NotFound)?;
    sqlx::query!("UPDATE guest_links SET last_used_at = now() WHERE id = $1", link.id)
        .execute(&state.pool)
        .await?;
    Ok(Valid {
        organization_id: link.organization_id,
        name: link.name,
        ensemble: link.ensemble,
        expires_on: link.expires_on,
        part_ids: link.part_ids,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestPart {
    id: Uuid,
    label: String,
    work_title: String,
    composer: Option<String>,
    page_count: i32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestView {
    name: String,
    ensemble: String,
    expires_on: String,
    parts: Vec<GuestPart>,
}

async fn open(State(state): State<AppState>, Path(token): Path<String>) -> ApiResult<Json<GuestView>> {
    let link = valid(&state, &token).await?;
    let parts = sqlx::query_as!(
        GuestPart,
        r#"SELECT p.id, p.label, w.title AS work_title, w.composer, cardinality(p.pages) AS "page_count!"
           FROM parts p JOIN works w ON w.id = p.work_id
           WHERE p.id = ANY($1) AND w.organization_id = $2
           ORDER BY lower(w.title), lower(p.label)"#,
        &link.part_ids,
        link.organization_id
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(GuestView {
        name: link.name,
        ensemble: link.ensemble,
        expires_on: link.expires_on,
        parts,
    }))
}

async fn file(State(state): State<AppState>, Path((token, part_id)): Path<(String, Uuid)>) -> ApiResult<Response> {
    let link = valid(&state, &token).await?;
    if !link.part_ids.contains(&part_id) {
        return Err(ApiError::NotFound);
    }
    let row = sqlx::query!(
        "SELECT p.label, w.title FROM parts p JOIN works w ON w.id = p.work_id
         WHERE p.id = $1 AND w.organization_id = $2",
        part_id,
        link.organization_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::NotFound)?;
    let file_name = clean_file_name(&format!("{} – {}.pdf", row.title, row.label));
    serve_pdf(&state, &part_key(link.organization_id, part_id), &file_name).await
}

/// Tells whoever made a guest link the day before it stops working (run by the notification worker).
pub async fn notify_expiring(state: &AppState) -> anyhow::Result<()> {
    let links = sqlx::query!(
        r#"SELECT id, organization_id, name, created_by AS "created_by!", to_char(expires_on, 'YYYY-MM-DD') AS "expires_on!"
           FROM guest_links
           WHERE revoked_at IS NULL AND expiring_notified_at IS NULL AND created_by IS NOT NULL
             AND expires_on <= current_date + 1 AND expires_on >= current_date"#
    )
    .fetch_all(&state.pool)
    .await?;
    for link in links {
        crate::notify::notify(
            state,
            link.organization_id,
            crate::notify::Kind::GuestLinkExpiring,
            &[link.created_by],
            serde_json::json!({ "guest": link.name, "date": link.expires_on }),
            "/ensembles",
        )
        .await;
        sqlx::query!(
            "UPDATE guest_links SET expiring_notified_at = now() WHERE id = $1",
            link.id
        )
        .execute(&state.pool)
        .await?;
    }
    Ok(())
}
