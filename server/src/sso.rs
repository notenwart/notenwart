// SPDX-License-Identifier: AGPL-3.0-or-later

//! Signing in with the organisation's existing accounts through OpenID Connect (Microsoft 365, Google Workspace,
//! IServ, Keycloak …), configured by the operator (`NOTENWART_SSO_*`). Authorization code flow with PKCE. Only
//! people who already exist in notenwart get in, found by their email address – nobody is created on the way.
//!
//! The ID token comes straight from the provider's token endpoint over TLS, which OpenID Connect Core 1.0
//! (section 3.1.3.7) allows to stand in for checking its signature; issuer, audience, expiry and nonce are checked.

use std::time::Duration;

use axum::Router;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::http::header::LOCATION;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::sync::OnceCell;

use crate::app::AppState;
use crate::auth::{new_token, start_session};

const COOKIE: &str = "notenwart_sso";

#[derive(Debug, Clone)]
pub struct SsoConfig {
    /// The provider's issuer URL, e.g. https://login.microsoftonline.com/<tenant>/v2.0.
    pub issuer: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    /// Shown on the sign-in button, e.g. "Microsoft 365".
    pub name: String,
    /// The ID token claim with the email address.
    pub email_claim: String,
}

#[derive(Debug, Deserialize)]
struct Discovery {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
}

pub struct Sso {
    pub config: SsoConfig,
    http: reqwest::Client,
    discovery: OnceCell<Discovery>,
}

impl Sso {
    pub fn new(config: SsoConfig) -> anyhow::Result<Self> {
        Ok(Self {
            config,
            http: reqwest::Client::builder().timeout(Duration::from_secs(20)).build()?,
            discovery: OnceCell::new(),
        })
    }

    /// The provider's endpoints, fetched once (`/.well-known/openid-configuration`).
    async fn discovery(&self) -> anyhow::Result<&Discovery> {
        self.discovery
            .get_or_try_init(|| async {
                let url = format!(
                    "{}/.well-known/openid-configuration",
                    self.config.issuer.trim_end_matches('/')
                );
                Ok(self.http.get(url).send().await?.error_for_status()?.json().await?)
            })
            .await
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/sso/start", get(start))
        .route("/auth/sso/callback", get(callback))
}

fn redirect(to: &str) -> Response {
    (StatusCode::FOUND, [(LOCATION, to.to_string())]).into_response()
}

/// Only paths of this site, never another host.
fn safe_next(next: Option<&str>) -> String {
    match next {
        Some(next) if next.starts_with('/') && !next.starts_with("//") && !next.contains('\\') => next.to_string(),
        _ => "/".to_string(),
    }
}

fn redirect_uri(state: &AppState) -> String {
    format!("{}/api/auth/sso/callback", state.config.public_url)
}

#[derive(Deserialize)]
struct StartQuery {
    next: Option<String>,
}

async fn start(State(state): State<AppState>, jar: CookieJar, Query(query): Query<StartQuery>) -> Response {
    let Some(sso) = state.sso.clone() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let discovery = match sso.discovery().await {
        Ok(d) => d,
        Err(error) => {
            tracing::error!("single sign-on: cannot reach {}: {error:#}", sso.config.issuer);
            return redirect("/login?sso=failed");
        }
    };
    let (flow_state, _) = new_token();
    let (nonce, _) = new_token();
    let (verifier, _) = new_token();
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let next = URL_SAFE_NO_PAD.encode(safe_next(query.next.as_deref()));
    let cookie = Cookie::build((COOKIE, format!("{flow_state}.{nonce}.{verifier}.{next}")))
        .path("/api/auth/sso")
        .http_only(true)
        .secure(state.config.secure_cookies)
        .same_site(SameSite::Lax)
        .max_age(time::Duration::minutes(10))
        .build();
    let mut url = match reqwest::Url::parse(&discovery.authorization_endpoint) {
        Ok(url) => url,
        Err(_) => return redirect("/login?sso=failed"),
    };
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", &sso.config.client_id)
        .append_pair("redirect_uri", &redirect_uri(&state))
        .append_pair("scope", "openid email profile")
        .append_pair("state", &flow_state)
        .append_pair("nonce", &nonce)
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256");
    (jar.add(cookie), redirect(url.as_str())).into_response()
}

#[derive(Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

async fn callback(State(state): State<AppState>, jar: CookieJar, Query(query): Query<CallbackQuery>) -> Response {
    let Some(sso) = state.sso.clone() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let flow = jar.get(COOKIE).map(|c| c.value().to_string());
    let jar = jar.remove(Cookie::build(COOKIE).path("/api/auth/sso").build());
    match sign_in(&state, &sso, flow, query).await {
        Ok((user_id, next)) => match start_session(&state, jar, user_id).await {
            Ok((jar, _)) => (jar, redirect(&next)).into_response(),
            Err(_) => redirect("/login?sso=failed"),
        },
        Err(reason) => {
            tracing::warn!("single sign-on refused: {reason}");
            let code = if reason == "unknown" { "unknown" } else { "failed" };
            (jar, redirect(&format!("/login?sso={code}"))).into_response()
        }
    }
}

/// The checks of the callback; `Err("unknown")` when the provider vouched for someone notenwart does not know.
async fn sign_in(
    state: &AppState,
    sso: &Sso,
    flow: Option<String>,
    query: CallbackQuery,
) -> Result<(uuid::Uuid, String), String> {
    if let Some(error) = query.error {
        return Err(format!("the provider answered {error}"));
    }
    let flow = flow.ok_or("no sign-in in progress (cookie missing or expired)")?;
    let parts: Vec<&str> = flow.split('.').collect();
    let [expected_state, nonce, verifier, next] = parts[..] else {
        return Err("malformed cookie".into());
    };
    if query.state.as_deref() != Some(expected_state) {
        return Err("state does not match".into());
    }
    let next = URL_SAFE_NO_PAD
        .decode(next)
        .ok()
        .and_then(|n| String::from_utf8(n).ok());
    let next = safe_next(next.as_deref());
    let code = query.code.ok_or("no code")?;
    let discovery = sso.discovery().await.map_err(|e| format!("discovery: {e:#}"))?;

    let mut form = vec![
        ("grant_type", "authorization_code".to_string()),
        ("code", code),
        ("redirect_uri", redirect_uri(state)),
        ("client_id", sso.config.client_id.clone()),
        ("code_verifier", verifier.to_string()),
    ];
    if let Some(secret) = &sso.config.client_secret {
        form.push(("client_secret", secret.clone()));
    }
    let tokens: Value = sso
        .http
        .post(&discovery.token_endpoint)
        .form(&form)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("token endpoint: {e}"))?
        .json()
        .await
        .map_err(|e| format!("token response: {e}"))?;
    let id_token = tokens["id_token"].as_str().ok_or("no id_token")?;
    let claims = claims(id_token).ok_or("unreadable id_token")?;

    if claims["iss"].as_str() != Some(discovery.issuer.as_str()) {
        return Err("wrong issuer".into());
    }
    let audience_ok = match &claims["aud"] {
        Value::String(aud) => aud == &sso.config.client_id,
        Value::Array(list) => list.iter().any(|a| a.as_str() == Some(&sso.config.client_id)),
        _ => false,
    };
    if !audience_ok {
        return Err("wrong audience".into());
    }
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    if claims["exp"].as_i64().is_none_or(|exp| exp < now - 60) {
        return Err("expired id_token".into());
    }
    if claims["nonce"].as_str() != Some(nonce) {
        return Err("nonce does not match".into());
    }
    if claims["email_verified"] == Value::Bool(false) {
        return Err("email not verified by the provider".into());
    }
    let email = claims[sso.config.email_claim.as_str()]
        .as_str()
        .ok_or("no email in the id_token")?
        .trim()
        .to_lowercase();
    let user = sqlx::query_scalar!(
        "SELECT u.id FROM users u JOIN memberships m ON m.user_id = u.id WHERE lower(u.email) = $1",
        email
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| format!("database: {e}"))?;
    let user = user.ok_or("unknown")?;
    Ok((user, next))
}

/// The claims of a JWT, without checking its signature (see the module comment).
fn claims(jwt: &str) -> Option<Value> {
    let payload = jwt.split('.').nth(1)?;
    serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')).ok()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_stays_on_this_site() {
        assert_eq!(safe_next(Some("/works?tag=x")), "/works?tag=x");
        assert_eq!(safe_next(Some("//evil.example")), "/");
        assert_eq!(safe_next(Some("https://evil.example")), "/");
        assert_eq!(safe_next(Some("/\\evil.example")), "/");
        assert_eq!(safe_next(None), "/");
    }

    #[test]
    fn reads_claims() {
        let payload = URL_SAFE_NO_PAD.encode(br#"{"email":"a@b.c"}"#);
        assert_eq!(claims(&format!("e30.{payload}.sig")).unwrap()["email"], "a@b.c");
        assert!(claims("garbage").is_none());
    }
}
