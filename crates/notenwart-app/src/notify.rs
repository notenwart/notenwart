// SPDX-License-Identifier: AGPL-3.0-or-later

//! Notifications (`notifications` in api/openapi.json): something happened for someone – new parts, a changed
//! programme, a loan due, markings shared by a leader, a guest link about to expire. Each person decides per kind
//! whether it shows in the app, comes by email, or (later) by push. Emails are collected for a few minutes and sent
//! as one message, so twenty new parts make one email, not twenty.

use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::App;
use crate::auth::CurrentUser;
use crate::error::{AppError, AppResult};
use crate::mail::{Language, send_or_log};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = NotificationKind)]
pub enum Kind {
    /// Parts became visible to the person (uploaded, given to their ensemble, granted).
    PartsNew,
    /// A programme of their ensemble was created or changed.
    ProgramChanged,
    /// A loan is due soon or overdue.
    LoanDue,
    /// A leader shared markings on one of their parts.
    AnnotationsShared,
    /// A guest link the person made is about to expire.
    GuestLinkExpiring,
}

pub const KINDS: [Kind; 5] =
    [Kind::PartsNew, Kind::ProgramChanged, Kind::LoanDue, Kind::AnnotationsShared, Kind::GuestLinkExpiring];

impl Kind {
    pub fn key(self) -> &'static str {
        match self {
            Kind::PartsNew => "partsNew",
            Kind::ProgramChanged => "programChanged",
            Kind::LoanDue => "loanDue",
            Kind::AnnotationsShared => "annotationsShared",
            Kind::GuestLinkExpiring => "guestLinkExpiring",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        KINDS.into_iter().find(|k| k.key() == key)
    }

    /// Without a setting of their own: everything in the app, email for what needs acting on.
    fn default_channels(self) -> Channels {
        Channels { in_app: true, email: !matches!(self, Kind::AnnotationsShared), push: false }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Channels {
    pub in_app: bool,
    pub email: bool,
    /// Stored already; sending push messages comes later.
    pub push: bool,
}

/// Records a notification for each recipient as they set it. Never fails the action that caused it.
pub async fn notify(state: &App, organization_id: Uuid, kind: Kind, recipients: &[Uuid], params: Value, link: &str) {
    if recipients.is_empty() {
        return;
    }
    if let Err(error) = record(state, organization_id, kind, recipients, &params, link).await {
        tracing::warn!("cannot record {} notifications: {error:#}", kind.key());
    }
}

async fn record(
    state: &App,
    organization_id: Uuid,
    kind: Kind,
    recipients: &[Uuid],
    params: &Value,
    link: &str,
) -> anyhow::Result<()> {
    let settings: HashMap<Uuid, Channels> = sqlx::query!(
        "SELECT user_id, in_app, email, push FROM notification_settings WHERE kind = $1 AND user_id = ANY($2)",
        kind.key(),
        recipients
    )
    .fetch_all(&state.pool)
    .await?
    .into_iter()
    .map(|r| (r.user_id, Channels { in_app: r.in_app, email: r.email, push: r.push }))
    .collect();
    let (mut users, mut in_app, mut email) = (vec![], vec![], vec![]);
    for &user in recipients {
        let channels = settings.get(&user).copied().unwrap_or(kind.default_channels());
        if channels.in_app || channels.email {
            users.push(user);
            in_app.push(channels.in_app);
            email.push(channels.email);
        }
    }
    sqlx::query!(
        "INSERT INTO notifications (organization_id, user_id, kind, params, link, in_app, email_due)
         SELECT $1, u, $2, $3, $4, a, e FROM unnest($5::uuid[], $6::bool[], $7::bool[]) AS t(u, a, e)",
        organization_id,
        kind.key(),
        params,
        link,
        &users,
        &in_app,
        &email
    )
    .execute(&state.pool)
    .await?;
    Ok(())
}

/// `notify` in the background, so the request that caused it does not wait.
pub fn later(state: &App, organization_id: Uuid, kind: Kind, recipients: Vec<Uuid>, params: Value, link: String) {
    let state = state.clone();
    tokio::spawn(async move { notify(&state, organization_id, kind, &recipients, params, &link).await });
}

/// What a part is called and which work it belongs to.
pub async fn part_names(state: &App, part_id: Uuid) -> Option<(String, String)> {
    sqlx::query!("SELECT p.label, w.title FROM parts p JOIN works w ON w.id = p.work_id WHERE p.id = $1", part_id)
        .fetch_optional(&state.pool)
        .await
        .ok()
        .flatten()
        .map(|r| (r.label, r.title))
}

/// The people who see a part (its ensembles' members playing it, leaders, grants), except `but`.
pub async fn who_sees_part(state: &App, part_id: Uuid, but: Uuid) -> Vec<Uuid> {
    sqlx::query_scalar!(
        r#"SELECT DISTINCT user_id AS "user_id!" FROM visible_parts WHERE part_id = $1 AND user_id <> $2"#,
        part_id,
        but
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default()
}

/// Members of an ensemble, except `but`.
pub async fn ensemble_members(state: &App, ensemble_id: Uuid, but: Uuid) -> Vec<Uuid> {
    sqlx::query_scalar!(
        "SELECT user_id FROM ensemble_members WHERE ensemble_id = $1 AND user_id <> $2",
        ensemble_id,
        but
    )
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default()
}

// ---- the inbox ----

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    id: Uuid,
    #[schema(value_type = Kind)]
    kind: String,
    /// What the text is made of, by kind – partsNew: work, part (missing when a whole work was given);
    /// programChanged: program, ensemble; loanDue: work, item, date, overdue; annotationsShared: by, part, work;
    /// guestLinkExpiring: guest, date.
    #[schema(value_type = std::collections::HashMap<String, serde_json::Value>)]
    params: Value,
    /// A path of the web app.
    link: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
    read: bool,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Inbox {
    unread: i64,
    items: Vec<Notification>,
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct InboxQuery {
    /// Only the number of unread ones (for the bell).
    count: Option<bool>,
}

pub async fn inbox(state: &App, user: &CurrentUser, query: InboxQuery) -> AppResult<Inbox> {
    // One snapshot for the count and the list: notifications are recorded in the background, and one arriving in
    // between would show in the list but not in the count.
    let mut tx = state.pool.begin().await?;
    sqlx::query!("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY").execute(&mut *tx).await?;
    let unread = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM notifications WHERE user_id = $1 AND in_app AND read_at IS NULL"#,
        user.id
    )
    .fetch_one(&mut *tx)
    .await?;
    let items = if query.count == Some(true) {
        vec![]
    } else {
        sqlx::query_as!(
            Notification,
            r#"SELECT id, kind, params, link, created_at, read_at IS NOT NULL AS "read!"
               FROM notifications WHERE user_id = $1 AND in_app
               ORDER BY created_at DESC LIMIT 100"#,
            user.id
        )
        .fetch_all(&mut *tx)
        .await?
    };
    tx.commit().await?;
    Ok(Inbox { unread, items })
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct ReadInput {
    /// These; all when missing.
    ids: Option<Vec<Uuid>>,
}

pub async fn mark_read(state: &App, user: &CurrentUser, input: ReadInput) -> AppResult<()> {
    sqlx::query!(
        "UPDATE notifications SET read_at = now()
         WHERE user_id = $1 AND read_at IS NULL AND ($2::uuid[] IS NULL OR id = ANY($2))",
        user.id,
        input.ids.as_deref()
    )
    .execute(&state.pool)
    .await?;
    Ok(())
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = NotificationSetting)]
pub struct Setting {
    kind: Kind,
    #[serde(flatten)]
    #[schema(inline)]
    channels: Channels,
}

pub async fn settings(state: &App, user: &CurrentUser) -> AppResult<Vec<Setting>> {
    let stored: HashMap<String, Channels> =
        sqlx::query!("SELECT kind, in_app, email, push FROM notification_settings WHERE user_id = $1", user.id)
            .fetch_all(&state.pool)
            .await?
            .into_iter()
            .map(|r| (r.kind, Channels { in_app: r.in_app, email: r.email, push: r.push }))
            .collect();
    Ok(KINDS
        .into_iter()
        // Guest links are made by staff only.
        .filter(|k| *k != Kind::GuestLinkExpiring || user.is_staff() || user.role == crate::auth::Role::Leader)
        .map(|kind| Setting { kind, channels: stored.get(kind.key()).copied().unwrap_or(kind.default_channels()) })
        .collect())
}

pub async fn save_settings(state: &App, user: &CurrentUser, input: Vec<Setting>) -> AppResult<Vec<Setting>> {
    if input.len() > KINDS.len() {
        return Err(AppError::Invalid("one setting per kind".into()));
    }
    for setting in &input {
        sqlx::query!(
            "INSERT INTO notification_settings (user_id, kind, in_app, email, push) VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (user_id, kind) DO UPDATE SET in_app = $3, email = $4, push = $5",
            user.id,
            setting.kind.key(),
            setting.channels.in_app,
            setting.channels.email,
            setting.channels.push
        )
        .execute(&state.pool)
        .await?;
    }
    settings(state, user).await
}

// ---- email and reminders ----

/// Runs the email digests and loan reminders every minute, for as long as the server runs.
pub fn spawn_worker(state: App) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(60));
        loop {
            tick.tick().await;
            if let Err(error) = remind_loans(&state).await {
                tracing::warn!("loan reminders: {error:#}");
            }
            if let Err(error) = crate::guests::notify_expiring(&state).await {
                tracing::warn!("guest link reminders: {error:#}");
            }
            if let Err(error) = send_digests(&state, Duration::from_secs(120)).await {
                tracing::warn!("notification emails: {error:#}");
            }
        }
    });
}

/// Notifies borrowers a day before a loan is due, and once more when it is overdue.
pub async fn remind_loans(state: &App) -> anyhow::Result<()> {
    let due = sqlx::query!(
        r#"SELECT l.id, l.organization_id, l.borrower_id AS "borrower_id!", l.item, w.title,
                  to_char(l.due_on, 'YYYY-MM-DD') AS "due_on!", l.due_on < current_date AS "overdue!"
           FROM loans l JOIN works w ON w.id = l.work_id
           WHERE l.returned_on IS NULL AND l.borrower_id IS NOT NULL AND l.due_on IS NOT NULL
             AND ((l.reminded_at IS NULL AND l.due_on <= current_date + 1)
                  OR (l.overdue_reminded_at IS NULL AND l.due_on < current_date))"#
    )
    .fetch_all(&state.pool)
    .await?;
    for loan in due {
        let params = serde_json::json!({
            "work": loan.title, "item": loan.item, "date": loan.due_on, "overdue": loan.overdue,
        });
        notify(state, loan.organization_id, Kind::LoanDue, &[loan.borrower_id], params, "/").await;
        sqlx::query!(
            "UPDATE loans SET reminded_at = coalesce(reminded_at, now()),
                              overdue_reminded_at = CASE WHEN $2 THEN now() ELSE overdue_reminded_at END
             WHERE id = $1",
            loan.id,
            loan.overdue
        )
        .execute(&state.pool)
        .await?;
    }
    Ok(())
}

/// One email per person with everything collected for at least `wait`.
pub async fn send_digests(state: &App, wait: Duration) -> anyhow::Result<usize> {
    let rows = sqlx::query!(
        r#"SELECT n.id, n.user_id, n.kind, n.params, n.link, u.email AS "email?", u.display_name
           FROM notifications n JOIN users u ON u.id = n.user_id
           WHERE n.email_due AND n.emailed_at IS NULL AND n.created_at < now() - make_interval(secs => $1)
           ORDER BY n.user_id, n.created_at"#,
        wait.as_secs_f64()
    )
    .fetch_all(&state.pool)
    .await?;
    struct Digest {
        user: Uuid,
        email: Option<String>,
        name: String,
        ids: Vec<Uuid>,
        lines: Vec<String>,
    }
    let mut digests: Vec<Digest> = vec![];
    for row in rows {
        let line =
            describe(state.config.language, &row.kind, &row.params, row.link.as_deref(), &state.config.public_url);
        // Rows come ordered by person.
        match digests.last_mut() {
            Some(digest) if digest.user == row.user_id => {
                digest.ids.push(row.id);
                digest.lines.push(line);
            }
            _ => digests.push(Digest {
                user: row.user_id,
                email: row.email,
                name: row.display_name,
                ids: vec![row.id],
                lines: vec![line],
            }),
        }
    }
    let mut sent = 0;
    for digest in digests {
        if let Some(email) = &digest.email {
            let mail =
                crate::mail::digest(state.config.language, &digest.name, &digest.lines, &state.config.public_url);
            send_or_log(state, email, &mail.subject, &mail.body).await;
            sent += 1;
        }
        // Without an address (or mail) nothing will come of it: do not try again.
        sqlx::query!("UPDATE notifications SET emailed_at = now() WHERE id = ANY($1)", &digest.ids)
            .execute(&state.pool)
            .await?;
    }
    Ok(sent)
}

/// One line of an email for a notification, in the server's language.
pub fn describe(language: Language, kind: &str, params: &Value, link: Option<&str>, public_url: &str) -> String {
    let p = |key: &str| params[key].as_str().unwrap_or("").to_string();
    let text = match (Kind::from_key(kind), language) {
        (Some(Kind::PartsNew), Language::En) if p("part").is_empty() => format!("New parts: {}", p("work")),
        (Some(Kind::PartsNew), Language::De) if p("part").is_empty() => format!("Neue Stimmen: {}", p("work")),
        (Some(Kind::PartsNew), Language::En) => format!("New part: {} – {}", p("part"), p("work")),
        (Some(Kind::PartsNew), Language::De) => format!("Neue Stimme: {} – {}", p("part"), p("work")),
        (Some(Kind::ProgramChanged), Language::En) => {
            format!("Programme “{}” ({}) was updated", p("program"), p("ensemble"))
        }
        (Some(Kind::ProgramChanged), Language::De) => {
            format!("Programm „{}“ ({}) wurde aktualisiert", p("program"), p("ensemble"))
        }
        (Some(Kind::LoanDue), Language::En) if params["overdue"] == true => {
            format!("Overdue: please return {} of “{}” (was due {})", p("item"), p("work"), p("date"))
        }
        (Some(Kind::LoanDue), Language::De) if params["overdue"] == true => {
            format!("Überfällig: bitte {} von „{}“ zurückgeben (fällig am {})", p("item"), p("work"), p("date"))
        }
        (Some(Kind::LoanDue), Language::En) => {
            format!("Please return {} of “{}” by {}", p("item"), p("work"), p("date"))
        }
        (Some(Kind::LoanDue), Language::De) => {
            format!("Bitte {} von „{}“ bis {} zurückgeben", p("item"), p("work"), p("date"))
        }
        (Some(Kind::AnnotationsShared), Language::En) => {
            format!("New markings by {} in {} – {}", p("by"), p("part"), p("work"))
        }
        (Some(Kind::AnnotationsShared), Language::De) => {
            format!("Neue Eintragungen von {} in {} – {}", p("by"), p("part"), p("work"))
        }
        (Some(Kind::GuestLinkExpiring), Language::En) => {
            format!("The guest link for {} expires on {}", p("guest"), p("date"))
        }
        (Some(Kind::GuestLinkExpiring), Language::De) => {
            format!("Der Gastlink für {} läuft am {} ab", p("guest"), p("date"))
        }
        (None, _) => kind.to_string(),
    };
    match link {
        Some(link) if link != "/" => format!("{text}\n  {public_url}{link}"),
        _ => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn lines_in_both_languages() {
        let params = json!({ "part": "Trompete 2", "work": "Polka" });
        assert_eq!(
            describe(Language::De, "partsNew", &params, Some("/works/1"), "https://n.example"),
            "Neue Stimme: Trompete 2 – Polka\n  https://n.example/works/1"
        );
        let loan = json!({ "item": "Partitur", "work": "Polka", "date": "2027-01-10", "overdue": true });
        assert!(describe(Language::En, "loanDue", &loan, Some("/"), "x").starts_with("Overdue: please return"));
    }
}
