// SPDX-License-Identifier: AGPL-3.0-or-later
// Needs PostgreSQL: DATABASE_URL (see .env.example); every test gets its own database.

mod common;

use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use common::{TestApp, sample_pdf};
use notenwart::auth::Role;

#[sqlx::test(migrator = "notenwart::app::MIGRATOR")]
async fn substitutes_see_their_parts_until_the_date(pool: PgPool) {
    let app = TestApp::new(pool.clone()).await;
    let (leader, leader_id) = app.user(Role::Leader, "Lea").await;
    let (member, _) = app.user(Role::Member, "Mia").await;
    let (_, ensemble) = app
        .admin
        .json("POST", "/api/ensembles", Some(json!({ "name": "Band" })))
        .await;
    let ensemble_id = ensemble["id"].as_str().unwrap();
    app.admin
        .json(
            "PUT",
            &format!("/api/ensembles/{ensemble_id}/members/{leader_id}"),
            Some(json!({ "leader": true, "plays": [] })),
        )
        .await;
    let (_, work) = app
        .admin
        .json("POST", "/api/works", Some(json!({ "title": "Polka" })))
        .await;
    let work_id = work["id"].as_str().unwrap();
    app.admin
        .json("PUT", &format!("/api/ensembles/{ensemble_id}/works/{work_id}"), None)
        .await;
    let (_, source) = app.admin.upload(work_id, "p.pdf", &sample_pdf(3)).await;
    let mut parts = vec![];
    for (label, pages) in [("Trumpet 2", json!([1])), ("Score", json!([2, 3]))] {
        let (_, part) = app
            .admin
            .json(
                "POST",
                &format!("/api/works/{work_id}/parts"),
                Some(json!({ "label": label, "sourceId": source["id"], "pages": pages })),
            )
            .await;
        parts.push(part["id"].as_str().unwrap().to_string());
    }
    let (_, other_work) = app
        .admin
        .json("POST", "/api/works", Some(json!({ "title": "Elsewhere" })))
        .await;
    let (_, other_source) = app
        .admin
        .upload(other_work["id"].as_str().unwrap(), "o.pdf", &sample_pdf(1))
        .await;
    let (_, foreign) = app
        .admin
        .json(
            "POST",
            &format!("/api/works/{}/parts", other_work["id"].as_str().unwrap()),
            Some(json!({ "label": "X", "sourceId": other_source["id"], "pages": [1] })),
        )
        .await;

    let uri = format!("/api/ensembles/{ensemble_id}/guest-links");
    let next_week = (time::OffsetDateTime::now_utc() + time::Duration::days(7))
        .date()
        .to_string();
    let input = json!({ "name": "Max, trumpet", "partIds": [parts[0]], "expiresOn": next_week });
    // Someone outside the ensemble does not even see it.
    assert_eq!(
        member.json("POST", &uri, Some(input.clone())).await.0,
        StatusCode::NOT_FOUND
    );
    let (status, _) = leader
        .json(
            "POST",
            &uri,
            Some(json!({ "name": "x", "partIds": [foreign["id"]], "expiresOn": next_week })),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "only the ensemble's parts");
    let (status, _) = leader
        .json(
            "POST",
            &uri,
            Some(json!({ "name": "x", "partIds": [parts[0]], "expiresOn": "2020-01-01" })),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "not in the past");
    let (status, link) = leader.json("POST", &uri, Some(input)).await;
    assert_eq!(status, StatusCode::CREATED, "{link}");
    let url = link["url"].as_str().unwrap();
    let token = url.rsplit('/').next().unwrap();

    // The guest, without an account.
    let guest = app.anonymous();
    let (status, view) = guest.json("GET", &format!("/api/guest/{token}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(view["ensemble"], "Band");
    assert_eq!(view["parts"].as_array().unwrap().len(), 1);
    assert_eq!(view["parts"][0]["label"], "Trumpet 2");
    let (status, pdf) = guest
        .get_bytes(&format!("/api/guest/{token}/parts/{}/file", parts[0]))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(pdf.starts_with(b"%PDF"));
    let (status, _) = guest
        .get_bytes(&format!("/api/guest/{token}/parts/{}/file", parts[1]))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "not their part");
    assert_eq!(
        guest.json("GET", "/api/guest/wrong", None).await.0,
        StatusCode::NOT_FOUND
    );

    let (_, list) = leader.json("GET", &uri, None).await;
    assert!(list[0]["lastUsedAt"].is_string());
    assert!(list[0].get("url").is_none(), "the link is shown only once");

    // Expiring tomorrow: whoever made it hears about it; revoked: gone.
    sqlx::query("UPDATE guest_links SET expires_on = current_date + 1")
        .execute(&pool)
        .await
        .unwrap();
    notenwart::guests::notify_expiring(&app.state).await.unwrap();
    // Only this kind: notices about the new parts arrive in the background and may or may not be there yet.
    let (_, inbox) = leader.json("GET", "/api/notifications", None).await;
    let expiring = inbox["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| n["kind"] == "guestLinkExpiring")
        .count();
    assert_eq!(expiring, 1, "{inbox}");
    let (status, _) = leader
        .json(
            "DELETE",
            &format!("/api/guest-links/{}", link["id"].as_str().unwrap()),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        guest.json("GET", &format!("/api/guest/{token}"), None).await.0,
        StatusCode::NOT_FOUND
    );
}
