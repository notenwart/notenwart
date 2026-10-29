// SPDX-License-Identifier: AGPL-3.0-or-later
// Needs PostgreSQL: DATABASE_URL (see .env.example); every test gets its own database.

mod common;

use axum::http::StatusCode;
use serde_json::{Value, json};
use sqlx::PgPool;

use common::{Client, TestApp, sample_pdf};
use notenwart_app::auth::Role;

async fn instrument(app: &TestApp, name: &str) -> String {
    let (_, instruments) = app.admin.json("GET", "/api/instruments", None).await;
    instruments.as_array().unwrap().iter().find(|i| i["name"] == name).unwrap()["id"].as_str().unwrap().to_string()
}

fn labels(music: &Value) -> Vec<String> {
    music
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|e| e["works"].as_array().unwrap().iter())
        .flat_map(|w| w["parts"].as_array().unwrap().iter())
        .map(|p| p["label"].as_str().unwrap().to_string())
        .collect()
}

async fn my_labels(client: &Client) -> Vec<String> {
    let (status, music) = client.json("GET", "/api/my/music", None).await;
    assert_eq!(status, StatusCode::OK, "{music}");
    labels(&music)
}

#[sqlx::test(migrator = "notenwart_store::MIGRATOR")]
async fn members_see_the_parts_they_play(pool: PgPool) {
    let app = TestApp::new(pool).await;
    let trumpet = instrument(&app, "Trumpet").await;
    let clarinet = instrument(&app, "Clarinet").await;

    let (_, work) = app.admin.json("POST", "/api/works", Some(json!({ "title": "March" }))).await;
    let work_id = work["id"].as_str().unwrap().to_string();
    let (_, source) = app.admin.upload(&work_id, "all.pdf", &sample_pdf(4)).await;
    let source_id = source["id"].as_str().unwrap();
    let mut part_ids = std::collections::HashMap::new();
    for (label, instrument, voice, page) in [
        ("Full score", None, None, 1),
        ("Clarinet", Some(&clarinet), None, 2),
        ("Trumpet 1", Some(&trumpet), Some(1), 3),
        ("Trumpet 2", Some(&trumpet), Some(2), 4),
    ] {
        let (status, part) = app
            .admin
            .json(
                "POST",
                &format!("/api/works/{work_id}/parts"),
                Some(json!({ "label": label, "instrumentId": instrument, "voice": voice, "sourceId": source_id, "pages": [page] })),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{part}");
        part_ids.insert(label, part["id"].as_str().unwrap().to_string());
    }

    let (status, ensemble) = app.admin.json("POST", "/api/ensembles", Some(json!({ "name": "Wind band" }))).await;
    assert_eq!(status, StatusCode::CREATED, "{ensemble}");
    let ensemble_id = ensemble["id"].as_str().unwrap().to_string();
    let (status, _) = app.admin.json("POST", "/api/ensembles", Some(json!({ "name": "wind BAND" }))).await;
    assert_eq!(status, StatusCode::CONFLICT);

    let (anna, anna_id) = app.user(Role::Member, "Anna").await;
    let (ben, ben_id) = app.user(Role::Member, "Ben").await;
    let (carla, carla_id) = app.user(Role::Leader, "Carla").await;
    let (_outsider, _) = app.user(Role::Member, "Otto").await;
    let member = |id: &str| format!("/api/ensembles/{ensemble_id}/members/{id}");

    let plays_trumpet_2 = json!({ "leader": false, "plays": [{ "instrumentId": trumpet, "voice": 2 }] });
    assert_eq!(app.admin.json("PUT", &member(&anna_id), Some(plays_trumpet_2)).await.0, StatusCode::OK);
    let plays_clarinet_1 = json!({ "leader": false, "plays": [{ "instrumentId": clarinet, "voice": 1 }] });
    assert_eq!(app.admin.json("PUT", &member(&ben_id), Some(plays_clarinet_1)).await.0, StatusCode::OK);
    let leads = json!({ "leader": true, "plays": [] });
    assert_eq!(app.admin.json("PUT", &member(&carla_id), Some(leads.clone())).await.0, StatusCode::OK);
    assert_eq!(
        app.admin.json("PUT", &member(&anna_id), Some(leads)).await.0,
        StatusCode::UNPROCESSABLE_ENTITY,
        "members cannot lead"
    );

    // Without the work, nobody sees anything.
    assert!(my_labels(&anna).await.is_empty());
    let (status, _) = carla.json("PUT", &format!("/api/ensembles/{ensemble_id}/works/{work_id}"), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "leaders give works to their ensemble");

    assert_eq!(my_labels(&anna).await, ["Trumpet 2"]);
    assert_eq!(my_labels(&ben).await, ["Clarinet"], "a part without number is for every clarinet");
    assert_eq!(my_labels(&carla).await, ["Full score", "Clarinet", "Trumpet 1", "Trumpet 2"]);

    let file = |label: &str| format!("/api/parts/{}/file", part_ids[label]);
    assert_eq!(anna.get_bytes(&file("Trumpet 2")).await.0, StatusCode::OK);
    assert_eq!(anna.get_bytes(&file("Trumpet 1")).await.0, StatusCode::NOT_FOUND);
    assert_eq!(anna.get_bytes(&file("Full score")).await.0, StatusCode::NOT_FOUND);
    assert_eq!(anna.json("GET", &format!("/api/sources/{source_id}/file"), None).await.0, StatusCode::FORBIDDEN);

    // Members see only themselves; leaders see everyone.
    let (_, detail) = anna.json("GET", &format!("/api/ensembles/{ensemble_id}"), None).await;
    assert_eq!(detail["members"].as_array().unwrap().len(), 1);
    assert_eq!(detail["canManage"], false);
    let (_, detail) = carla.json("GET", &format!("/api/ensembles/{ensemble_id}"), None).await;
    assert_eq!(detail["members"].as_array().unwrap().len(), 3);
    assert_eq!(detail["members"][0]["name"], "Carla", "leaders first");

    // Leaders manage players but not who leads; members manage nothing.
    let plays_trumpet = json!({ "leader": false, "plays": [{ "instrumentId": trumpet, "voice": null }] });
    assert_eq!(carla.json("PUT", &member(&anna_id), Some(plays_trumpet.clone())).await.0, StatusCode::OK);
    assert_eq!(my_labels(&anna).await, ["Trumpet 1", "Trumpet 2"]);
    let make_leader = json!({ "leader": true, "plays": [] });
    assert_eq!(carla.json("PUT", &member(&ben_id), Some(make_leader)).await.0, StatusCode::FORBIDDEN);
    assert_eq!(anna.json("PUT", &member(&ben_id), Some(plays_trumpet)).await.0, StatusCode::FORBIDDEN);
    assert_eq!(carla.json("POST", "/api/ensembles", Some(json!({ "name": "Choir" }))).await.0, StatusCode::FORBIDDEN);

    // Someone outside the ensemble does not find it.
    let (otto, _) = app.user(Role::Member, "Otto 2").await;
    assert_eq!(otto.json("GET", &format!("/api/ensembles/{ensemble_id}"), None).await.0, StatusCode::NOT_FOUND);
    let (_, list) = otto.json("GET", "/api/ensembles", None).await;
    assert_eq!(list, json!([]));

    // Demoting the leader to member ends leading.
    let demote = json!({ "name": "Carla", "email": "carla@example.org", "role": "member" });
    assert_eq!(app.admin.json("PUT", &format!("/api/users/{carla_id}"), Some(demote)).await.0, StatusCode::OK);
    assert!(my_labels(&carla).await.is_empty());

    // Taking the work away hides it again.
    let (status, _) = app.admin.json("DELETE", &format!("/api/ensembles/{ensemble_id}/works/{work_id}"), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(my_labels(&anna).await.is_empty());
}

#[sqlx::test(migrator = "notenwart_store::MIGRATOR")]
async fn part_info_follows_visibility_and_versions(pool: PgPool) {
    let app = TestApp::new(pool).await;
    let trumpet = instrument(&app, "Trumpet").await;
    let (_, work) = app.admin.json("POST", "/api/works", Some(json!({ "title": "Waltz", "composer": "J. S." }))).await;
    let work_id = work["id"].as_str().unwrap().to_string();
    let (_, source) = app.admin.upload(&work_id, "waltz.pdf", &sample_pdf(3)).await;
    let source_id = source["id"].as_str().unwrap();
    let body =
        |pages: Value| json!({ "label": "Trumpet", "instrumentId": trumpet, "sourceId": source_id, "pages": pages });
    let (_, part) = app.admin.json("POST", &format!("/api/works/{work_id}/parts"), Some(body(json!([1])))).await;
    let part_id = part["id"].as_str().unwrap().to_string();

    let (member, member_id) = app.user(Role::Member, "Mia").await;
    let info = format!("/api/parts/{part_id}/info");
    assert_eq!(member.json("GET", &info, None).await.0, StatusCode::NOT_FOUND);

    let (_, ensemble) = app.admin.json("POST", "/api/ensembles", Some(json!({ "name": "Band" }))).await;
    let ensemble_id = ensemble["id"].as_str().unwrap();
    app.admin
        .json(
            "PUT",
            &format!("/api/ensembles/{ensemble_id}/members/{member_id}"),
            Some(json!({ "leader": false, "plays": [{ "instrumentId": trumpet }] })),
        )
        .await;
    app.admin.json("PUT", &format!("/api/ensembles/{ensemble_id}/works/{work_id}"), None).await;

    let (status, first) = member.json("GET", &info, None).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!((first["workTitle"].as_str(), first["composer"].as_str()), (Some("Waltz"), Some("J. S.")));
    assert_eq!(first["pageCount"], 1);
    let (_, music) = member.json("GET", "/api/my/music", None).await;
    assert_eq!(music[0]["works"][0]["parts"][0]["updatedAt"], first["updatedAt"]);

    // Rendering again gives a new version, which offline copies notice.
    app.admin.json("PUT", &format!("/api/parts/{part_id}"), Some(body(json!([1, 2])))).await;
    let (_, second) = member.json("GET", &info, None).await;
    assert_eq!(second["pageCount"], 2);
    assert_ne!(second["updatedAt"], first["updatedAt"]);
}
