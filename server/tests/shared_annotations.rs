// SPDX-License-Identifier: AGPL-3.0-or-later
// Needs PostgreSQL: DATABASE_URL (see .env.example); every test gets its own database.

mod common;

use std::time::Duration;

use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use common::{TestApp, sample_pdf};
use notenwart::auth::Role;

#[sqlx::test(migrator = "notenwart::app::MIGRATOR")]
async fn leaders_share_markings_with_their_players(pool: PgPool) {
    let app = TestApp::new(pool).await;
    let (leader, leader_id) = app.user(Role::Leader, "Lea").await;
    let (player, player_id) = app.user(Role::Member, "Pia").await;
    let (other, other_id) = app.user(Role::Member, "Ole").await;
    let (_, instruments) = app.admin.json("GET", "/api/instruments", None).await;
    let violin = instruments
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["name"] == "Violin")
        .unwrap()["id"]
        .clone();
    let (_, work) = app
        .admin
        .json("POST", "/api/works", Some(json!({ "title": "Air" })))
        .await;
    let work_id = work["id"].as_str().unwrap();
    let (_, source) = app.admin.upload(work_id, "a.pdf", &sample_pdf(2)).await;
    let (_, part) = app
        .admin
        .json(
            "POST",
            &format!("/api/works/{work_id}/parts"),
            Some(json!({ "label": "Violin", "instrumentId": violin, "sourceId": source["id"], "pages": [1, 2] })),
        )
        .await;
    let part_id = part["id"].as_str().unwrap();
    // Two ensembles play the same work; each keeps its own markings.
    let mut ensembles = vec![];
    for (name, members) in [
        ("Strings", vec![(&leader_id, true), (&player_id, false)]),
        ("Orchestra", vec![(&other_id, false)]),
    ] {
        let (_, ensemble) = app
            .admin
            .json("POST", "/api/ensembles", Some(json!({ "name": name })))
            .await;
        let id = ensemble["id"].as_str().unwrap().to_string();
        app.admin
            .json("PUT", &format!("/api/ensembles/{id}/works/{work_id}"), None)
            .await;
        for (user, leads) in members {
            let plays = if leads {
                json!([])
            } else {
                json!([{ "instrumentId": violin, "voice": null }])
            };
            app.admin
                .json(
                    "PUT",
                    &format!("/api/ensembles/{id}/members/{user}"),
                    Some(json!({ "leader": leads, "plays": plays })),
                )
                .await;
        }
        ensembles.push(id);
    }

    let uri = format!("/api/parts/{part_id}/shared-annotations");
    let (_, layers) = player.json("GET", &uri, None).await;
    assert_eq!(layers, json!([]), "nothing shared yet, and players start none");
    let (_, layers) = leader.json("GET", &uri, None).await;
    assert_eq!(layers.as_array().unwrap().len(), 1);
    assert_eq!(layers[0]["canEdit"], true);

    let bowing = json!({ "pages": { "1": [{ "tool": "pen", "color": "#c62828", "width": 0.003, "points": [[0.1, 0.1], [0.2, 0.1]] }] } });
    let save = format!("{uri}/{}", ensembles[0]);
    assert_eq!(
        player.json("PUT", &save, Some(bowing.clone())).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        leader
            .json("PUT", &format!("{uri}/{}", ensembles[1]), Some(bowing.clone()))
            .await
            .0,
        StatusCode::NOT_FOUND,
        "not their ensemble"
    );
    let (status, saved) = leader.json("PUT", &save, Some(bowing.clone())).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["updatedBy"], "Lea");
    let bad = json!({ "pages": { "9": [] } });
    assert_eq!(
        leader.json("PUT", &save, Some(bad)).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );

    // The player sees it (read only) and hears about it once; the other ensemble's player sees nothing.
    let (_, layers) = player.json("GET", &uri, None).await;
    assert_eq!(layers[0]["ensemble"], "Strings");
    assert_eq!(layers[0]["canEdit"], false);
    assert_eq!(layers[0]["pages"]["1"].as_array().unwrap().len(), 1);
    assert_eq!(other.json("GET", &uri, None).await.1, json!([]));
    leader.json("PUT", &save, Some(bowing)).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let (_, inbox) = player.json("GET", "/api/notifications", None).await;
    let shared: Vec<_> = inbox["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| n["kind"] == "annotationsShared")
        .collect();
    assert_eq!(shared.len(), 1, "one notification, not one per save");
    assert_eq!(shared[0]["params"]["by"], "Lea");

    // Printing: the leader's markings come along unless left out; the stored part is untouched.
    let printed = format!("/api/parts/{part_id}/annotated");
    let (status, pdf) = player.get_bytes(&printed).await;
    assert_eq!(status, StatusCode::OK);
    assert!(pdf.starts_with(b"%PDF"));
    let marked = String::from_utf8_lossy(&pdf);
    assert!(marked.contains(" RG "), "the stroke is drawn");
    let (_, plain) = player.get_bytes(&format!("{printed}?shared=false")).await;
    assert!(!String::from_utf8_lossy(&plain).contains(" RG "));
    let (_, original) = player.get_bytes(&format!("/api/parts/{part_id}/file")).await;
    assert!(!String::from_utf8_lossy(&original).contains(" RG "));
    // The other ensemble's player gets the part without the Strings' markings.
    let (status, theirs) = other.get_bytes(&printed).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!String::from_utf8_lossy(&theirs).contains(" RG "));
}
