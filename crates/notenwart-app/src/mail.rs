// SPDX-License-Identifier: AGPL-3.0-or-later

//! Outgoing email (sign-in links, invitations) over SMTP. Without SMTP the links are written to the log.

use anyhow::Context;
use lettre::message::Mailbox;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

pub use notenwart_core::email::*;

use crate::App;

#[derive(Debug)]
pub struct Mailer {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
}

impl Mailer {
    /// `url` as lettre understands it, e.g. `smtps://user:password@mail.example.org` or
    /// `smtp://mail.example.org:587?tls=required`.
    pub fn new(url: &str, from: &str) -> anyhow::Result<Self> {
        let transport = AsyncSmtpTransport::<Tokio1Executor>::from_url(url).context("SMTP_URL is not valid")?.build();
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
pub async fn send_or_log(state: &App, to: &str, subject: &str, body: &str) -> bool {
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
