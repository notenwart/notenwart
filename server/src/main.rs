// SPDX-License-Identifier: AGPL-3.0-or-later

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use clap::{Parser, Subcommand};
use sqlx::postgres::PgPoolOptions;
use tracing_subscriber::EnvFilter;

use notenwart::mail::{Language, Mailer};
use notenwart::storage::StorageConfig;
use notenwart::{app, catalogue, organization};

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
    language: Language,
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
    storage: StorageConfig,
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
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .context("cannot connect to the database")?;

    app::MIGRATOR.run(&pool).await.context("migrations failed")?;

    match cli.command {
        Command::Migrate => {
            tracing::info!("database is up to date");
        }
        Command::Serve(args) => {
            let store = args.storage.open().context("cannot open the storage")?;
            let organization_id = organization::ensure_default(&pool, &args.organization).await?;
            catalogue::ensure(&pool, organization_id, args.language).await?;
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
                Some("anthropic") => Some(notenwart::ai::Provider::Anthropic),
                Some("openai") => Some(notenwart::ai::Provider::Openai),
                Some(other) => anyhow::bail!("NOTENWART_AI_PROVIDER must be anthropic or openai, not {other}"),
            };
            let ai = match provider {
                Some(provider) => {
                    let ai = notenwart::ai::Ai::new(notenwart::ai::AiConfig {
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
            let recognizer = notenwart::recognize::Recognizer::new(&args.ocr_languages, ai).await;
            let sso = match (filled(&args.sso_issuer), filled(&args.sso_client_id)) {
                (Some(issuer), Some(client_id)) => {
                    tracing::info!("single sign-on through {issuer}");
                    Some(Arc::new(notenwart::sso::Sso::new(notenwart::sso::SsoConfig {
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
            let state = app::AppState {
                pool,
                store,
                config: Arc::new(app::Config {
                    secure_cookies: public_url.starts_with("https://"),
                    public_url,
                    max_upload: args.max_upload_mb * 1024 * 1024,
                    language: args.language,
                    storage: match args.storage.storage {
                        notenwart::storage::Backend::S3 => "s3",
                        notenwart::storage::Backend::Fs => "fs",
                    },
                }),
                mailer,
                limiter: Arc::default(),
                recognizer,
                sso,
            };
            notenwart::notify::spawn_worker(state.clone());
            let listen = args.listen;
            let listener = tokio::net::TcpListener::bind(listen)
                .await
                .with_context(|| format!("cannot listen on {listen}"))?;
            tracing::info!("listening on http://{listen}");
            axum::serve(listener, app::router(state))
                .with_graceful_shutdown(shutdown())
                .await?;
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
