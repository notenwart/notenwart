// SPDX-License-Identifier: AGPL-3.0-or-later
// Needs PostgreSQL: DATABASE_URL (see .env.example); every test gets its own database.

mod common;

use axum::http::StatusCode;
use serde_json::{Value, json};
use sqlx::PgPool;

use common::TestApp;
use notenwart_app::auth::Role;

fn step<'a>(system: &'a Value, key: &str) -> &'a Value {
    system["checklist"].as_array().unwrap().iter().find(|s| s["key"] == key).unwrap()
}

#[sqlx::test(migrator = "notenwart_store::MIGRATOR")]
async fn admins_see_the_system_and_the_setup_checklist(pool: PgPool) {
    let app = TestApp::new(pool).await;
    let (librarian, _) = app.user(Role::Librarian, "Lib").await;
    assert_eq!(librarian.json("GET", "/api/system", None).await.0, StatusCode::FORBIDDEN);

    let (status, system) = app.admin.json("GET", "/api/system", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(system["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(system["mailFrom"], Value::Null);
    assert_eq!(step(&system, "people")["done"], true, "the admin and the librarian");
    assert_eq!(step(&system, "ensembles")["done"], false);
    assert_eq!(step(&system, "instruments")["manual"], true);

    // Without SMTP there is no test email.
    assert_eq!(app.admin.json("POST", "/api/system/test-mail", None).await.0, StatusCode::CONFLICT);

    // Steps ticked by hand; recognised ones cannot be.
    let tick = |key: &str, done: bool| {
        let uri = format!("/api/system/checklist/{key}");
        let admin = app.admin.clone();
        async move { admin.json("PUT", &uri, Some(json!({ "done": done }))).await }
    };
    assert_eq!(tick("instruments", true).await.0, StatusCode::NO_CONTENT);
    assert_eq!(tick("instruments", true).await.0, StatusCode::NO_CONTENT);
    assert_eq!(tick("ensembles", true).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    app.admin.json("POST", "/api/ensembles", Some(json!({ "name": "Band" }))).await;
    let (_, system) = app.admin.json("GET", "/api/system", None).await;
    assert_eq!(step(&system, "instruments")["done"], true);
    assert_eq!(step(&system, "ensembles")["done"], true);
    assert_eq!(system["counts"]["ensembles"], 1);
    tick("instruments", false).await;
    let (_, system) = app.admin.json("GET", "/api/system", None).await;
    assert_eq!(step(&system, "instruments")["done"], false);
}
