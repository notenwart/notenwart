// SPDX-License-Identifier: AGPL-3.0-or-later
// Needs PostgreSQL: DATABASE_URL (see .env.example); every test gets its own database.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use sqlx::PgPool;

use common::{TestApp, sample_pdf};
use notenwart::auth::Role;

#[sqlx::test(migrator = "notenwart::app::MIGRATOR")]
async fn annotations_are_personal_and_follow_visibility(pool: PgPool) {
    let app = TestApp::new(pool).await;
    let (_, instruments) = app.admin.json("GET", "/api/instruments", None).await;
    let horn = instruments
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["name"] == "Horn")
        .unwrap()["id"]
        .clone();
    let (_, work) = app
        .admin
        .json("POST", "/api/works", Some(json!({ "title": "Nocturne" })))
        .await;
    let work_id = work["id"].as_str().unwrap();
    let (_, source) = app.admin.upload(work_id, "horn.pdf", &sample_pdf(2)).await;
    let (_, part) = app
        .admin
        .json(
            "POST",
            &format!("/api/works/{work_id}/parts"),
            Some(json!({ "label": "Horn", "instrumentId": horn, "sourceId": source["id"], "pages": [1, 2] })),
        )
        .await;
    let url = format!("/api/parts/{}/annotations", part["id"].as_str().unwrap());

    let (player, player_id) = app.user(Role::Member, "Pia").await;
    let (other, other_id) = app.user(Role::Member, "Olli").await;
    let (outsider, _) = app.user(Role::Member, "Udo").await;
    let (_, ensemble) = app
        .admin
        .json("POST", "/api/ensembles", Some(json!({ "name": "Horns" })))
        .await;
    let ensemble_id = ensemble["id"].as_str().unwrap();
    for id in [&player_id, &other_id] {
        app.admin
            .json(
                "PUT",
                &format!("/api/ensembles/{ensemble_id}/members/{id}"),
                Some(json!({ "leader": false, "plays": [{ "instrumentId": horn }] })),
            )
            .await;
    }
    app.admin
        .json("PUT", &format!("/api/ensembles/{ensemble_id}/works/{work_id}"), None)
        .await;

    let (status, empty) = player.json("GET", &url, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(empty, json!({ "pages": {}, "updatedAt": null }));

    let marks = json!({ "pages": {
        "1": [{ "tool": "pen", "color": "#1d2b45", "width": 0.004, "points": [[0.1, 0.2], [0.15, 0.25]] }],
        "2": [] } });
    let (status, saved) = player.json("PUT", &url, Some(marks)).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert!(saved["updatedAt"].is_string());
    assert_eq!(saved["pages"].as_object().unwrap().len(), 1, "empty pages are dropped");

    let (_, loaded) = player.json("GET", &url, None).await;
    assert_eq!(loaded["pages"]["1"][0]["points"], json!([[0.1, 0.2], [0.15, 0.25]]));
    let (_, theirs) = other.json("GET", &url, None).await;
    assert_eq!(theirs["pages"], json!({}), "annotations are personal");

    assert_eq!(outsider.json("GET", &url, None).await.0, StatusCode::NOT_FOUND);
    let invalid =
        json!({ "pages": { "3": [{ "tool": "pen", "color": "#000000", "width": 0.004, "points": [[0.1, 0.1]] }] } });
    assert_eq!(
        player.json("PUT", &url, Some(invalid)).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    // An unknown tool fails deserialisation; axum answers in plain text.
    let laser =
        r##"{ "pages": { "1": [{ "tool": "laser", "color": "#000000", "width": 0.004, "points": [[0.1, 0.1]] }] } }"##;
    let request = Request::put(&url)
        .header("content-type", "application/json")
        .body(Body::from(laser))
        .unwrap();
    assert!(player.send(request).await.0.is_client_error());
}
