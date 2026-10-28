// SPDX-License-Identifier: AGPL-3.0-or-later
// Needs PostgreSQL: DATABASE_URL (see .env.example); every test gets its own database.

mod common;

use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use common::TestApp;
use notenwart::auth::Role;

#[sqlx::test(migrator = "notenwart::app::MIGRATOR")]
async fn bulk_actions_on_people(pool: PgPool) {
    let app = TestApp::new(pool).await;
    let mut ids = Vec::new();
    for name in ["Ana", "Bo", "Cy"] {
        ids.push(app.user(Role::Member, name).await.1);
    }
    let (_, me) = app.admin.json("GET", "/api/me", None).await;
    let admin_id = me["id"].as_str().unwrap().to_string();
    let bulk = |body: serde_json::Value| app.admin.json("POST", "/api/users/bulk", Some(body));

    // Roles, never one's own.
    let (status, result) =
        bulk(json!({ "userIds": [ids[0], ids[1], admin_id], "action": "setRole", "role": "leader" })).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["done"], 2);
    assert_eq!(result["skipped"][0]["reason"], "not yourself");

    // Into an ensemble, playing trumpet 2.
    let (_, ensemble) = app
        .admin
        .json("POST", "/api/ensembles", Some(json!({ "name": "Band" })))
        .await;
    let ensemble_id = ensemble["id"].as_str().unwrap();
    let (_, instruments) = app.admin.json("GET", "/api/instruments", None).await;
    let trumpet = instruments
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["name"] == "Trumpet")
        .unwrap()["id"]
        .clone();
    let (status, result) = bulk(json!({ "userIds": ids, "action": "addToEnsemble", "ensembleId": ensemble_id, "instrumentId": trumpet, "voice": 2 })).await;
    assert_eq!((status, result["done"].as_i64()), (StatusCode::OK, Some(3)));
    let (_, detail) = app
        .admin
        .json("GET", &format!("/api/ensembles/{ensemble_id}"), None)
        .await;
    assert_eq!(detail["members"].as_array().unwrap().len(), 3);
    assert_eq!(detail["members"][0]["plays"][0]["voice"], 2);
    let (_, users) = app.admin.json("GET", "/api/users", None).await;
    let ana = users
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["name"] == "Ana")
        .unwrap()
        .clone();
    assert_eq!(ana["role"], "leader");
    assert_eq!(ana["ensembles"][0]["name"], "Band");

    // Out again, new links, delete.
    bulk(json!({ "userIds": [ids[2]], "action": "removeFromEnsemble", "ensembleId": ensemble_id })).await;
    let (_, detail) = app
        .admin
        .json("GET", &format!("/api/ensembles/{ensemble_id}"), None)
        .await;
    assert_eq!(detail["members"].as_array().unwrap().len(), 2);
    let (_, result) = bulk(json!({ "userIds": [ids[0], ids[1]], "action": "invite" })).await;
    assert_eq!(result["invitations"].as_array().unwrap().len(), 2);
    assert!(
        result["invitations"][0]["url"]
            .as_str()
            .unwrap()
            .contains("/auth?token=")
    );
    let (_, result) = bulk(json!({ "userIds": [ids[2], admin_id], "action": "delete" })).await;
    assert_eq!(result["done"], 1);
    let (_, users) = app.admin.json("GET", "/api/users", None).await;
    assert_eq!(users.as_array().unwrap().len(), 3);

    // Admins only; an ensemble is required for ensemble actions.
    let (librarian, _) = app.user(Role::Librarian, "Lib").await;
    let (status, _) = librarian
        .json(
            "POST",
            "/api/users/bulk",
            Some(json!({ "userIds": [ids[0]], "action": "invite" })),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = bulk(json!({ "userIds": [ids[0]], "action": "addToEnsemble" })).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}
