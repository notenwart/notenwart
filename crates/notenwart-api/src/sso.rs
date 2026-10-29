// SPDX-License-Identifier: AGPL-3.0-or-later

//! Single sign-on over HTTP: redirects to and from the provider, with the flow kept in a short-lived cookie.

use axum::Router;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::http::header::LOCATION;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use notenwart_app::App;
use notenwart_app::sso::{self as app, Callback, CallbackQuery, Start, StartQuery};

const COOKIE: &str = "notenwart_sso";

pub fn router() -> Router<App> {
    Router::new().route("/auth/sso/start", get(start)).route("/auth/sso/callback", get(callback))
}

fn redirect(to: &str) -> Response {
    (StatusCode::FOUND, [(LOCATION, to.to_string())]).into_response()
}

async fn start(State(state): State<App>, jar: CookieJar, Query(query): Query<StartQuery>) -> Response {
    match app::start(&state, query).await {
        Start::NotConfigured => StatusCode::NOT_FOUND.into_response(),
        Start::Failed => redirect("/login?sso=failed"),
        Start::Provider { url, flow } => {
            let cookie = Cookie::build((COOKIE, flow))
                .path("/api/auth/sso")
                .http_only(true)
                .secure(state.config.secure_cookies)
                .same_site(SameSite::Lax)
                .max_age(time::Duration::minutes(10))
                .build();
            (jar.add(cookie), redirect(&url)).into_response()
        }
    }
}

async fn callback(State(state): State<App>, jar: CookieJar, Query(query): Query<CallbackQuery>) -> Response {
    if state.sso.is_none() {
        return StatusCode::NOT_FOUND.into_response();
    }
    let flow = jar.get(COOKIE).map(|c| c.value().to_string());
    let jar = jar.remove(Cookie::build(COOKIE).path("/api/auth/sso").build());
    match app::callback(&state, flow, query).await {
        Callback::NotConfigured => StatusCode::NOT_FOUND.into_response(),
        Callback::SignedIn { session, next } => {
            let (jar, _) = crate::auth::with_session(&state, jar, session);
            (jar, redirect(&next)).into_response()
        }
        Callback::NoSession => redirect("/login?sso=failed"),
        Callback::Refused { code } => (jar, redirect(&format!("/login?sso={code}"))).into_response(),
    }
}
