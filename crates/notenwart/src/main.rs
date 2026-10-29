// SPDX-License-Identifier: AGPL-3.0-or-later

//! The command line of notenwart: `serve` (the API with the web app) and `migrate`. It reads the configuration,
//! connects to the database and the object store and hands an [`App`] to an interface; room for `tui` and
//! `openapi` later.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum};
use tracing_subscriber::EnvFilter;

use notenwart_app::mail::{Language, Mailer};
use notenwart_app::{App, Config, catalogue};
use notenwart_store::objects::{Backend, StorageConfig};

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// PostgreSQL connection string.
    #[arg(long, env = "DATABASE_URL", global = true)]
    database_url: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Args)]
struct ServeArgs {
    #[arg(long, env = "NOTENWART_LISTEN", default_value = "0.0.0.0:8080")]
    listen: SocketAddr,
    /// Name of the organisation created on the first start.
    #[arg(long, env = "NOTENWART_ORGANIZATION", default_value = "notenwart")]
    organization: String,
    /// Largest accepted upload in MiB.
    #[arg(long, env = "NOTENWART_MAX_UPLOAD_MB", default_value_t = 200)]
    max_upload_mb: usize,
    /// Where users reach the server; links in emails point here. https also marks the session cookie Secure.
    #[arg(long, env = "NOTENWART_PUBLIC_URL", default_value = "http://localhost:8080")]
    public_url: String,
    /// SMTP server for sign-in links and invitations, e.g. smtps://user:password@mail.example.org.
    /// Without it, links are written to the log and admins hand out invitation links themselves.
    #[arg(long, env = "SMTP_URL", hide_env_values = true)]
    smtp_url: Option<String>,
    /// Language of emails (sign-in links, invitations): en or de.
    #[arg(long, env = "NOTENWART_LANGUAGE", value_enum, default_value = "en")]
    language: LanguageArg,
    /// Sender address, e.g. "notenwart <noten@example.org>".
    #[arg(long, env = "MAIL_FROM", default_value = "notenwart <notenwart@localhost>")]
    mail_from: String,
    /// Tesseract languages for reading scans, e.g. deu+eng (those not installed are skipped).
    #[arg(long, env = "NOTENWART_OCR_LANGUAGES", default_value = "deu+eng")]
    ocr_languages: String,
    /// An external AI that also reads first pages (title, composer, part, clef, key): anthropic or openai (any
    /// OpenAI-compatible API). Off unless set; page images and text are sent to this provider.
    #[arg(long, env = "NOTENWART_AI_PROVIDER")]
    ai_provider: Option<String>,
    /// Base URL of the AI API; defaults to the provider's public API (e.g. http://localhost:11434/v1 for Ollama).
    #[arg(long, env = "NOTENWART_AI_URL")]
    ai_url: Option<String>,
    #[arg(long, env = "NOTENWART_AI_KEY", hide_env_values = true)]
    ai_key: Option<String>,
    /// Model name; for Anthropic claude-opus-5-5 unless set, required for OpenAI-compatible APIs.
    #[arg(long, env = "NOTENWART_AI_MODEL")]
    ai_model: Option<String>,
    /// Send the page image to the AI (needs a model that reads images); false sends only the text.
    #[arg(long, env = "NOTENWART_AI_IMAGES", default_value_t = true, action = clap::ArgAction::Set)]
    ai_images: bool,
    /// Sign-in with the organisation's accounts (OpenID Connect): the provider's issuer URL, e.g.
    /// https://login.microsoftonline.com/<tenant>/v2.0. Only people who exist in notenwart get in (by email).
    #[arg(long, env = "NOTENWART_SSO_ISSUER")]
    sso_issuer: Option<String>,
    #[arg(long, env = "NOTENWART_SSO_CLIENT_ID")]
    sso_client_id: Option<String>,
    #[arg(long, env = "NOTENWART_SSO_CLIENT_SECRET", hide_env_values = true)]
    sso_client_secret: Option<String>,
    /// Shown on the sign-in button, e.g. "Microsoft 365".
    #[arg(long, env = "NOTENWART_SSO_NAME")]
    sso_name: Option<String>,
    /// The ID token claim with the email address.
    #[arg(long, env = "NOTENWART_SSO_EMAIL_CLAIM")]
    sso_email_claim: Option<String>,
    #[command(flatten)]
    storage: StorageArgs,
}

/// Language of emails.
#[derive(Clone, Copy, ValueEnum)]
enum LanguageArg {
    En,
    De,
}

impl From<LanguageArg> for Language {
    fn from(language: LanguageArg) -> Self {
        match language {
            LanguageArg::En => Language::En,
            LanguageArg::De => Language::De,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum BackendArg {
    S3,
    Fs,
}

#[derive(clap::Args)]
struct StorageArgs {
    /// Where to keep PDFs.
    #[arg(long, env = "STORAGE", value_enum, default_value = "fs")]
    storage: BackendArg,
    /// Directory for `--storage fs`.
    #[arg(long, env = "STORAGE_PATH", default_value = "data")]
    storage_path: PathBuf,
    /// S3 endpoint, e.g. http://garage:3900 (empty: AWS).
    #[arg(long, env = "S3_ENDPOINT")]
    s3_endpoint: Option<String>,
    #[arg(long, env = "S3_REGION", default_value = "garage")]
    s3_region: String,
    #[arg(long, env = "S3_BUCKET", default_value = "notenwart")]
    s3_bucket: String,
    #[arg(long, env = "S3_ACCESS_KEY_ID")]
    s3_access_key_id: Option<String>,
    #[arg(long, env = "S3_SECRET_ACCESS_KEY", hide_env_values = true)]
    s3_secret_access_key: Option<String>,
}

impl From<StorageArgs> for StorageConfig {
    fn from(args: StorageArgs) -> Self {
        StorageConfig {
            storage: match args.storage {
                BackendArg::S3 => Backend::S3,
                BackendArg::Fs => Backend::Fs,
            },
            storage_path: args.storage_path,
            s3_endpoint: args.s3_endpoint,
            s3_region: args.s3_region,
            s3_bucket: args.s3_bucket,
            s3_access_key_id: args.s3_access_key_id,
            s3_secret_access_key: args.s3_secret_access_key,
        }
    }
}

#[derive(Subcommand)]
enum Command {
    /// Run pending migrations, then serve the API and the web app.
    Serve(Box<ServeArgs>),
    /// Run pending database migrations and exit.
    Migrate,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // A local `.env` (see `.env.example`) is a convenience for development; real environment variables win.
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "notenwart=info,tower_http=info".into()))
        .init();

    let cli = Cli::parse();
    let database_url = cli.database_url.context("DATABASE_URL is not set")?;
    let pool = notenwart_store::connect(&database_url).await.context("cannot connect to the database")?;

    notenwart_store::MIGRATOR.run(&pool).await.context("migrations failed")?;

    match cli.command {
        Command::Migrate => {
            tracing::info!("database is up to date");
        }
        Command::Serve(args) => {
            let args = *args;
            let language = Language::from(args.language);
            let storage = StorageConfig::from(args.storage);
            let store = storage.open().context("cannot open the storage")?;
            let organization_id = notenwart_store::organization::ensure_default(&pool, &args.organization).await?;
            catalogue::ensure(&pool, organization_id, language).await?;
            // Compose passes unset variables as empty strings: treat those as not set.
            let mailer = match args.smtp_url.as_deref().map(str::trim).filter(|u| !u.is_empty()) {
                Some(url) => Some(Arc::new(Mailer::new(url, &args.mail_from)?)),
                None => {
                    tracing::warn!("SMTP_URL is not set: sign-in links and invitations are only written to the log");
                    None
                }
            };
            // Compose passes unset variables as empty strings: treat those as not set.
            let filled = |v: &Option<String>| v.as_deref().map(str::trim).filter(|v| !v.is_empty()).map(String::from);
            let provider = match filled(&args.ai_provider).as_deref() {
                None => None,
                Some("anthropic") => Some(notenwart_app::ai::Provider::Anthropic),
                Some("openai") => Some(notenwart_app::ai::Provider::Openai),
                Some(other) => anyhow::bail!("NOTENWART_AI_PROVIDER must be anthropic or openai, not {other}"),
            };
            let ai = match provider {
                Some(provider) => {
                    let ai = notenwart_app::ai::Ai::new(notenwart_app::ai::AiConfig {
                        provider,
                        url: filled(&args.ai_url),
                        key: filled(&args.ai_key),
                        model: filled(&args.ai_model),
                        images: args.ai_images,
                    })?;
                    tracing::info!("first pages are also read by {}", ai.describe());
                    Some(ai)
                }
                None => None,
            };
            let recognizer = notenwart_app::recognize::Recognizer::new(&args.ocr_languages, ai).await;
            let sso = match (filled(&args.sso_issuer), filled(&args.sso_client_id)) {
                (Some(issuer), Some(client_id)) => {
                    tracing::info!("single sign-on through {issuer}");
                    Some(Arc::new(notenwart_app::sso::Sso::new(notenwart_app::sso::SsoConfig {
                        issuer,
                        client_id,
                        client_secret: filled(&args.sso_client_secret),
                        name: filled(&args.sso_name).unwrap_or_else(|| "single sign-on".into()),
                        email_claim: filled(&args.sso_email_claim).unwrap_or_else(|| "email".into()),
                    })?))
                }
                (None, None) => None,
                _ => anyhow::bail!("single sign-on needs both NOTENWART_SSO_ISSUER and NOTENWART_SSO_CLIENT_ID"),
            };
            let public_url = args.public_url.trim_end_matches('/').to_string();
            let state = App {
                pool,
                store,
                config: Arc::new(Config {
                    secure_cookies: public_url.starts_with("https://"),
                    public_url,
                    max_upload: args.max_upload_mb * 1024 * 1024,
                    language,
                    storage: storage.storage.name(),
                }),
                mailer,
                limiter: Arc::default(),
                recognizer,
                sso,
            };
            notenwart_app::notify::spawn_worker(state.clone());
            let listen = args.listen;
            let listener =
                tokio::net::TcpListener::bind(listen).await.with_context(|| format!("cannot listen on {listen}"))?;
            tracing::info!("listening on http://{listen}");
            notenwart_api::serve(listener, state, shutdown()).await?;
        }
    }
    Ok(())
}

async fn shutdown() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.ok();
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            signal.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
