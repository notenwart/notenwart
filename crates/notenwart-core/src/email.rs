// SPDX-License-Identifier: AGPL-3.0-or-later

//! The emails notenwart sends, in English and German: their subjects and texts. Sending them is the app's part.

/// Language of the emails the server sends (the web app picks its own per user).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    En,
    De,
}

#[derive(Debug)]
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
