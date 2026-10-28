// SPDX-License-Identifier: AGPL-3.0-or-later
// Needs PostgreSQL: DATABASE_URL (see .env.example); every test gets its own database.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{Value, json};
use sqlx::PgPool;

use common::{Client, TestApp, sample_pdf};
use notenwart::auth::Role;

async fn upload(client: &Client, work_id: &str, name: &str, bytes: &[u8], meta: Value) -> (StatusCode, Value) {
    let boundary = "notenwart-test-boundary";
    let mut body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"meta\"\r\n\r\n{meta}\r\n\
         --{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\n\
         Content-Type: application/octet-stream\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let request = Request::post(format!("/api/works/{work_id}/recordings"))
        .header("content-type", format!("multipart/form-data; boundary={boundary}"))
        .body(Body::from(body))
        .unwrap();
    let (status, bytes, _) = client.send(request).await;
    (status, serde_json::from_slice(&bytes).unwrap_or_default())
}

#[sqlx::test(migrator = "notenwart::app::MIGRATOR")]
async fn choirs_practise_with_recordings(pool: PgPool) {
    let app = TestApp::new(pool).await;
    let (leader, leader_id) = app.user(Role::Leader, "Lea").await;
    let (singer, singer_id) = app.user(Role::Member, "Sam").await;
    let (outsider, _) = app.user(Role::Member, "Otto").await;
    let (_, instruments) = app.admin.json("GET", "/api/instruments", None).await;
    let soprano = instruments
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["name"] == "Soprano")
        .unwrap()["id"]
        .clone();
    let (_, choir) = app
        .admin
        .json("POST", "/api/ensembles", Some(json!({ "name": "Choir" })))
        .await;
    let choir_id = choir["id"].as_str().unwrap();
    for (id, leads, plays) in [
        (&leader_id, true, json!([])),
        (&singer_id, false, json!([{ "instrumentId": soprano, "voice": null }])),
    ] {
        app.admin
            .json(
                "PUT",
                &format!("/api/ensembles/{choir_id}/members/{id}"),
                Some(json!({ "leader": leads, "plays": plays })),
            )
            .await;
    }
    let (_, work) = app
        .admin
        .json("POST", "/api/works", Some(json!({ "title": "Abendlied" })))
        .await;
    let work_id = work["id"].as_str().unwrap();
    app.admin
        .json("PUT", &format!("/api/ensembles/{choir_id}/works/{work_id}"), None)
        .await;
    let (_, source) = app.admin.upload(work_id, "s.pdf", &sample_pdf(1)).await;
    let (_, part) = app
        .admin
        .json(
            "POST",
            &format!("/api/works/{work_id}/parts"),
            Some(json!({ "label": "Soprano", "instrumentId": soprano, "sourceId": source["id"], "pages": [1] })),
        )
        .await;

    let mp3: Vec<u8> = [b"ID3\x04\0\0\0\0\0\0".as_slice(), &[0xAB; 4000]].concat();
    let meta = json!({ "kind": "practice", "origin": "generated", "title": "Soprano, slow", "partId": part["id"], "durationSeconds": 95.5 });
    assert_eq!(
        upload(&singer, work_id, "s.mp3", &mp3, meta.clone()).await.0,
        StatusCode::FORBIDDEN
    );
    let (status, recording) = upload(&leader, work_id, "Sopran langsam.mp3", &mp3, meta.clone()).await;
    assert_eq!(status, StatusCode::CREATED, "{recording}");
    assert_eq!(recording["contentType"], "audio/mpeg");
    assert_eq!(recording["durationSeconds"], 95.5);
    assert_eq!(
        upload(&leader, work_id, "again.mp3", &mp3, meta.clone()).await.0,
        StatusCode::CONFLICT
    );
    let video = [b"\0\0\0\x20ftypisom".as_slice(), &[0; 100]].concat();
    let (status, _) = upload(&leader, work_id, "clip.mp4", &video, meta).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "no videos");

    // The singer hears it, with seeking; someone without a part of the work does not.
    let (status, list) = singer
        .json("GET", &format!("/api/works/{work_id}/recordings"), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 1);
    let id = recording["id"].as_str().unwrap();
    let range = Request::get(format!("/api/recordings/{id}/file"))
        .header("range", "bytes=10-19")
        .body(Body::empty())
        .unwrap();
    let (status, bytes, headers) = singer.send(range).await;
    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(bytes, vec![0xAB; 10]);
    assert_eq!(headers["content-range"], format!("bytes 10-19/{}", mp3.len()));
    let (status, whole) = singer.get_bytes(&format!("/api/recordings/{id}/file")).await;
    assert_eq!((status, whole.len()), (StatusCode::OK, mp3.len()));
    assert_eq!(
        outsider
            .json("GET", &format!("/api/works/{work_id}/recordings"), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        outsider.get_bytes(&format!("/api/recordings/{id}/file")).await.0,
        StatusCode::NOT_FOUND
    );

    // Leaders change and remove it; the file goes too.
    let (status, changed) = leader
        .json(
            "PUT",
            &format!("/api/recordings/{id}"),
            Some(json!({ "kind": "full", "origin": "recorded", "title": "Concert 2026", "partId": part["id"] })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        changed["partId"],
        Value::Null,
        "full recordings belong to the whole work"
    );
    assert_eq!(
        leader.json("DELETE", &format!("/api/recordings/{id}"), None).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        singer.get_bytes(&format!("/api/recordings/{id}/file")).await.0,
        StatusCode::NOT_FOUND
    );
}
