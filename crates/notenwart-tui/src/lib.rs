// SPDX-License-Identifier: AGPL-3.0-or-later
//! The terminal interface of notenwart: an operator console on the server (`notenwart tui`). The operator chooses an
//! organisation if there are several and then acts as an admin of it ([`CurrentUser::operator`]): the overview
//! (server facts, counts, setup checklist), people (search, new, role, invitation link, delete), ensembles (members,
//! works) and works (search, sources, parts). Everything with the keyboard, and with the mouse – a click does what
//! the key or the row under it does, the wheel moves like the arrow keys; `m` hands the mouse back to the terminal
//! (to select text, say an invitation link). Like every interface it works through `notenwart-app` only; the role
//! checks of its commands apply to the operator as to any admin.

pub mod console;
mod run;

use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph, Wrap};

pub use console::{Console, Outcome, Request};
pub use notenwart_app::auth::CurrentUser;
pub use run::run;

use console::{Input, Tab, role_name};

/// Why the console ended with an error.
#[derive(Debug, thiserror::Error)]
pub enum TuiError {
    /// The terminal.
    #[error("terminal: {0}")]
    Terminal(#[from] std::io::Error),
}

fn key_label(code: KeyCode) -> String {
    match code {
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Enter => "enter".into(),
        KeyCode::Esc => "esc".into(),
        KeyCode::Tab => "tab".into(),
        other => format!("{other}").to_lowercase(),
    }
}

/// Draws the console and remembers where the clickable parts are.
pub fn draw(frame: &mut Frame, console: &mut Console) {
    let [tabs, main, input, bar] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(3), Constraint::Length(3), Constraint::Length(1)])
            .areas(frame.area());
    let highlight = Style::default().add_modifier(Modifier::REVERSED);

    // The tabs, clickable.
    console.areas.tabs.clear();
    let mut spans = vec![Span::styled(" notenwart ", Style::default().add_modifier(Modifier::BOLD))];
    let mut x = tabs.x + spans[0].width() as u16;
    if let Some(organization) = &console.organization {
        for (i, tab) in Tab::ALL.into_iter().enumerate() {
            let text = format!(" {} {} ", i + 1, tab.title());
            let width = text.chars().count() as u16;
            console.areas.tabs.push((Rect::new(x, tabs.y, width, 1), tab));
            let style = if tab == console.tab() { highlight } else { Style::default() };
            spans.push(Span::styled(text, style));
            spans.push(Span::raw(" "));
            x += width + 1;
        }
        spans.push(Span::raw(format!(" – {} (operator)", organization.name)));
    }
    frame.render_widget(Line::from(spans), tabs);

    // The list or the page.
    console.areas.list = Rect::default();
    if console.choosing() {
        let rows = console.organizations.iter().map(|o| ListItem::new(o.name.clone())).collect();
        list(frame, console, main, " organisations ", rows);
    } else {
        match console.tab() {
            Tab::Overview => overview(frame, console, main),
            Tab::People => {
                let rows = console
                    .visible_people()
                    .into_iter()
                    .map(|p| {
                        ListItem::new(format!(
                            "{:<28} {:<34} {:<10} {}",
                            clip(&p.name, 28),
                            clip(&p.email, 34),
                            role_name(p.role),
                            p.last_login.as_deref().unwrap_or("never signed in")
                        ))
                    })
                    .collect();
                let title = if console.people_filter.is_empty() {
                    " people ".to_string()
                } else {
                    format!(" people – “{}” ", console.people_filter)
                };
                list(frame, console, main, &title, rows);
            }
            Tab::Ensembles => match &console.ensemble {
                Some(view) => {
                    let mut lines = vec![Line::from("members").style(Style::default().add_modifier(Modifier::BOLD))];
                    for (name, leader, plays) in &view.members {
                        let role = if *leader { " (leader)" } else { "" };
                        lines.push(Line::from(format!("  {name}{role}  {plays}")));
                    }
                    lines.push(Line::from(""));
                    lines.push(Line::from("works").style(Style::default().add_modifier(Modifier::BOLD)));
                    lines.extend(view.works.iter().map(|w| Line::from(format!("  {w}"))));
                    page(frame, main, &format!(" {} ", view.name), lines);
                }
                None => {
                    let rows = console
                        .ensembles
                        .iter()
                        .map(|e| {
                            ListItem::new(format!(
                                "{:<40} {:>12} {:>10}",
                                clip(&e.name, 40),
                                count(e.members, "member"),
                                count(e.works, "work")
                            ))
                        })
                        .collect();
                    list(frame, console, main, " ensembles ", rows);
                }
            },
            Tab::Works => match &console.work {
                Some(view) => {
                    let mut lines: Vec<Line> =
                        view.details.iter().map(|(label, value)| Line::from(format!("{label}: {value}"))).collect();
                    for (title, items) in [("sources", &view.sources), ("parts", &view.parts)] {
                        lines.push(Line::from(""));
                        lines.push(Line::from(title).style(Style::default().add_modifier(Modifier::BOLD)));
                        lines.extend(items.iter().map(|i| Line::from(format!("  {i}"))));
                    }
                    page(frame, main, &format!(" {} ", view.title), lines);
                }
                None => {
                    let rows = console
                        .works
                        .iter()
                        .map(|w| {
                            ListItem::new(format!(
                                "{:<44} {:<28} {:>10}",
                                clip(&w.title, 44),
                                clip(w.composer.as_deref().unwrap_or(""), 28),
                                count(w.parts, "part")
                            ))
                        })
                        .collect();
                    let title = if console.works_query.is_empty() {
                        " works ".to_string()
                    } else {
                        format!(" works – “{}” ", console.works_query)
                    };
                    list(frame, console, main, &title, rows);
                }
            },
        }
    }

    // What is being typed, or the last message.
    let text = match &console.input {
        Some(Input::Search(text)) => format!("search: {text}▏"),
        Some(Input::ConfirmDelete { name, .. }) => format!("delete {name} and everything stored about them? y / esc"),
        Some(Input::NewPerson { name, email, role, field }) => {
            let mark = |i: usize| if *field == i { "▏" } else { "" };
            format!("name: {name}{}   email: {email}{}   role: {}{} (←/→)", mark(0), mark(1), role_name(*role), mark(2))
        }
        None => console.message.clone(),
    };
    frame.render_widget(Paragraph::new(text).block(Block::bordered()).wrap(Wrap { trim: false }), input);

    // The actions, clickable.
    console.areas.actions.clear();
    let mut spans = Vec::new();
    let mut x = bar.x;
    for (code, label) in console.actions() {
        let text = format!(" {} {label} ", key_label(code));
        let width = text.chars().count() as u16;
        console.areas.actions.push((Rect::new(x, bar.y, width, 1), code));
        spans.push(Span::styled(text, highlight));
        spans.push(Span::raw(" "));
        x += width + 1;
    }
    frame.render_widget(Line::from(spans), bar);
}

fn list(frame: &mut Frame, console: &mut Console, area: Rect, title: &str, rows: Vec<ListItem>) {
    let mut state = ListState::default().with_selected(console.selected()).with_offset(console.areas.offset);
    let highlight = Style::default().add_modifier(Modifier::REVERSED);
    frame.render_stateful_widget(
        List::new(rows).block(Block::bordered().title(title.to_string())).highlight_style(highlight),
        area,
        &mut state,
    );
    console.areas.list = Rect::new(area.x + 1, area.y + 1, area.width.saturating_sub(2), area.height.saturating_sub(2));
    console.areas.offset = state.offset();
}

fn page(frame: &mut Frame, area: Rect, title: &str, lines: Vec<Line>) {
    frame.render_widget(Paragraph::new(lines).block(Block::bordered().title(title.to_string())), area);
}

fn overview(frame: &mut Frame, console: &Console, area: Rect) {
    let o = &console.overview;
    let bold = Style::default().add_modifier(Modifier::BOLD);
    let mut lines: Vec<Line> =
        o.facts.iter().map(|(label, value)| Line::from(format!("{label:<14} {value}"))).collect();
    lines.push(Line::from(""));
    lines.push(Line::from("holds").style(bold));
    lines.push(Line::from(o.counts.iter().map(|(label, n)| format!("{n} {label}")).collect::<Vec<_>>().join(" · ")));
    lines.push(Line::from(""));
    lines.push(Line::from("setting up").style(bold));
    for (step, done, optional) in &o.checklist {
        let mark = if *done { "[x]" } else { "[ ]" };
        let note = if *optional { " (optional)" } else { "" };
        lines.push(Line::from(format!("  {mark} {step}{note}")));
    }
    page(frame, area, " overview ", lines);
}

/// "1 part", "3 parts".
fn count(n: i64, thing: &str) -> String {
    if n == 1 { format!("1 {thing}") } else { format!("{n} {thing}s") }
}

/// At most `width` characters.
fn clip(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        text.to_string()
    } else {
        text.chars().take(width.saturating_sub(1)).chain(['…']).collect()
    }
}

#[cfg(test)]
mod tests;
