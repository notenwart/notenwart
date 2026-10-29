// SPDX-License-Identifier: AGPL-3.0-or-later

//! For admins (`system` in api/openapi.yaml): what this server is and can do, how much it holds, a test email, and
//! the checklist for setting notenwart up.

use serde::{Deserialize, Serialize};

use crate::App;
use crate::auth::CurrentUser;
use crate::error::{AppError, AppResult};

/// Steps admins tick off themselves.
const MANUAL: [&str; 3] = ["instruments", "backups", "testMail"];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    key: &'static str,
    done: bool,
    /// Not needed for every organisation.
    optional: bool,
    /// Ticked by hand rather than recognised.
    manual: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    people: i64,
    ensembles: i64,
    works: i64,
    parts: i64,
    recordings: i64,
    /// Uploaded sources, recordings and images; parts are cut from the sources and not counted twice.
    stored_bytes: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct System {
    version: &'static str,
    public_url: String,
    language: &'static str,
    storage: &'static str,
    max_upload_bytes: usize,
    /// The sender of emails; none when the server sends none.
    mail_from: Option<String>,
    recognition: crate::recognize::Abilities,
    ai: Option<String>,
    sso: Option<String>,
    counts: Counts,
    checklist: Vec<Step>,
}

pub async fn system(state: &App, user: &CurrentUser) -> AppResult<System> {
    user.require(user.is_admin())?;
    let org = user.organization_id;
    let c = sqlx::query!(
        r#"SELECT (SELECT count(*) FROM memberships WHERE organization_id = $1) AS "people!",
                  (SELECT count(*) FROM ensembles WHERE organization_id = $1) AS "ensembles!",
                  (SELECT count(*) FROM works WHERE organization_id = $1) AS "works!",
                  (SELECT count(*) FROM parts p JOIN works w ON w.id = p.work_id WHERE w.organization_id = $1) AS "parts!",
                  (SELECT count(*) FROM recordings WHERE organization_id = $1) AS "recordings!",
                  (SELECT coalesce(sum(s.size), 0) FROM source_files s JOIN works w ON w.id = s.work_id
                     WHERE w.organization_id = $1)::bigint
                  + (SELECT coalesce(sum(size), 0) FROM recordings WHERE organization_id = $1)::bigint
                  + (SELECT coalesce(sum(size), 0) FROM images WHERE organization_id = $1)::bigint AS "bytes!",
                  o.logo_id IS NOT NULL AS "logo!", o.setup_done
           FROM organizations o WHERE o.id = $1"#,
        org
    )
    .fetch_one(&state.pool)
    .await?;
    let manual = |key: &str| c.setup_done.iter().any(|d| d == key);
    let checklist = vec![
        Step { key: "logo", done: c.logo, optional: true, manual: false },
        Step { key: "instruments", done: manual("instruments"), optional: false, manual: true },
        Step { key: "ensembles", done: c.ensembles > 0, optional: false, manual: false },
        Step { key: "people", done: c.people > 1, optional: false, manual: false },
        Step { key: "music", done: c.parts > 0, optional: false, manual: false },
        Step { key: "mail", done: state.mailer.is_some(), optional: false, manual: false },
        Step { key: "testMail", done: manual("testMail"), optional: false, manual: true },
        Step { key: "sso", done: state.sso.is_some(), optional: true, manual: false },
        Step { key: "backups", done: manual("backups"), optional: false, manual: true },
    ];
    Ok(System {
        version: env!("CARGO_PKG_VERSION"),
        public_url: state.config.public_url.clone(),
        language: match state.config.language {
            crate::mail::Language::En => "en",
            crate::mail::Language::De => "de",
        },
        storage: state.config.storage,
        max_upload_bytes: state.config.max_upload,
        mail_from: state.mailer.as_ref().map(|m| m.from()),
        recognition: state.recognizer.abilities,
        ai: state.recognizer.ai_description(),
        sso: state.sso.as_ref().map(|s| format!("{} ({})", s.config.name, s.config.issuer)),
        counts: Counts {
            people: c.people,
            ensembles: c.ensembles,
            works: c.works,
            parts: c.parts,
            recordings: c.recordings,
            stored_bytes: c.bytes,
        },
        checklist,
    })
}

#[derive(Debug, Serialize)]
pub struct TestMail {
    sent: bool,
    to: String,
}

/// Sends a test email to the admin; a success ticks off "test email".
pub async fn test_mail(state: &App, user: &CurrentUser) -> AppResult<TestMail> {
    user.require(user.is_admin())?;
    let Some(mailer) = &state.mailer else {
        return Err(AppError::Conflict("this server sends no email (SMTP_URL is not set)".into()));
    };
    let (subject, body) = match state.config.language {
        crate::mail::Language::En => (
            "notenwart: test email",
            format!("Hello {},\n\nemail from notenwart works.\n\n{}\n", user.name, state.config.public_url),
        ),
        crate::mail::Language::De => (
            "notenwart: Test-E-Mail",
            format!("Hallo {},\n\nE-Mails von notenwart kommen an.\n\n{}\n", user.name, state.config.public_url),
        ),
    };
    let sent = match mailer.send(&user.email, subject, &body).await {
        Ok(()) => true,
        Err(error) => {
            tracing::warn!("test email to {} failed: {error:#}", user.email);
            false
        }
    };
    if sent {
        mark(state, user.organization_id, "testMail", true).await?;
    }
    Ok(TestMail { sent, to: user.email.clone() })
}

#[derive(Debug, Deserialize)]
pub struct TickInput {
    done: bool,
}

pub async fn tick(state: &App, user: &CurrentUser, key: String, input: TickInput) -> AppResult<()> {
    user.require(user.is_admin())?;
    if !MANUAL.contains(&key.as_str()) {
        return Err(AppError::Invalid("this step is recognised by itself".into()));
    }
    mark(state, user.organization_id, &key, input.done).await?;
    Ok(())
}

async fn mark(state: &App, organization_id: uuid::Uuid, key: &str, done: bool) -> AppResult<()> {
    sqlx::query!(
        "UPDATE organizations SET setup_done = CASE WHEN $3 THEN array_append(array_remove(setup_done, $2), $2)
                                                  ELSE array_remove(setup_done, $2) END
         WHERE id = $1",
        organization_id,
        key,
        done
    )
    .execute(&state.pool)
    .await?;
    Ok(())
}
