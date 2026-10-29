// SPDX-License-Identifier: AGPL-3.0-or-later

//! For admins (`system` in api/openapi.json): what this server is and can do, how much it holds, a test email, and
//! the checklist for setting notenwart up.

use serde::{Deserialize, Serialize};

use crate::App;
use crate::auth::CurrentUser;
use crate::error::{AppError, AppResult};
use notenwart_store::system as db;

/// Steps admins tick off themselves.
const MANUAL: [&str; 3] = ["instruments", "backups", "testMail"];

/// The steps of the checklist, as the API names them.
#[derive(Debug)]
pub struct StepKey;

impl crate::schema::Strings for StepKey {
    const VALUES: &[&str] =
        &["logo", "instruments", "ensembles", "people", "music", "mail", "testMail", "sso", "backups"];
}

/// The steps admins tick off themselves, as the API names them.
#[derive(Debug)]
pub struct ManualStep;

impl crate::schema::Strings for ManualStep {
    const VALUES: &[&str] = &MANUAL;
}

impl utoipa::PartialSchema for ManualStep {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::Schema> {
        crate::schema::strings::<ManualStep>().into()
    }
}

impl utoipa::ToSchema for ManualStep {}

/// The languages of emails, as the API names them.
#[derive(Debug)]
pub struct LanguageName;

impl crate::schema::Strings for LanguageName {
    const VALUES: &[&str] = &["en", "de"];
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    #[schema(schema_with = crate::schema::strings::<StepKey>)]
    pub key: &'static str,
    pub done: bool,
    /// Not needed for every organisation.
    pub optional: bool,
    /// Ticked by hand rather than recognised.
    pub manual: bool,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub people: i64,
    pub ensembles: i64,
    pub works: i64,
    pub parts: i64,
    pub recordings: i64,
    /// Uploaded sources, recordings and images; parts are cut from the sources and not counted twice.
    pub stored_bytes: i64,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct System {
    pub version: &'static str,
    pub public_url: String,
    #[schema(schema_with = crate::schema::strings::<LanguageName>)]
    pub language: &'static str,
    /// s3 or fs.
    pub storage: &'static str,
    pub max_upload_bytes: usize,
    /// The sender of emails; none when the server sends none.
    pub mail_from: Option<String>,
    pub recognition: crate::recognize::Abilities,
    /// The external AI reading pages, if any.
    pub ai: Option<String>,
    pub sso: Option<String>,
    #[schema(inline)]
    pub counts: Counts,
    #[schema(inline)]
    pub checklist: Vec<Step>,
}

/// An organisation on this server, for the operator to choose at the console.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrganizationChoice {
    pub id: uuid::Uuid,
    pub name: String,
}

/// Every organisation on this server, oldest first. Only for the operator at the console (`notenwart tui`), who
/// already holds the server's own access; no person of an organisation reaches it.
pub async fn organizations(state: &App) -> AppResult<Vec<OrganizationChoice>> {
    Ok(db::organizations(&state.pool)
        .await
        .map(|rows| rows.into_iter().map(OrganizationChoice::from).collect::<Vec<_>>())?)
}

pub async fn system(state: &App, user: &CurrentUser) -> AppResult<System> {
    user.require(user.is_admin())?;
    let org = user.organization_id;
    let c = db::system(&state.pool, org).await?;
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

#[derive(Debug, Serialize, utoipa::ToSchema)]
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

#[derive(Debug, Deserialize, utoipa::ToSchema)]
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
    db::mark(&state.pool, organization_id, key, done).await?;
    Ok(())
}

impl From<db::OrganizationchoiceRow> for OrganizationChoice {
    fn from(row: db::OrganizationchoiceRow) -> Self {
        OrganizationChoice { id: row.id, name: row.name }
    }
}
