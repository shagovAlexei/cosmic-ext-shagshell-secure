//! Session Manager pane: the session tree (SecureCRT's dock). Files and rows come from
//! `shagshell_core::session`; this module only draws them and turns clicks into store calls.

use crate::app::Message;
use crate::fl;
use cosmic::app::Task;
use cosmic::iced::keyboard::Modifiers;
use cosmic::iced::{Length, mouse};
use cosmic::widget::{self, menu};
use cosmic::{Element, iced};
use shagshell_core::session::rows::{self, Row};
use shagshell_core::session::store::{NameError, Node, Store, valid_name};
use std::collections::{BTreeSet, HashMap};

pub const MIN_WIDTH: f32 = 160.0;
pub const MAX_WIDTH: f32 = 600.0;

/// Row menu commands; about the right-clicked row (`SmMsg::Context` came first).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SmCmd {
    Connect,
    Rename,
    Delete,
    NewSession,
    NewFolder,
}

impl menu::Action for SmCmd {
    type Message = Message;
    fn message(&self) -> Message {
        Message::Sessions(SmMsg::Menu(*self))
    }
}

#[derive(Clone, Debug)]
pub enum SmMsg {
    Press(String),
    DoubleClick(String),
    Toggle(String),
    Hover(Option<String>),
    /// The left button came up anywhere (only listened to while a row or the splitter is held).
    Release,
    Cursor(f32),
    Mods(Modifiers),
    Filter(String),
    ClearFilter,
    Context(String),
    Menu(SmCmd),
    Input(String),
    Submit,
    Cancel,
    SplitStart,
    Key(TreeKey),
}

/// Keys for the tree when it, not a terminal, was used last.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeKey {
    Up,
    Down,
    Delete,
    Rename,
}

#[derive(Clone, Debug)]
enum SmDialog {
    Rename {
        id: String,
        value: String,
        error: Option<String>,
    },
    NewFolder {
        parent: String,
        value: String,
        error: Option<String>,
    },
    Delete {
        id: String,
        sessions: usize,
        folder: bool,
    },
}

/// What the app does after an update.
#[derive(Default)]
pub struct Outcome {
    pub task: Option<Task<Message>>,
    /// Sessions to open, each in a tab.
    pub open: Vec<String>,
    /// Width or open folders changed: save the config.
    pub save: bool,
    /// «New session…»: Quick Connect with «Save» on, into this folder.
    pub new_session: Option<String>,
}

pub struct SessionManager {
    pub store: Store,
    tree: Vec<Node>,
    pub expanded: BTreeSet<String>,
    rows: Vec<Row>,
    selected: BTreeSet<String>,
    anchor: Option<String>,
    filter: String,
    pub filter_id: widget::Id,
    mods: Modifiers,
    /// The row the left button went down on, while it is held.
    pressed: Option<String>,
    /// Being dragged (the pointer left the pressed row).
    drag: Option<String>,
    hover: Option<String>,
    context: Option<String>,
    dialog: Option<SmDialog>,
    input_id: widget::Id,
    /// The last error or result, under the tree.
    pub notice: Option<String>,
    pub width: f32,
    resizing: bool,
    /// The tree was used last (Enter connects, not restarts a tab).
    pub focused: bool,
}

impl SessionManager {
    pub fn new(store: Store, expanded: &[String], width: u16) -> Self {
        let mut sm = Self {
            store,
            tree: Vec::new(),
            expanded: expanded.iter().cloned().collect(),
            rows: Vec::new(),
            selected: BTreeSet::new(),
            anchor: None,
            filter: String::new(),
            filter_id: widget::Id::unique(),
            mods: Modifiers::empty(),
            pressed: None,
            drag: None,
            hover: None,
            context: None,
            dialog: None,
            input_id: widget::Id::unique(),
            notice: None,
            width: f32::from(width).clamp(MIN_WIDTH, MAX_WIDTH),
            resizing: false,
            focused: false,
        };
        sm.reload();
        sm
    }

    /// Read the files again (after any change, ours or by hand).
    pub fn reload(&mut self) {
        self.tree = self.store.load();
        self.refresh();
    }

    fn refresh(&mut self) {
        self.rows = rows::visible(&self.tree, &self.expanded, &self.filter);
    }

    /// Mouse events are wanted: a row or the splitter is held.
    pub fn holding(&self) -> bool {
        self.pressed.is_some() || self.resizing
    }

    /// Select one id (a session just saved, moved, renamed), showing it: its folders opened,
    /// the filter cleared.
    pub fn select(&mut self, id: &str) {
        self.filter.clear();
        if let Some((parent, _)) = id.rsplit_once('/') {
            let mut p = String::new();
            for part in parent.split('/') {
                p = if p.is_empty() {
                    part.into()
                } else {
                    format!("{p}/{part}")
                };
                self.expanded.insert(p.clone());
            }
        }
        self.reload();
        self.selected = BTreeSet::from([id.to_string()]);
        self.anchor = Some(id.to_string());
    }

    /// The folder new things go into: the selected folder, or the selected session's.
    fn target_folder(&self) -> String {
        let Some(id) = self.anchor.as_ref().filter(|a| self.selected.contains(*a)) else {
            return String::new();
        };
        match self.rows.iter().find(|r| &r.id == id) {
            Some(r) if r.folder => id.clone(),
            _ => parent(id).to_string(),
        }
    }

    pub fn update(&mut self, msg: SmMsg) -> Outcome {
        let mut out = Outcome::default();
        match msg {
            SmMsg::Mods(m) => self.mods = m,
            SmMsg::Press(id) => {
                self.focused = true;
                self.notice = None;
                out.task = Some(unfocus());
                if self.mods.control() {
                    if !self.selected.remove(&id) {
                        self.selected.insert(id.clone());
                    }
                    self.anchor = Some(id);
                } else if self.mods.shift() {
                    let from = self.anchor.clone().unwrap_or_else(|| id.clone());
                    self.selected = rows::range(&self.rows, &from, &id).into_iter().collect();
                } else {
                    if !self.selected.contains(&id) {
                        self.selected = BTreeSet::from([id.clone()]);
                    }
                    self.anchor = Some(id.clone());
                    self.pressed = Some(id);
                }
            }
            SmMsg::DoubleClick(id) => {
                (self.pressed, self.drag) = (None, None);
                match self
                    .rows
                    .iter()
                    .find(|r| r.id == id)
                    .is_some_and(|r| r.folder)
                {
                    true => return self.toggle(id),
                    false => {
                        self.selected = BTreeSet::from([id]);
                        out.open = self.connect();
                    }
                }
            }
            SmMsg::Toggle(id) => return self.toggle(id),
            SmMsg::Hover(h) => {
                if let Some(p) = &self.pressed
                    && h.as_ref() != Some(p)
                {
                    self.drag = Some(p.clone());
                }
                self.hover = h;
            }
            SmMsg::Release => {
                if self.resizing {
                    self.resizing = false;
                    out.save = true;
                }
                let dragged = self.drag.take();
                self.pressed = None;
                if let Some(id) = dragged {
                    let into = match self.hover.as_ref() {
                        None => String::new(),
                        Some(h) if self.rows.iter().any(|r| &r.id == h && r.folder) => h.clone(),
                        Some(h) => parent(h).to_string(),
                    };
                    if into != parent(&id) {
                        match self.store.move_to(&id, &into) {
                            Ok(new) => {
                                if !into.is_empty() {
                                    self.expanded.insert(into);
                                    out.save = true;
                                }
                                self.select(&new);
                            }
                            Err(e) => self.notice = Some(io_text(&e)),
                        }
                    }
                }
            }
            SmMsg::Cursor(x) => {
                if self.resizing {
                    self.width = x.clamp(MIN_WIDTH, MAX_WIDTH);
                }
            }
            SmMsg::SplitStart => self.resizing = true,
            SmMsg::Filter(f) => {
                self.filter = f;
                self.refresh();
            }
            SmMsg::ClearFilter => {
                self.filter.clear();
                self.refresh();
            }
            SmMsg::Context(id) => {
                self.focused = true;
                out.task = Some(unfocus());
                if !self.selected.contains(&id) {
                    self.selected = BTreeSet::from([id.clone()]);
                    self.anchor = Some(id.clone());
                }
                self.context = Some(id);
            }
            SmMsg::Menu(cmd) => {
                self.context = None;
                return self.cmd(cmd);
            }
            SmMsg::Key(k) => {
                if !self.focused || self.dialog.is_some() {
                    return out;
                }
                match k {
                    TreeKey::Up | TreeKey::Down => self.step(k == TreeKey::Down),
                    TreeKey::Delete => return self.cmd(SmCmd::Delete),
                    TreeKey::Rename => return self.cmd(SmCmd::Rename),
                }
            }
            SmMsg::Input(v) => {
                if let Some(
                    SmDialog::Rename { value, error, .. }
                    | SmDialog::NewFolder { value, error, .. },
                ) = &mut self.dialog
                {
                    (*value, *error) = (v, None);
                }
            }
            SmMsg::Submit => self.submit(&mut out),
            SmMsg::Cancel => self.dialog = None,
        }
        out
    }

    fn toggle(&mut self, id: String) -> Outcome {
        if !self.expanded.remove(&id) {
            self.expanded.insert(id);
        }
        self.refresh();
        Outcome {
            save: true,
            ..Default::default()
        }
    }

    fn step(&mut self, down: bool) {
        let pos = (self.anchor.as_ref()).and_then(|a| self.rows.iter().position(|r| &r.id == a));
        let next = match (pos, down) {
            (None, _) => 0,
            (Some(p), true) => (p + 1).min(self.rows.len().saturating_sub(1)),
            (Some(p), false) => p.saturating_sub(1),
        };
        if let Some(r) = self.rows.get(next) {
            self.selected = BTreeSet::from([r.id.clone()]);
            self.anchor = Some(r.id.clone());
        }
    }

    /// The selected sessions, folders standing for everything in them (SecureCRT "Connect all").
    pub fn connect(&self) -> Vec<String> {
        rows::sessions_under(&self.tree, &self.selected)
    }

    pub fn cmd(&mut self, cmd: SmCmd) -> Outcome {
        let mut out = Outcome::default();
        let one = self.anchor.clone().filter(|a| self.selected.contains(a));
        match cmd {
            SmCmd::Connect => out.open = self.connect(),
            SmCmd::NewSession => out.new_session = Some(self.target_folder()),
            SmCmd::NewFolder => {
                self.dialog = Some(SmDialog::NewFolder {
                    parent: self.target_folder(),
                    value: String::new(),
                    error: None,
                });
                out.task = Some(widget::text_input::focus(self.input_id.clone()));
            }
            SmCmd::Rename => {
                if let Some(id) = one {
                    let value = name_of(&id).to_string();
                    self.dialog = Some(SmDialog::Rename {
                        id,
                        value,
                        error: None,
                    });
                    out.task = Some(widget::text_input::focus(self.input_id.clone()));
                }
            }
            SmCmd::Delete => {
                if let Some(id) = one {
                    let folder = self.rows.iter().any(|r| r.id == id && r.folder);
                    let sessions = self.store.count_sessions(&id);
                    self.dialog = Some(SmDialog::Delete {
                        id,
                        sessions,
                        folder,
                    });
                }
            }
        }
        out
    }

    fn submit(&mut self, out: &mut Outcome) {
        let done = match &mut self.dialog {
            Some(SmDialog::Rename { id, value, error }) => {
                let value = value.trim().to_string();
                match check(&value, parent(id).is_empty()) {
                    Err(e) => Err((error, e)),
                    Ok(()) => match self.store.rename(id, &value) {
                        Ok(new) => Ok(Some(new)),
                        Err(e) => Err((error, io_text(&e))),
                    },
                }
            }
            Some(SmDialog::NewFolder {
                parent,
                value,
                error,
            }) => {
                let value = value.trim().to_string();
                match check(&value, parent.is_empty()) {
                    Err(e) => Err((error, e)),
                    Ok(()) => match self.store.new_folder(parent, &value) {
                        Ok(new) => Ok(Some(new)),
                        Err(e) => Err((error, io_text(&e))),
                    },
                }
            }
            Some(SmDialog::Delete { id, .. }) => match self.store.delete(id) {
                Ok(()) => Ok(None),
                Err(e) => {
                    self.notice = Some(io_text(&e));
                    Ok(None)
                }
            },
            None => return,
        };
        match done {
            Err((slot, e)) => *slot = Some(e),
            Ok(new) => {
                self.dialog = None;
                self.selected.clear();
                match new {
                    Some(id) => self.select(&id),
                    None => self.reload(),
                }
                out.save = true;
            }
        }
    }

    pub fn dialog(&self) -> Option<Element<'_, Message>> {
        let m = |x| Message::Sessions(x);
        let input = move |title: String, value: &str, error: &Option<String>, ok: String| {
            let field = widget::text_input("", value.to_string())
                .id(self.input_id.clone())
                .on_input(|v| Message::Sessions(SmMsg::Input(v)))
                .on_submit(|_| Message::Sessions(SmMsg::Submit));
            let mut body = widget::column::with_children(vec![field.into()]).spacing(8);
            if let Some(e) = error {
                body =
                    body.push(widget::text::caption(e.clone()).class(cosmic::theme::Text::Accent));
            }
            widget::dialog()
                .title(title)
                .control(body)
                .primary_action(widget::button::suggested(ok).on_press(m(SmMsg::Submit)))
                .secondary_action(
                    widget::button::standard(fl!("cancel")).on_press(m(SmMsg::Cancel)),
                )
                .into()
        };
        Some(match self.dialog.as_ref()? {
            SmDialog::Rename { value, error, .. } => {
                input(fl!("sm-rename-title"), value, error, fl!("sm-rename"))
            }
            SmDialog::NewFolder { value, error, .. } => {
                input(fl!("sm-new-folder-title"), value, error, fl!("sm-create"))
            }
            SmDialog::Delete {
                id,
                sessions,
                folder,
            } => {
                let body = match folder {
                    true => fl!("sm-delete-folder-body", count = sessions.to_string()),
                    false => fl!("sm-delete-body"),
                };
                widget::dialog()
                    .title(fl!("sm-delete-title", name = name_of(id)))
                    .body(body)
                    .primary_action(
                        widget::button::destructive(fl!("sm-delete")).on_press(m(SmMsg::Submit)),
                    )
                    .secondary_action(
                        widget::button::standard(fl!("cancel")).on_press(m(SmMsg::Cancel)),
                    )
                    .into()
            }
        })
    }

    pub fn view(&self, window: iced::window::Id) -> Element<'_, Message> {
        let m = |x| Message::Sessions(x);
        let tool =
            |icon: &'static str, tip: String, msg: Option<Message>| -> Element<'_, Message> {
                let b = widget::button::icon(widget::icon::from_name(icon)).on_press_maybe(msg);
                widget::tooltip(b, widget::text(tip), widget::tooltip::Position::Bottom).into()
            };
        let any = !self.selected.is_empty();
        let toolbar = widget::row::with_children(vec![
            tool(
                "document-new-symbolic",
                fl!("sm-new-session"),
                Some(m(SmMsg::Menu(SmCmd::NewSession))),
            ),
            tool(
                "folder-new-symbolic",
                fl!("sm-new-folder"),
                Some(m(SmMsg::Menu(SmCmd::NewFolder))),
            ),
            tool(
                "edit-delete-symbolic",
                fl!("sm-delete"),
                any.then(|| m(SmMsg::Menu(SmCmd::Delete))),
            ),
            tool(
                "media-playback-start-symbolic",
                fl!("sm-connect"),
                any.then(|| m(SmMsg::Menu(SmCmd::Connect))),
            ),
        ])
        .spacing(2);
        let filter = widget::search_input(fl!("sm-filter"), self.filter.clone())
            .id(self.filter_id.clone())
            .on_input(|f| Message::Sessions(SmMsg::Filter(f)))
            .on_clear(m(SmMsg::ClearFilter));
        let list: Element<'_, Message> = match self.rows.is_empty() {
            true => widget::container(widget::text::caption(match self.filter.is_empty() {
                true => fl!("sessions-empty"),
                false => fl!("sm-no-match"),
            }))
            .center(Length::Fill)
            .into(),
            false => {
                let rows = self.rows.iter().map(|r| self.row(r, window));
                let column = widget::column::with_children(rows.collect::<Vec<_>>());
                widget::scrollable(column).height(Length::Shrink).into()
            }
        };
        // Below the rows: dropping there means «into the root»; a click there clears the focus.
        let rest = widget::mouse_area(
            widget::Space::new()
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .on_enter(m(SmMsg::Hover(None)));
        let mut body = widget::column::with_children(vec![
            widget::text::heading(fl!("menu-session-manager")).into(),
            toolbar.into(),
            filter.into(),
            list,
            rest.into(),
        ])
        .spacing(6)
        .padding(8);
        if let Some(n) = &self.notice {
            body = body.push(widget::text::caption(n.clone()));
        }
        let pane = widget::container(body)
            .width(self.width)
            .height(Length::Fill)
            .class(cosmic::theme::Container::Card);
        let splitter = widget::mouse_area(widget::container(
            widget::Space::new().width(6.0).height(Length::Fill),
        ))
        .interaction(mouse::Interaction::ResizingHorizontally)
        .on_press(m(SmMsg::SplitStart));
        widget::row::with_children(vec![pane.into(), splitter.into()]).into()
    }

    fn row(&self, r: &Row, window: iced::window::Id) -> Element<'_, Message> {
        let m = |x| Message::Sessions(x);
        let chevron: Element<'_, Message> = match r.folder {
            true => {
                let icon = if r.expanded {
                    "go-down-symbolic"
                } else {
                    "go-next-symbolic"
                };
                widget::button::icon(widget::icon::from_name(icon).size(12))
                    .padding(2)
                    .on_press(m(SmMsg::Toggle(r.id.clone())))
                    .into()
            }
            false => widget::Space::new().width(20.0).into(),
        };
        let icon = match (r.folder, r.expanded) {
            (true, true) => "folder-open-symbolic",
            (true, false) => "folder-symbolic",
            (false, _) => "network-server-symbolic",
        };
        let line = widget::row::with_children(vec![
            widget::Space::new().width(f32::from(r.depth) * 16.0).into(),
            chevron,
            widget::icon::from_name(icon).size(16).into(),
            widget::text::body(r.name.clone()).into(),
        ])
        .spacing(4)
        .align_y(iced::Alignment::Center);
        let selected = self.selected.contains(&r.id);
        let target = self.drag.is_some() && r.folder && self.hover.as_ref() == Some(&r.id);
        let cell = widget::container(line)
            .width(Length::Fill)
            .padding([2, 4])
            .class(cosmic::theme::Container::custom(move |theme| {
                let c = theme.cosmic();
                let mut s = widget::container::Style::default();
                if selected {
                    let mut bg: iced::Color = c.accent_color().into();
                    bg.a = 0.25;
                    s.background = Some(bg.into());
                    s.border.radius = c.corner_radii.radius_xs.into();
                }
                if target {
                    s.border.color = c.accent_color().into();
                    s.border.width = 1.0;
                }
                s
            }));
        let area = widget::mouse_area(cell)
            .on_press(m(SmMsg::Press(r.id.clone())))
            .on_double_click(m(SmMsg::DoubleClick(r.id.clone())))
            .on_enter(m(SmMsg::Hover(Some(r.id.clone()))))
            .on_right_press(m(SmMsg::Context(r.id.clone())));
        widget::context_menu(area, Some(row_menu(r.folder)))
            .window_id(window)
            .on_surface_action(Message::Surface)
            .into()
    }
}

fn row_menu(_folder: bool) -> Vec<menu::Tree<Message>> {
    use menu::Item::{Button, Divider};
    menu::items(
        &HashMap::new(),
        vec![
            Button(fl!("sm-connect"), None, SmCmd::Connect),
            Divider,
            Button(fl!("sm-rename-menu"), None, SmCmd::Rename),
            Button(fl!("sm-delete-menu"), None, SmCmd::Delete),
            Divider,
            Button(fl!("sm-new-session-menu"), None, SmCmd::NewSession),
            Button(fl!("sm-new-folder-menu"), None, SmCmd::NewFolder),
        ],
    )
}

/// Take the keys from the terminal (and any field): focusing an id no widget has (as
/// shagoff-commander does), so Enter / Delete / F2 reach the tree.
fn unfocus() -> Task<Message> {
    widget::text_input::focus(widget::Id::unique())
}

fn parent(id: &str) -> &str {
    id.rsplit_once('/').map_or("", |(p, _)| p)
}

fn name_of(id: &str) -> &str {
    id.rsplit_once('/').map_or(id, |(_, n)| n)
}

fn check(name: &str, at_root: bool) -> Result<(), String> {
    valid_name(name, at_root).map_err(|e| match e {
        NameError::Empty => fl!("name-empty"),
        NameError::Slash => fl!("name-slash"),
        NameError::Dot => fl!("name-dot"),
        NameError::Reserved => fl!("name-reserved"),
    })
}

pub fn io_text(e: &std::io::Error) -> String {
    match e.kind() {
        std::io::ErrorKind::AlreadyExists => fl!("name-taken"),
        std::io::ErrorKind::InvalidInput => fl!("move-into-itself"),
        _ => e.to_string(),
    }
}
