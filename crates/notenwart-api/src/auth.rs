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

#[utoipa::path(
    get,
    path = "/auth/status",
    tag = "auth",
    operation_id = "getAuthStatus",
    summary = "Whether the server needs setup, and who is signed in",
    responses(
        (status = 200, description = "Status.", body = AuthStatus),
    ),
    security(),
)]
async fn status(State(state): State<App>, jar: CookieJar) -> ApiResult<Json<AuthStatus>> {
    let session = jar.get(SESSION_COOKIE).map(|c| c.value().to_string());
    Ok(Json(app::status(&state, session.as_deref()).await?))
}

#[utoipa::path(
    post,
    path = "/auth/setup",
    tag = "auth",
    operation_id = "setup",
    summary = "Create the first admin (only while the server has no users) and sign in",
    request_body = SetupInput,
    responses(
        (status = 201, description = "Set up and signed in.", body = Me),
        (status = 409, response = crate::openapi::Conflict),
        (status = 422, response = crate::openapi::Invalid),
    ),
    security(),
)]
async fn setup(
    State(state): State<App>,
    jar: CookieJar,
    Json(input): Json<SetupInput>,
) -> ApiResult<(StatusCode, CookieJar, Json<Me>)> {
    let session = app::setup(&state, input).await?;
    let (jar, me) = with_session(&state, jar, session);
    Ok((StatusCode::CREATED, jar, me))
}

#[utoipa::path(
    post,
    path = "/auth/login",
    tag = "auth",
    operation_id = "login",
    summary = "Sign in with email and password",
    request_body = LoginInput,
    responses(
        (status = 200, description = "Signed in.", body = Me),
        (status = 401, response = crate::openapi::Unauthorized),
        (status = 429, response = crate::openapi::TooManyRequests),
    ),
    security(),
)]
async fn login(
    State(state): State<App>,
    jar: CookieJar,
    Json(input): Json<LoginInput>,
) -> ApiResult<(CookieJar, Json<Me>)> {
    let session = app::login(&state, input).await?;
    Ok(with_session(&state, jar, session))
}

#[utoipa::path(
    post,
    path = "/auth/magic-link",
    tag = "auth",
    operation_id = "requestMagicLink",
    summary = "Email a one-time sign-in link",
    description = "Answers 204 whether or not the address is known, so it cannot be used to probe for accounts.",
    request_body = inline(MagicLinkInput),
    responses(
        (status = 204, description = "If the address belongs to a user, a link is on its way (valid for 20 minutes)."),
        (status = 429, response = crate::openapi::TooManyRequests),
    ),
    security(),
)]
async fn magic_link(State(state): State<App>, Json(input): Json<MagicLinkInput>) -> ApiResult<StatusCode> {
    app::magic_link(&state, input).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/auth/token",
    tag = "auth",
    operation_id = "redeemToken",
    summary = "Sign in with a one-time token from a sign-in or invitation link",
    request_body = inline(TokenInput),
    responses(
        (status = 200, description = "Signed in; the token is used up.", body = Me),
        (status = 401, response = crate::openapi::Unauthorized),
    ),
    security(),
)]
async fn redeem(
    State(state): State<App>,
    jar: CookieJar,
    Json(input): Json<TokenInput>,
) -> ApiResult<(CookieJar, Json<Me>)> {
    let session = app::redeem(&state, input).await?;
    Ok(with_session(&state, jar, session))
}

#[utoipa::path(
    post,
    path = "/auth/logout",
    tag = "auth",
    operation_id = "logout",
    summary = "Sign out",
    responses(
        (status = 204, description = "Signed out."),
    ),
)]
async fn logout(State(state): State<App>, jar: CookieJar) -> ApiResult<(CookieJar, StatusCode)> {
    if let Some(cookie) = jar.get(SESSION_COOKIE) {
        app::logout(&state, cookie.value()).await?;
    }
    Ok((jar.remove(Cookie::build(SESSION_COOKIE).path("/")), StatusCode::NO_CONTENT))
}

#[utoipa::path(
    get,
    path = "/me",
    tag = "auth",
    operation_id = "getMe",
    summary = "The signed-in user",
    responses(
        (status = 200, description = "The user.", body = Me),
        (status = 401, response = crate::openapi::Unauthorized),
    ),
)]
async fn me(user: SignedIn) -> Json<Me> {
    Json(app::me(&user))
}

#[utoipa::path(
    put,
    path = "/me",
    tag = "auth",
    operation_id = "updateMe",
    summary = "Change own name or password",
    request_body = MeInput,
    responses(
        (status = 200, description = "Updated.", body = Me),
        (status = 401, response = crate::openapi::Unauthorized),
        (status = 422, response = crate::openapi::Invalid),
    ),
)]
async fn update_me(State(state): State<App>, user: SignedIn, Json(input): Json<MeInput>) -> ApiResult<Json<Me>> {
    Ok(Json(app::update_me(&state, &user, input).await?))
}
