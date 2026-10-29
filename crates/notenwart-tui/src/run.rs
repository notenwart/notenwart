// SPDX-License-Identifier: AGPL-3.0-or-later

//! The console in a terminal: draws, reads keys and the mouse, and carries out what the console asks for through
//! the commands of `notenwart-app`, as the operator.

use std::collections::HashMap;

use ratatui::crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind};
use ratatui::crossterm::execute;
use tokio::runtime::Handle;

use notenwart_app::auth::CurrentUser;
use notenwart_app::people::UserInput;
use notenwart_app::{App, catalogue, ensembles, library, people, system};

use crate::console::{
    Console, EnsembleRow, EnsembleView, Organization, Outcome, Overview, Person, Request, WorkRow, WorkView, role_name,
};
use crate::{TuiError, draw};

/// Runs the console until it is left. Commands run on `handle`, one at a time.
pub fn run(app: &App, handle: &Handle) -> Result<(), TuiError> {
    let (mut console, first) = Console::new();
    let mut pending = Some(first);
    let mut terminal = ratatui::try_init()?;
    let mut mouse = false;
    let result = (|| -> Result<(), TuiError> {
        while !console.quit {
            // A request may lead to the next one (choosing the only organisation loads the overview).
            while let Some(request) = pending.take() {
                let outcome = handle.block_on(carry_out(app, &console, request));
                pending = console.outcome(outcome);
            }
            if console.mouse != mouse {
                mouse = console.mouse;
                if mouse {
                    execute!(std::io::stdout(), EnableMouseCapture)?;
                } else {
                    execute!(std::io::stdout(), DisableMouseCapture)?;
                }
            }
            terminal.draw(|frame| draw(frame, &mut console))?;
            pending = match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => console.key(key),
                Event::Mouse(event) if console.mouse => console.mouse(event),
                _ => None,
            };
        }
        Ok(())
    })();
    if mouse {
        let _ = execute!(std::io::stdout(), DisableMouseCapture);
    }
    ratatui::restore();
    result
}

/// Carries out a request as the operator of the chosen organisation.
async fn carry_out(app: &App, console: &Console, request: Request) -> Outcome {
    let operator = console.organization.as_ref().map(|o| CurrentUser::operator(o.id, o.name.clone()));
    let result = match (request, operator) {
        (Request::Organizations, _) => system::organizations(app).await.map(|list| {
            Outcome::Organizations(list.into_iter().map(|o| Organization { id: o.id, name: o.name }).collect())
        }),
        (_, None) => return Outcome::Failed("choose an organisation first".into()),
        (request, Some(operator)) => as_operator(app, &operator, request).await,
    };
    result.unwrap_or_else(|error| Outcome::Failed(describe(error)))
}

async fn as_operator(
    app: &App,
    operator: &CurrentUser,
    request: Request,
) -> Result<Outcome, notenwart_app::error::AppError> {
    Ok(match request {
        Request::Organizations => unreachable!("answered without an organisation"),
        Request::Overview => Outcome::Overview(overview(system::system(app, operator).await?)),
        Request::People => Outcome::People(people::list(app, operator).await?.into_iter().map(person).collect()),
        Request::CreatePerson { name, email, role } => {
            let invitation = people::create(app, operator, UserInput { name, email, role }).await?;
            Outcome::Done(invited(&invitation))
        }
        Request::SetRole { id, name, email, role } => {
            let user = people::update(app, operator, id, UserInput { name, email, role }).await?;
            Outcome::Done(format!("{} is now {}", user.row.name, role_name(user.row.role)))
        }
        Request::Invite { id } => Outcome::Done(invited(&people::invite(app, operator, id).await?)),
        Request::Delete { id } => {
            people::remove(app, operator, id).await?;
            Outcome::Done("deleted, with a record of the deletion".into())
        }
        Request::Ensembles => Outcome::Ensembles(
            ensembles::list(app, operator)
                .await?
                .into_iter()
                .map(|e| EnsembleRow { id: e.id, name: e.name, members: e.member_count, works: e.work_count })
                .collect(),
        ),
        Request::Ensemble { id } => {
            let detail = ensembles::detail(app, operator, id).await?;
            let instruments: HashMap<_, _> =
                catalogue::list_instruments(app, operator).await?.into_iter().map(|i| (i.id, i.name)).collect();
            Outcome::Ensemble(EnsembleView {
                name: detail.ensemble.name,
                members: detail
                    .members
                    .into_iter()
                    .map(|m| {
                        let plays = m
                            .plays
                            .iter()
                            .map(|p| {
                                let name = instruments.get(&p.instrument_id).map_or("?", String::as_str);
                                match p.voice {
                                    Some(voice) => format!("{name} {voice}"),
                                    None => name.to_string(),
                                }
                            })
                            .collect::<Vec<_>>()
                            .join(", ");
                        (m.name, m.leader, plays)
                    })
                    .collect(),
                works: detail
                    .works
                    .into_iter()
                    .map(|w| match w.composer {
                        Some(composer) => format!("{} – {composer}", w.title),
                        None => w.title,
                    })
                    .collect(),
            })
        }
        Request::Works { query } => {
            let filter = library::WorkFilter { q: Some(query).filter(|q| !q.is_empty()), tag: None };
            Outcome::Works(
                library::list_works(app, operator, filter)
                    .await?
                    .into_iter()
                    .map(|w| WorkRow {
                        id: w.work.id,
                        title: w.work.title,
                        composer: w.work.composer,
                        parts: w.part_count,
                    })
                    .collect(),
            )
        }
        Request::Work { id } => Outcome::Work(work(library::get_work(app, operator, id).await?)),
    })
}

fn overview(system: system::System) -> Overview {
    let yes_no = |b: bool| if b { "yes" } else { "no" }.to_string();
    Overview {
        facts: vec![
            ("version".into(), system.version.into()),
            ("address".into(), system.public_url),
            ("language".into(), system.language.into()),
            ("storage".into(), system.storage.into()),
            ("upload limit".into(), format!("{} MiB", system.max_upload_bytes / 1024 / 1024)),
            ("email from".into(), system.mail_from.unwrap_or_else(|| "– (links go to the log)".into())),
            ("reads text".into(), yes_no(system.recognition.text)),
            ("reads scans".into(), yes_no(system.recognition.ocr)),
            ("AI".into(), system.ai.unwrap_or_else(|| "–".into())),
            ("single sign-on".into(), system.sso.unwrap_or_else(|| "–".into())),
        ],
        counts: vec![
            ("people".into(), system.counts.people),
            ("ensembles".into(), system.counts.ensembles),
            ("works".into(), system.counts.works),
            ("parts".into(), system.counts.parts),
            ("recordings".into(), system.counts.recordings),
            ("MiB stored".into(), system.counts.stored_bytes / 1024 / 1024),
        ],
        checklist: system.checklist.into_iter().map(|s| (s.key.to_string(), s.done, s.optional)).collect(),
    }
}

fn person(user: people::User) -> Person {
    let row = user.row;
    Person {
        id: row.id,
        name: row.name,
        email: row.email.unwrap_or_default(),
        role: row.role,
        last_login: row.last_login_at.map(|t| t.date().to_string()),
    }
}

fn work(detail: library::WorkDetail) -> WorkView {
    let w = detail.work;
    let mut details = Vec::new();
    for (label, value) in [
        ("composer", w.composer),
        ("arranger", w.arranger),
        ("publisher", w.publisher),
        ("instrumentation", w.instrumentation),
        ("archive", w.archive_location),
    ] {
        if let Some(value) = value {
            details.push((label.to_string(), value));
        }
    }
    if !w.tags.is_empty() {
        details.push(("tags".into(), w.tags.join(", ")));
    }
    WorkView {
        title: w.title,
        details,
        sources: detail.sources.into_iter().map(|s| format!("{} ({})", s.file_name, pages(s.page_count))).collect(),
        parts: detail.parts.into_iter().map(|p| format!("{} ({})", p.label, pages(p.page_count))).collect(),
    }
}

fn pages(n: i32) -> String {
    if n == 1 { "1 page".into() } else { format!("{n} pages") }
}

fn invited(invitation: &people::Invitation) -> String {
    let how = if invitation.emailed { "emailed" } else { "hand it over" };
    format!("invitation for {} ({how}): {}", invitation.user.row.name, invitation.url)
}

fn describe(error: notenwart_app::error::AppError) -> String {
    use notenwart_app::error::AppError;
    match error {
        AppError::Invalid(message) | AppError::Conflict(message) | AppError::TooLarge(message) => message,
        AppError::NotFound => "not found".into(),
        AppError::Forbidden => "not allowed".into(),
        AppError::Unauthorized => "not signed in".into(),
        AppError::TooManyRequests => "too many attempts".into(),
        AppError::Internal(error) => format!("failed: {error:#}"),
    }
}
