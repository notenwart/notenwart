// SPDX-License-Identifier: AGPL-3.0-or-later
// Needs PostgreSQL: DATABASE_URL (see .env.example); every test gets its own database.

mod common;

use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use common::TestApp;
use notenwart::auth::Role;

#[sqlx::test(migrator = "notenwart::app::MIGRATOR")]
async fn librarians_lend_and_members_see_their_loans(pool: PgPool) {
    let app = TestApp::new(pool).await;
    let (librarian, _) = app.user(Role::Librarian, "Lib").await;
    let (member, member_id) = app.user(Role::Member, "Mia").await;
    let (_, work) = app
        .admin
        .json(
            "POST",
            "/api/works",
            Some(json!({ "title": "Alte Kameraden", "archiveLocation": "Schrank 3" })),
        )
        .await;
    assert_eq!(work["archiveLocation"], "Schrank 3");
    let work_id = work["id"].as_str().unwrap();
    let uri = format!("/api/works/{work_id}/loans");

    let lend = json!({ "item": "Trompete 2", "quantity": 2, "borrowerId": member_id, "dueOn": "2027-06-30" });
    assert_eq!(
        member.json("POST", &uri, Some(lend.clone())).await.0,
        StatusCode::FORBIDDEN
    );
    let (status, loan) = librarian.json("POST", &uri, Some(lend)).await;
    assert_eq!(status, StatusCode::CREATED, "{loan}");
    assert_eq!(loan["borrowerName"], "Mia");
    assert_eq!(loan["workTitle"], "Alte Kameraden");
    assert!(loan["lentOn"].as_str().unwrap().len() == 10);

    // Someone outside the organisation, by name.
    let (status, _) = librarian
        .json(
            "POST",
            &uri,
            Some(json!({ "item": "Partitur", "borrowerName": "Gastdirigent" })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, _) = librarian.json("POST", &uri, Some(json!({ "item": "Partitur" }))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "a loan needs a borrower");
    let (status, _) = librarian
        .json(
            "POST",
            &uri,
            Some(json!({ "item": "x", "borrowerName": "y", "dueOn": "2027-02-30" })),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (_, mine) = member.json("GET", "/api/me/loans", None).await;
    assert_eq!(mine.as_array().unwrap().len(), 1);
    assert_eq!(member.json("GET", "/api/loans", None).await.0, StatusCode::FORBIDDEN);
    assert_eq!(
        member.json("GET", "/api/borrowers", None).await.0,
        StatusCode::FORBIDDEN
    );
    let (_, people) = librarian.json("GET", "/api/borrowers", None).await;
    assert!(people.as_array().unwrap().iter().any(|p| p["name"] == "Mia"));
    assert!(people[0].get("email").is_none(), "names only");

    // Returned: off the open list and the member's, still on the work's.
    let returned = json!({ "item": "Trompete 2", "quantity": 2, "borrowerId": member_id, "returnedOn": "2027-07-01" });
    let (status, _) = librarian
        .json(
            "PUT",
            &format!("/api/loans/{}", loan["id"].as_str().unwrap()),
            Some(returned),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (_, open) = librarian.json("GET", "/api/loans", None).await;
    assert_eq!(open.as_array().unwrap().len(), 1);
    let (_, all) = librarian.json("GET", "/api/loans?show=all", None).await;
    assert_eq!(all.as_array().unwrap().len(), 2);
    assert_eq!(member.json("GET", "/api/me/loans", None).await.1, json!([]));
    let (_, of_work) = librarian.json("GET", &uri, None).await;
    assert_eq!(of_work.as_array().unwrap().len(), 2);
}
