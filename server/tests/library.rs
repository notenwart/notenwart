// SPDX-License-Identifier: AGPL-3.0-or-later
// Needs PostgreSQL: DATABASE_URL (see .env.example); every test gets its own database.

mod common;

use axum::http::StatusCode;
use serde_json::{Value, json};
use sqlx::PgPool;

use common::{TestApp, page_widths, sample_pdf};

async fn work(app: &TestApp, title: &str, tags: &[&str]) -> String {
    let (status, work) = app
        .admin
        .json("POST", "/api/works", Some(json!({ "title": title, "tags": tags })))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{work}");
    work["id"].as_str().unwrap().to_string()
}

async fn instrument(app: &TestApp, name: &str) -> String {
    let (_, instruments) = app.admin.json("GET", "/api/instruments", None).await;
    instruments
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["name"] == name)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string()
}

#[sqlx::test(migrator = "notenwart::app::MIGRATOR")]
async fn works_are_created_normalized_and_searched(pool: PgPool) {
    let app = TestApp::new(pool).await;
    let (status, created) = app
        .admin
        .json(
            "POST",
            "/api/works",
            Some(json!({ "title": "  Böhmischer Traum ", "composer": " ", "tags": ["Polka", "polka", " Blasmusik "] })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(created["title"], "Böhmischer Traum");
    assert_eq!(created["composer"], Value::Null);
    assert_eq!(created["tags"], json!(["Polka", "Blasmusik"]));
    work(&app, "Pomp and Circumstance 100%", &["Marsch"]).await;

    let (_, found) = app.admin.json("GET", "/api/works?q=traum", None).await;
    assert_eq!(found.as_array().unwrap().len(), 1);
    // `%` is literal, not a wildcard
    let (_, found) = app.admin.json("GET", "/api/works?q=0%25", None).await;
    assert_eq!(found[0]["title"], "Pomp and Circumstance 100%");
    let (_, found) = app.admin.json("GET", "/api/works?tag=Marsch", None).await;
    assert_eq!(found.as_array().unwrap().len(), 1);
    let (_, tags) = app.admin.json("GET", "/api/tags", None).await;
    assert_eq!(tags, json!(["Blasmusik", "Marsch", "Polka"]));

    let (status, error) = app.admin.json("POST", "/api/works", Some(json!({ "title": "" }))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(error["error"], "invalid");
}

#[sqlx::test(migrator = "notenwart::app::MIGRATOR")]
async fn upload_split_and_delete(pool: PgPool) {
    let app = TestApp::new(pool).await;
    let work_id = work(&app, "Test march", &[]).await;

    let (status, source) = app.admin.upload(&work_id, "Trompete 1+2.pdf", &sample_pdf(4)).await;
    assert_eq!(status, StatusCode::CREATED, "{source}");
    assert_eq!(source["pageCount"], 4);
    assert_eq!(source["fileName"], "Trompete 1+2.pdf");
    let source_id = source["id"].as_str().unwrap();

    let (status, error) = app.admin.upload(&work_id, "again.pdf", &sample_pdf(4)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{error}");
    let (status, _) = app.admin.upload(&work_id, "notes.txt", b"just text").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let trumpet = instrument(&app, "Trumpet").await;
    let (status, part) = app.admin
        .json(
            "POST",
            &format!("/api/works/{work_id}/parts"),
            Some(json!({ "label": "Trumpet 2", "instrumentId": trumpet, "voice": 2, "sourceId": source_id, "pages": [4, 2] })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{part}");
    assert_eq!(part["pageCount"], 2);
    let part_id = part["id"].as_str().unwrap();

    let (status, pdf) = app.admin.get_bytes(&format!("/api/parts/{part_id}/file")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page_widths(&pdf), vec![400, 200]);

    // Re-render with other pages
    let (status, part) = app.admin
        .json(
            "PUT",
            &format!("/api/parts/{part_id}"),
            Some(json!({ "label": "Trumpet 1", "instrumentId": trumpet, "voice": 1, "sourceId": source_id, "pages": [1] })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{part}");
    let (_, pdf) = app.admin.get_bytes(&format!("/api/parts/{part_id}/file")).await;
    assert_eq!(page_widths(&pdf), vec![100]);

    for (pages, why) in [(json!([5]), "outside"), (json!([1, 1]), "twice"), (json!([]), "empty")] {
        let (status, error) = app
            .admin
            .json(
                "POST",
                &format!("/api/works/{work_id}/parts"),
                Some(json!({ "label": "X", "sourceId": source_id, "pages": pages })),
            )
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{why}: {error}");
    }

    let (_, detail) = app.admin.json("GET", &format!("/api/works/{work_id}"), None).await;
    assert_eq!(detail["sources"].as_array().unwrap().len(), 1);
    assert_eq!(detail["parts"][0]["label"], "Trumpet 1");
    assert_eq!(app.object_count().await, 2);

    let (status, _) = app.admin.json("DELETE", &format!("/api/works/{work_id}"), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(app.object_count().await, 0);
    let (status, _) = app.admin.json("GET", &format!("/api/works/{work_id}"), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrator = "notenwart::app::MIGRATOR")]
async fn deleting_a_source_deletes_its_parts(pool: PgPool) {
    let app = TestApp::new(pool).await;
    let work_id = work(&app, "Suite", &[]).await;
    let (_, source) = app.admin.upload(&work_id, "score.pdf", &sample_pdf(2)).await;
    let source_id = source["id"].as_str().unwrap();
    let (status, _) = app
        .admin
        .json(
            "POST",
            &format!("/api/works/{work_id}/parts"),
            Some(json!({ "label": "Full score", "sourceId": source_id, "pages": [1, 2] })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, _) = app
        .admin
        .json("DELETE", &format!("/api/sources/{source_id}"), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, detail) = app.admin.json("GET", &format!("/api/works/{work_id}"), None).await;
    assert_eq!(detail["parts"], json!([]));
    assert_eq!(app.object_count().await, 0);
}

#[sqlx::test(migrator = "notenwart::app::MIGRATOR")]
async fn uploads_over_the_limit_are_rejected(pool: PgPool) {
    let app = TestApp::new(pool).await;
    let work_id = work(&app, "Big", &[]).await;
    let mut big = sample_pdf(1);
    big.resize(2 * 1024 * 1024, b' ');
    let (status, _) = app.admin.upload(&work_id, "big.pdf", &big).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
}

#[sqlx::test(migrator = "notenwart::app::MIGRATOR")]
async fn facets_suggest_values_already_used(pool: PgPool) {
    let app = TestApp::new(pool).await;
    for (title, composer, tags) in [
        ("A", "Carl Teike", vec!["Marsch"]),
        ("B", "Ernst Mosch", vec!["Polka", "Marsch"]),
        ("C", "Ernst Mosch", vec![]),
    ] {
        app.admin
            .json(
                "POST",
                "/api/works",
                Some(json!({ "title": title, "composer": composer, "tags": tags })),
            )
            .await;
    }
    let (status, work) = app
        .admin
        .json(
            "POST",
            "/api/works",
            Some(json!({ "title": "Stille Nacht", "lyricist": " Joseph Mohr ", "societyNumber": "1234567-001" })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{work}");
    assert_eq!(
        (work["lyricist"].as_str(), work["societyNumber"].as_str()),
        (Some("Joseph Mohr"), Some("1234567-001"))
    );
    // A save without the fields clears them: clients send the whole work.
    let (_, saved) = app
        .admin
        .json(
            "PUT",
            &format!("/api/works/{}", work["id"].as_str().unwrap()),
            Some(json!({ "title": "Stille Nacht" })),
        )
        .await;
    assert_eq!(saved["lyricist"], Value::Null);
    let (status, facets) = app.admin.json("GET", "/api/works/facets", None).await;
    assert_eq!(status, StatusCode::OK, "{facets}");
    assert_eq!(facets["composers"], json!(["Ernst Mosch", "Carl Teike"]));
    assert_eq!(facets["tags"], json!(["Marsch", "Polka"]));
    assert_eq!(facets["arrangers"], json!([]));

    let (member, _) = app.user(notenwart::auth::Role::Member, "Mia").await;
    assert_eq!(
        member.json("GET", "/api/works/facets", None).await.0,
        StatusCode::FORBIDDEN
    );
}

#[sqlx::test(migrator = "notenwart::app::MIGRATOR")]
async fn lists_are_compressed_files_are_not(pool: PgPool) {
    let app = TestApp::new(pool).await;
    for n in 0..40 {
        work(&app, &format!("Work number {n} with a fairly long title"), &["Concert"]).await;
    }
    let request = |uri: &str| {
        axum::http::Request::get(uri)
            .header("accept-encoding", "gzip")
            .body(axum::body::Body::empty())
            .unwrap()
    };
    let (status, _, headers) = app.admin.send(request("/api/works")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["content-encoding"], "gzip");

    let id = work(&app, "With a file", &[]).await;
    let (_, source) = app.admin.upload(&id, "f.pdf", &sample_pdf(2)).await;
    let (_, part) = app
        .admin
        .json(
            "POST",
            &format!("/api/works/{id}/parts"),
            Some(json!({ "label": "Score", "sourceId": source["id"], "pages": [1, 2] })),
        )
        .await;
    let (status, _, headers) = app
        .admin
        .send(request(&format!("/api/parts/{}/file", part["id"].as_str().unwrap())))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(headers.get("content-encoding").is_none(), "PDFs are compressed already");

    // The search looks into tags and the other fields too.
    let (_, found) = app.admin.json("GET", "/api/works?q=concert", None).await;
    assert_eq!(found.as_array().unwrap().len(), 40);
}
