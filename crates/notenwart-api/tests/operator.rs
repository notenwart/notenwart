// SPDX-License-Identifier: AGPL-3.0-or-later

//! The operator at the console (`notenwart tui`) acts through the same commands as an admin, without being a person.

mod common;

use common::TestApp;
use notenwart_app::auth::{CurrentUser, Role};
use notenwart_app::people::{self, UserInput};
use notenwart_app::system;
use sqlx::PgPool;

#[sqlx::test(migrator = "notenwart_store::MIGRATOR")]
async fn the_operator_acts_as_an_admin_without_a_name(pool: PgPool) {
    let app = TestApp::new(pool).await;
    let organizations = system::organizations(&app.state).await.unwrap();
    assert_eq!(organizations.len(), 1);
    let operator = CurrentUser::operator(organizations[0].id, organizations[0].name.clone());
    assert_eq!(operator.actor(), None);

    // Reads what an admin reads.
    let before = people::list(&app.state, &operator).await.unwrap().len();
    assert!(system::system(&app.state, &operator).await.is_ok());

    // Invites someone, makes them a leader, deletes them; the deletion is recorded without a name.
    let input = UserInput { name: "Dora".into(), email: "dora@example.org".into(), role: Role::Member };
    let invitation = people::create(&app.state, &operator, input).await.unwrap();
    assert!(invitation.url.contains("/auth?token="));
    let id = invitation.user.row.id;
    let input = UserInput { name: "Dora".into(), email: "dora@example.org".into(), role: Role::Leader };
    assert_eq!(people::update(&app.state, &operator, id, input).await.unwrap().row.role, Role::Leader);
    assert_eq!(people::list(&app.state, &operator).await.unwrap().len(), before + 1);
    people::remove(&app.state, &operator, id).await.unwrap();
    let deleted_by: Option<uuid::Uuid> = sqlx::query_scalar("SELECT deleted_by FROM deletion_log WHERE user_id = $1")
        .bind(id)
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(deleted_by, None);

    // Another organisation's id gives nothing to see.
    let stranger = CurrentUser::operator(uuid::Uuid::new_v4(), "elsewhere".into());
    assert!(people::list(&app.state, &stranger).await.unwrap().is_empty());
}
