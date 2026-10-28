// SPDX-License-Identifier: AGPL-3.0-or-later
// Needs PostgreSQL: DATABASE_URL (see .env.example); every test gets its own database. Reading needs pdftotext
// (poppler-utils); without it the test only checks the rules around it.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use lopdf::content::{Content, Operation};
use lopdf::{Document, Object, Stream, dictionary};
use sqlx::PgPool;

use common::{Client, TestApp};
use notenwart::auth::Role;

/// A one-page PDF with text lines at (x, y from the top, size) in points on an A4 page.
fn page_with_text(lines: &[(f32, f32, f32, &str)]) -> Vec<u8> {
    pages_with_text(&[lines])
}

/// A PDF with one page per entry, each with its text lines.
fn pages_with_text(pages: &[&[(f32, f32, f32, &str)]]) -> Vec<u8> {
    let mut document = Document::with_version("1.5");
    let root = document.new_object_id();
    let font = document.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica", "Encoding" => "WinAnsiEncoding",
    });
    let mut kids = vec![];
    for lines in pages {
        let mut operations = vec![];
        for (x, y, size, text) in lines.iter() {
            operations.extend([
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), (*size).into()]),
                Operation::new("Td", vec![(*x).into(), (842.0 - y).into()]),
                Operation::new("Tj", vec![Object::String(latin1(text), lopdf::StringFormat::Literal)]),
                Operation::new("ET", vec![]),
            ]);
        }
        let content = document.add_object(Stream::new(dictionary! {}, Content { operations }.encode().unwrap()));
        let page = document.add_object(dictionary! {
            "Type" => "Page", "Parent" => root, "Contents" => content,
            "Resources" => dictionary! { "Font" => dictionary! { "F1" => font } },
        });
        kids.push(Object::Reference(page));
    }
    let count = kids.len() as i64;
    document.objects.insert(
        root,
        Object::Dictionary(dictionary! {
            "Type" => "Pages", "Kids" => kids, "Count" => count,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        }),
    );
    let catalog = document.add_object(dictionary! { "Type" => "Catalog", "Pages" => root });
    document.trailer.set("Root", catalog);
    let mut out = Vec::new();
    document.save_to(&mut out).unwrap();
    out
}

/// WinAnsi agrees with Latin-1 for the letters used here (©, ä).
fn latin1(text: &str) -> Vec<u8> {
    text.chars().map(|c| c as u32 as u8).collect()
}

async fn recognize(client: &Client, pdf: &[u8]) -> (StatusCode, serde_json::Value) {
    let boundary = "notenwart-test-boundary";
    let mut body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"p.pdf\"\r\n\
         Content-Type: application/pdf\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(pdf);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let request = Request::post("/api/recognize")
        .header("content-type", format!("multipart/form-data; boundary={boundary}"))
        .body(Body::from(body))
        .unwrap();
    let (status, bytes, _) = client.send(request).await;
    (status, serde_json::from_slice(&bytes).unwrap_or_default())
}

#[sqlx::test(migrator = "notenwart::app::MIGRATOR")]
async fn reads_the_first_page(pool: PgPool) {
    let app = TestApp::new(pool).await;
    let pdf = page_with_text(&[
        (50.0, 50.0, 11.0, "2. Trumpet in Bb"),
        (150.0, 100.0, 28.0, "Bohemian Dream"),
        (260.0, 125.0, 13.0, "Polka"),
        (400.0, 150.0, 10.0, "Music: Norbert Gälle"),
        (400.0, 163.0, 10.0, "Arr.: Kurt Gäble"),
        (120.0, 815.0, 8.0, "© 1998 by Musikverlag Rundel. All rights reserved."),
    ]);

    let (member, _) = app.user(Role::Member, "Mia").await;
    assert_eq!(recognize(&member, &pdf).await.0, StatusCode::FORBIDDEN);
    assert_eq!(
        recognize(&app.admin, b"no pdf").await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let (_, status) = app.admin.json("GET", "/api/auth/status", None).await;
    if status["recognition"]["text"] != true {
        eprintln!("pdftotext is not installed: skipping the reading itself");
        return;
    }
    let (code, found) = recognize(&app.admin, &pdf).await;
    assert_eq!(code, StatusCode::OK, "{found}");
    assert_eq!(found["method"], "text");
    assert_eq!(found["title"], "Bohemian Dream");
    assert_eq!(found["subtitle"], "Polka");
    assert_eq!(found["composer"], "Norbert Gälle");
    assert_eq!(found["arranger"], "Kurt Gäble");
    assert_eq!(found["publisher"], "Musikverlag Rundel");
    assert_eq!(found["voice"], 2);
    let (_, instruments) = app.admin.json("GET", "/api/instruments", None).await;
    let trumpet = instruments
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["name"] == "Trumpet")
        .unwrap();
    assert_eq!(found["instrumentId"], trumpet["id"]);
}

#[sqlx::test(migrator = "notenwart::app::MIGRATOR")]
async fn proposes_the_parts_of_a_file_with_all_parts(pool: PgPool) {
    let app = TestApp::new(pool).await;
    let (_, status) = app.admin.json("GET", "/api/auth/status", None).await;
    if status["recognition"]["text"] != true {
        eprintln!("pdftotext is not installed: skipping");
        return;
    }
    let music = (50.0, 400.0, 10.0, "Allegro moderato");
    let pdf = pages_with_text(&[
        &[(200.0, 100.0, 24.0, "Stille Nacht"), (60.0, 40.0, 11.0, "Score"), music],
        &[(60.0, 40.0, 11.0, "1. Trumpet in Bb"), music],
        &[music],
        &[(60.0, 40.0, 11.0, "2. Trumpet in Bb"), music],
        &[(60.0, 40.0, 11.0, "Trombone"), music],
    ]);
    let (_, work) = app
        .admin
        .json(
            "POST",
            "/api/works",
            Some(serde_json::json!({ "title": "Stille Nacht", "tags": [] })),
        )
        .await;
    let work_id = work["id"].as_str().unwrap();
    let (status, source) = app.admin.upload(work_id, "Alle Stimmen.pdf", &pdf).await;
    assert_eq!(status, StatusCode::CREATED, "{source}");
    let source_id = source["id"].as_str().unwrap();

    let (librarian, _) = app.user(Role::Librarian, "Lib").await;
    let (member, _) = app.user(Role::Member, "Mem").await;
    let split = format!("/api/sources/{source_id}/split");
    assert_eq!(member.json("POST", &split, None).await.0, StatusCode::FORBIDDEN);
    let (code, parts) = librarian.json("POST", &split, None).await;
    assert_eq!(code, StatusCode::OK, "{parts}");
    let summary: Vec<(bool, serde_json::Value, serde_json::Value)> = parts
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (p["score"].as_bool().unwrap(), p["voice"].clone(), p["pages"].clone()))
        .collect();
    use serde_json::json;
    assert_eq!(
        summary,
        vec![
            (true, json!(null), json!([1])),
            (false, json!(1), json!([2, 3])),
            (false, json!(2), json!([4])),
            (false, json!(null), json!([5])),
        ]
    );
    assert_eq!(parts[1]["printed"], "1. Trumpet in Bb");

    let (code, first) = librarian
        .json("POST", &format!("/api/sources/{source_id}/recognize"), None)
        .await;
    assert_eq!(code, StatusCode::OK, "{first}");
    assert_eq!(first["title"], "Stille Nacht");
}
