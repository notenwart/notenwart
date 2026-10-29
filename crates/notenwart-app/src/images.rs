// SPDX-License-Identifier: AGPL-3.0-or-later

//! Avatars (`images` in api/openapi.yaml): the organisation's logo, ensemble logos and people's pictures. PNG, JPEG
//! or WebP up to 2 MB (the web app scales them down before uploading); no SVG, which can carry scripts. Ensembles
//! without a logo show a symbol (an emoji on a colour), people without a picture their initials.
//!
//! Who sees an image: the organisation's logo everyone (sign-in page), ensemble logos the organisation's people,
//! a person's picture themselves, staff, and the leaders of their ensembles.

use object_store::{ObjectStoreExt, PutPayload};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::App;
use crate::auth::{CurrentUser, check_name};
use crate::ensembles::require_manage;
use crate::error::{AppError, AppResult};
use crate::files::Upload;
use crate::storage::image_key;

pub const MAX_BYTES: usize = 2 * 1024 * 1024;

/// Where the web app loads an image.
pub fn url(id: Option<Uuid>) -> Option<String> {
    id.map(|id| format!("/api/images/{id}"))
}

/// PNG, JPEG or WebP, judged by the first bytes.
pub fn image_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

/// An uploaded image, checked: its content and type. `upload` is the file as the interface read it.
fn read_image(upload: AppResult<Option<Upload>>) -> AppResult<(bytes::Bytes, &'static str)> {
    let bytes = upload?.ok_or_else(|| AppError::Invalid("missing form field `file`".into()))?.bytes;
    if bytes.len() > MAX_BYTES {
        return Err(AppError::TooLarge("an image has at most 2 MB".into()));
    }
    let kind = image_type(&bytes).ok_or_else(|| AppError::Invalid("only PNG, JPEG or WebP images".into()))?;
    Ok((bytes, kind))
}

/// Stores an image for an owner and returns its id; the owner's previous image goes.
async fn store(
    state: &App,
    organization_id: Uuid,
    owner_kind: &str,
    owner_id: Uuid,
    previous: Option<Uuid>,
    bytes: bytes::Bytes,
    content_type: &str,
) -> AppResult<Uuid> {
    let id = Uuid::new_v4();
    let size = bytes.len() as i32;
    state.store.put(&image_key(organization_id, id), PutPayload::from(bytes)).await?;
    sqlx::query!(
        "INSERT INTO images (id, organization_id, owner_kind, owner_id, content_type, size) VALUES ($1, $2, $3, $4, $5, $6)",
        id,
        organization_id,
        owner_kind,
        owner_id,
        content_type,
        size
    )
    .execute(&state.pool)
    .await?;
    if let Some(previous) = previous {
        discard(state, organization_id, previous).await?;
    }
    Ok(id)
}

async fn discard(state: &App, organization_id: Uuid, id: Uuid) -> AppResult<()> {
    sqlx::query!("DELETE FROM images WHERE id = $1", id).execute(&state.pool).await?;
    let _ = state.store.delete(&image_key(organization_id, id)).await;
    Ok(())
}

/// A stored image: its type and content.
#[derive(Debug)]
pub struct Image {
    pub content_type: String,
    pub bytes: bytes::Bytes,
}

/// An image for whoever may see it; the organisation's logo also without signing in.
pub async fn image(state: &App, user: Option<&CurrentUser>, image_id: Uuid) -> AppResult<Image> {
    let row =
        sqlx::query!("SELECT organization_id, owner_kind, owner_id, content_type FROM images WHERE id = $1", image_id)
            .fetch_optional(&state.pool)
            .await?
            .ok_or(AppError::NotFound)?;
    if row.owner_kind != "organization" {
        let user = user.filter(|u| u.organization_id == row.organization_id).ok_or(AppError::NotFound)?;
        let allowed = match row.owner_kind.as_str() {
            "ensemble" => true,
            _ => user.id == row.owner_id || user.is_staff() || leads(state, user.id, row.owner_id).await?,
        };
        if !allowed {
            return Err(AppError::NotFound);
        }
    }
    let bytes = state.store.get(&image_key(row.organization_id, image_id)).await?.bytes().await?;
    Ok(Image { content_type: row.content_type, bytes })
}

/// Whether `leader` leads an ensemble `member` belongs to.
async fn leads(state: &App, leader: Uuid, member: Uuid) -> AppResult<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM ensemble_members l JOIN ensemble_members m ON m.ensemble_id = l.ensemble_id
                          WHERE l.user_id = $1 AND l.leader AND m.user_id = $2) AS "x!""#,
        leader,
        member
    )
    .fetch_one(&state.pool)
    .await?)
}

// ---- the organisation ----

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Organization {
    id: Uuid,
    name: String,
    logo_url: Option<String>,
    profile_pictures: bool,
}

pub async fn find_organization(state: &App, id: Uuid) -> AppResult<Organization> {
    let row = sqlx::query!("SELECT id, name, logo_id, profile_pictures FROM organizations WHERE id = $1", id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Organization { id: row.id, name: row.name, logo_url: url(row.logo_id), profile_pictures: row.profile_pictures })
}

pub async fn organization(state: &App, user: &CurrentUser) -> AppResult<Organization> {
    find_organization(state, user.organization_id).await
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OrganizationInput {
    name: String,
    profile_pictures: bool,
}

pub async fn update_organization(state: &App, user: &CurrentUser, input: OrganizationInput) -> AppResult<Organization> {
    user.require(user.is_admin())?;
    let name = check_name(&input.name)?;
    sqlx::query!(
        "UPDATE organizations SET name = $2, profile_pictures = $3 WHERE id = $1",
        user.organization_id,
        name,
        input.profile_pictures
    )
    .execute(&state.pool)
    .await?;
    find_organization(state, user.organization_id).await
}

/// A new logo for the organisation; `upload` as in [`read_image`].
pub async fn organization_logo(
    state: &App,
    user: &CurrentUser,
    upload: AppResult<Option<Upload>>,
) -> AppResult<Organization> {
    user.require(user.is_admin())?;
    let (bytes, kind) = read_image(upload)?;
    let previous = sqlx::query_scalar!("SELECT logo_id FROM organizations WHERE id = $1", user.organization_id)
        .fetch_one(&state.pool)
        .await?;
    let org = user.organization_id;
    let id = store(state, org, "organization", org, None, bytes, kind).await?;
    sqlx::query!("UPDATE organizations SET logo_id = $2 WHERE id = $1", org, id).execute(&state.pool).await?;
    if let Some(previous) = previous {
        discard(state, org, previous).await?;
    }
    find_organization(state, org).await
}

pub async fn remove_organization_logo(state: &App, user: &CurrentUser) -> AppResult<()> {
    user.require(user.is_admin())?;
    let previous = sqlx::query_scalar!("SELECT logo_id FROM organizations WHERE id = $1", user.organization_id)
        .fetch_one(&state.pool)
        .await?;
    sqlx::query!("UPDATE organizations SET logo_id = NULL WHERE id = $1", user.organization_id)
        .execute(&state.pool)
        .await?;
    if let Some(previous) = previous {
        discard(state, user.organization_id, previous).await?;
    }
    Ok(())
}

// ---- ensembles ----

/// A new logo for an ensemble; `upload` as in [`read_image`].
pub async fn ensemble_logo(
    state: &App,
    user: &CurrentUser,
    ensemble_id: Uuid,
    upload: AppResult<Option<Upload>>,
) -> AppResult<()> {
    require_manage(state, user, ensemble_id).await?;
    let (bytes, kind) = read_image(upload)?;
    let previous =
        sqlx::query_scalar!("SELECT logo_id FROM ensembles WHERE id = $1", ensemble_id).fetch_one(&state.pool).await?;
    let id = store(state, user.organization_id, "ensemble", ensemble_id, None, bytes, kind).await?;
    sqlx::query!("UPDATE ensembles SET logo_id = $2 WHERE id = $1", ensemble_id, id).execute(&state.pool).await?;
    if let Some(previous) = previous {
        discard(state, user.organization_id, previous).await?;
    }
    Ok(())
}

pub async fn remove_ensemble_logo(state: &App, user: &CurrentUser, ensemble_id: Uuid) -> AppResult<()> {
    require_manage(state, user, ensemble_id).await?;
    let previous =
        sqlx::query_scalar!("SELECT logo_id FROM ensembles WHERE id = $1", ensemble_id).fetch_one(&state.pool).await?;
    sqlx::query!("UPDATE ensembles SET logo_id = NULL WHERE id = $1", ensemble_id).execute(&state.pool).await?;
    if let Some(previous) = previous {
        discard(state, user.organization_id, previous).await?;
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SymbolInput {
    /// An emoji (or a few characters); null for initials.
    symbol: Option<String>,
    /// "#RRGGBB"; null for the colour derived from the name.
    color: Option<String>,
}

pub async fn ensemble_symbol(state: &App, user: &CurrentUser, ensemble_id: Uuid, input: SymbolInput) -> AppResult<()> {
    require_manage(state, user, ensemble_id).await?;
    let symbol = input.symbol.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    if symbol.as_ref().is_some_and(|s| s.chars().count() > 4 || s.len() > 16) {
        return Err(AppError::Invalid("a symbol is one emoji or up to four characters".into()));
    }
    let color = input.color.filter(|c| !c.is_empty());
    if color
        .as_ref()
        .is_some_and(|c| !(c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|h| h.is_ascii_hexdigit())))
    {
        return Err(AppError::Invalid("color must be #RRGGBB".into()));
    }
    sqlx::query!("UPDATE ensembles SET symbol = $2, color = $3 WHERE id = $1", ensemble_id, symbol, color)
        .execute(&state.pool)
        .await?;
    Ok(())
}

// ---- people ----

/// A new picture of the user; `upload` as in [`read_image`].
pub async fn my_avatar(state: &App, user: &CurrentUser, upload: AppResult<Option<Upload>>) -> AppResult<()> {
    let org = find_organization(state, user.organization_id).await?;
    if !org.profile_pictures {
        return Err(AppError::Conflict("profile pictures are switched off in this organisation".into()));
    }
    let (bytes, kind) = read_image(upload)?;
    let previous =
        sqlx::query_scalar!("SELECT avatar_id FROM users WHERE id = $1", user.id).fetch_one(&state.pool).await?;
    let id = store(state, user.organization_id, "user", user.id, None, bytes, kind).await?;
    sqlx::query!("UPDATE users SET avatar_id = $2 WHERE id = $1", user.id, id).execute(&state.pool).await?;
    if let Some(previous) = previous {
        discard(state, user.organization_id, previous).await?;
    }
    Ok(())
}

pub async fn remove_my_avatar(state: &App, user: &CurrentUser) -> AppResult<()> {
    let previous =
        sqlx::query_scalar!("SELECT avatar_id FROM users WHERE id = $1", user.id).fetch_one(&state.pool).await?;
    sqlx::query!("UPDATE users SET avatar_id = NULL WHERE id = $1", user.id).execute(&state.pool).await?;
    if let Some(previous) = previous {
        discard(state, user.organization_id, previous).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_raster_images() {
        assert_eq!(image_type(b"\x89PNG\r\n\x1a\n...."), Some("image/png"));
        assert_eq!(image_type(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("image/jpeg"));
        assert_eq!(image_type(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(image_type(b"<svg xmlns=\"http://www.w3.org/2000/svg\"><script/>"), None);
    }
}
