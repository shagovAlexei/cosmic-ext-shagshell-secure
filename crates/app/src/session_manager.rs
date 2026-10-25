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
use shagshell_core::session::store::{NameError, Node, Store, name_of, valid_name};
use std::collections::{BTreeSet, HashMap};

pub const MIN_WIDTH: f32 = 160.0;
pub const MAX_WIDTH: f32 = 600.0;

/// Row menu commands; about the right-clicked row (`SmMsg::Context` came first).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SmCmd {
    Connect,
    /// An SFTP tab for the selected sessions.
    Sftp,
    /// Session Options for the session.
    Edit,
    Rename,
    Delete,
    NewSession,
    NewFolder,
    /// A copy of the session beside it («name (2)»).
    Duplicate,
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
    /// The pointer left the tree: a drop now goes nowhere.
    Leave,
    /// The left button came up anywhere (only listened to while a row or the splitter is held).
    Release,
    Cursor(f32, f32),
    /// The strip between two stacked panes is held / the stack's height (from the view).
    SplitStartV,
    ColHeight(f32),
    Mods(Modifiers),
    Filter(String),
    ClearFilter,
    Context(String),
    Menu(SmCmd),
    Input(String),
    Submit,
    Cancel,
    SplitStart,
    SplitStartCmd,
    Key(TreeKey),
}

/// Keys for the tree when it, not a terminal, was used last.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeKey {
    Up,
    Down,
    Delete,
    Rename,
    /// Esc: clear the filter.
    Clear,
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
    /// «New session…»: Session Options for a new one in this folder.
    pub new_session: Option<String>,
    /// «Edit…»: Session Options for this session.
    pub edit: Option<String>,
    /// A session or folder renamed / moved (old id, new id): its open tabs follow.
    pub moved: Option<(String, String)>,
    /// A session or folder deleted: its saved passwords go too.
    pub deleted: Option<String>,
    /// «Открыть SFTP»: an SFTP tab for each.
    pub sftp: Vec<String>,
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
    /// The pointer is over the tree (a drop outside it is no move).
    over_tree: bool,
    context: Option<String>,
    dialog: Option<SmDialog>,
    input_id: widget::Id,
    /// The last error or result, under the tree.
    pub notice: Option<String>,
    pub width: f32,
    resizing: bool,
    /// The Command Manager alone on its side has a width of its own, resized the same way.
    pub cmd_width: f32,
    pub cmd_right: bool,
    resizing_cmd: bool,
    /// The two panes of a side stacked: the top one's share of the column (0.15–0.85), the
    /// column's height (measured by the view), and a drag of the strip between them.
    pub split: f32,
    pub col_h: f32,
    resizing_v: bool,
    drag_from_v: Option<(f32, f32)>,
    /// The pane is at the right of the terminals (set by the app from the config).
    pub right: bool,
    /// While the splitter is dragged: the cursor and the width at its first move.
    drag_from: Option<(f32, f32)>,
    /// The tree was used last (Enter connects, not restarts a tab).
    pub focused: bool,
}

impl SessionManager {
    /// Every saved session's id, in tree order.
    pub fn session_ids(&self) -> Vec<String> {
        let top = self.tree.iter().map(|n| n.name().to_string()).collect();
        shagshell_core::session::rows::sessions_under(&self.tree, &top)
    }

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
            over_tree: false,
            context: None,
            dialog: None,
            input_id: widget::Id::unique(),
            notice: None,
            width: f32::from(width).clamp(MIN_WIDTH, MAX_WIDTH),
            resizing: false,
            cmd_width: 260.0,
            cmd_right: true,
            resizing_cmd: false,
            split: 0.5,
            col_h: 0.0,
            resizing_v: false,
            drag_from_v: None,
            right: false,
            drag_from: None,
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

    /// Rows again; the selection keeps only what is still shown (Enter / Delete never act on
    /// rows the filter or a closed folder hides).
    fn refresh(&mut self) {
        self.rows = rows::visible(&self.tree, &self.expanded, &self.filter);
        let shown: BTreeSet<&str> = self.rows.iter().map(|r| r.id.as_str()).collect();
        self.selected.retain(|id| shown.contains(id.as_str()));
        if self
            .anchor
            .as_ref()
            .is_some_and(|a| !self.selected.contains(a))
        {
            self.anchor = self.selected.iter().next_back().cloned();
        }
    }

    /// A folder renamed or moved: its open folders keep open under the new id.
    fn retarget_expanded(&mut self, old: &str, new: &str) {
        let moved: Vec<String> = (self.expanded.iter())
            .filter(|e| *e == old || e.starts_with(&format!("{old}/")))
            .cloned()
            .collect();
        for e in moved {
            self.expanded.remove(&e);
            self.expanded.insert(format!("{new}{}", &e[old.len()..]));
        }
    }

    /// Mouse events are wanted: a row or the splitter is held.
    pub fn holding(&self) -> bool {
        self.pressed.is_some() || self.resizing || self.resizing_cmd || self.resizing_v
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
                    match self.selected.remove(&id) {
                        // Taken out: the anchor stays on something still selected.
                        true if self.anchor.as_ref() == Some(&id) => {
                            self.anchor = self.selected.iter().next_back().cloned();
                        }
                        true => {}
                        false => {
                            self.selected.insert(id.clone());
                            self.anchor = Some(id);
                        }
                    }
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
            SmMsg::Leave => self.over_tree = false,
            SmMsg::Hover(h) => {
                self.over_tree = true;
                if let Some(p) = &self.pressed
                    && h.as_ref() != Some(p)
                {
                    self.drag = Some(p.clone());
                }
                self.hover = h;
            }
            SmMsg::Release => {
                if self.resizing || self.resizing_cmd || self.resizing_v {
                    self.resizing = false;
                    self.resizing_cmd = false;
                    self.resizing_v = false;
                    out.save = true;
                }
                let dragged = self.drag.take();
                // A plain click (no drag) in a multi-selection picks just that row.
                if let Some(p) = self.pressed.take()
                    && dragged.is_none()
                    && !self.mods.control()
                    && !self.mods.shift()
                {
                    self.selected = BTreeSet::from([p.clone()]);
                    self.anchor = Some(p);
                }
                if let Some(id) = dragged.filter(|_| self.over_tree) {
                    let into = match self.hover.as_ref() {
                        None => String::new(),
                        Some(h) if self.rows.iter().any(|r| &r.id == h && r.folder) => h.clone(),
                        Some(h) => parent(h).to_string(),
                    };
                    if into != parent(&id) {
                        match self.store.move_to(&id, &into) {
                            Ok(new) => {
                                self.retarget_expanded(&id, &new);
                                out.moved = Some((id.clone(), new.clone()));
                                if !into.is_empty() {
                                    self.expanded.insert(into);
                                }
                                self.select(&new);
                                out.save = true;
                            }
                            Err(e) => self.notice = Some(io_text(&e)),
                        }
                    }
                }
            }
            SmMsg::ColHeight(h) => self.col_h = h,
            SmMsg::SplitStartV => {
                self.resizing_v = true;
                self.drag_from_v = None;
            }
            SmMsg::Cursor(x, y) => {
                if self.resizing_v && self.col_h > 0.0 {
                    let (y0, r0) = *self.drag_from_v.get_or_insert((y, self.split));
                    self.split = (r0 + (y - y0) / self.col_h).clamp(0.15, 0.85);
                }
                if self.resizing {
                    let (x0, w0) = *self.drag_from.get_or_insert((x, self.width));
                    let dx = if self.right { x0 - x } else { x - x0 };
                    self.width = (w0 + dx).clamp(MIN_WIDTH, MAX_WIDTH);
                } else if self.resizing_cmd {
                    let (x0, w0) = *self.drag_from.get_or_insert((x, self.cmd_width));
                    let dx = if self.cmd_right { x0 - x } else { x - x0 };
                    self.cmd_width = (w0 + dx).clamp(MIN_WIDTH, MAX_WIDTH);
                }
            }
            SmMsg::SplitStartCmd => {
                self.resizing_cmd = true;
                self.drag_from = None;
            }
            SmMsg::SplitStart => {
                self.resizing = true;
                self.drag_from = None;
            }
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
                    TreeKey::Clear => {
                        self.filter.clear();
                        self.refresh();
                    }
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

    pub fn mods(&self) -> Modifiers {
        self.mods
    }

    /// The one selected row, if it is a session (Options → Session Options… with no tab).
    pub fn one_session(&self) -> Option<String> {
        let mut it = self.selected.iter();
        match (it.next(), it.next()) {
            (Some(id), None) if !self.store.is_folder(id) => Some(id.clone()),
            _ => None,
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
            SmCmd::Sftp => out.sftp = self.connect(),
            SmCmd::Edit => out.edit = one.filter(|id| !self.store.is_folder(id)),
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
            SmCmd::Duplicate => {
                if let Some(id) = one.filter(|id| !self.store.is_folder(id)) {
                    match self.store.duplicate(&id) {
                        Ok(new) => {
                            self.reload();
                            self.select(&new);
                        }
                        Err(e) => self.notice = Some(io_text(&e)),
                    }
                }
            }
            SmCmd::Delete => {
                if let Some(id) = one {
                    // From disk, not the rows: what `delete` will remove.
                    let folder = self.store.is_folder(&id);
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
                        Ok(new) => {
                            let old = id.clone();
                            Ok(Some((old, new)))
                        }
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
                        Ok(new) => Ok(Some((new.clone(), new))),
                        Err(e) => Err((error, io_text(&e))),
                    },
                }
            }
            Some(SmDialog::Delete { id, .. }) => match self.store.delete(id) {
                Ok(()) => {
                    out.deleted = Some(id.clone());
                    Ok(None)
                }
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
                    Some((old, id)) => {
                        self.retarget_expanded(&old, &id);
                        out.moved = Some((old, id.clone()));
                        self.select(&id);
                    }
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
                .icon(crate::dialog_icons::Kind::Folder.icon(48))
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
                    .icon(crate::dialog_icons::Kind::Warning.icon(48))
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

    /// The strip at the pane's inner edge that resizes it.
    pub fn splitter(&self) -> Element<'_, Message> {
        widget::mouse_area(widget::container(
            widget::Space::new().width(6.0).height(Length::Fill),
        ))
        .interaction(mouse::Interaction::ResizingHorizontally)
        .on_press(Message::Sessions(SmMsg::SplitStart))
        .into()
    }

    /// The strip between two stacked panes: drags them taller / shorter.
    pub fn splitter_v(&self) -> Element<'_, Message> {
        widget::mouse_area(widget::container(
            widget::Space::new().width(Length::Fill).height(6.0),
        ))
        .interaction(mouse::Interaction::ResizingVertically)
        .on_press(Message::Sessions(SmMsg::SplitStartV))
        .into()
    }

    /// The same strip for the Command Manager when it has its side to itself.
    pub fn splitter_cmd(&self) -> Element<'_, Message> {
        widget::mouse_area(widget::container(
            widget::Space::new().width(6.0).height(Length::Fill),
        ))
        .interaction(mouse::Interaction::ResizingHorizontally)
        .on_press(Message::Sessions(SmMsg::SplitStartCmd))
        .into()
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
                "document-edit-symbolic",
                fl!("sm-edit"),
                self.one_session().map(|_| m(SmMsg::Menu(SmCmd::Edit))),
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
            .on_focus(Message::FocusField(self.filter_id.clone()))
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
        let body =
            widget::column::with_children(vec![toolbar.into(), filter.into(), list, rest.into()])
                .spacing(6)
                .padding(8);
        let pane = widget::container(body)
            .width(Length::Fill)
            .height(Length::Fill);
        widget::mouse_area(pane).on_exit(m(SmMsg::Leave)).into()
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
        // A session: its protocol's glyph (as on its tab), in the text colour.
        let icon = match (r.protocol, r.expanded) {
            (Some(p), _) => {
                use shagshell_core::tab_icon::{Glyph, State, svg};
                let svg = svg(Glyph::of(p), State::Plain).into_bytes();
                widget::icon::from_svg_bytes(svg).symbolic(true)
            }
            (None, true) => widget::icon::from_name("folder-open-symbolic").handle(),
            (None, false) => widget::icon::from_name("folder-symbolic").handle(),
        };
        let line = widget::row::with_children(vec![
            widget::Space::new().width(f32::from(r.depth) * 16.0).into(),
            chevron,
            widget::icon(icon).size(16).into(),
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

fn row_menu(folder: bool) -> Vec<menu::Tree<Message>> {
    use menu::Item::{Button, ButtonDisabled, Divider};
    let edit = match folder {
        true => ButtonDisabled(fl!("sm-edit-menu"), None, SmCmd::Edit),
        false => Button(fl!("sm-edit-menu"), None, SmCmd::Edit),
    };
    let duplicate = match folder {
        true => ButtonDisabled(fl!("sm-duplicate"), None, SmCmd::Duplicate),
        false => Button(fl!("sm-duplicate"), None, SmCmd::Duplicate),
    };
    menu::items(
        &HashMap::new(),
        vec![
            Button(fl!("sm-connect"), None, SmCmd::Connect),
            Button(fl!("sm-sftp"), None, SmCmd::Sftp),
            edit,
            Divider,
            Button(fl!("sm-rename-menu"), None, SmCmd::Rename),
            duplicate,
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

pub fn parent(id: &str) -> &str {
    id.rsplit_once('/').map_or("", |(p, _)| p)
}

pub fn check(name: &str, at_root: bool) -> Result<(), String> {
    valid_name(name, at_root).map_err(name_text)
}

pub fn name_text(e: NameError) -> String {
    match e {
        NameError::Empty => fl!("name-empty"),
        NameError::Slash => fl!("name-slash"),
        NameError::Dot => fl!("name-dot"),
        NameError::Reserved => fl!("name-reserved"),
    }
}

pub fn io_text(e: &std::io::Error) -> String {
    match e.kind() {
        std::io::ErrorKind::AlreadyExists => fl!("name-taken"),
        std::io::ErrorKind::InvalidInput => fl!("move-into-itself"),
        std::io::ErrorKind::InvalidFilename => fl!("name-invalid"),
        _ => e.to_string(),
    }
}
