// SPDX-License-Identifier: AGPL-3.0-or-later

//! Avatars (`images` in api/openapi.yaml): the organisation's logo, ensemble logos and people's pictures. PNG, JPEG
//! or WebP up to 2 MB (the web app scales them down before uploading); no SVG, which can carry scripts. Ensembles
//! without a logo show a symbol (an emoji on a colour), people without a picture their initials.
//!
//! Who sees an image: the organisation's logo everyone (sign-in page), ensemble logos the organisation's people,
//! a person's picture themselves, staff, and the leaders of their ensembles.

use axum::extract::{DefaultBodyLimit, Multipart, Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, put};
use axum::{Json, Router};
use axum_extra::extract::CookieJar;
use object_store::{ObjectStoreExt, PutPayload};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::app::AppState;
use crate::auth::{CurrentUser, SESSION_COOKIE, check_name, current_user};
use crate::ensembles::require_manage;
use crate::error::{ApiError, ApiResult};
use crate::storage::image_key;

const MAX_BYTES: usize = 2 * 1024 * 1024;

pub fn router() -> Router<AppState> {
    let upload = DefaultBodyLimit::max(MAX_BYTES + 64 * 1024);
    Router::new()
        .route("/images/{image_id}", get(image))
        .route("/organization", get(organization).put(update_organization))
        .route(
            "/organization/logo",
            put(organization_logo).delete(remove_organization_logo).layer(upload),
        )
        .route(
            "/ensembles/{ensemble_id}/logo",
            put(ensemble_logo).delete(remove_ensemble_logo).layer(upload),
        )
        .route("/ensembles/{ensemble_id}/symbol", put(ensemble_symbol))
        .route("/me/avatar", put(my_avatar).delete(remove_my_avatar).layer(upload))
}

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

async fn read_image(mut multipart: Multipart) -> ApiResult<(bytes::Bytes, &'static str)> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::Invalid(format!("upload: {e}")))?
    {
        if field.name() == Some("file") {
            let bytes = field
                .bytes()
                .await
                .map_err(|e| ApiError::Invalid(format!("upload: {e}")))?;
            if bytes.len() > MAX_BYTES {
                return Err(ApiError::TooLarge("an image has at most 2 MB".into()));
            }
            let kind = image_type(&bytes).ok_or_else(|| ApiError::Invalid("only PNG, JPEG or WebP images".into()))?;
            return Ok((bytes, kind));
        }
    }
    Err(ApiError::Invalid("missing form field `file`".into()))
}

/// Stores an image for an owner and returns its id; the owner's previous image goes.
async fn store(
    state: &AppState,
    organization_id: Uuid,
    owner_kind: &str,
    owner_id: Uuid,
    previous: Option<Uuid>,
    bytes: bytes::Bytes,
    content_type: &str,
) -> ApiResult<Uuid> {
    let id = Uuid::new_v4();
    let size = bytes.len() as i32;
    state
        .store
        .put(&image_key(organization_id, id), PutPayload::from(bytes))
        .await?;
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

async fn discard(state: &AppState, organization_id: Uuid, id: Uuid) -> ApiResult<()> {
    sqlx::query!("DELETE FROM images WHERE id = $1", id)
        .execute(&state.pool)
        .await?;
    let _ = state.store.delete(&image_key(organization_id, id)).await;
    Ok(())
}

async fn image(State(state): State<AppState>, jar: CookieJar, Path(image_id): Path<Uuid>) -> ApiResult<Response> {
    let row = sqlx::query!(
        "SELECT organization_id, owner_kind, owner_id, content_type FROM images WHERE id = $1",
        image_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::NotFound)?;
    if row.owner_kind != "organization" {
        let user = match jar.get(SESSION_COOKIE) {
            Some(cookie) => current_user(&state, cookie.value()).await?,
            None => None,
        }
        .filter(|u| u.organization_id == row.organization_id)
        .ok_or(ApiError::NotFound)?;
        let allowed = match row.owner_kind.as_str() {
            "ensemble" => true,
            _ => user.id == row.owner_id || user.is_staff() || leads(&state, user.id, row.owner_id).await?,
        };
        if !allowed {
            return Err(ApiError::NotFound);
        }
    }
    let bytes = state
        .store
        .get(&image_key(row.organization_id, image_id))
        .await?
        .bytes()
        .await?;
    // Ids change with every new image, so they can be kept for long.
    Ok((
        [
            (header::CONTENT_TYPE, row.content_type),
            (
                header::CACHE_CONTROL,
                "private, max-age=31536000, immutable".to_string(),
            ),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
        ],
        bytes,
    )
        .into_response())
}

/// Whether `leader` leads an ensemble `member` belongs to.
async fn leads(state: &AppState, leader: Uuid, member: Uuid) -> ApiResult<bool> {
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Organization {
    id: Uuid,
    name: String,
    logo_url: Option<String>,
    profile_pictures: bool,
}

pub async fn find_organization(state: &AppState, id: Uuid) -> ApiResult<Organization> {
    let row = sqlx::query!(
        "SELECT id, name, logo_id, profile_pictures FROM organizations WHERE id = $1",
        id
    )
    .fetch_one(&state.pool)
    .await?;
    Ok(Organization {
        id: row.id,
        name: row.name,
        logo_url: url(row.logo_id),
        profile_pictures: row.profile_pictures,
    })
}

async fn organization(State(state): State<AppState>, user: CurrentUser) -> ApiResult<Json<Organization>> {
    Ok(Json(find_organization(&state, user.organization_id).await?))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OrganizationInput {
    name: String,
    profile_pictures: bool,
}

async fn update_organization(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(input): Json<OrganizationInput>,
) -> ApiResult<Json<Organization>> {
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
    Ok(Json(find_organization(&state, user.organization_id).await?))
}

async fn organization_logo(
    State(state): State<AppState>,
    user: CurrentUser,
    multipart: Multipart,
) -> ApiResult<Json<Organization>> {
    user.require(user.is_admin())?;
    let (bytes, kind) = read_image(multipart).await?;
    let previous = sqlx::query_scalar!("SELECT logo_id FROM organizations WHERE id = $1", user.organization_id)
        .fetch_one(&state.pool)
        .await?;
    let org = user.organization_id;
    let id = store(&state, org, "organization", org, None, bytes, kind).await?;
    sqlx::query!("UPDATE organizations SET logo_id = $2 WHERE id = $1", org, id)
        .execute(&state.pool)
        .await?;
    if let Some(previous) = previous {
        discard(&state, org, previous).await?;
    }
    Ok(Json(find_organization(&state, org).await?))
}

async fn remove_organization_logo(State(state): State<AppState>, user: CurrentUser) -> ApiResult<StatusCode> {
    user.require(user.is_admin())?;
    let previous = sqlx::query_scalar!("SELECT logo_id FROM organizations WHERE id = $1", user.organization_id)
        .fetch_one(&state.pool)
        .await?;
    sqlx::query!(
        "UPDATE organizations SET logo_id = NULL WHERE id = $1",
        user.organization_id
    )
    .execute(&state.pool)
    .await?;
    if let Some(previous) = previous {
        discard(&state, user.organization_id, previous).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

// ---- ensembles ----

async fn ensemble_logo(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(ensemble_id): Path<Uuid>,
    multipart: Multipart,
) -> ApiResult<StatusCode> {
    require_manage(&state, &user, ensemble_id).await?;
    let (bytes, kind) = read_image(multipart).await?;
    let previous = sqlx::query_scalar!("SELECT logo_id FROM ensembles WHERE id = $1", ensemble_id)
        .fetch_one(&state.pool)
        .await?;
    let id = store(&state, user.organization_id, "ensemble", ensemble_id, None, bytes, kind).await?;
    sqlx::query!("UPDATE ensembles SET logo_id = $2 WHERE id = $1", ensemble_id, id)
        .execute(&state.pool)
        .await?;
    if let Some(previous) = previous {
        discard(&state, user.organization_id, previous).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_ensemble_logo(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(ensemble_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    require_manage(&state, &user, ensemble_id).await?;
    let previous = sqlx::query_scalar!("SELECT logo_id FROM ensembles WHERE id = $1", ensemble_id)
        .fetch_one(&state.pool)
        .await?;
    sqlx::query!("UPDATE ensembles SET logo_id = NULL WHERE id = $1", ensemble_id)
        .execute(&state.pool)
        .await?;
    if let Some(previous) = previous {
        discard(&state, user.organization_id, previous).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SymbolInput {
    /// An emoji (or a few characters); null for initials.
    symbol: Option<String>,
    /// "#RRGGBB"; null for the colour derived from the name.
    color: Option<String>,
}

async fn ensemble_symbol(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(ensemble_id): Path<Uuid>,
    Json(input): Json<SymbolInput>,
) -> ApiResult<StatusCode> {
    require_manage(&state, &user, ensemble_id).await?;
    let symbol = input.symbol.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    if symbol.as_ref().is_some_and(|s| s.chars().count() > 4 || s.len() > 16) {
        return Err(ApiError::Invalid(
            "a symbol is one emoji or up to four characters".into(),
        ));
    }
    let color = input.color.filter(|c| !c.is_empty());
    if color
        .as_ref()
        .is_some_and(|c| !(c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|h| h.is_ascii_hexdigit())))
    {
        return Err(ApiError::Invalid("color must be #RRGGBB".into()));
    }
    sqlx::query!(
        "UPDATE ensembles SET symbol = $2, color = $3 WHERE id = $1",
        ensemble_id,
        symbol,
        color
    )
    .execute(&state.pool)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---- people ----

async fn my_avatar(State(state): State<AppState>, user: CurrentUser, multipart: Multipart) -> ApiResult<StatusCode> {
    let org = find_organization(&state, user.organization_id).await?;
    if !org.profile_pictures {
        return Err(ApiError::Conflict(
            "profile pictures are switched off in this organisation".into(),
        ));
    }
    let (bytes, kind) = read_image(multipart).await?;
    let previous = sqlx::query_scalar!("SELECT avatar_id FROM users WHERE id = $1", user.id)
        .fetch_one(&state.pool)
        .await?;
    let id = store(&state, user.organization_id, "user", user.id, None, bytes, kind).await?;
    sqlx::query!("UPDATE users SET avatar_id = $2 WHERE id = $1", user.id, id)
        .execute(&state.pool)
        .await?;
    if let Some(previous) = previous {
        discard(&state, user.organization_id, previous).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_my_avatar(State(state): State<AppState>, user: CurrentUser) -> ApiResult<StatusCode> {
    let previous = sqlx::query_scalar!("SELECT avatar_id FROM users WHERE id = $1", user.id)
        .fetch_one(&state.pool)
        .await?;
    sqlx::query!("UPDATE users SET avatar_id = NULL WHERE id = $1", user.id)
        .execute(&state.pool)
        .await?;
    if let Some(previous) = previous {
        discard(&state, user.organization_id, previous).await?;
    }
    Ok(StatusCode::NO_CONTENT)
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
