// SPDX-License-Identifier: AGPL-3.0-or-later
// Needs PostgreSQL: DATABASE_URL (see .env.example); every test gets its own database.

mod common;

use axum::http::StatusCode;
use serde_json::{Value, json};
use sqlx::PgPool;

use common::{Client, TestApp, sample_pdf};
use notenwart::auth::Role;

struct Setup {
    ensemble: String,
    works: Vec<String>,
    /// Part ids of the first work: full score, trumpet 1, trumpet 2.
    parts: Vec<String>,
    leader: Client,
    member: Client,
    member_id: String,
}

async fn setup(app: &TestApp) -> Setup {
    let (_, instruments) = app.admin.json("GET", "/api/instruments", None).await;
    let trumpet = instruments
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["name"] == "Trumpet")
        .unwrap()["id"]
        .clone();
    let mut works = Vec::new();
    let mut parts = Vec::new();
    for title in ["March", "Waltz", "Polka"] {
        let (_, work) = app
            .admin
            .json("POST", "/api/works", Some(json!({ "title": title })))
            .await;
        let id = work["id"].as_str().unwrap().to_string();
        let (_, source) = app.admin.upload(&id, "all.pdf", &sample_pdf(3)).await;
        for (label, instrument, voice, page) in [
            ("Full score", Value::Null, Value::Null, 1),
            ("Trumpet 1", trumpet.clone(), json!(1), 2),
            ("Trumpet 2", trumpet.clone(), json!(2), 3),
        ] {
            let (status, part) = app
                .admin
                .json(
                    "POST",
                    &format!("/api/works/{id}/parts"),
                    Some(json!({ "label": label, "instrumentId": instrument, "voice": voice, "sourceId": source["id"], "pages": [page] })),
                )
                .await;
            assert_eq!(status, StatusCode::CREATED, "{part}");
            if title == "March" {
                parts.push(part["id"].as_str().unwrap().to_string());
            }
        }
        works.push(id);
    }
    let (_, ensemble) = app
        .admin
        .json("POST", "/api/ensembles", Some(json!({ "name": "Band" })))
        .await;
    let ensemble = ensemble["id"].as_str().unwrap().to_string();
    let (leader, leader_id) = app.user(Role::Leader, "Lea").await;
    let (member, member_id) = app.user(Role::Member, "Mo").await;
    app.admin
        .json(
            "PUT",
            &format!("/api/ensembles/{ensemble}/members/{leader_id}"),
            Some(json!({ "leader": true, "plays": [] })),
        )
        .await;
    app.admin
        .json(
            "PUT",
            &format!("/api/ensembles/{ensemble}/members/{member_id}"),
            Some(json!({ "leader": false, "plays": [{ "instrumentId": trumpet, "voice": 2 }] })),
        )
        .await;
    app.admin
        .json("PUT", &format!("/api/ensembles/{ensemble}/works/{}", works[0]), None)
        .await;
    Setup {
        ensemble,
        works,
        parts,
        leader,
        member,
        member_id,
    }
}

fn labels(music: &Value) -> Vec<String> {
    music[0]["works"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|w| {
            w["parts"]
                .as_array()
                .unwrap()
                .iter()
                .map(move |p| format!("{} {}", w["title"].as_str().unwrap(), p["label"].as_str().unwrap()))
        })
        .collect()
}

#[sqlx::test(migrator = "notenwart::app::MIGRATOR")]
async fn programmes_keep_order_and_bring_their_works(pool: PgPool) {
    let app = TestApp::new(pool).await;
    let s = setup(&app).await;

    let (status, program) = s
        .leader
        .json(
            "POST",
            &format!("/api/ensembles/{}/programs", s.ensemble),
            Some(
                json!({ "name": "Spring concert", "date": "2027-04-25", "startTime": "19:30", "venue": " Town hall " }),
            ),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{program}");
    let program_id = program["id"].as_str().unwrap().to_string();
    let (status, _) = s
        .leader
        .json(
            "POST",
            &format!("/api/ensembles/{}/programs", s.ensemble),
            Some(json!({ "name": "Later", "date": "2027-02-30" })),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "no 30 February");
    assert_eq!(program["startTime"], "19:30");
    assert_eq!(program["venue"], "Town hall");
    let (status, _) = s
        .leader
        .json(
            "POST",
            &format!("/api/ensembles/{}/programs", s.ensemble),
            Some(json!({ "name": "Late", "startTime": "25:00" })),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "no 25 o'clock");
    let (status, _) = s
        .member
        .json(
            "POST",
            &format!("/api/ensembles/{}/programs", s.ensemble),
            Some(json!({ "name": "Mine" })),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Waltz and Polka are not the ensemble's yet: the programme brings them.
    let order = json!({ "items": [
        { "kind": "work", "workId": s.works[2], "note": "with repeat" },
        { "kind": "break", "label": "Interval", "durationSeconds": 900 },
        { "kind": "work", "workId": s.works[0] },
        { "kind": "work", "workId": s.works[1] }
    ] });
    let (status, program) = s
        .leader
        .json("PUT", &format!("/api/programs/{program_id}/items"), Some(order))
        .await;
    assert_eq!(status, StatusCode::OK, "{program}");
    assert_eq!(program["workIds"], json!([s.works[2], s.works[0], s.works[1]]));
    assert_eq!(program["items"][0]["note"], "with repeat");
    assert_eq!(
        program["items"][1],
        json!({ "kind": "break", "label": "Interval", "durationSeconds": 900, "note": null })
    );
    assert_eq!(program["date"], "2027-04-25");
    let twice =
        json!({ "items": [{ "kind": "work", "workId": s.works[0] }, { "kind": "work", "workId": s.works[0] }] });
    assert_eq!(
        s.leader
            .json("PUT", &format!("/api/programs/{program_id}/items"), Some(twice))
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let (_, music) = s.member.json("GET", "/api/my/music", None).await;
    assert_eq!(
        labels(&music),
        ["March Trumpet 2", "Polka Trumpet 2", "Waltz Trumpet 2"]
    );
    assert_eq!(music[0]["programs"][0]["name"], "Spring concert");
    assert_eq!(music[0]["programs"][0]["workIds"][0], json!(s.works[2]));
    let (_, detail) = s
        .member
        .json("GET", &format!("/api/ensembles/{}", s.ensemble), None)
        .await;
    assert_eq!(detail["programs"].as_array().unwrap().len(), 1);

    // Taking a work from the ensemble takes it out of the programme too.
    s.leader
        .json(
            "DELETE",
            &format!("/api/ensembles/{}/works/{}", s.ensemble, s.works[2]),
            None,
        )
        .await;
    let (_, detail) = s
        .leader
        .json("GET", &format!("/api/ensembles/{}", s.ensemble), None)
        .await;
    assert_eq!(detail["programs"][0]["workIds"], json!([s.works[0], s.works[1]]));

    let (status, _) = s
        .leader
        .json("DELETE", &format!("/api/programs/{program_id}"), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, music) = s.member.json("GET", "/api/my/music", None).await;
    assert_eq!(music[0]["programs"], json!([]));
    assert_eq!(
        labels(&music),
        ["March Trumpet 2", "Waltz Trumpet 2"],
        "the works stay with the ensemble"
    );
}

#[sqlx::test(migrator = "notenwart::app::MIGRATOR")]
async fn grants_add_single_parts_until_taken_back(pool: PgPool) {
    let app = TestApp::new(pool).await;
    let s = setup(&app).await;
    let score = &s.parts[0];
    let grant = format!("/api/ensembles/{}/grants/{score}/{}", s.ensemble, s.member_id);

    let (_, parts) = s
        .leader
        .json("GET", &format!("/api/ensembles/{}/parts", s.ensemble), None)
        .await;
    assert_eq!(parts.as_array().unwrap().len(), 3);
    assert_eq!(
        s.member
            .json("GET", &format!("/api/ensembles/{}/parts", s.ensemble), None)
            .await
            .0,
        StatusCode::FORBIDDEN
    );

    assert_eq!(
        s.member.get_bytes(&format!("/api/parts/{score}/file")).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        s.member.json("PUT", &grant, None).await.0,
        StatusCode::FORBIDDEN,
        "members cannot grant"
    );
    assert_eq!(s.leader.json("PUT", &grant, None).await.0, StatusCode::NO_CONTENT);
    let (_, music) = s.member.json("GET", "/api/my/music", None).await;
    assert_eq!(labels(&music), ["March Full score", "March Trumpet 2"]);
    assert_eq!(
        s.member.get_bytes(&format!("/api/parts/{score}/file")).await.0,
        StatusCode::OK
    );
    let (_, grants) = s
        .leader
        .json("GET", &format!("/api/ensembles/{}/grants", s.ensemble), None)
        .await;
    assert_eq!(grants[0]["userName"], "Mo");
    assert_eq!(grants[0]["label"], "Full score");

    // Only parts of the ensemble's works, only for its members.
    let (_, waltz) = app.admin.json("GET", &format!("/api/works/{}", s.works[1]), None).await;
    let waltz_part = waltz["parts"][0]["id"].as_str().unwrap();
    let foreign = format!("/api/ensembles/{}/grants/{waltz_part}/{}", s.ensemble, s.member_id);
    assert_eq!(
        s.leader.json("PUT", &foreign, None).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let (_, outsider_id) = app.user(Role::Member, "Out").await;
    let outsider = format!("/api/ensembles/{}/grants/{score}/{outsider_id}", s.ensemble);
    assert_eq!(
        s.leader.json("PUT", &outsider, None).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );

    assert_eq!(s.leader.json("DELETE", &grant, None).await.0, StatusCode::NO_CONTENT);
    let (_, music) = s.member.json("GET", "/api/my/music", None).await;
    assert_eq!(labels(&music), ["March Trumpet 2"]);

    // Leaving the ensemble ends grants.
    s.leader.json("PUT", &grant, None).await;
    s.leader
        .json(
            "DELETE",
            &format!("/api/ensembles/{}/members/{}", s.ensemble, s.member_id),
            None,
        )
        .await;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM part_grants")
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}
