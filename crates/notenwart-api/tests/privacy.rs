// SPDX-License-Identifier: AGPL-3.0-or-later
// Needs PostgreSQL: DATABASE_URL (see .env.example); every test gets its own database.

mod common;

use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use common::TestApp;
use notenwart_app::auth::Role;

#[sqlx::test(migrator = "notenwart_store::MIGRATOR")]
async fn people_get_their_data_and_deletions_are_recorded(pool: PgPool) {
    let app = TestApp::new(pool.clone()).await;
    let (member, member_id) = app.user(Role::Member, "Mia").await;
    let (other, other_id) = app.user(Role::Member, "Ole").await;
    let (_, ensemble) = app.admin.json("POST", "/api/ensembles", Some(json!({ "name": "Band" }))).await;
    let (status, _) = app
        .admin
        .json(
            "PUT",
            &format!("/api/ensembles/{}/members/{member_id}", ensemble["id"].as_str().unwrap()),
            Some(json!({ "leader": false, "plays": [] })),
        )
        .await;
    assert!(status.is_success());
    let (_, work) = app.admin.json("POST", "/api/works", Some(json!({ "title": "Polka" }))).await;
    let (_, loan) = app
        .admin
        .json(
            "POST",
            &format!("/api/works/{}/loans", work["id"].as_str().unwrap()),
            Some(json!({ "item": "Trompete 1", "borrowerId": member_id, "note": "Mia's copy" })),
        )
        .await;

    // Everyone gets their own data; only admins anybody's.
    let (status, mine) = member.json("GET", "/api/me/export", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(mine["account"]["name"], "Mia");
    assert_eq!(mine["ensembles"][0]["name"], "Band");
    assert_eq!(mine["loans"][0]["item"], "Trompete 1");
    assert_eq!(other.json("GET", &format!("/api/users/{member_id}/export"), None).await.0, StatusCode::FORBIDDEN);
    let (status, theirs) = app.admin.json("GET", &format!("/api/users/{member_id}/export"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(theirs["account"]["name"], "Mia");
    assert!(theirs.to_string().find("password_hash").is_none());

    // Not while parts are out – singly or in bulk.
    let delete = format!("/api/users/{member_id}");
    assert_eq!(app.admin.json("DELETE", &delete, None).await.0, StatusCode::CONFLICT);
    let (_, result) = app
        .admin
        .json("POST", "/api/users/bulk", Some(json!({ "userIds": [member_id, other_id], "action": "delete" })))
        .await;
    assert_eq!(result["done"], 1);
    assert_eq!(result["skipped"][0]["reason"], "has borrowed parts");

    app.admin
        .json(
            "PUT",
            &format!("/api/loans/{}", loan["id"].as_str().unwrap()),
            Some(json!({ "item": "Trompete 1", "borrowerId": member_id, "returnedOn": "2027-01-10" })),
        )
        .await;
    assert_eq!(app.admin.json("DELETE", &delete, None).await.0, StatusCode::NO_CONTENT);

    // The loan stays for the archive, without the name; the record keeps only ids and counts.
    let (_, loans) = app.admin.json("GET", "/api/loans?show=all", None).await;
    assert_eq!(loans[0]["borrowerName"], "–");
    assert_eq!(loans[0]["note"], serde_json::Value::Null);
    assert_eq!(member.json("GET", "/api/deletions", None).await.0, StatusCode::UNAUTHORIZED);
    let (_, log) = app.admin.json("GET", "/api/deletions", None).await;
    assert_eq!(log.as_array().unwrap().len(), 2);
    let entry = log.as_array().unwrap().iter().find(|d| d["userId"] == json!(member_id)).unwrap();
    assert_eq!(entry["deletedBy"], "Admin");
    assert_eq!(entry["records"]["ensembles"], 1);
    assert_eq!(entry["records"]["loans"], 1);
    assert!(!entry.to_string().contains("Mia"));
}
