// SPDX-License-Identifier: AGPL-3.0-or-later
// Needs PostgreSQL: DATABASE_URL (see .env.example); every test gets its own database.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use sqlx::PgPool;

use common::{Client, TestApp};
use notenwart::auth::Role;

#[sqlx::test(migrator = "notenwart::app::MIGRATOR")]
async fn setup_login_logout(pool: PgPool) {
    let app = TestApp::without_users(pool).await;
    let anonymous = app.anonymous();
    let (_, status) = anonymous.json("GET", "/api/auth/status", None).await;
    assert_eq!(status["setupRequired"], true);
    assert_eq!(status["user"], serde_json::Value::Null);
    assert_eq!(status["maxUploadBytes"], 1024 * 1024);

    let setup = json!({ "organizationName": "Musikschule", "name": "Ada", "email": "Ada@Example.org", "password": "correct horse battery" });
    let (status, me, headers) = anonymous
        .json_with_headers("POST", "/api/auth/setup", Some(setup.clone()))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{me}");
    assert_eq!(me["role"], "admin");
    assert_eq!(me["email"], "ada@example.org");
    assert_eq!(me["organization"]["name"], "Musikschule");
    let cookie = headers.get("set-cookie").unwrap().to_str().unwrap();
    assert!(
        cookie.contains("HttpOnly") && cookie.contains("SameSite=Lax"),
        "{cookie}"
    );

    let (status, _) = anonymous.json("POST", "/api/auth/setup", Some(setup)).await;
    assert_eq!(status, StatusCode::CONFLICT);

    let (status, _) = anonymous
        .json(
            "POST",
            "/api/auth/login",
            Some(json!({ "email": "ada@example.org", "password": "wrong password" })),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = anonymous
        .json(
            "POST",
            "/api/auth/login",
            Some(json!({ "email": "nobody@example.org", "password": "whatever123" })),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, me, headers) = anonymous
        .json_with_headers(
            "POST",
            "/api/auth/login",
            Some(json!({ "email": " ADA@example.org", "password": "correct horse battery" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{me}");
    let ada = Client {
        cookie: Client::session_from(&headers),
        ..anonymous.clone()
    };
    let (status, me) = ada.json("GET", "/api/me", None).await;
    assert_eq!((status, me["name"].as_str()), (StatusCode::OK, Some("Ada")));

    let (status, _) = ada.json("POST", "/api/auth/logout", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = ada.json("GET", "/api/me", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrator = "notenwart::app::MIGRATOR")]
async fn invitation_link_signs_in_once_and_allows_setting_a_password(pool: PgPool) {
    let app = TestApp::new(pool).await;
    let (status, invitation) = app
        .admin
        .json(
            "POST",
            "/api/users",
            Some(json!({ "name": "Ben", "email": "ben@example.org", "role": "member" })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{invitation}");
    assert_eq!(invitation["emailed"], false);
    let url = invitation["url"].as_str().unwrap();
    assert!(url.starts_with("http://notenwart.test/auth?token="), "{url}");
    let token = url.split("token=").nth(1).unwrap();

    let anonymous = app.anonymous();
    let (status, me, headers) = anonymous
        .json_with_headers("POST", "/api/auth/token", Some(json!({ "token": token })))
        .await;
    assert_eq!(status, StatusCode::OK, "{me}");
    assert_eq!(me["hasPassword"], false);
    let ben = Client {
        cookie: Client::session_from(&headers),
        ..anonymous.clone()
    };

    let (status, _) = anonymous
        .json("POST", "/api/auth/token", Some(json!({ "token": token })))
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "a token works once");

    let (status, me) = ben
        .json("PUT", "/api/me", Some(json!({ "name": "Ben B.", "password": "short" })))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{me}");
    let (status, me) = ben
        .json(
            "PUT",
            "/api/me",
            Some(json!({ "name": "Ben B.", "password": "a long enough password" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{me}");
    assert_eq!(
        (me["name"].as_str(), me["hasPassword"].as_bool()),
        (Some("Ben B."), Some(true))
    );
    // Changing it again needs the current one.
    let (status, _) = ben
        .json(
            "PUT",
            "/api/me",
            Some(json!({ "name": "Ben B.", "password": "another long password" })),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (status, _) = app
        .admin
        .json(
            "POST",
            "/api/users",
            Some(json!({ "name": "Ben 2", "email": "BEN@example.org", "role": "member" })),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[sqlx::test(migrator = "notenwart::app::MIGRATOR")]
async fn magic_link_answers_the_same_for_unknown_addresses(pool: PgPool) {
    let app = TestApp::new(pool).await;
    let anonymous = app.anonymous();
    for email in ["admin@example.org", "nobody@example.org"] {
        let (status, _) = anonymous
            .json("POST", "/api/auth/magic-link", Some(json!({ "email": email })))
            .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }
    let tokens: i64 = sqlx::query_scalar("SELECT count(*) FROM login_tokens WHERE purpose = 'login'")
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(tokens, 1);
    for _ in 0..2 {
        anonymous
            .json(
                "POST",
                "/api/auth/magic-link",
                Some(json!({ "email": "admin@example.org" })),
            )
            .await;
    }
    let (status, _) = anonymous
        .json(
            "POST",
            "/api/auth/magic-link",
            Some(json!({ "email": "admin@example.org" })),
        )
        .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
}

#[sqlx::test(migrator = "notenwart::app::MIGRATOR")]
async fn login_attempts_are_limited(pool: PgPool) {
    let app = TestApp::new(pool).await;
    let anonymous = app.anonymous();
    let wrong = json!({ "email": "admin@example.org", "password": "not the password" });
    for _ in 0..10 {
        let (status, _) = anonymous.json("POST", "/api/auth/login", Some(wrong.clone())).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    let (status, _) = anonymous.json("POST", "/api/auth/login", Some(wrong)).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
}

#[sqlx::test(migrator = "notenwart::app::MIGRATOR")]
async fn roles_guard_the_api(pool: PgPool) {
    let app = TestApp::new(pool).await;
    let (librarian, _) = app.user(Role::Librarian, "Lib").await;
    let (leader, _) = app.user(Role::Leader, "Lea").await;
    let (member, _) = app.user(Role::Member, "Mo").await;
    let anonymous = app.anonymous();
    let work = json!({ "title": "March" });

    assert_eq!(
        anonymous.json("GET", "/api/works", None).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(member.json("GET", "/api/works", None).await.0, StatusCode::FORBIDDEN);
    assert_eq!(leader.json("GET", "/api/works", None).await.0, StatusCode::OK);
    assert_eq!(
        leader.json("POST", "/api/works", Some(work.clone())).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        librarian.json("POST", "/api/works", Some(work)).await.0,
        StatusCode::CREATED
    );
    assert_eq!(member.json("GET", "/api/instruments", None).await.0, StatusCode::OK);

    let new_user = json!({ "name": "X", "email": "x@example.org", "role": "member" });
    assert_eq!(
        librarian.json("POST", "/api/users", Some(new_user)).await.0,
        StatusCode::FORBIDDEN
    );
    let (status, users) = leader.json("GET", "/api/users", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        users.as_array().unwrap().iter().all(|u| u["email"].is_null()),
        "emails are for admins"
    );
    let (_, users) = app.admin.json("GET", "/api/users", None).await;
    assert!(users.as_array().unwrap().iter().all(|u| u["email"].is_string()));
    assert_eq!(member.json("GET", "/api/users", None).await.0, StatusCode::FORBIDDEN);

    let (_, me) = app.admin.json("GET", "/api/me", None).await;
    let admin_id = me["id"].as_str().unwrap();
    let demote = json!({ "name": "Admin", "email": "admin@example.org", "role": "member" });
    assert_eq!(
        app.admin
            .json("PUT", &format!("/api/users/{admin_id}"), Some(demote))
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        app.admin
            .json("DELETE", &format!("/api/users/{admin_id}"), None)
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[sqlx::test(migrator = "notenwart::app::MIGRATOR")]
async fn cross_origin_changes_are_refused(pool: PgPool) {
    let app = TestApp::new(pool).await;
    let request = |origin: &str| {
        Request::post("/api/works")
            .header("host", "notenwart.test")
            .header("origin", origin)
            .header("content-type", "application/json")
            .body(Body::from(r#"{"title":"X"}"#))
            .unwrap()
    };
    let (status, _, _) = app.admin.send(request("https://evil.example")).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _, _) = app.admin.send(request("http://notenwart.test")).await;
    assert_eq!(status, StatusCode::CREATED);
}
