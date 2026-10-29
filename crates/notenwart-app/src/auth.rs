// SPDX-License-Identifier: AGPL-3.0-or-later

//! Sign-in: first-run setup, email and password, one-time links (sign-in and invitation), sessions and roles.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use argon2::Argon2;
use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub use notenwart_core::Role;

use crate::App;
use crate::error::{AppError, AppResult};

/// How long a session lasts without being used.
pub const SESSION_DAYS: i64 = 30;
const LOGIN_LINK_MINUTES: i64 = 20;
pub const INVITATION_DAYS: i64 = 7;
const MIN_PASSWORD: usize = 10;

// ---- the current user ----

/// Who asks: the signed-in user every command and query is done for. An interface finds them (the API by the
/// session cookie, see [`current_user`]) and hands them to the command, which checks the role.
#[derive(Clone, Debug)]
pub struct CurrentUser {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub role: Role,
    pub has_password: bool,
    pub organization_id: Uuid,
    pub organization_name: String,
    /// The person's picture (none when the organisation switched pictures off).
    pub avatar_id: Option<Uuid>,
    pub organization_logo_id: Option<Uuid>,
    pub profile_pictures: bool,
    /// The server's operator at the console rather than a person of the organisation (see [`CurrentUser::operator`]).
    pub operator: bool,
}

impl CurrentUser {
    /// The server's operator at the console (`notenwart tui`), acting as an admin of one organisation. They are no
    /// person in it: no account, no session, `id` is the nil UUID that matches nobody, and what they do is recorded
    /// without a name ([`CurrentUser::actor`]). The role checks of every command apply to them as to any admin.
    pub fn operator(organization_id: Uuid, organization_name: String) -> Self {
        CurrentUser {
            id: Uuid::nil(),
            name: "operator".into(),
            email: String::new(),
            role: Role::Admin,
            has_password: false,
            organization_id,
            organization_name,
            avatar_id: None,
            organization_logo_id: None,
            profile_pictures: false,
            operator: true,
        }
    }

    /// Who is recorded as having done something (created by, deleted by …): the person, or nobody for the operator.
    pub fn actor(&self) -> Option<Uuid> {
        (!self.operator).then_some(self.id)
    }

    pub fn is_admin(&self) -> bool {
        self.role == Role::Admin
    }

    /// Admins and librarians: change the library, manage every ensemble.
    pub fn is_staff(&self) -> bool {
        matches!(self.role, Role::Admin | Role::Librarian)
    }

    /// Everyone but plain members may browse the whole library.
    pub fn reads_library(&self) -> bool {
        self.role != Role::Member
    }

    pub fn require(&self, allowed: bool) -> AppResult<()> {
        if allowed { Ok(()) } else { Err(AppError::Forbidden) }
    }
}

async fn user_for_session(state: &App, token_hash: &[u8]) -> AppResult<Option<CurrentUser>> {
    let row = sqlx::query!(
        r#"SELECT u.id, u.display_name, u.email, m.role AS "role: Role", u.password_hash IS NOT NULL AS "has_password!",
                  o.id AS organization_id, o.name AS organization_name, u.avatar_id, o.logo_id, o.profile_pictures,
                  s.last_seen_at < now() - interval '1 hour' AS "stale!"
           FROM sessions s
           JOIN users u ON u.id = s.user_id
           JOIN memberships m ON m.user_id = s.user_id AND m.organization_id = s.organization_id
           JOIN organizations o ON o.id = s.organization_id
           WHERE s.token_hash = $1 AND s.expires_at > now()"#,
        token_hash
    )
    .fetch_optional(&state.pool)
    .await?;
    let Some(row) = row else { return Ok(None) };
    // Sliding expiry, written at most once an hour per session.
    if row.stale {
        sqlx::query!(
            "UPDATE sessions SET last_seen_at = now(), expires_at = now() + make_interval(days => $2)
             WHERE token_hash = $1",
            token_hash,
            SESSION_DAYS as i32
        )
        .execute(&state.pool)
        .await?;
    }
    Ok(Some(CurrentUser {
        id: row.id,
        name: row.display_name,
        email: row.email,
        role: row.role,
        has_password: row.has_password,
        organization_id: row.organization_id,
        organization_name: row.organization_name,
        avatar_id: row.avatar_id.filter(|_| row.profile_pictures),
        organization_logo_id: row.logo_id,
        profile_pictures: row.profile_pictures,
        operator: false,
    }))
}

/// The signed-in user behind a session token, if it is valid.
pub async fn current_user(state: &App, token: &str) -> AppResult<Option<CurrentUser>> {
    user_for_session(state, &hash_token(token)).await
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Me {
    id: Uuid,
    name: String,
    email: String,
    role: Role,
    /// False after an invitation until the user sets a password.
    has_password: bool,
    /// Path of the user's picture, if any (and allowed).
    avatar_url: Option<String>,
    #[schema(inline)]
    organization: OrganizationRef,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationRef {
    id: Uuid,
    name: String,
    logo_url: Option<String>,
    /// People may show a picture.
    profile_pictures: bool,
}

impl From<CurrentUser> for Me {
    fn from(user: CurrentUser) -> Self {
        Me {
            id: user.id,
            name: user.name,
            email: user.email,
            role: user.role,
            has_password: user.has_password,
            avatar_url: crate::images::url(user.avatar_id),
            organization: OrganizationRef {
                id: user.organization_id,
                name: user.organization_name,
                logo_url: crate::images::url(user.organization_logo_id),
                profile_pictures: user.profile_pictures,
            },
        }
    }
}

// ---- tokens, passwords, sessions ----

/// A random token for a cookie or link, and the hash that is stored.
pub fn new_token() -> (String, Vec<u8>) {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("the operating system's random number generator failed");
    let token = URL_SAFE_NO_PAD.encode(bytes);
    let hash = hash_token(&token);
    (token, hash)
}

pub fn hash_token(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

pub fn normalize_email(email: &str) -> AppResult<String> {
    let email = email.trim().to_lowercase();
    let valid = email.len() <= 254
        && email
            .split_once('@')
            .is_some_and(|(local, domain)| !local.is_empty() && domain.contains('.') && !domain.starts_with('.'))
        && !email.contains(char::is_whitespace);
    if valid { Ok(email) } else { Err(AppError::Invalid("not a valid email address".into())) }
}

pub fn check_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if (1..=120).contains(&name.chars().count()) {
        Ok(name.to_string())
    } else {
        Err(AppError::Invalid("name must have 1 to 120 characters".into()))
    }
}

async fn hash_password(password: String) -> AppResult<String> {
    if password.chars().count() < MIN_PASSWORD || password.len() > 1024 {
        return Err(AppError::Invalid(format!("the password needs at least {MIN_PASSWORD} characters")));
    }
    tokio::task::spawn_blocking(move || {
        Argon2::default()
            .hash_password(password.as_bytes())
            .map(|hash| hash.to_string())
            .map_err(|e| anyhow::anyhow!("hashing failed: {e}"))
    })
    .await?
    .map_err(AppError::from)
}

async fn verify_password(password: String, hash: Option<String>) -> AppResult<bool> {
    // Without a stored hash, verify against a dummy anyway so unknown accounts take as long as known ones.
    static DUMMY: LazyLock<String> = LazyLock::new(|| {
        let (random, _) = new_token();
        Argon2::default().hash_password(random.as_bytes()).map(|h| h.to_string()).unwrap_or_default()
    });
    let known = hash.is_some();
    let hash = hash.unwrap_or_else(|| DUMMY.clone());
    let ok = tokio::task::spawn_blocking(move || {
        PasswordHash::new(&hash)
            .map(|parsed| Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
            .unwrap_or(false)
    })
    .await?;
    Ok(known && ok)
}

/// A new session: the token the interface keeps (the API in a cookie) and who it belongs to.
#[derive(Debug)]
pub struct Session {
    pub token: String,
    pub me: Me,
}

pub(crate) async fn start_session(state: &App, user_id: Uuid) -> AppResult<Session> {
    let organization_id = sqlx::query_scalar!(
        "SELECT organization_id FROM memberships WHERE user_id = $1 ORDER BY created_at LIMIT 1",
        user_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::Unauthorized)?;
    let (token, hash) = new_token();
    sqlx::query!(
        "INSERT INTO sessions (token_hash, user_id, organization_id, expires_at)
         VALUES ($1, $2, $3, now() + make_interval(days => $4))",
        hash,
        user_id,
        organization_id,
        SESSION_DAYS as i32
    )
    .execute(&state.pool)
    .await?;
    sqlx::query!("UPDATE users SET last_login_at = now() WHERE id = $1", user_id).execute(&state.pool).await?;
    let user = user_for_session(state, &hash).await?.ok_or(AppError::Unauthorized)?;
    Ok(Session { token, me: user.into() })
}

/// Creates a one-time link token for a user; returns the link and its expiry.
pub async fn create_link_token(
    state: &App,
    user_id: Uuid,
    invitation: bool,
) -> AppResult<(String, time::OffsetDateTime)> {
    let (token, hash) = new_token();
    let (purpose, minutes) = if invitation {
        // A new invitation replaces older ones.
        sqlx::query!(
            "DELETE FROM login_tokens WHERE user_id = $1 AND purpose = 'invitation' AND used_at IS NULL",
            user_id
        )
        .execute(&state.pool)
        .await?;
        ("invitation", INVITATION_DAYS * 24 * 60)
    } else {
        ("login", LOGIN_LINK_MINUTES)
    };
    let expires_at = sqlx::query_scalar!(
        "INSERT INTO login_tokens (token_hash, user_id, purpose, expires_at)
         VALUES ($1, $2, $3::text::token_purpose, now() + make_interval(mins => $4)) RETURNING expires_at",
        hash,
        user_id,
        purpose,
        minutes as i32
    )
    .fetch_one(&state.pool)
    .await?;
    Ok((format!("{}/auth?token={token}", state.config.public_url), expires_at))
}

/// Counts attempts per key in a sliding window; in memory, so it resets with the server.
#[derive(Debug, Default)]
pub struct RateLimiter {
    hits: Mutex<HashMap<String, (u32, Instant)>>,
}

impl RateLimiter {
    /// Records an attempt; false once `max` attempts happened within `window`.
    pub fn allow(&self, key: &str, max: u32, window: Duration) -> bool {
        let mut hits = self.hits.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        hits.retain(|_, (_, start)| now.duration_since(*start) < window);
        let entry = hits.entry(key.to_string()).or_insert((0, now));
        entry.0 += 1;
        entry.0 <= max
    }

    pub fn reset(&self, key: &str) {
        self.hits.lock().unwrap_or_else(|e| e.into_inner()).remove(key);
    }
}

// ---- handlers ----

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    /// The server has no users yet; `/auth/setup` creates the first admin.
    setup_required: bool,
    /// The server can send email (sign-in links, invitations).
    mail_enabled: bool,
    /// Largest PDF the server accepts in one upload.
    max_upload_bytes: usize,
    recognition: crate::recognize::Abilities,
    /// Name of the identity provider to sign in with, if one is set up.
    sso: Option<String>,
    /// The organisation's name and logo, for the sign-in page.
    #[schema(inline)]
    organization: Option<PublicOrganization>,
    user: Option<Me>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublicOrganization {
    name: String,
    logo_url: Option<String>,
}

/// What the sign-in page needs to know, and who is signed in with `session` (a session token), if anyone.
pub async fn status(state: &App, session: Option<&str>) -> AppResult<AuthStatus> {
    let organization = sqlx::query!("SELECT name, logo_id FROM organizations ORDER BY created_at LIMIT 1")
        .fetch_optional(&state.pool)
        .await?
        .map(|o| PublicOrganization { name: o.name, logo_url: crate::images::url(o.logo_id) });
    let setup_required =
        !sqlx::query_scalar!(r#"SELECT EXISTS (SELECT 1 FROM users) AS "any!""#).fetch_one(&state.pool).await?;
    let user = match session {
        Some(token) => user_for_session(state, &hash_token(token)).await?,
        None => None,
    };
    Ok(AuthStatus {
        setup_required,
        mail_enabled: state.mailer.is_some(),
        max_upload_bytes: state.config.max_upload,
        recognition: state.recognizer.abilities,
        sso: state.sso.as_ref().map(|s| s.config.name.clone()),
        organization,
        user: user.map(Me::from),
    })
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetupInput {
    organization_name: String,
    name: String,
    email: String,
    password: String,
}

/// The first start: names the organisation and creates the first admin, who is signed in.
pub async fn setup(state: &App, input: SetupInput) -> AppResult<Session> {
    let organization_name = check_name(&input.organization_name)?;
    let name = check_name(&input.name)?;
    let email = normalize_email(&input.email)?;
    let password_hash = hash_password(input.password).await?;

    let mut tx = state.pool.begin().await?;
    // Serialises concurrent setups: only the first one finds no users.
    sqlx::query!("LOCK TABLE users IN EXCLUSIVE MODE").execute(&mut *tx).await?;
    if sqlx::query_scalar!(r#"SELECT EXISTS (SELECT 1 FROM users) AS "any!""#).fetch_one(&mut *tx).await? {
        return Err(AppError::Conflict("this server is already set up".into()));
    }
    let organization_id =
        sqlx::query_scalar!("SELECT id FROM organizations ORDER BY created_at LIMIT 1").fetch_one(&mut *tx).await?;
    sqlx::query!("UPDATE organizations SET name = $2 WHERE id = $1", organization_id, organization_name)
        .execute(&mut *tx)
        .await?;
    let user_id = sqlx::query_scalar!(
        "INSERT INTO users (email, display_name, password_hash) VALUES ($1, $2, $3) RETURNING id",
        email,
        name,
        password_hash
    )
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query!(
        "INSERT INTO memberships (organization_id, user_id, role) VALUES ($1, $2, 'admin')",
        organization_id,
        user_id
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    tracing::info!("set up: first admin created");

    start_session(state, user_id).await
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct LoginInput {
    email: String,
    password: String,
}

pub async fn login(state: &App, input: LoginInput) -> AppResult<Session> {
    let email = input.email.trim().to_lowercase();
    let key = format!("login:{email}");
    if !state.limiter.allow(&key, 10, Duration::from_secs(15 * 60)) {
        return Err(AppError::TooManyRequests);
    }
    let user = sqlx::query!("SELECT id, password_hash FROM users WHERE lower(email) = $1", email)
        .fetch_optional(&state.pool)
        .await?;
    let (user_id, hash) = match user {
        Some(user) => (Some(user.id), user.password_hash),
        None => (None, None),
    };
    if !verify_password(input.password, hash).await? {
        return Err(AppError::Unauthorized);
    }
    let user_id = user_id.ok_or(AppError::Unauthorized)?;
    state.limiter.reset(&key);
    start_session(state, user_id).await
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct MagicLinkInput {
    email: String,
}

/// Sends a one-time sign-in link, if the address belongs to someone; says nothing about whether it does.
pub async fn magic_link(state: &App, input: MagicLinkInput) -> AppResult<()> {
    let email = input.email.trim().to_lowercase();
    if !state.limiter.allow(&format!("magic:{email}"), 3, Duration::from_secs(15 * 60)) {
        return Err(AppError::TooManyRequests);
    }
    let user = sqlx::query!("SELECT id, display_name FROM users WHERE lower(email) = $1", email)
        .fetch_optional(&state.pool)
        .await?;
    if let Some(user) = user {
        let (url, _) = create_link_token(state, user.id, false).await?;
        let mail = crate::mail::sign_in_link(state.config.language, &user.display_name, &url, LOGIN_LINK_MINUTES);
        crate::mail::send_or_log(state, &email, &mail.subject, &mail.body).await;
    }
    Ok(())
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct TokenInput {
    token: String,
}

/// Signs in with a one-time link.
pub async fn redeem(state: &App, input: TokenInput) -> AppResult<Session> {
    let user_id = sqlx::query_scalar!(
        "UPDATE login_tokens SET used_at = now()
         WHERE token_hash = $1 AND used_at IS NULL AND expires_at > now() RETURNING user_id",
        hash_token(input.token.trim())
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::Unauthorized)?;
    start_session(state, user_id).await
}

/// Ends the session with this token.
pub async fn logout(state: &App, session: &str) -> AppResult<()> {
    sqlx::query!("DELETE FROM sessions WHERE token_hash = $1", hash_token(session)).execute(&state.pool).await?;
    Ok(())
}

/// The signed-in user as the web app shows them.
pub fn me(user: &CurrentUser) -> Me {
    user.clone().into()
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MeInput {
    name: String,
    /// New password; null keeps the current one.
    password: Option<String>,
    /// Required to change an existing password.
    current_password: Option<String>,
}

pub async fn update_me(state: &App, user: &CurrentUser, input: MeInput) -> AppResult<Me> {
    let name = check_name(&input.name)?;
    if let Some(password) = input.password.filter(|p| !p.is_empty()) {
        if user.has_password {
            let hash = sqlx::query_scalar!("SELECT password_hash FROM users WHERE id = $1", user.id)
                .fetch_one(&state.pool)
                .await?;
            let current = input.current_password.unwrap_or_default();
            if !verify_password(current, hash).await? {
                return Err(AppError::Invalid("the current password is wrong".into()));
            }
        }
        let hash = hash_password(password).await?;
        sqlx::query!("UPDATE users SET password_hash = $2 WHERE id = $1", user.id, hash).execute(&state.pool).await?;
    }
    sqlx::query!("UPDATE users SET display_name = $2 WHERE id = $1", user.id, name).execute(&state.pool).await?;
    let has_password =
        sqlx::query_scalar!(r#"SELECT password_hash IS NOT NULL AS "has!" FROM users WHERE id = $1"#, user.id)
            .fetch_one(&state.pool)
            .await?;
    Ok(Me::from(CurrentUser { name, has_password, ..user.clone() }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emails_are_normalized_and_checked() {
        assert_eq!(normalize_email(" Anna@Example.ORG ").unwrap(), "anna@example.org");
        for bad in ["", "anna", "@example.org", "anna@example", "an na@example.org", "anna@.org"] {
            assert!(normalize_email(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn tokens_are_random_and_hashed() {
        let (a, hash) = new_token();
        let (b, _) = new_token();
        assert_ne!(a, b);
        assert_eq!(a.len(), 43);
        assert_eq!(hash, hash_token(&a));
    }

    #[test]
    fn limiter_blocks_after_max() {
        let limiter = RateLimiter::default();
        let window = Duration::from_secs(60);
        assert!(limiter.allow("k", 2, window));
        assert!(limiter.allow("k", 2, window));
        assert!(!limiter.allow("k", 2, window));
        limiter.reset("k");
        assert!(limiter.allow("k", 2, window));
    }
}
