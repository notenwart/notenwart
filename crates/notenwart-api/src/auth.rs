// SPDX-License-Identifier: AGPL-3.0-or-later

//! Sign-in over HTTP: the session cookie, the extractor for the signed-in user, and the `/auth` and `/me` routes.

use axum::extract::{FromRequestParts, State};
use axum::http::StatusCode;
use axum::http::request::Parts;
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use notenwart_app::App;
use notenwart_app::auth::{self as app, *};
use notenwart_app::error::AppError;

use crate::error::{ApiError, ApiResult};

/// The cookie that holds the session token.
pub const SESSION_COOKIE: &str = "notenwart_session";

pub fn router() -> Router<App> {
    Router::new()
        .route("/auth/status", get(status))
        .route("/auth/setup", post(setup))
        .route("/auth/login", post(login))
        .route("/auth/magic-link", post(magic_link))
        .route("/auth/token", post(redeem))
        .route("/auth/logout", post(logout))
        .route("/me", get(me).put(update_me))
}

/// The signed-in user of a request – the caller of the app layer's commands. Handlers that take it answer 401
/// without a valid session.
#[derive(Clone, Debug)]
pub struct SignedIn(pub CurrentUser);

impl std::ops::Deref for SignedIn {
    type Target = CurrentUser;

    fn deref(&self) -> &CurrentUser {
        &self.0
    }
}

impl FromRequestParts<App> for SignedIn {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &App) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);
        let token = jar.get(SESSION_COOKIE).ok_or(AppError::Unauthorized)?;
        Ok(SignedIn(current_user(state, token.value()).await?.ok_or(AppError::Unauthorized)?))
    }
}

/// The signed-in user, if any (for routes that are also open without one).
pub async fn from_cookie(state: &App, jar: &CookieJar) -> ApiResult<Option<CurrentUser>> {
    Ok(match jar.get(SESSION_COOKIE) {
        Some(cookie) => current_user(state, cookie.value()).await?,
        None => None,
    })
}

/// Keeps a new session in the browser.
pub fn with_session(state: &App, jar: CookieJar, session: Session) -> (CookieJar, Json<Me>) {
    let cookie = Cookie::build((SESSION_COOKIE, session.token))
        .path("/")
        .http_only(true)
        .secure(state.config.secure_cookies)
        .same_site(SameSite::Lax)
        .max_age(time::Duration::days(SESSION_DAYS))
        .build();
    (jar.add(cookie), Json(session.me))
}

async fn status(State(state): State<App>, jar: CookieJar) -> ApiResult<Json<AuthStatus>> {
    let session = jar.get(SESSION_COOKIE).map(|c| c.value().to_string());
    Ok(Json(app::status(&state, session.as_deref()).await?))
}

async fn setup(
    State(state): State<App>,
    jar: CookieJar,
    Json(input): Json<SetupInput>,
) -> ApiResult<(StatusCode, CookieJar, Json<Me>)> {
    let session = app::setup(&state, input).await?;
    let (jar, me) = with_session(&state, jar, session);
    Ok((StatusCode::CREATED, jar, me))
}

async fn login(
    State(state): State<App>,
    jar: CookieJar,
    Json(input): Json<LoginInput>,
) -> ApiResult<(CookieJar, Json<Me>)> {
    let session = app::login(&state, input).await?;
    Ok(with_session(&state, jar, session))
}

async fn magic_link(State(state): State<App>, Json(input): Json<MagicLinkInput>) -> ApiResult<StatusCode> {
    app::magic_link(&state, input).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn redeem(
    State(state): State<App>,
    jar: CookieJar,
    Json(input): Json<TokenInput>,
) -> ApiResult<(CookieJar, Json<Me>)> {
    let session = app::redeem(&state, input).await?;
    Ok(with_session(&state, jar, session))
}

async fn logout(State(state): State<App>, jar: CookieJar) -> ApiResult<(CookieJar, StatusCode)> {
    if let Some(cookie) = jar.get(SESSION_COOKIE) {
        app::logout(&state, cookie.value()).await?;
    }
    Ok((jar.remove(Cookie::build(SESSION_COOKIE).path("/")), StatusCode::NO_CONTENT))
}

async fn me(user: SignedIn) -> Json<Me> {
    Json(app::me(&user))
}

async fn update_me(State(state): State<App>, user: SignedIn, Json(input): Json<MeInput>) -> ApiResult<Json<Me>> {
    Ok(Json(app::update_me(&state, &user, input).await?))
}
