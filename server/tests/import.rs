// SPDX-License-Identifier: AGPL-3.0-or-later
// Needs PostgreSQL: DATABASE_URL (see .env.example); every test gets its own database.

mod common;

use axum::http::StatusCode;
use serde_json::{Value, json};
use sqlx::PgPool;

use common::TestApp;
use notenwart::auth::Role;

fn rows() -> Value {
    json!([
        { "name": "Anna Alt", "email": "Anna@example.org", "ensemble": "Blasorchester", "instrument": "Trompete", "voice": 2 },
        { "name": "Anna Alt", "email": "anna@example.org", "ensemble": "Jugendorchester", "instrument": "Trumpet" },
        { "name": "Ben Bass", "email": "ben@example.org", "role": "Leitung", "ensemble": "blasorchester" },
        { "name": "Cara Cello", "email": "cara@example.org" }
    ])
}

async fn count(app: &TestApp, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(&app.pool).await.unwrap()
}

#[sqlx::test(migrator = "notenwart::app::MIGRATOR")]
async fn dry_run_checks_without_changing_anything(pool: PgPool) {
    let app = TestApp::new(pool).await;
    let (status, result) = app
        .admin
        .json(
            "POST",
            "/api/users/import",
            Some(json!({ "rows": rows(), "dryRun": true })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["applied"], false);
    assert_eq!(result["newPeople"], 3);
    assert_eq!(result["ensemblesCreated"], json!(["Blasorchester", "Jugendorchester"]));
    let outcomes: Vec<&str> = result["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["outcome"].as_str().unwrap())
        .collect();
    assert_eq!(outcomes, ["new", "existing", "new", "new"]);
    assert_eq!(count(&app, "SELECT count(*) FROM users").await, 1, "only the admin");
    assert_eq!(count(&app, "SELECT count(*) FROM ensembles").await, 0);
}

#[sqlx::test(migrator = "notenwart::app::MIGRATOR")]
async fn import_creates_people_ensembles_and_parts_once(pool: PgPool) {
    let app = TestApp::new(pool).await;
    let (status, result) = app
        .admin
        .json(
            "POST",
            "/api/users/import",
            Some(json!({ "rows": rows(), "dryRun": false })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["applied"], true);
    let urls: Vec<&str> = result["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|r| r["invitationUrl"].as_str())
        .collect();
    assert_eq!(urls.len(), 3, "one invitation per new person");
    assert!(urls.iter().all(|u| u.starts_with("http://notenwart.test/auth?token=")));

    assert_eq!(count(&app, "SELECT count(*) FROM users").await, 4);
    assert_eq!(count(&app, "SELECT count(*) FROM ensembles").await, 2);
    assert_eq!(
        count(&app, "SELECT count(*) FROM memberships m JOIN users u ON u.id = m.user_id WHERE u.email = 'ben@example.org' AND m.role = 'leader'").await,
        1
    );
    assert_eq!(count(&app, "SELECT count(*) FROM ensemble_members").await, 3);
    assert_eq!(count(&app, "SELECT count(*) FROM ensemble_member_parts").await, 2);
    assert_eq!(
        count(&app, "SELECT count(*) FROM ensemble_member_parts WHERE voice = 2").await,
        1,
        "Anna plays trumpet 2 in the band, every trumpet part in the youth orchestra"
    );

    // Importing the same table again changes nothing and invites nobody.
    let (status, again) = app
        .admin
        .json(
            "POST",
            "/api/users/import",
            Some(json!({ "rows": rows(), "dryRun": false })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(again["newPeople"], 0);
    assert_eq!(again["ensemblesCreated"], json!([]));
    assert_eq!(count(&app, "SELECT count(*) FROM users").await, 4);
    assert_eq!(count(&app, "SELECT count(*) FROM ensemble_member_parts").await, 2);
}

#[sqlx::test(migrator = "notenwart::app::MIGRATOR")]
async fn one_bad_row_stops_the_whole_import(pool: PgPool) {
    let app = TestApp::new(pool).await;
    let bad = json!([
        { "name": "Anna", "email": "anna@example.org", "ensemble": "Band", "instrument": "Trompete" },
        { "name": "Ben", "email": "not-an-email" },
        { "name": "Cara", "email": "cara@example.org", "instrument": "Cello" },
        { "name": "Dan", "email": "dan@example.org", "ensemble": "Band", "instrument": "Nasenflöte" },
        { "name": "Eva", "email": "eva@example.org", "role": "Dirigent" },
        { "name": "Fin", "email": "fin@example.org", "ensemble": "Band", "voice": 2 }
    ]);
    let (status, result) = app
        .admin
        .json(
            "POST",
            "/api/users/import",
            Some(json!({ "rows": bad, "dryRun": false })),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{result}");
    assert_eq!(result["applied"], false);
    assert_eq!(result["errors"], 5);
    let messages: Vec<&str> = result["rows"].as_array().unwrap()[1..]
        .iter()
        .map(|r| r["message"].as_str().unwrap())
        .collect();
    assert_eq!(
        messages,
        [
            "not a valid email address",
            "an instrument needs an ensemble",
            "unknown instrument",
            "unknown role",
            "a part number needs an instrument"
        ]
    );
    assert_eq!(count(&app, "SELECT count(*) FROM users").await, 1);
    assert_eq!(count(&app, "SELECT count(*) FROM ensembles").await, 0);
}

#[sqlx::test(migrator = "notenwart::app::MIGRATOR")]
async fn only_admins_import(pool: PgPool) {
    let app = TestApp::new(pool).await;
    let (librarian, _) = app.user(Role::Librarian, "Lib").await;
    let (status, _) = librarian
        .json(
            "POST",
            "/api/users/import",
            Some(json!({ "rows": rows(), "dryRun": true })),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
