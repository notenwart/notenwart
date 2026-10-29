// SPDX-License-Identifier: AGPL-3.0-or-later

//! The state of the console and how keys and the mouse change it – without a terminal and without a database: a key
//! may ask for a [`Request`], which the runner carries out through `notenwart-app` and answers with an [`Outcome`].

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};
use uuid::Uuid;

use notenwart_app::auth::Role;

/// The parts of the console, in the order of their keys 1–4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Overview,
    People,
    Ensembles,
    Works,
}

impl Tab {
    pub const ALL: [Tab; 4] = [Tab::Overview, Tab::People, Tab::Ensembles, Tab::Works];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Overview => "overview",
            Tab::People => "people",
            Tab::Ensembles => "ensembles",
            Tab::Works => "works",
        }
    }

    fn index(self) -> usize {
        Tab::ALL.iter().position(|t| *t == self).unwrap_or_default()
    }
}

/// The roles in the order `r` goes through them.
pub const ROLES: [Role; 4] = [Role::Admin, Role::Librarian, Role::Leader, Role::Member];

pub fn role_name(role: Role) -> &'static str {
    match role {
        Role::Admin => "admin",
        Role::Librarian => "librarian",
        Role::Leader => "leader",
        Role::Member => "member",
    }
}

fn next_role(role: Role) -> Role {
    ROLES[(ROLES.iter().position(|r| *r == role).unwrap_or_default() + 1) % ROLES.len()]
}

// ---- what the console shows ----

/// An organisation to choose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Organization {
    pub id: Uuid,
    pub name: String,
}

/// The server and the organisation at a glance.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Overview {
    /// What the server is: label and value.
    pub facts: Vec<(String, String)>,
    /// How much the organisation holds.
    pub counts: Vec<(String, i64)>,
    /// The setup checklist: step, done, optional.
    pub checklist: Vec<(String, bool, bool)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub role: Role,
    /// When they last signed in (date), or never.
    pub last_login: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnsembleRow {
    pub id: Uuid,
    pub name: String,
    pub members: i64,
    pub works: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnsembleView {
    pub name: String,
    /// Name, leader, what they play.
    pub members: Vec<(String, bool, String)>,
    /// Title and composer.
    pub works: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkRow {
    pub id: Uuid,
    pub title: String,
    pub composer: Option<String>,
    pub parts: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkView {
    pub title: String,
    /// Label and value of what is known about the work.
    pub details: Vec<(String, String)>,
    /// Uploaded PDFs: name and pages.
    pub sources: Vec<String>,
    /// Parts: label and pages.
    pub parts: Vec<String>,
}

/// What the console asks the runner to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    Organizations,
    Overview,
    People,
    CreatePerson { name: String, email: String, role: Role },
    SetRole { id: Uuid, name: String, email: String, role: Role },
    Invite { id: Uuid },
    Delete { id: Uuid },
    Ensembles,
    Ensemble { id: Uuid },
    Works { query: String },
    Work { id: Uuid },
}

/// What came back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Organizations(Vec<Organization>),
    Overview(Overview),
    People(Vec<Person>),
    Ensembles(Vec<EnsembleRow>),
    Ensemble(EnsembleView),
    Works(Vec<WorkRow>),
    Work(WorkView),
    /// A change went through; the message says what happened (an invitation link, for one).
    Done(String),
    /// It did not; why.
    Failed(String),
}

/// A line of text being typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// Searching people (as you type) or works (on Enter).
    Search(String),
    /// A new person: name, email, role, and which of the three is being typed (0–2).
    NewPerson { name: String, email: String, role: Role, field: usize },
    /// Deleting this person, once confirmed with `y`.
    ConfirmDelete { id: Uuid, name: String },
}

/// The state of the console; drawing it is [`crate::draw`], changing it [`Console::key`], [`Console::mouse`] and
/// [`Console::outcome`].
#[derive(Debug, Default)]
pub struct Console {
    /// The organisations to choose from, while none is chosen.
    pub organizations: Vec<Organization>,
    /// The organisation the operator acts in.
    pub organization: Option<Organization>,
    pub tab: TabState,
    pub overview: Overview,
    pub people: Vec<Person>,
    pub people_filter: String,
    pub ensembles: Vec<EnsembleRow>,
    /// An ensemble opened from the list.
    pub ensemble: Option<EnsembleView>,
    pub works: Vec<WorkRow>,
    pub works_query: String,
    /// A work opened from the list.
    pub work: Option<WorkView>,
    /// The selected row of each list (organisations, people, ensembles, works).
    pub selected: [usize; 4],
    pub input: Option<Input>,
    /// The last thing that happened.
    pub message: String,
    /// Whether the console has the mouse.
    pub mouse: bool,
    /// Whether to end.
    pub quit: bool,
    /// Where the last frame put clickable things.
    pub areas: Areas,
}

/// The current tab; overview first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabState(pub Tab);

impl Default for TabState {
    fn default() -> Self {
        TabState(Tab::Overview)
    }
}

/// Where the last frame put the tabs, the list and the actions.
#[derive(Debug, Default, Clone)]
pub struct Areas {
    pub tabs: Vec<(Rect, Tab)>,
    /// The rows of the list inside its border, and the first row shown.
    pub list: Rect,
    pub offset: usize,
    pub actions: Vec<(Rect, KeyCode)>,
}

/// Which list a row index belongs to.
const ORGANIZATIONS: usize = 0;

fn list_index(tab: Tab) -> usize {
    match tab {
        Tab::Overview => 0,
        Tab::People => 1,
        Tab::Ensembles => 2,
        Tab::Works => 3,
    }
}

impl Console {
    /// A console that first asks for the organisations.
    pub fn new() -> (Self, Request) {
        (Console { mouse: true, message: "loading …".into(), ..Default::default() }, Request::Organizations)
    }

    pub fn tab(&self) -> Tab {
        self.tab.0
    }

    /// Whether the operator still has to choose an organisation.
    pub fn choosing(&self) -> bool {
        self.organization.is_none()
    }

    /// The people shown: those whose name or email contains the filter.
    pub fn visible_people(&self) -> Vec<&Person> {
        let filter = self.people_filter.to_lowercase();
        self.people
            .iter()
            .filter(|p| filter.is_empty() || p.name.to_lowercase().contains(&filter) || p.email.contains(&filter))
            .collect()
    }

    /// How many rows the current list has (none on the overview and in a detail).
    pub fn rows(&self) -> usize {
        if self.choosing() {
            return self.organizations.len();
        }
        match self.tab() {
            Tab::Overview => 0,
            Tab::People => self.visible_people().len(),
            Tab::Ensembles if self.ensemble.is_none() => self.ensembles.len(),
            Tab::Works if self.work.is_none() => self.works.len(),
            _ => 0,
        }
    }

    fn current(&self) -> usize {
        if self.choosing() { ORGANIZATIONS } else { list_index(self.tab()) }
    }

    /// The selected row of the current list.
    pub fn selected(&self) -> Option<usize> {
        let rows = self.rows();
        (rows > 0).then(|| self.selected[self.current()].min(rows - 1))
    }

    fn select(&mut self, row: usize) {
        let i = self.current();
        self.selected[i] = row;
    }

    /// The keys that do something now, with what they do – the action bar.
    pub fn actions(&self) -> Vec<(KeyCode, &'static str)> {
        let mut actions = Vec::new();
        match &self.input {
            Some(Input::ConfirmDelete { .. }) => {
                return vec![(KeyCode::Char('y'), "delete"), (KeyCode::Esc, "keep")];
            }
            Some(Input::NewPerson { .. }) => {
                return vec![(KeyCode::Enter, "create"), (KeyCode::Tab, "next field"), (KeyCode::Esc, "cancel")];
            }
            Some(Input::Search(_)) => return vec![(KeyCode::Enter, "search"), (KeyCode::Esc, "cancel")],
            None => {}
        }
        if self.choosing() {
            actions.push((KeyCode::Enter, "choose"));
        } else {
            match self.tab() {
                Tab::Overview => {}
                Tab::People => actions.extend([
                    (KeyCode::Char('/'), "search"),
                    (KeyCode::Char('n'), "new"),
                    (KeyCode::Char('r'), "role"),
                    (KeyCode::Char('i'), "invite"),
                    (KeyCode::Char('d'), "delete"),
                ]),
                Tab::Ensembles | Tab::Works if self.ensemble.is_some() || self.work.is_some() => {
                    actions.push((KeyCode::Esc, "back"));
                }
                Tab::Ensembles => actions.push((KeyCode::Enter, "open")),
                Tab::Works => actions.extend([(KeyCode::Char('/'), "search"), (KeyCode::Enter, "open")]),
            }
        }
        actions.push((KeyCode::Char('m'), if self.mouse { "mouse" } else { "mouse off" }));
        actions.push((KeyCode::Char('q'), "quit"));
        actions
    }

    /// Handles a key; what to ask for, if anything.
    pub fn key(&mut self, key: KeyEvent) -> Option<Request> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.quit = true;
            return None;
        }
        if self.input.is_some() {
            return self.typing(key);
        }
        let rows = self.rows();
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('m') => self.mouse = !self.mouse,
            KeyCode::Down if rows > 0 => self.select(self.selected().map_or(0, |s| (s + 1).min(rows - 1))),
            KeyCode::Up if rows > 0 => self.select(self.selected().map_or(0, |s| s.saturating_sub(1))),
            KeyCode::PageDown if rows > 0 => self.select(self.selected().map_or(0, |s| (s + 10).min(rows - 1))),
            KeyCode::PageUp if rows > 0 => self.select(self.selected().map_or(0, |s| s.saturating_sub(10))),
            KeyCode::Home if rows > 0 => self.select(0),
            KeyCode::End if rows > 0 => self.select(rows - 1),
            _ if self.choosing() => return self.choose(key.code),
            KeyCode::Char(c @ '1'..='4') => return self.switch(Tab::ALL[c as usize - '1' as usize]),
            KeyCode::Tab => return self.switch(Tab::ALL[(self.tab().index() + 1) % Tab::ALL.len()]),
            KeyCode::BackTab => {
                return self.switch(Tab::ALL[(self.tab().index() + Tab::ALL.len() - 1) % Tab::ALL.len()]);
            }
            KeyCode::Esc if self.ensemble.is_some() || self.work.is_some() => {
                self.ensemble = None;
                self.work = None;
            }
            KeyCode::Esc => self.quit = true,
            _ => return self.act(key.code),
        }
        None
    }

    fn choose(&mut self, code: KeyCode) -> Option<Request> {
        if code == KeyCode::Esc {
            self.quit = true;
        }
        if code != KeyCode::Enter {
            return None;
        }
        let chosen = self.organizations.get(self.selected()?)?.clone();
        self.message = format!("acting as an admin of {}", chosen.name);
        self.organization = Some(chosen);
        Some(Request::Overview)
    }

    fn switch(&mut self, tab: Tab) -> Option<Request> {
        self.tab = TabState(tab);
        self.ensemble = None;
        self.work = None;
        Some(match tab {
            Tab::Overview => Request::Overview,
            Tab::People => Request::People,
            Tab::Ensembles => Request::Ensembles,
            Tab::Works => Request::Works { query: self.works_query.clone() },
        })
    }

    /// The keys of a tab.
    fn act(&mut self, code: KeyCode) -> Option<Request> {
        match (self.tab(), code) {
            (Tab::People | Tab::Works, KeyCode::Char('/')) if self.work.is_none() => {
                let start = if self.tab() == Tab::People { &self.people_filter } else { &self.works_query };
                self.input = Some(Input::Search(start.clone()));
                None
            }
            (Tab::People, KeyCode::Char('n')) => {
                self.input =
                    Some(Input::NewPerson { name: String::new(), email: String::new(), role: Role::Member, field: 0 });
                None
            }
            (Tab::People, KeyCode::Char('r')) => {
                let p = self.visible_people().get(self.selected()?).map(|p| (*p).clone())?;
                Some(Request::SetRole { id: p.id, name: p.name, email: p.email, role: next_role(p.role) })
            }
            (Tab::People, KeyCode::Char('i')) => {
                let id = self.visible_people().get(self.selected()?)?.id;
                Some(Request::Invite { id })
            }
            (Tab::People, KeyCode::Char('d') | KeyCode::Delete) => {
                let p = self.visible_people().get(self.selected()?).map(|p| (*p).clone())?;
                self.input = Some(Input::ConfirmDelete { id: p.id, name: p.name });
                None
            }
            (Tab::Ensembles, KeyCode::Enter) if self.ensemble.is_none() => {
                Some(Request::Ensemble { id: self.ensembles.get(self.selected()?)?.id })
            }
            (Tab::Works, KeyCode::Enter) if self.work.is_none() => {
                Some(Request::Work { id: self.works.get(self.selected()?)?.id })
            }
            _ => None,
        }
    }

    fn typing(&mut self, key: KeyEvent) -> Option<Request> {
        let Some(input) = &mut self.input else { return None };
        match input {
            Input::ConfirmDelete { id, name } => {
                let request = (key.code == KeyCode::Char('y')).then_some(Request::Delete { id: *id });
                if request.is_some() {
                    self.message = format!("deleting {name} …");
                }
                self.input = None;
                request
            }
            Input::Search(text) => match key.code {
                KeyCode::Esc => {
                    self.input = None;
                    None
                }
                KeyCode::Enter => {
                    let text = std::mem::take(text);
                    self.input = None;
                    self.select(0);
                    if self.tab() == Tab::People {
                        self.people_filter = text;
                        None
                    } else {
                        self.works_query = text.clone();
                        Some(Request::Works { query: text })
                    }
                }
                KeyCode::Backspace => {
                    text.pop();
                    self.live_filter();
                    None
                }
                KeyCode::Char(c) => {
                    text.push(c);
                    self.live_filter();
                    None
                }
                _ => None,
            },
            Input::NewPerson { name, email, role, field } => {
                match key.code {
                    KeyCode::Esc => self.input = None,
                    KeyCode::Tab | KeyCode::Down => *field = (*field + 1) % 3,
                    KeyCode::BackTab | KeyCode::Up => *field = (*field + 2) % 3,
                    KeyCode::Left | KeyCode::Right | KeyCode::Char(' ') if *field == 2 => *role = next_role(*role),
                    KeyCode::Enter => {
                        let request =
                            Request::CreatePerson { name: name.trim().into(), email: email.trim().into(), role: *role };
                        self.input = None;
                        return Some(request);
                    }
                    KeyCode::Backspace if *field < 2 => {
                        if *field == 0 {
                            name.pop()
                        } else {
                            email.pop()
                        };
                    }
                    KeyCode::Char(c) if *field < 2 => {
                        if *field == 0 {
                            name.push(c)
                        } else {
                            email.push(c)
                        }
                    }
                    _ => {}
                }
                None
            }
        }
    }

    /// People are filtered while the search is typed.
    fn live_filter(&mut self) {
        if let (Tab::People, Some(Input::Search(text))) = (self.tab(), &self.input) {
            self.people_filter = text.clone();
            self.select(0);
        }
    }

    /// Handles the mouse: a click does what the key or the row under it does, the wheel moves like the arrow keys.
    pub fn mouse(&mut self, event: MouseEvent) -> Option<Request> {
        let at = Position::new(event.column, event.row);
        match event.kind {
            MouseEventKind::ScrollDown => self.key(KeyCode::Down.into()),
            MouseEventKind::ScrollUp => self.key(KeyCode::Up.into()),
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(&(_, code)) = self.areas.actions.iter().find(|(area, _)| area.contains(at)) {
                    return self.key(code.into());
                }
                if self.input.is_some() {
                    return None;
                }
                if let Some(&(_, tab)) = self.areas.tabs.iter().find(|(area, _)| area.contains(at)) {
                    return (!self.choosing()).then(|| self.switch(tab)).flatten();
                }
                let list = self.areas.list;
                if list.contains(at) {
                    let row = self.areas.offset + usize::from(at.y - list.y);
                    if row < self.rows() {
                        if self.selected() == Some(row) {
                            return self.key(KeyCode::Enter.into());
                        }
                        self.select(row);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Takes what the runner found.
    pub fn outcome(&mut self, outcome: Outcome) -> Option<Request> {
        match outcome {
            Outcome::Organizations(organizations) => {
                if organizations.is_empty() {
                    self.message = "no organisation yet – start `notenwart serve` once".into();
                    return None;
                }
                let only = organizations.len() == 1;
                self.organizations = organizations;
                if only {
                    self.selected[ORGANIZATIONS] = 0;
                    return self.choose(KeyCode::Enter);
                }
                self.message = "choose an organisation".into();
            }
            Outcome::Overview(overview) => self.overview = overview,
            Outcome::People(people) => self.people = people,
            Outcome::Ensembles(ensembles) => self.ensembles = ensembles,
            Outcome::Ensemble(view) => self.ensemble = Some(view),
            Outcome::Works(works) => self.works = works,
            Outcome::Work(view) => self.work = Some(view),
            Outcome::Done(message) => {
                self.message = message;
                return (self.tab() == Tab::People).then_some(Request::People);
            }
            Outcome::Failed(message) => self.message = message,
        }
        None
    }
}
