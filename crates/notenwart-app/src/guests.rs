// SPDX-License-Identifier: AGPL-3.0-or-later

//! Guest links (`guests` in api/openapi.json): a substitute player without an account sees chosen parts of an
//! ensemble until a date. Leaders and staff make them; the link holds a random token, stored only as a hash.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::App;
use crate::auth::{CurrentUser, check_name, hash_token, new_token};
use crate::ensembles::require_manage;
use crate::error::{AppError, AppResult};
use crate::library::{PdfFile, clean_file_name, open_pdf};
use crate::storage::part_key;
use notenwart_store::guests as db;

/// At most this many parts per link, and this far ahead.
const MAX_PARTS: usize = 200;
const MAX_DAYS: i64 = 366;

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GuestLink {
    id: Uuid,
    name: String,
    part_ids: Vec<Uuid>,
    #[schema(format = Date)]
    expires_on: String,
    #[serde(with = "time::serde::rfc3339::option")]
    last_used_at: Option<OffsetDateTime>,
    revoked: bool,
    created_by: Option<String>,
    /// Only when just created: the link itself (the server keeps only its hash).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(nullable = false)]
    url: Option<String>,
}

async fn links(state: &App, ensemble_id: Uuid) -> AppResult<Vec<GuestLink>> {
    Ok(db::links(&state.pool, ensemble_id)
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

pub async fn list(state: &App, user: &CurrentUser, ensemble_id: Uuid) -> AppResult<Vec<GuestLink>> {
    require_manage(state, user, ensemble_id).await?;
    links(state, ensemble_id).await
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GuestLinkInput {
    name: String,
    part_ids: Vec<Uuid>,
    #[schema(format = Date)]
    expires_on: String,
}

pub async fn create(state: &App, user: &CurrentUser, ensemble_id: Uuid, input: GuestLinkInput) -> AppResult<GuestLink> {
    require_manage(state, user, ensemble_id).await?;
    let name = check_name(&input.name)?;
    let mut parts = input.part_ids;
    parts.sort();
    parts.dedup();
    if parts.is_empty() || parts.len() > MAX_PARTS {
        return Err(AppError::Invalid(format!("choose 1 to {MAX_PARTS} parts")));
    }
    let format = time::format_description::parse_borrowed::<2>("[year]-[month]-[day]")
        .map_err(|e| AppError::Internal(anyhow::anyhow!("date format: {e}")))?;
    let expires = time::Date::parse(input.expires_on.trim(), &format)
        .map_err(|_| AppError::Invalid("expiresOn must be YYYY-MM-DD".into()))?;
    let today = OffsetDateTime::now_utc().date();
    if expires < today || expires > today + time::Duration::days(MAX_DAYS) {
        return Err(AppError::Invalid("the link must expire within a year from today".into()));
    }
    // Only parts of the ensemble's works.
    let known = db::create_select_parts(&state.pool, ensemble_id, &parts).await?;
    if known as usize != parts.len() {
        return Err(AppError::Invalid("only parts of the ensemble's works".into()));
    }
    let (token, hash) = new_token();
    let id = db::create_insert_guest_links(
        &state.pool,
        user.organization_id,
        ensemble_id,
        &name,
        &hash,
        &parts,
        expires,
        user.actor(),
    )
    .await?;
    let mut link = links(state, ensemble_id).await?.into_iter().find(|l| l.id == id).ok_or(AppError::NotFound)?;
    link.url = Some(format!("{}/guest/{token}", state.config.public_url));
    Ok(link)
}

pub async fn revoke(state: &App, user: &CurrentUser, link_id: Uuid) -> AppResult<()> {
    let ensemble_id =
        db::revoke_select_guest_links(&state.pool, link_id, user.organization_id).await?.ok_or(AppError::NotFound)?;
    require_manage(state, user, ensemble_id).await?;
    db::revoke_update_guest_links(&state.pool, link_id).await?;
    Ok(())
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
async fn valid(state: &App, token: &str) -> AppResult<Valid> {
    let link = db::valid_select_guest_links(&state.pool, &hash_token(token)).await?.ok_or(AppError::NotFound)?;
    db::valid_update_guest_links(&state.pool, link.id).await?;
    Ok(Valid {
        organization_id: link.organization_id,
        name: link.name,
        ensemble: link.ensemble,
        expires_on: link.expires_on,
        part_ids: link.part_ids,
    })
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GuestPart {
    id: Uuid,
    label: String,
    work_title: String,
    composer: Option<String>,
    page_count: i32,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GuestView {
    name: String,
    ensemble: String,
    #[schema(format = Date)]
    expires_on: String,
    #[schema(inline)]
    parts: Vec<GuestPart>,
}

pub async fn open(state: &App, token: String) -> AppResult<GuestView> {
    let link = valid(state, &token).await?;
    let parts = db::open(&state.pool, &link.part_ids, link.organization_id)
        .await
        .map(|rows| rows.into_iter().map(GuestPart::from).collect::<Vec<_>>())?;
    Ok(GuestView { name: link.name, ensemble: link.ensemble, expires_on: link.expires_on, parts })
}

/// A part's PDF through a guest link.
pub async fn file(state: &App, token: &str, part_id: Uuid) -> AppResult<PdfFile> {
    let link = valid(state, token).await?;
    if !link.part_ids.contains(&part_id) {
        return Err(AppError::NotFound);
    }
    let row = db::file(&state.pool, part_id, link.organization_id).await?.ok_or(AppError::NotFound)?;
    let file_name = clean_file_name(&format!("{} – {}.pdf", row.title, row.label));
    open_pdf(state, &part_key(link.organization_id, part_id), &file_name).await
}

/// Tells whoever made a guest link the day before it stops working (run by the notification worker).
pub async fn notify_expiring(state: &App) -> anyhow::Result<()> {
    let links = db::notify_expiring_select_guest_links(&state.pool).await?;
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
        db::notify_expiring_update_guest_links(&state.pool, link.id).await?;
    }
    Ok(())
}

impl From<db::GuestpartRow> for GuestPart {
    fn from(row: db::GuestpartRow) -> Self {
        GuestPart {
            id: row.id,
            label: row.label,
            work_title: row.work_title,
            composer: row.composer,
            page_count: row.page_count,
        }
    }
}
