// SPDX-License-Identifier: AGPL-3.0-or-later
// Needs PostgreSQL: DATABASE_URL (see .env.example); every test gets its own database. The identity provider is a
// small stand-in server on localhost.

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Form, State};
use axum::http::{Request, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde_json::{Value, json};
use sqlx::PgPool;

use common::{Client, TestApp};
use notenwart::auth::Role;
use notenwart::sso::{Sso, SsoConfig};

/// A provider whose codes say what the ID token should contain: "<nonce>|<email>|<verified>".
async fn provider() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new()
        .route(
            "/.well-known/openid-configuration",
            get(|State(base): State<String>| async move {
                Json(json!({
                    "issuer": base,
                    "authorization_endpoint": format!("{base}/authorize"),
                    "token_endpoint": format!("{base}/token"),
                }))
            }),
        )
        .route(
            "/token",
            post(
                |State(base): State<String>, Form(form): Form<std::collections::HashMap<String, String>>| async move {
                    assert!(form.contains_key("code_verifier"), "PKCE");
                    assert_eq!(form["client_id"], "notenwart");
                    let parts: Vec<&str> = form["code"].split('|').collect();
                    let claims = json!({
                        "iss": base, "aud": "notenwart", "exp": time::OffsetDateTime::now_utc().unix_timestamp() + 300,
                        "nonce": parts[0], "email": parts[1], "email_verified": parts[2] == "true",
                    });
                    let token = format!("e30.{}.signature", URL_SAFE_NO_PAD.encode(claims.to_string()));
                    Json(json!({ "id_token": token, "access_token": "x", "token_type": "Bearer" }))
                },
            ),
        )
        .with_state(base.clone());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    base
}

fn cookie(headers: &axum::http::HeaderMap, name: &str) -> Option<String> {
    headers.get_all("set-cookie").iter().find_map(|v| {
        let v = v.to_str().ok()?;
        let value = v.strip_prefix(&format!("{name}="))?.split(';').next()?;
        (!value.is_empty()).then(|| value.to_string())
    })
}

fn param(url: &str, name: &str) -> String {
    let url = reqwest::Url::parse(url).unwrap();
    url.query_pairs().find(|(k, _)| k == name).unwrap().1.to_string()
}

/// Starts a sign-in and answers it like the provider would; returns where the browser is sent and its cookies.
async fn sign_in(client: &Client, email: &str, verified: bool, tamper: bool, next: &str) -> (String, Option<String>) {
    let (status, _, headers) = client
        .send(
            Request::get(format!("/api/auth/sso/start?next={next}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(status, StatusCode::FOUND);
    let authorize = headers["location"].to_str().unwrap().to_string();
    assert!(authorize.contains("code_challenge_method=S256"));
    let flow = cookie(&headers, "notenwart_sso").expect("flow cookie");
    let (state, nonce) = (param(&authorize, "state"), param(&authorize, "nonce"));
    let nonce = if tamper { "other".to_string() } else { nonce };
    let code = format!("{nonce}|{email}|{verified}");
    let request = Request::get(format!(
        "/api/auth/sso/callback?code={}&state={state}",
        percent_encoding::utf8_percent_encode(&code, percent_encoding::NON_ALPHANUMERIC)
    ))
    .header("cookie", format!("notenwart_sso={flow}"))
    .body(Body::empty())
    .unwrap();
    let (status, _, headers) = client.send(request).await;
    assert_eq!(status, StatusCode::FOUND);
    (
        headers["location"].to_str().unwrap().to_string(),
        cookie(&headers, "notenwart_session"),
    )
}

#[sqlx::test(migrator = "notenwart::app::MIGRATOR")]
async fn people_sign_in_with_the_organisations_accounts(pool: PgPool) {
    let issuer = provider().await;
    let sso = Sso::new(SsoConfig {
        issuer,
        client_id: "notenwart".into(),
        client_secret: Some("secret".into()),
        name: "Schulkonto".into(),
        email_claim: "email".into(),
    })
    .unwrap();
    let mut app = TestApp::with_sso(pool, Some(Arc::new(sso))).await;
    app.admin = app.user(Role::Admin, "Admin").await.0;
    app.user(Role::Member, "Mia").await;
    let anonymous = app.anonymous();

    let (_, status) = anonymous.json("GET", "/api/auth/status", None).await;
    assert_eq!(status["sso"], "Schulkonto");

    // Known and verified: signed in, sent where they wanted to go.
    let (location, session) = sign_in(&anonymous, "Mia@Example.org", true, false, "/works").await;
    assert_eq!(location, "/works");
    let me = Client {
        cookie: session,
        ..anonymous.clone()
    };
    let (_, who) = me.json("GET", "/api/me", None).await;
    assert_eq!(who["name"], "Mia");

    // Nobody is created, unverified addresses and replayed nonces are refused, and nobody is sent off-site.
    let (location, session) = sign_in(&anonymous, "stranger@example.org", true, false, "/").await;
    assert_eq!((location.as_str(), session), ("/login?sso=unknown", None));
    let (location, session) = sign_in(&anonymous, "mia@example.org", false, false, "/").await;
    assert_eq!((location.as_str(), session), ("/login?sso=failed", None));
    let (location, session) = sign_in(&anonymous, "mia@example.org", true, true, "/").await;
    assert_eq!((location.as_str(), session), ("/login?sso=failed", None));
    let (location, _) = sign_in(&anonymous, "mia@example.org", true, false, "//evil.example").await;
    assert_eq!(location, "/");

    // Without the flow cookie (another browser, or forged): refused.
    let (status, _, headers) = anonymous
        .send(
            Request::get("/api/auth/sso/callback?code=x&state=y")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(status, StatusCode::FOUND);
    assert_eq!(headers["location"], "/login?sso=failed");
}

#[sqlx::test(migrator = "notenwart::app::MIGRATOR")]
async fn without_configuration_there_is_no_sso(pool: PgPool) {
    let app = TestApp::new(pool).await;
    let (_, status) = app.anonymous().json("GET", "/api/auth/status", None).await;
    assert_eq!(status["sso"], Value::Null);
    let (status, _, _) = app
        .anonymous()
        .send(Request::get("/api/auth/sso/start").body(Body::empty()).unwrap())
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
