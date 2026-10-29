// SPDX-License-Identifier: AGPL-3.0-or-later
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use uuid::Uuid;

use super::console::{EnsembleRow, Input, Organization, Person, Tab, WorkRow};
use super::*;
use notenwart_app::auth::Role;

fn click(column: u16, row: u16) -> MouseEvent {
    MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column, row, modifiers: KeyModifiers::NONE }
}

fn wheel(down: bool) -> MouseEvent {
    let kind = if down { MouseEventKind::ScrollDown } else { MouseEventKind::ScrollUp };
    MouseEvent { kind, column: 0, row: 0, modifiers: KeyModifiers::NONE }
}

fn key(console: &mut Console, code: KeyCode) -> Option<Request> {
    console.key(code.into())
}

fn typed(console: &mut Console, text: &str) {
    for c in text.chars() {
        assert_eq!(key(console, KeyCode::Char(c)), None);
    }
}

fn person(name: &str, role: Role) -> Person {
    Person {
        id: Uuid::new_v4(),
        name: name.into(),
        email: format!("{}@example.org", name.to_lowercase()),
        role,
        last_login: None,
    }
}

fn organization(name: &str) -> Organization {
    Organization { id: Uuid::new_v4(), name: name.into() }
}

fn frame(console: &mut Console) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(100, 20)).unwrap();
    terminal.draw(|f| draw(f, console)).unwrap();
    terminal
}

/// A console in an organisation, on the people tab with three people.
fn with_people() -> Console {
    let (mut console, _) = Console::new();
    assert_eq!(console.outcome(Outcome::Organizations(vec![organization("Music school")])), Some(Request::Overview));
    assert_eq!(key(&mut console, KeyCode::Char('2')), Some(Request::People));
    console.outcome(Outcome::People(vec![
        person("Anna", Role::Admin),
        person("Ben", Role::Member),
        person("Clara", Role::Leader),
    ]));
    console
}

#[test]
fn several_organisations_are_chosen_from() {
    let (mut console, first) = Console::new();
    assert_eq!(first, Request::Organizations);
    let two = vec![organization("One"), organization("Two")];
    assert_eq!(console.outcome(Outcome::Organizations(two.clone())), None);
    assert!(console.choosing());
    assert_eq!(key(&mut console, KeyCode::Char('2')), None, "no tabs before choosing");
    key(&mut console, KeyCode::Down);
    assert_eq!(key(&mut console, KeyCode::Enter), Some(Request::Overview));
    assert_eq!(console.organization, Some(two[1].clone()));
}

#[test]
fn no_organisation_says_so() {
    let (mut console, _) = Console::new();
    assert_eq!(console.outcome(Outcome::Organizations(vec![])), None);
    assert!(console.message.contains("notenwart serve"));
}

#[test]
fn tabs_by_key_and_by_click() {
    let (mut console, _) = Console::new();
    console.outcome(Outcome::Organizations(vec![organization("Music school")]));
    assert_eq!(key(&mut console, KeyCode::Tab), Some(Request::People));
    assert_eq!(key(&mut console, KeyCode::Char('4')), Some(Request::Works { query: String::new() }));
    assert_eq!(key(&mut console, KeyCode::BackTab), Some(Request::Ensembles));
    frame(&mut console);
    let (area, tab) = console.areas.tabs[0];
    assert_eq!(tab, Tab::Overview);
    assert_eq!(console.mouse(click(area.x + 1, area.y)), Some(Request::Overview));
    assert_eq!(console.tab(), Tab::Overview);
}

#[test]
fn people_are_searched_as_typed() {
    let mut console = with_people();
    assert_eq!(key(&mut console, KeyCode::Char('/')), None);
    typed(&mut console, "cla");
    assert_eq!(console.visible_people().len(), 1);
    key(&mut console, KeyCode::Enter);
    assert_eq!(console.people_filter, "cla");
    assert_eq!(console.input, None);
    assert_eq!(console.visible_people()[0].name, "Clara");
}

#[test]
fn roles_go_round_and_people_are_created() {
    let mut console = with_people();
    key(&mut console, KeyCode::Down);
    let ben = console.people[1].clone();
    assert_eq!(
        key(&mut console, KeyCode::Char('r')),
        Some(Request::SetRole { id: ben.id, name: ben.name, email: ben.email, role: Role::Admin })
    );
    key(&mut console, KeyCode::Char('n'));
    typed(&mut console, "Dora");
    key(&mut console, KeyCode::Tab);
    typed(&mut console, "dora@example.org");
    key(&mut console, KeyCode::Tab);
    key(&mut console, KeyCode::Right);
    assert_eq!(
        key(&mut console, KeyCode::Enter),
        Some(Request::CreatePerson { name: "Dora".into(), email: "dora@example.org".into(), role: Role::Admin })
    );
    assert_eq!(console.outcome(Outcome::Done("invitation for Dora".into())), Some(Request::People));
}

#[test]
fn deleting_needs_a_yes() {
    let mut console = with_people();
    let anna = console.people[0].clone();
    assert_eq!(key(&mut console, KeyCode::Char('d')), None);
    assert!(matches!(console.input, Some(Input::ConfirmDelete { .. })));
    assert_eq!(key(&mut console, KeyCode::Char('n')), None, "anything but y keeps them");
    key(&mut console, KeyCode::Char('d'));
    assert_eq!(key(&mut console, KeyCode::Char('y')), Some(Request::Delete { id: anna.id }));
}

#[test]
fn clicks_and_the_wheel_do_what_keys_do() {
    let mut console = with_people();
    frame(&mut console);
    let list = console.areas.list;
    // A click selects a row, a second click on it opens it (here: nothing to open for people).
    assert_eq!(console.mouse(click(list.x + 2, list.y + 2)), None);
    assert_eq!(console.selected(), Some(2));
    console.mouse(wheel(false));
    assert_eq!(console.selected(), Some(1));
    console.mouse(wheel(true));
    assert_eq!(console.selected(), Some(2));
    // The action bar: "i invite" asks for a new link for the selected person.
    let (area, _) = *console.areas.actions.iter().find(|(_, code)| *code == KeyCode::Char('i')).unwrap();
    let clara = console.people[2].id;
    assert_eq!(console.mouse(click(area.x, area.y)), Some(Request::Invite { id: clara }));
    // Nothing there.
    assert_eq!(console.mouse(click(99, 19)), None);
    let (quit, _) = *console.areas.actions.iter().find(|(_, code)| *code == KeyCode::Char('q')).unwrap();
    console.mouse(click(quit.x + 1, quit.y));
    assert!(console.quit);
}

#[test]
fn ensembles_and_works_open_and_close() {
    let (mut console, _) = Console::new();
    console.outcome(Outcome::Organizations(vec![organization("Music school")]));
    key(&mut console, KeyCode::Char('3'));
    let id = Uuid::new_v4();
    console.outcome(Outcome::Ensembles(vec![EnsembleRow { id, name: "Choir".into(), members: 12, works: 3 }]));
    frame(&mut console);
    let list = console.areas.list;
    console.mouse(click(list.x, list.y));
    assert_eq!(console.mouse(click(list.x, list.y)), Some(Request::Ensemble { id }), "second click opens");
    console.outcome(Outcome::Ensemble(console::EnsembleView {
        name: "Choir".into(),
        members: vec![("Anna".into(), true, "Soprano".into())],
        works: vec![],
    }));
    let screen = format!("{:?}", frame(&mut console).backend().buffer());
    assert!(screen.contains("Anna (leader)"));
    key(&mut console, KeyCode::Esc);
    assert!(console.ensemble.is_none());
    assert!(!console.quit, "esc leaves the detail first");

    key(&mut console, KeyCode::Char('4'));
    key(&mut console, KeyCode::Char('/'));
    typed(&mut console, "bach");
    assert_eq!(key(&mut console, KeyCode::Enter), Some(Request::Works { query: "bach".into() }));
    let work = Uuid::new_v4();
    console.outcome(Outcome::Works(vec![WorkRow { id: work, title: "Air".into(), composer: None, parts: 4 }]));
    assert_eq!(key(&mut console, KeyCode::Enter), Some(Request::Work { id: work }));
}

#[test]
fn the_mouse_can_be_handed_back() {
    let mut console = with_people();
    assert!(console.mouse);
    key(&mut console, KeyCode::Char('m'));
    assert!(!console.mouse);
    assert!(console.actions().iter().any(|(_, label)| *label == "mouse off"));
}
