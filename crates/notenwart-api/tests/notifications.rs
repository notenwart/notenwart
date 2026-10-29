// SPDX-License-Identifier: AGPL-3.0-or-later
// Needs PostgreSQL: DATABASE_URL (see .env.example); every test gets its own database.

mod common;

use std::time::Duration;

use axum::http::StatusCode;
use serde_json::{Value, json};
use sqlx::PgPool;

use common::{Client, TestApp, sample_pdf};
use notenwart_app::auth::Role;

/// Notifications are recorded in the background: wait until `n` arrived.
async fn inbox(client: &Client, n: usize) -> Value {
    for _ in 0..50 {
        let (_, inbox) = client.json("GET", "/api/notifications", None).await;
        if inbox["items"].as_array().unwrap().len() >= n {
            return inbox;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    client.json("GET", "/api/notifications", None).await.1
}

#[sqlx::test(migrator = "notenwart_store::MIGRATOR")]
async fn people_hear_about_new_parts_programmes_and_loans(pool: PgPool) {
    let app = TestApp::new(pool.clone()).await;
    let (mia, mia_id) = app.user(Role::Member, "Mia").await;
    let (_, instruments) = app.admin.json("GET", "/api/instruments", None).await;
    let trumpet = instruments.as_array().unwrap().iter().find(|i| i["name"] == "Trumpet").unwrap()["id"].clone();
    let (_, ensemble) = app.admin.json("POST", "/api/ensembles", Some(json!({ "name": "Band" }))).await;
    let ensemble_id = ensemble["id"].as_str().unwrap();
    app.admin
        .json(
            "PUT",
            &format!("/api/ensembles/{ensemble_id}/members/{mia_id}"),
            Some(json!({ "leader": false, "plays": [{ "instrumentId": trumpet, "voice": null }] })),
        )
        .await;
    let (_, work) = app.admin.json("POST", "/api/works", Some(json!({ "title": "Polka" }))).await;
    let work_id = work["id"].as_str().unwrap();
    app.admin.json("PUT", &format!("/api/ensembles/{ensemble_id}/works/{work_id}"), None).await;
    let (_, source) = app.admin.upload(work_id, "p.pdf", &sample_pdf(2)).await;
    let (status, _) = app
        .admin
        .json(
            "POST",
            &format!("/api/works/{work_id}/parts"),
            Some(json!({ "label": "Trumpet 1", "instrumentId": trumpet, "voice": 1, "sourceId": source["id"], "pages": [1] })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);

    let received = inbox(&mia, 1).await;
    assert_eq!(received["unread"], 1, "{received}");
    assert_eq!(received["items"][0]["kind"], "partsNew");
    assert_eq!(received["items"][0]["params"], json!({ "part": "Trumpet 1", "work": "Polka" }));
    // The admin who made it hears nothing about it.
    assert_eq!(app.admin.json("GET", "/api/notifications", None).await.1["unread"], 0);

    // Mia wants programmes by email only.
    let (_, settings) = mia.json("GET", "/api/notifications/settings", None).await;
    assert!(
        settings.as_array().unwrap().iter().all(|s| s["kind"] != "guestLinkExpiring"),
        "members make no guest links"
    );
    let (status, _) = mia
        .json(
            "PUT",
            "/api/notifications/settings",
            Some(json!([{ "kind": "programChanged", "inApp": false, "email": true, "push": false }])),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    app.admin.json("POST", &format!("/api/ensembles/{ensemble_id}/programs"), Some(json!({ "name": "Concert" }))).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(inbox(&mia, 2).await["items"].as_array().unwrap().len(), 1, "not in the app");

    // A loan due tomorrow: reminded once.
    let tomorrow = (time::OffsetDateTime::now_utc() + time::Duration::days(1)).date().to_string();
    app.admin
        .json(
            "POST",
            &format!("/api/works/{work_id}/loans"),
            Some(json!({ "item": "Trumpet 1", "borrowerId": mia_id, "dueOn": tomorrow })),
        )
        .await;
    notenwart_app::notify::remind_loans(&app.state).await.unwrap();
    notenwart_app::notify::remind_loans(&app.state).await.unwrap();
    let received = inbox(&mia, 2).await;
    assert_eq!(received["items"].as_array().unwrap().len(), 2);
    assert_eq!(received["items"][0]["kind"], "loanDue");

    // One email for everything waiting – new part, programme, loan – then nothing more.
    assert_eq!(notenwart_app::notify::send_digests(&app.state, Duration::ZERO).await.unwrap(), 1);
    assert_eq!(notenwart_app::notify::send_digests(&app.state, Duration::ZERO).await.unwrap(), 0);

    let (status, _) = mia.json("POST", "/api/notifications/read", Some(json!({}))).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(mia.json("GET", "/api/notifications?count=true", None).await.1["unread"], 0);
}
