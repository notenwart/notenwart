// SPDX-License-Identifier: AGPL-3.0-or-later
// Needs PostgreSQL: DATABASE_URL (see .env.example); every test gets its own database.

mod common;

use axum::http::StatusCode;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use common::TestApp;
use notenwart_app::auth::Role;
use notenwart_app::mail::Language;

fn find<'a>(list: &'a Value, name: &str) -> &'a Value {
    list.as_array().unwrap().iter().find(|i| i["name"] == name).unwrap_or_else(|| panic!("{name} missing"))
}

#[sqlx::test(migrator = "notenwart_store::MIGRATOR")]
async fn an_organisation_gets_its_own_copy_in_its_language(pool: PgPool) {
    // An organisation from before the catalogue: a part and a member still point at a built-in instrument.
    let organization_id = notenwart_store::organization::ensure_default(&pool, "Old").await.unwrap();
    let trumpet: Uuid =
        sqlx::query_scalar("SELECT id FROM instruments WHERE organization_id IS NULL AND name = 'Trumpet'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let work: Uuid = sqlx::query_scalar("INSERT INTO works (organization_id, title) VALUES ($1, 'W') RETURNING id")
        .bind(organization_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let source: Uuid = sqlx::query_scalar(
        "INSERT INTO source_files (work_id, file_name, size, sha256, page_count, storage_key)
         VALUES ($1, 'w.pdf', 1, 'x', 1, 'sources/x') RETURNING id",
    )
    .bind(work)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO parts (work_id, source_id, label, instrument_id, pages, storage_key)
         VALUES ($1, $2, 'Trumpet', $3, '{1}', 'parts/x')",
    )
    .bind(work)
    .bind(source)
    .bind(trumpet)
    .execute(&pool)
    .await
    .unwrap();

    notenwart_app::catalogue::ensure(&pool, organization_id, Language::De).await.unwrap();
    // A second start changes nothing.
    notenwart_app::catalogue::ensure(&pool, organization_id, Language::De).await.unwrap();

    let app = TestApp::new(pool.clone()).await;
    let (status, instruments) = app.admin.json("GET", "/api/instruments", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(instruments.as_array().unwrap().len(), 39);
    let trompete = find(&instruments, "Trompete");
    assert_eq!(trompete["family"], "Blechbläser");
    assert_eq!(trompete["transposition"], "B");
    assert!(trompete["aliases"].as_array().unwrap().contains(&json!("Trumpet")));
    assert_eq!(trompete["usage"], 1);
    let part_instrument: Uuid = sqlx::query_scalar("SELECT instrument_id FROM parts").fetch_one(&pool).await.unwrap();
    assert_eq!(json!(part_instrument.to_string()), trompete["id"]);

    assert_eq!(find(&instruments, "Oboe")["aliases"], json!(["Ob"]));
    let (_, families) = app.admin.json("GET", "/api/instrument-families", None).await;
    assert_eq!(families[0]["name"], "Holzbläser");
    assert_eq!(families.as_array().unwrap().len(), 8);
}

#[sqlx::test(migrator = "notenwart_store::MIGRATOR")]
async fn staff_edit_the_catalogue(pool: PgPool) {
    let app = TestApp::new(pool).await;
    let (librarian, _) = app.user(Role::Librarian, "Lib").await;
    let (member, _) = app.user(Role::Member, "Mem").await;
    let (_, instruments) = app.admin.json("GET", "/api/instruments", None).await;
    let sax = find(&instruments, "Alto saxophone").clone();
    let sax_uri = format!("/api/instruments/{}", sax["id"].as_str().unwrap());

    // Spelling the organisation prefers; the old name stays as an alias.
    let input = json!({ "name": "Altsaxofon", "familyId": sax["familyId"], "transposition": "Eb",
                        "aliases": ["Alto saxophone", " Altsax ", "altsax", ""] });
    assert_eq!(member.json("PUT", &sax_uri, Some(input.clone())).await.0, StatusCode::FORBIDDEN);
    let (status, renamed) = librarian.json("PUT", &sax_uri, Some(input)).await;
    assert_eq!(status, StatusCode::OK, "{renamed}");
    assert_eq!(renamed["name"], "Altsaxofon");
    assert_eq!(renamed["aliases"], json!(["Alto saxophone", "Altsax"]));

    // A family of its own with a new instrument.
    let (status, family) = librarian.json("POST", "/api/instrument-families", Some(json!({ "name": "Folk" }))).await;
    assert_eq!(status, StatusCode::CREATED, "{family}");
    assert_eq!(
        librarian.json("POST", "/api/instrument-families", Some(json!({ "name": "folk" }))).await.0,
        StatusCode::CONFLICT
    );
    let (status, bagpipe) = librarian
        .json(
            "POST",
            "/api/instruments",
            Some(json!({ "name": "Dudelsack", "familyId": family["id"], "aliases": ["Bagpipes"] })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{bagpipe}");
    assert_eq!(bagpipe["family"], "Folk");
    let (_, list) = member.json("GET", "/api/instruments", None).await;
    assert_eq!(list.as_array().unwrap().last().unwrap()["name"], "Dudelsack");

    // A family with instruments stays; another organisation's family is unknown.
    let family_uri = format!("/api/instrument-families/{}", family["id"].as_str().unwrap());
    assert_eq!(librarian.json("DELETE", &family_uri, None).await.0, StatusCode::CONFLICT);
    let (status, _) =
        librarian.json("POST", "/api/instruments", Some(json!({ "name": "X", "familyId": Uuid::new_v4() }))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    // An instrument a member plays stays too.
    let (_, ensemble) = app.admin.json("POST", "/api/ensembles", Some(json!({ "name": "Band" }))).await;
    let (_, me) = app.admin.json("GET", "/api/me", None).await;
    let (status, body) = app
        .admin
        .json(
            "PUT",
            &format!("/api/ensembles/{}/members/{}", ensemble["id"].as_str().unwrap(), me["id"].as_str().unwrap()),
            Some(json!({ "leader": false, "plays": [{ "instrumentId": bagpipe["id"], "voice": null }] })),
        )
        .await;
    assert!(status.is_success(), "{status} {body}");
    let bagpipe_uri = format!("/api/instruments/{}", bagpipe["id"].as_str().unwrap());
    assert_eq!(librarian.json("DELETE", &bagpipe_uri, None).await.0, StatusCode::CONFLICT);
    app.admin
        .json(
            "DELETE",
            &format!("/api/ensembles/{}/members/{}", ensemble["id"].as_str().unwrap(), me["id"].as_str().unwrap()),
            None,
        )
        .await;
    assert_eq!(librarian.json("DELETE", &bagpipe_uri, None).await.0, StatusCode::NO_CONTENT);
    assert_eq!(librarian.json("DELETE", &family_uri, None).await.0, StatusCode::NO_CONTENT);

    // Two names for one instrument: merged, nothing that used either is lost.
    let (_, list) = librarian.json("GET", "/api/instruments", None).await;
    let tenor = find(&list, "Tenor saxophone").clone();
    let (_, duplicate) = librarian
        .json(
            "POST",
            "/api/instruments",
            Some(json!({ "name": "Tenorsaxofon", "familyId": tenor["familyId"], "aliases": ["TS"] })),
        )
        .await;
    app.admin
        .json(
            "PUT",
            &format!("/api/ensembles/{}/members/{}", ensemble["id"].as_str().unwrap(), me["id"].as_str().unwrap()),
            Some(json!({ "leader": false, "plays": [
                { "instrumentId": duplicate["id"], "voice": 1 },
                { "instrumentId": tenor["id"], "voice": 1 }
            ] })),
        )
        .await;
    let merge = format!("/api/instruments/{}/merge", duplicate["id"].as_str().unwrap());
    assert_eq!(member.json("POST", &merge, Some(json!({ "into": tenor["id"] }))).await.0, StatusCode::FORBIDDEN);
    assert_eq!(
        librarian.json("POST", &merge, Some(json!({ "into": duplicate["id"] }))).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let (status, merged) = librarian.json("POST", &merge, Some(json!({ "into": tenor["id"] }))).await;
    assert_eq!(status, StatusCode::OK, "{merged}");
    let aliases = merged["aliases"].as_array().unwrap();
    assert!(aliases.contains(&json!("Tenorsaxofon")) && aliases.contains(&json!("TS")));
    assert_eq!(merged["usage"], 1, "one assignment, not two");
    let (_, list) = librarian.json("GET", "/api/instruments", None).await;
    assert!(list.as_array().unwrap().iter().all(|i| i["name"] != "Tenorsaxofon"));
}
