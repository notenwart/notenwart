// SPDX-License-Identifier: AGPL-3.0-or-later
// Needs PostgreSQL: DATABASE_URL (see .env.example); every test gets its own database.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{Value, json};
use sqlx::PgPool;

use common::{Client, TestApp};
use notenwart::auth::Role;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0\x1f\x15\xc4\x89\0\0\0\rIDATx\x9cc\xf8\x0f\0\0\x01\x01\0\x05\x18\xd8N\0\0\0\0IEND\xaeB`\x82";

async fn put_image(client: &Client, uri: &str, bytes: &[u8]) -> StatusCode {
    let boundary = "notenwart-test-boundary";
    let mut body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"i.png\"\r\nContent-Type: image/png\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let request = Request::put(uri)
        .header("content-type", format!("multipart/form-data; boundary={boundary}"))
        .body(Body::from(body))
        .unwrap();
    client.send(request).await.0
}

#[sqlx::test(migrator = "notenwart::app::MIGRATOR")]
async fn logos_symbols_and_pictures(pool: PgPool) {
    let app = TestApp::new(pool).await;
    let (leader, leader_id) = app.user(Role::Leader, "Lea").await;
    let (mia, mia_id) = app.user(Role::Member, "Mia").await;
    let (ole, _) = app.user(Role::Member, "Ole").await;

    // The organisation's logo: admins set it, everyone sees it, also before signing in.
    assert_eq!(
        put_image(&mia, "/api/organization/logo", PNG).await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        put_image(&app.admin, "/api/organization/logo", PNG).await,
        StatusCode::OK
    );
    let (_, status) = app.anonymous().json("GET", "/api/auth/status", None).await;
    let logo = status["organization"]["logoUrl"].as_str().unwrap().to_string();
    let (code, bytes) = app.anonymous().get_bytes(&logo).await;
    assert_eq!((code, bytes.as_slice()), (StatusCode::OK, PNG));
    let svg = b"<svg xmlns=\"http://www.w3.org/2000/svg\"><script>alert(1)</script></svg>";
    assert_eq!(
        put_image(&app.admin, "/api/organization/logo", svg).await,
        StatusCode::UNPROCESSABLE_ENTITY
    );

    // An ensemble: a symbol on a colour, or a logo.
    let (_, ensemble) = app
        .admin
        .json("POST", "/api/ensembles", Some(json!({ "name": "Kammerchor" })))
        .await;
    let id = ensemble["id"].as_str().unwrap();
    for (user, leads) in [(&leader_id, true), (&mia_id, false)] {
        app.admin
            .json(
                "PUT",
                &format!("/api/ensembles/{id}/members/{user}"),
                Some(json!({ "leader": leads, "plays": [] })),
            )
            .await;
    }
    let (status, _) = leader
        .json(
            "PUT",
            &format!("/api/ensembles/{id}/symbol"),
            Some(json!({ "symbol": "🎶", "color": "#1D2B45" })),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = leader
        .json(
            "PUT",
            &format!("/api/ensembles/{id}/symbol"),
            Some(json!({ "symbol": "🎶", "color": "red" })),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        put_image(&leader, &format!("/api/ensembles/{id}/logo"), PNG).await,
        StatusCode::NO_CONTENT
    );
    let (_, music) = mia.json("GET", "/api/my/music", None).await;
    assert_eq!(music[0]["symbol"], "🎶");
    assert!(music[0]["logoUrl"].is_string());

    // A person's picture: they, staff and their leaders see it; other members do not.
    assert_eq!(put_image(&mia, "/api/me/avatar", PNG).await, StatusCode::NO_CONTENT);
    let (_, me) = mia.json("GET", "/api/me", None).await;
    let avatar = me["avatarUrl"].as_str().unwrap().to_string();
    assert_eq!(mia.get_bytes(&avatar).await.0, StatusCode::OK);
    assert_eq!(leader.get_bytes(&avatar).await.0, StatusCode::OK);
    assert_eq!(app.admin.get_bytes(&avatar).await.0, StatusCode::OK);
    assert_eq!(ole.get_bytes(&avatar).await.0, StatusCode::NOT_FOUND);
    assert_eq!(app.anonymous().get_bytes(&avatar).await.0, StatusCode::NOT_FOUND);

    // Switched off by the organisation: no pictures shown, none uploaded.
    let (status, organization) = app
        .admin
        .json(
            "PUT",
            "/api/organization",
            Some(json!({ "name": "Musikschule", "profilePictures": false })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(organization["profilePictures"], false);
    let (_, me) = mia.json("GET", "/api/me", None).await;
    assert_eq!(me["avatarUrl"], Value::Null);
    let (_, people) = app.admin.json("GET", "/api/users", None).await;
    assert!(people.as_array().unwrap().iter().all(|p| p["avatarUrl"].is_null()));
    assert_eq!(put_image(&mia, "/api/me/avatar", PNG).await, StatusCode::CONFLICT);
}
