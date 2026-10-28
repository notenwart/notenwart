// SPDX-License-Identifier: AGPL-3.0-or-later

//! Outgoing email (sign-in links, invitations) over SMTP. Without SMTP the links are written to the log.

use anyhow::Context;
use clap::ValueEnum;
use lettre::message::Mailbox;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::app::AppState;

pub struct Mailer {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
}

impl Mailer {
    /// `url` as lettre understands it, e.g. `smtps://user:password@mail.example.org` or
    /// `smtp://mail.example.org:587?tls=required`.
    pub fn new(url: &str, from: &str) -> anyhow::Result<Self> {
        let transport = AsyncSmtpTransport::<Tokio1Executor>::from_url(url)
            .context("SMTP_URL is not valid")?
            .build();
        let from = from.parse().context("MAIL_FROM is not a valid address")?;
        Ok(Mailer { transport, from })
    }

    /// The sender address.
    pub fn from(&self) -> String {
        self.from.to_string()
    }

    pub async fn send(&self, to: &str, subject: &str, body: &str) -> anyhow::Result<()> {
        let message = Message::builder()
            .from(self.from.clone())
            .to(to.parse().context("invalid recipient")?)
            .subject(subject)
            .body(body.to_string())?;
        self.transport.send(message).await?;
        Ok(())
    }
}

/// Sends the mail; returns false and logs the text when mail is not configured or sending fails, so an admin can
/// still hand the link over.
pub async fn send_or_log(state: &AppState, to: &str, subject: &str, body: &str) -> bool {
    match &state.mailer {
        Some(mailer) => match mailer.send(to, subject, body).await {
            Ok(()) => true,
            Err(error) => {
                tracing::warn!("cannot send mail to {to}: {error:#}");
                false
            }
        },
        None => {
            tracing::info!("mail is not configured; would send to {to}: {subject}\n{body}");
            false
        }
    }
}

/// Language of the emails the server sends (the web app picks its own per user).
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Language {
    En,
    De,
}

pub struct Email {
    pub subject: String,
    pub body: String,
}

/// What happened since the last email: one line per notification, each with its link.
pub fn digest(language: Language, name: &str, lines: &[String], public_url: &str) -> Email {
    let list = lines.iter().map(|l| format!("- {l}")).collect::<Vec<_>>().join("\n");
    match language {
        Language::En => Email {
            subject: if lines.len() == 1 {
                "notenwart: 1 new notification".into()
            } else {
                format!("notenwart: {} new notifications", lines.len())
            },
            body: format!("Hello {name},\n\n{list}\n\nWhich notifications come by email: {public_url}/account\n"),
        },
        Language::De => Email {
            subject: if lines.len() == 1 {
                "notenwart: 1 neue Benachrichtigung".into()
            } else {
                format!("notenwart: {} neue Benachrichtigungen", lines.len())
            },
            body: format!(
                "Hallo {name},\n\n{list}\n\nWelche Benachrichtigungen per E-Mail kommen: {public_url}/account\n"
            ),
        },
    }
}

/// The one-time sign-in link someone asked for.
pub fn sign_in_link(language: Language, name: &str, url: &str, minutes: i64) -> Email {
    match language {
        Language::En => Email {
            subject: "Your notenwart sign-in link".into(),
            body: format!(
                "Hello {name},\n\nopen this link to sign in to notenwart:\n\n{url}\n\nThe link works once and expires \
                 in {minutes} minutes. If you did not ask for it, ignore this email.\n"
            ),
        },
        Language::De => Email {
            subject: "Ihr Anmeldelink für notenwart".into(),
            body: format!(
                "Hallo {name},\n\nmit diesem Link melden Sie sich bei notenwart an:\n\n{url}\n\nDer Link funktioniert \
                 einmal und gilt {minutes} Minuten. Wenn Sie ihn nicht angefordert haben, ignorieren Sie diese E-Mail.\n"
            ),
        },
    }
}

/// An invitation from an admin: sign in once with the link, then choose a password.
pub fn invitation(language: Language, name: &str, url: &str, days: i64) -> Email {
    match language {
        Language::En => Email {
            subject: "Your invitation to notenwart".into(),
            body: format!(
                "Hello {name},\n\nyou have been invited to notenwart. Open this link to sign in and choose a \
                 password:\n\n{url}\n\nThe link works once and expires in {days} days.\n"
            ),
        },
        Language::De => Email {
            subject: "Ihre Einladung zu notenwart".into(),
            body: format!(
                "Hallo {name},\n\nSie wurden zu notenwart eingeladen. Öffnen Sie diesen Link, um sich anzumelden und \
                 ein Passwort zu wählen:\n\n{url}\n\nDer Link funktioniert einmal und gilt {days} Tage.\n"
            ),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emails_exist_in_both_languages() {
        for language in [Language::En, Language::De] {
            let link = sign_in_link(language, "Anna", "https://x.test/auth?token=t", 20);
            assert!(link.body.contains("Anna") && link.body.contains("https://x.test/auth?token=t"));
            assert!(link.body.contains("20"));
            let invite = invitation(language, "Ben", "https://x.test/auth?token=u", 7);
            assert!(invite.body.contains("Ben") && invite.body.contains("token=u") && invite.body.contains('7'));
        }
        assert!(invitation(Language::De, "Ben", "u", 7).subject.contains("Einladung"));
    }
}
