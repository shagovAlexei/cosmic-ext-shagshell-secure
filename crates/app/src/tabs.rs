//! Tabs and tab groups (SecureCRT "tab groups"): a `pane_grid` of groups, each a tab strip over
//! its active tab's terminal. Tabs are kept by id; a group's model holds the ids in order.

use crate::app::Message;
use crate::config::Config;
use crate::fl;
use crate::menu::Action;
use crate::sftp_view::{Connect, SftpMsg, SftpView};
use crate::term::{self, Events, Outcome, TabId, terminal::Terminal};
use alacritty_terminal::grid::Dimensions;
use cosmic::app::Task;
use cosmic::iced::Length;
use cosmic::iced::keyboard::{Key, Modifiers, key::Physical};
use cosmic::widget::segmented_button::{Entity, ReorderEvent, SingleSelectModel};
use cosmic::widget::{self, menu, pane_grid, tab_bar};
use cosmic::{Element, iced};
use shagshell_core::tabs as rules;
use std::collections::HashMap;
use std::sync::Mutex;

const TAB_MIME: &str = "application/x-shagshell-tab";

/// Who a tab talks to: the status bar's protocol.
#[derive(Clone, Debug)]
pub enum Protocol {
    Local,
    Telnet {
        host: String,
        session: Option<String>,
    },
    Serial {
        device: String,
        session: Option<String>,
    },
    /// `session`: the saved one it was opened from (Quick Connect without saving: none).
    Ssh {
        host: String,
        session: Option<String>,
    },
}

/// A session log asked for: the file name template and how to write it.
#[derive(Clone, Debug, Default)]
pub struct LogSpec {
    pub file: String,
    pub append: bool,
    pub raw: bool,
    /// For the template's `%P` and `%U`.
    pub port: Option<u16>,
    pub user: String,
}

/// Logon Actions of a session: the table, and what `%user%` / `%password%` stand for.
#[derive(Clone, Debug, Default)]
pub struct LogonSpec {
    pub steps: Vec<shagshell_core::logon::Step>,
    pub user: String,
    /// Whose keyring password is `%password%` (the session's, or its credential set's).
    pub owner: Option<shagshell_core::keyring::Owner>,
}

/// Answers due are typed into the tab by one worker, in order: the keyring may take a while and
/// `\p` pauses, and a later answer must not overtake an earlier one. A password that is not saved
/// is said, not typed.
fn answer(tap: &term::log::Tap, spec: &LogonSpec) -> term::log::Sender {
    use shagshell_core::keyring::{Secret, lookup};
    use shagshell_core::macros::{Piece, parse};
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    let (tap, spec) = (tap.clone(), spec.clone());
    std::thread::spawn(move || {
        for send in rx {
            let password = match shagshell_core::logon::needs_password(&send) {
                true => {
                    (spec.owner.as_ref()).and_then(|o| lookup(o, Secret::Password).ok().flatten())
                }
                false => None,
            };
            // The escapes are read first: a password is typed as it is, backslashes and all.
            for piece in parse(&send) {
                match piece {
                    Piece::Bytes(b) => {
                        match shagshell_core::logon::fill(&b, &spec.user, password.as_deref()) {
                            Some(bytes) => tap.input(bytes),
                            None => {
                                tap.fail(fl!("logon-no-password"));
                                break;
                            }
                        }
                    }
                    Piece::Pause(d) => std::thread::sleep(d),
                }
            }
        }
    });
    let tx = Mutex::new(tx);
    std::sync::Arc::new(move |send: String| {
        let _ = tx.lock().unwrap_or_else(|e| e.into_inner()).send(send);
    })
}

/// A session that connects again by itself: after `delay` s, `tries` times in a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AutoReconnect {
    pub delay: u32,
    pub tries: u32,
}

/// What a tab runs (again on Reconnect).
#[derive(Clone, Debug)]
pub struct Launch {
    pub program: String,
    pub args: Vec<String>,
    /// More environment (ssh's askpass); the tab adds `SHAGSHELL_TAB` when there is any.
    pub env: Vec<(String, String)>,
    /// Ours (a session's name, «Local Shell»), numbered if taken.
    pub title: String,
    pub protocol: Protocol,
    /// An SFTP tab (`args`: `sftp_args`), not a terminal.
    pub sftp: bool,
    /// A note: this file, not a program.
    pub note: Option<std::path::PathBuf>,
    /// The Script Console: a tab drawn by the app, not a program.
    pub console: bool,
    /// Logged from the start (the session says so).
    pub log: Option<LogSpec>,
    /// Connects again by itself when the connection drops.
    pub auto_reconnect: Option<AutoReconnect>,
    /// Answers to what the session says after connecting.
    pub logon: Option<LogonSpec>,
    /// The terminal colour scheme of the session ("": the global one).
    pub scheme: String,
    /// The session's tab label: shown instead of the program's title.
    pub label: String,
    /// The session's own icon (`Session::icon`); "": by protocol.
    pub icon: String,
    /// The saved session (its id) this tab was opened from: it is opened again at the next start.
    pub session: Option<String>,
}

impl Launch {
    pub fn local_shell() -> Self {
        let shell = shagshell_core::shell::pick(std::env::var_os("SHELL").as_deref());
        Self {
            program: shell.to_string_lossy().into_owned(),
            args: Vec::new(),
            env: Vec::new(),
            title: fl!("tab-local-shell"),
            protocol: Protocol::Local,
            sftp: false,
            note: None,
            console: false,
            log: None,
            auto_reconnect: None,
            logon: None,
            scheme: String::new(),
            label: String::new(),
            icon: String::new(),
            session: None,
        }
    }

    /// This ssh tab's SFTP tab: the same connection (ControlPath), `-s … sftp`.
    pub fn sftp(&self) -> Option<Self> {
        if self.sftp {
            return Some(self.clone());
        }
        let args = shagshell_core::launch::sftp_args(&self.args)?;
        Some(Self {
            args,
            title: format!("SFTP · {}", self.title),
            label: match self.label.is_empty() {
                true => String::new(),
                false => format!("SFTP · {}", self.label),
            },
            sftp: true,
            note: None,
            console: false,
            session: None,
            ..self.clone()
        })
    }

    fn label(&self) -> String {
        match (&self.protocol, self.sftp) {
            (Protocol::Local, _) => fl!("tab-local-shell"),
            (Protocol::Telnet { host, .. }, _) => format!("Telnet · {host}"),
            (Protocol::Serial { device, .. }, _) => format!("Serial · {device}"),
            (Protocol::Ssh { host, .. }, true) => format!("SFTP · {host}"),
            (Protocol::Ssh { host, .. }, false) => format!("SSH · {host}"),
        }
    }
}

/// What a tab shows.
pub enum Body {
    Terminal(Box<Mutex<Terminal>>),
    Sftp(Box<SftpView>),
    /// A text / markdown file.
    Note(Box<crate::notes::NoteView>),
    /// The Script Console (the app draws it: `Scripts::console_view`).
    Console,
}

pub struct Tab {
    /// When its program started (a drop after a while is a lost connection, not a failed start).
    started: std::time::Instant,
    /// Automatic reconnects since it last stayed up; `stopped`: the user ended it (Disconnect).
    attempts: u32,
    stopped: bool,
    pub body: Body,
    launch: Launch,
    /// «web-01 (2)»: ours, shown while the program sets none.
    base: String,
    /// OSC 0 / 2 from the program; empty: none.
    program_title: String,
    pub alive: bool,
    /// It printed something since it started (else: still connecting).
    seen: bool,
    /// It printed something while not in front.
    unread: bool,
    /// When it last printed something.
    last_output: std::time::Instant,
    /// Its unread output stopped `QUIET_SECS` ago (the icon says so).
    quiet: bool,
    widget_id: widget::Id,
}

impl Tab {
    /// The label, else (a running shell) the program's title, else ours: a session's tab keeps its name
    /// (SecureCRT), not the `user@host:~` its shell sets.
    fn title(&self) -> String {
        // An ended shell's last title is a command («exit»): ours then.
        let local =
            matches!(self.launch.protocol, Protocol::Local) && !self.launch.sftp && self.alive;
        match (
            self.launch.label.is_empty(),
            local && !self.program_title.is_empty(),
        ) {
            (false, _) => self.launch.label.clone(),
            (_, true) => self.program_title.clone(),
            _ => self.base.clone(),
        }
    }

    /// What it is, in the colour of its state.
    fn icon(&self) -> widget::Icon {
        use shagshell_core::tab_icon::{Glyph, State, state, svg};
        let glyph = match (&self.body, &self.launch.protocol) {
            (Body::Note(_), _) => Glyph::Note,
            (Body::Console, _) => Glyph::Console,
            (Body::Sftp(_), _) => Glyph::Sftp,
            (_, _) if !self.launch.icon.is_empty() => {
                shagshell_core::tab_icon::Glyph::for_session(&self.launch.icon, Default::default())
            }
            (_, Protocol::Ssh { .. }) => Glyph::Ssh,
            (_, Protocol::Telnet { .. }) => Glyph::Telnet,
            (_, Protocol::Serial { .. }) => Glyph::Serial,
            (_, Protocol::Local) => Glyph::Local,
        };
        let locked = self
            .terminal()
            .is_some_and(|t| t.lock().unwrap_or_else(|e| e.into_inner()).locked());
        let st = match &self.body {
            Body::Note(v) if v.locked => State::Locked,
            Body::Note(_) | Body::Console => State::Plain,
            Body::Sftp(v) => state(
                v.connected || v.connecting(),
                v.connected,
                false,
                false,
                false,
            ),
            Body::Terminal(_) => state(self.live(), self.seen, self.unread, self.quiet, locked),
        };
        widget::icon(widget::icon::from_svg_bytes(svg(glyph, st).into_bytes())).size(16)
    }

    /// Its program runs (a terminal), or it is connected (SFTP).
    fn live(&self) -> bool {
        match &self.body {
            Body::Terminal(_) => self.alive,
            Body::Sftp(v) => v.connected,
            // Never «live»: closing it asks nothing, Reconnect does nothing.
            Body::Note(_) | Body::Console => false,
        }
    }

    fn terminal(&self) -> Option<&Mutex<Terminal>> {
        match &self.body {
            Body::Terminal(t) => Some(t),
            Body::Sftp(_) | Body::Note(_) | Body::Console => None,
        }
    }
}

#[derive(Default)]
pub struct Group {
    model: SingleSelectModel,
}

impl Group {
    fn active_id(&self) -> Option<TabId> {
        self.model.data::<TabId>(self.model.active()).copied()
    }
}

/// How a terminal looks: text style, dark theme or not, window background (0xAARRGGBB).
pub type Look = (term::style::Style, bool, u32);

/// The tab menu's commands: about the right-clicked tab (keys and the menu bar: the active one).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TabCmd {
    Reconnect,
    Disconnect,
    Clone,
    /// «Connect SFTP Tab»: an SFTP tab over this ssh tab's connection.
    Sftp,
    SplitRight,
    SplitDown,
    /// Into the neighbouring group on this side.
    MoveTo(Side),
    /// Into a window of its own.
    Detach,
    Close,
    /// Every other tab of the window.
    CloseOthers,
    CloseAll,
    /// Lock Session / Unlock Session (a password asked).
    Lock,
    /// A note's own: write now, write as…, wrap long lines, line numbers, rename.
    NoteSave,
    NoteSaveAs,
    NoteWrap,
    NoteNumbers,
    NoteRename,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    Left,
    Right,
    Up,
    Down,
}

impl Side {
    const ALL: [Side; 4] = [Side::Left, Side::Right, Side::Up, Side::Down];

    fn direction(self) -> pane_grid::Direction {
        match self {
            Side::Left => pane_grid::Direction::Left,
            Side::Right => pane_grid::Direction::Right,
            Side::Up => pane_grid::Direction::Up,
            Side::Down => pane_grid::Direction::Down,
        }
    }

    fn label(self) -> String {
        match self {
            Side::Left => fl!("menu-move-left"),
            Side::Right => fl!("menu-move-right"),
            Side::Up => fl!("menu-move-up"),
            Side::Down => fl!("menu-move-down"),
        }
    }
}

/// `cosmic::Action`: the tab bar's menu is a popup of its own (see `Tabs::view`).
impl menu::Action for TabCmd {
    type Message = cosmic::Action<Message>;
    fn message(&self) -> Self::Message {
        cosmic::Action::App(Message::Tabs(TabsMsg::Cmd(*self)))
    }
}

#[derive(Clone, Debug)]
pub enum TabsMsg {
    Cmd(TabCmd),
    Activate(pane_grid::Pane, Entity),
    Close(pane_grid::Pane, Entity),
    CloseConfirmed,
    CloseCancelled,
    Context(pane_grid::Pane, Entity),
    Reorder(pane_grid::Pane, ReorderEvent),
    /// A tab was dropped on this group (its bar or its body).
    /// A tab drag over its own bar: which tab is under the pointer (the first: the dragged one).
    DropHint(pane_grid::Pane, Option<(Entity, bool)>),
    Drop(pane_grid::Pane),
    PaneClicked(pane_grid::Pane),
    Resized(pane_grid::ResizeEvent),
    /// What the clipboard gave, for this tab.
    Paste(TabId, Option<String>),
    /// A tab's window: bring the tab back / drag the window.
    Reattach(iced::window::Id),
    DragWindow(iced::window::Id),
    /// Bytes to these tabs' programs; a pause sends the rest later.
    Send(Vec<TabId>, Vec<shagshell_core::macros::Piece>),
    /// The wait before an automatic reconnect is over.
    AutoReconnect(TabId),
    /// The Lock / Unlock dialog: the password, again, OK, Cancel.
    LockPw(String),
    LockPw2(String),
    LockOk,
    LockCancel,
}

/// The Lock / Unlock Session dialog.
struct LockAsk {
    id: TabId,
    /// Unlocking (one field), else locking (two).
    unlock: bool,
    pw: String,
    pw2: String,
    error: Option<String>,
    field: widget::Id,
}

#[derive(Default)]
pub struct Tabs {
    /// Tabs in windows of their own (window, tab), and the window just opened (to be titled).
    detached: Vec<(iced::window::Id, TabId)>,
    new_windows: Vec<(iced::window::Id, String)>,
    /// A tab's window to bring to the front (an askpass dialog is about its tab).
    raise: Option<iced::window::Id>,
    /// While an event of a tab's own window is handled: that tab is the active one.
    pub front: Option<TabId>,
    /// A recording whose tab was closed: to be saved.
    closed_recordings: Vec<(String, shagshell_core::recorder::Recorder)>,
    grid: Option<pane_grid::State<Group>>,
    focus: Option<pane_grid::Pane>,
    tabs: HashMap<TabId, Tab>,
    next_id: TabId,
    tx: Option<Events>,
    sftp_tx: Option<crate::sftp_worker::Events>,
    /// Global Options → SFTP: the answer to a conflict, dot files shown in a new tab.
    sftp_prefs: (Option<shagshell_core::sftp::queue::Choice>, bool),
    /// Global Options → Logging: the folder, and the name used when a log is started by hand.
    log_prefs: (std::path::PathBuf, String),
    /// Global Options: the terminal colour scheme ("": the theme's) and keyword highlighting.
    scheme: String,
    highlights: Vec<shagshell_core::highlight::Rule>,
    /// Global Options → Cursor.
    cursor: alacritty_terminal::vte::ansi::CursorShape,
    /// Tabs by when they were last in front (most recent first); `mru`: Ctrl+Tab goes back to the
    /// previous one (Global Options), not to the next in the row.
    recent: Vec<TabId>,
    mru: bool,
    /// The tabs the Command Window sends to besides the active one (SecureCRT marks them `*`).
    starred: std::collections::BTreeSet<TabId>,
    /// What the last log action had to say (a log that could not start / stopped).
    notice: Mutex<Option<String>>,
    /// A live tab asked to close: waits for the dialog.
    confirm: Option<(pane_grid::Pane, Entity)>,
    /// «Close others / all» with live tabs among them: asked once (the tabs, then closed).
    confirm_many: Option<Vec<(pane_grid::Pane, Entity)>>,
    /// Tabs locked by the user (Lock Session), with their passwords (memory only).
    user_locks: HashMap<TabId, shagshell_core::tablock::Lock>,
    lock_ask: Option<LockAsk>,
    /// Config: the buttons a note's bar shows.
    pub note_buttons: Vec<String>,
    /// `crt.Session.Lock`: tab → the script run that locked it.
    locks: HashMap<TabId, u64>,
    /// The tab pressed last: a drag that starts is about it (the drag carries no tab).
    pressed: Option<(pane_grid::Pane, Entity)>,
    /// The tab being dragged. libcosmic says which only through its drop hints (the drag data
    /// is one byte, and `on_activate` comes on release, so not for a drag): only the source bar
    /// gives hints, and the first is the tab under the pointer when the drag began.
    drag_src: Option<(pane_grid::Pane, Entity)>,
    /// The last tab clicked and when: a second click soon after is a double-click.
    last_click: Option<(Entity, std::time::Instant)>,
    /// Global Options → double-click on a tab (`Config::tab_double_click`).
    double_click: u8,
    /// The tab right-clicked last: `TabCmd`s are about it.
    context: Option<(pane_grid::Pane, Entity)>,
    /// Every terminal's tap, for the scripts running on other threads.
    pub taps: crate::script::Taps,
    /// Where a Zmodem send from the server is received (none: lrzsz is missing).
    download: Option<std::path::PathBuf>,
    /// The tab `open` made last (none: it could not start).
    last_opened: Option<TabId>,
}

impl Tabs {
    pub fn set_sender(&mut self, tx: Events) {
        self.tx = Some(tx);
    }

    /// An SFTP tab has a file open in a local editor (the timer runs only then).
    pub fn has_edits(&self) -> bool {
        self.tabs
            .values()
            .any(|t| matches!(&t.body, Body::Sftp(v) if v.has_edits()))
    }

    /// A note has an edit not written yet (the timer runs only then).
    pub fn has_unsaved_notes(&self) -> bool {
        (self.tabs.values()).any(|t| matches!(&t.body, Body::Note(v) if v.has_unsaved()))
    }

    pub fn notes_tick(&mut self) {
        for tab in self.tabs.values_mut() {
            if let Body::Note(v) = &mut tab.body {
                v.tick();
            }
        }
    }

    /// Everything written (the app is going).
    pub fn save_notes(&mut self) {
        for tab in self.tabs.values_mut() {
            if let Body::Note(v) = &mut tab.body {
                v.save();
            }
        }
    }

    /// A note's message; true: it wants the file chooser. Its title follows its file name.
    pub fn on_note(&mut self, id: TabId, msg: crate::notes::NoteMsg) -> (bool, Task<Message>) {
        let Some(tab) = self.tabs.get_mut(&id) else {
            return (false, Task::none());
        };
        let Body::Note(v) = &mut tab.body else {
            return (false, Task::none());
        };
        use crate::notes::NoteMsg;
        let clip = |t: Option<String>| match t {
            Some(t) => iced::clipboard::write(t),
            None => Task::none(),
        };
        match msg {
            NoteMsg::Copy => return (false, clip(v.copy())),
            NoteMsg::Cut => return (false, clip(v.cut())),
            NoteMsg::Paste => {
                return (
                    false,
                    iced::clipboard::read().map(move |t| {
                        cosmic::Action::App(Message::Note(
                            id,
                            NoteMsg::PasteText(t.unwrap_or_default()),
                        ))
                    }),
                );
            }
            NoteMsg::Lock => {
                return (
                    false,
                    self.own_cmd(TabCmd::Lock, id).unwrap_or_else(Task::none),
                );
            }
            NoteMsg::SaveAsAsk => {
                return (
                    false,
                    self.own_cmd(TabCmd::NoteSaveAs, id)
                        .unwrap_or_else(Task::none),
                );
            }
            _ => {}
        }
        let rename = matches!(msg, NoteMsg::RenameStart);
        let field = v.rename_field();
        let before = v.path.clone();
        let open = v.update(msg);
        // Only a rename of the file changes the title (a tab renamed by hand keeps its name).
        if v.path != before {
            tab.launch.title = v.name();
            tab.base = v.name();
            tab.launch.note = Some(v.path.clone());
        }
        self.retitle(id);
        let task = match rename {
            true => widget::text_input::focus(field),
            false => Task::none(),
        };
        (open, task)
    }

    /// The tab that shows this file, if any.
    pub fn note_tab(&self, path: &std::path::Path) -> Option<TabId> {
        (self.tabs.iter())
            .find(|(_, t)| matches!(&t.body, Body::Note(v) if v.path == path))
            .map(|(id, _)| *id)
    }

    /// The files the notes show (what the next start opens).
    pub fn note_paths(&self) -> Vec<std::path::PathBuf> {
        let mut v: Vec<_> = (self.tabs.iter())
            .filter_map(|(id, t)| match &t.body {
                Body::Note(n) => Some((*id, n.path.clone())),
                _ => None,
            })
            .collect();
        v.sort();
        v.into_iter().map(|(_, p)| p).collect()
    }

    /// The saved sessions with a tab open (not their SFTP tabs), oldest tab first.
    /// The tabs to open again at the next start, in their order: saved sessions' ids and local
    /// shells (`rules::LOCAL`); notes are kept apart, SFTP and the rest are not.
    pub fn session_ids(&self) -> Vec<String> {
        let mut v: Vec<_> = (self.tabs.iter())
            .filter_map(|(id, t)| {
                let l = &t.launch;
                let local = matches!(l.protocol, Protocol::Local)
                    && !l.sftp
                    && l.note.is_none()
                    && !l.console
                    && matches!(t.body, Body::Terminal(_));
                match (&l.session, local) {
                    (Some(s), _) => Some((*id, s.clone())),
                    (None, true) => Some((*id, rules::LOCAL.to_string())),
                    _ => None,
                }
            })
            .collect();
        v.sort();
        v.into_iter().map(|(_, s)| s).collect()
    }

    pub fn edit_tick(&mut self) {
        for tab in self.tabs.values_mut() {
            if let Body::Sftp(v) = &mut tab.body {
                drop(v.update(SftpMsg::EditTick));
            }
        }
    }

    /// The terminals a Command Window line goes to (ones whose program runs).
    pub fn send_targets(&self, to: crate::command_window::SendTo) -> Vec<TabId> {
        use crate::command_window::SendTo;
        let live = |id: &TabId| {
            self.tabs
                .get(id)
                .is_some_and(|t| t.terminal().is_some() && t.alive)
        };
        let ids: Vec<TabId> = match to {
            SendTo::Current => self.active_id().into_iter().collect(),
            SendTo::Visible => (self.grid.iter().flat_map(|g| g.iter()))
                .filter_map(|(_, group)| group.active_id())
                .chain(self.detached.iter().map(|(_, id)| *id))
                .collect(),
            SendTo::All => self.tabs.keys().copied().collect(),
        };
        let mut ids: Vec<TabId> = ids.into_iter().filter(live).collect();
        ids.sort_unstable();
        ids
    }

    pub fn send(
        &mut self,
        ids: Vec<TabId>,
        mut pieces: Vec<shagshell_core::macros::Piece>,
    ) -> Task<Message> {
        use shagshell_core::macros::Piece;
        while !pieces.is_empty() {
            match pieces.remove(0) {
                Piece::Bytes(b) => {
                    for id in &ids {
                        if let Some(t) = self.terminal(*id) {
                            t.lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .send_scroll(b.clone());
                        }
                    }
                }
                Piece::Pause(d) => {
                    return Task::perform(tokio::time::sleep(d), move |()| {
                        cosmic::Action::App(Message::Tabs(TabsMsg::Send(
                            ids.clone(),
                            pieces.clone(),
                        )))
                    });
                }
            }
        }
        Task::none()
    }

    pub fn set_sftp_prefs(&mut self, config: &Config) {
        let hidden_changed = self.sftp_prefs.1 != config.sftp_show_hidden;
        self.sftp_prefs = (config.sftp_overwrite.choice(), config.sftp_show_hidden);
        for tab in self.tabs.values_mut() {
            if let Body::Sftp(v) = &mut tab.body {
                v.set_rule(self.sftp_prefs.0);
                // The option flips every tab; a Ctrl+H in a tab lasts until it does.
                if hidden_changed {
                    v.set_show_hidden(self.sftp_prefs.1);
                }
            }
        }
    }

    pub fn set_sftp_sender(&mut self, tx: crate::sftp_worker::Events) {
        self.sftp_tx = Some(tx);
    }

    /// What the status bar says about the active tab.
    pub fn status(&self) -> Option<crate::status_bar::Status> {
        let tab = self.tabs.get(&self.active_id()?)?;
        if matches!(tab.body, Body::Note(_) | Body::Console) {
            return None;
        }
        let mut s = crate::status_bar::Status {
            alive: tab.live(),
            protocol: tab.launch.label(),
            size: None,
            cursor: None,
            up: tab.live().then(|| tab.started.elapsed().as_secs()),
            log: None,
            recording: false,
        };
        if let Some(t) = tab.terminal() {
            let t = t.lock().unwrap_or_else(|e| e.into_inner());
            {
                let term = t.term.lock();
                s.size = Some((term.columns(), term.screen_lines()));
                let p = term.grid().cursor.point;
                s.cursor = Some((usize::try_from(p.line.0).unwrap_or(0) + 1, p.column.0 + 1));
            }
            s.log = (t.log.active())
                .then(|| t.log.path())
                .flatten()
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()));
            s.recording = t.log.recording();
        }
        Some(s)
    }

    /// The 1 s clock: unread output that stopped `QUIET_SECS` ago turns its tab «quiet».
    pub fn tick(&mut self) {
        let quiet = std::time::Duration::from_secs(shagshell_core::tab_icon::QUIET_SECS);
        let ids: Vec<TabId> = (self.tabs.iter())
            .filter(|(_, t)| t.unread && !t.quiet && t.alive && t.last_output.elapsed() >= quiet)
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            if let Some(t) = self.tabs.get_mut(&id) {
                t.quiet = true;
            }
            self.retitle(id);
        }
    }

    /// The Command Window's target (`None`: hidden): the terminals it sends to get a `*`.
    pub fn set_broadcast(&mut self, to: Option<crate::command_window::SendTo>) {
        use crate::command_window::SendTo;
        let terminals = (self.tabs.iter()).filter(|(_, t)| matches!(t.body, Body::Terminal(_)));
        let now: std::collections::BTreeSet<TabId> = match to {
            Some(SendTo::All) => terminals.map(|(id, _)| *id).collect(),
            Some(SendTo::Visible) => terminals
                .map(|(id, _)| *id)
                .filter(|id| self.in_front(*id))
                .collect(),
            _ => Default::default(),
        };
        if now != self.starred {
            let changed: Vec<TabId> = now.symmetric_difference(&self.starred).copied().collect();
            self.starred = now;
            for id in changed {
                self.retitle(id);
            }
        }
    }

    /// Edit → Find: the active terminal's next (or previous) match of the regex `re`, selected.
    pub fn find(&mut self, re: &str, forwards: bool) {
        if let Some(t) = self.active_terminal().and_then(|id| self.terminal(id)) {
            let mut t = t.lock().unwrap_or_else(|e| e.into_inner());
            t.search(re, forwards);
            t.needs_update = true;
        }
    }

    /// Global Options → blinking cursor: the active terminal's cursor goes off / on.
    pub fn blink(&mut self) {
        let active = self.active_terminal();
        self.show_cursors(active);
        if let Some(t) = active.and_then(|id| self.terminal(id)) {
            let mut t = t.lock().unwrap_or_else(|e| e.into_inner());
            t.blink_off = !t.blink_off;
            t.needs_update = true; // a block cursor is drawn into the text
        }
    }

    /// Every terminal but `except` shows its cursor (left in an «off» half by blinking).
    fn show_cursors(&self, except: Option<TabId>) {
        for (id, tab) in &self.tabs {
            if Some(*id) != except
                && let Some(t) = tab.terminal()
            {
                let mut t = t.lock().unwrap_or_else(|e| e.into_inner());
                if t.blink_off {
                    (t.blink_off, t.needs_update) = (false, true);
                }
            }
        }
    }

    /// Some tab is connected: the status bar's clock runs.
    pub fn any_live(&self) -> bool {
        self.tabs.values().any(Tab::live)
    }

    /// The active tab, if it is an SFTP one (its keys are routed to it).
    pub fn active_sftp(&self) -> Option<TabId> {
        let id = self.active_id()?;
        matches!(self.tabs.get(&id)?.body, Body::Sftp(_)).then_some(id)
    }

    /// The active SFTP tab shows a dialog of its own (its Enter / Esc are the dialog's).
    pub fn sftp_dialog(&self) -> Option<TabId> {
        let id = self.active_sftp()?;
        match &self.tabs.get(&id)?.body {
            Body::Sftp(v) if v.has_dialog() && self.confirm.is_none() => Some(id),
            _ => None,
        }
    }

    /// An SFTP tab's message; its title follows its connection.
    pub fn on_sftp(&mut self, id: TabId, msg: SftpMsg) -> Task<Message> {
        let Some(Body::Sftp(v)) = self.tabs.get_mut(&id).map(|t| &mut t.body) else {
            return Task::none();
        };
        let task = v.update(msg);
        self.retitle(id);
        task
    }

    /// What every tab runs (again on Reconnect / Clone).
    pub fn launches_mut(&mut self) -> impl Iterator<Item = &mut Launch> {
        self.tabs.values_mut().map(|t| &mut t.launch)
    }

    pub fn title(&self, id: TabId) -> Option<String> {
        self.tabs.get(&id).map(|t| t.launch.title.clone())
    }

    /// Its program runs (an askpass prompt from it is still wanted). An SFTP tab: always (its
    /// ssh asks while connecting).
    pub fn alive(&self, id: TabId) -> bool {
        self.tabs.get(&id).is_some_and(|t| match t.body {
            Body::Terminal(_) => t.alive,
            Body::Sftp(_) => true,
            Body::Note(_) | Body::Console => false,
        })
    }

    /// Bring tab `id` to the front of its group: an askpass dialog is about it.
    pub fn show(&mut self, id: TabId) {
        if let Some(w) = self.window_of(id) {
            self.raise = Some(w);
            return;
        }
        let Some(grid) = self.grid.as_mut() else {
            return;
        };
        for (pane, group) in grid.iter_mut() {
            let found = (group.model.iter()).find(|&e| group.model.data::<TabId>(e) == Some(&id));
            if let Some(e) = found {
                group.model.activate(e);
                self.focus = Some(*pane);
                break;
            }
        }
        self.mark_read();
    }

    /// The saved session the active tab was opened from.
    pub fn active_session(&self) -> Option<String> {
        self.session_of(self.active_id()?)
    }

    /// Focused group, its active entity and that tab's id.
    fn active(&self) -> Option<(pane_grid::Pane, Entity, TabId)> {
        let pane = self.focus?;
        let group = self.grid.as_ref()?.get(pane)?;
        Some((pane, group.model.active(), group.active_id()?))
    }

    /// The tab the user is working with: the one whose own window an event came from, else the
    /// active one of the focused group.
    pub fn active_id(&self) -> Option<TabId> {
        self.front.or_else(|| self.active().map(|(_, _, id)| id))
    }

    /// For the menu bar: (any tab, the active one ended, its group can be split).
    pub fn menu_state(&self) -> (bool, bool, bool, bool, bool) {
        let Some((pane, _, id)) = self.active() else {
            return (false, false, false, false, false);
        };
        let live = self.tabs.get(&id).is_some_and(Tab::live);
        let many = (self.grid.as_ref())
            .and_then(|g| g.get(pane))
            .is_some_and(|g| g.model.len() > 1);
        (true, live, many, self.logging(), self.recording())
    }

    fn tab_at(&self, pane: pane_grid::Pane, e: Entity) -> Option<(pane_grid::Pane, Entity, TabId)> {
        let id = *self.grid.as_ref()?.get(pane)?.model.data::<TabId>(e)?;
        Some((pane, e, id))
    }

    fn terminal(&self, id: TabId) -> Option<&Mutex<Terminal>> {
        self.tabs.get(&id)?.terminal()
    }

    pub fn last_opened(&self) -> Option<TabId> {
        self.last_opened
    }

    /// A new tab running `launch` in the focused group (the first group if none).
    pub fn open(&mut self, launch: Launch, look: &Look) -> Task<Message> {
        self.last_opened = None;
        let taken: Vec<&str> = self.tabs.values().map(|t| t.base.as_str()).collect();
        let base = rules::numbered(&launch.title, &taken);
        let id = self.next_id;
        let Some(body) = self.spawn(id, &launch, look) else {
            return Task::none();
        };
        self.next_id += 1;
        self.last_opened = Some(id);
        let widget_id = widget::Id::unique();
        let tab = Tab {
            started: std::time::Instant::now(),
            attempts: 0,
            stopped: false,
            body,
            launch,
            base,
            program_title: String::new(),
            alive: true,
            seen: false,
            unread: false,
            last_output: std::time::Instant::now(),
            quiet: false,
            widget_id: widget_id.clone(),
        };
        let title = tab.title();
        self.tabs.insert(id, tab);
        let pane = match (&mut self.grid, self.focus) {
            (Some(_), Some(p)) => p,
            _ => {
                let (grid, pane) = pane_grid::State::new(Group::default());
                (self.grid, self.focus) = (Some(grid), Some(pane));
                pane
            }
        };
        if let Some(group) = self.grid.as_mut().and_then(|g| g.get_mut(pane)) {
            group
                .model
                .insert()
                .text(title)
                .data(id)
                .closable()
                .activate();
        }
        self.retitle(id);
        focus(widget_id)
    }

    fn spawn(&self, id: TabId, launch: &Launch, look: &Look) -> Option<Body> {
        if launch.console {
            return Some(Body::Console);
        }
        if let Some(path) = &launch.note {
            return crate::notes::NoteView::open(path)
                .ok()
                .map(|v| Body::Note(Box::new(v)));
        }
        if launch.sftp {
            let connect = Connect {
                program: launch.program.clone(),
                args: launch.args.clone(),
                env: launch.env.clone(),
            };
            let v = SftpView::new(id, connect, self.sftp_tx.clone()?, self.sftp_prefs);
            return Some(Body::Sftp(Box::new(v)));
        }
        let tx = self.tx.clone()?;
        let (style, dark, bg) = look;
        let mut env = launch.env.clone();
        if !env.is_empty() {
            env.push(("SHAGSHELL_TAB".into(), id.to_string()));
        }
        let tap = term::log::Tap::default();
        let mut t = term::start(
            id,
            tx,
            &launch.program,
            &launch.args,
            &env,
            *dark,
            *bg,
            style,
            tap.clone(),
            self.scheme_of(launch),
        )
        .inspect_err(|e| log::error!("start {}: {e}", launch.program))
        .ok()?;
        tap.set_auto_receive(self.download.clone());
        self.taps
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id, tap.clone());
        t.set_highlights(self.highlights.clone());
        t.set_cursor_shape(self.cursor);
        if let Some(spec) = launch.log.as_ref().filter(|s| !s.file.is_empty())
            && let Err(e) = self.begin_log(&tap, launch, spec)
        {
            *self.notice.lock().unwrap_or_else(|e| e.into_inner()) = Some(e);
        }
        if let Some(l) = launch.logon.as_ref().filter(|l| !l.steps.is_empty()) {
            tap.set_logon(l.steps.clone(), answer(&tap, l));
        }
        Some(Body::Terminal(Box::new(Mutex::new(t))))
    }

    /// The log of `launch` started per `spec`; the file it writes, or why not.
    fn begin_log(
        &self,
        tap: &term::log::Tap,
        launch: &Launch,
        spec: &LogSpec,
    ) -> Result<std::path::PathBuf, String> {
        use shagshell_core::logfile::{Ctx, resolve};
        let host = match &launch.protocol {
            Protocol::Ssh { host, .. } => host.as_str(),
            Protocol::Telnet { host, .. } => host.as_str(),
            Protocol::Local | Protocol::Serial { .. } => "",
        };
        let ctx = Ctx {
            host,
            session: &launch.title,
            port: spec.port,
            user: &spec.user,
            now: shagshell_core::logfile::now(),
        };
        let home = std::env::home_dir().unwrap_or_default();
        let path = resolve(&self.log_prefs.0, &spec.file, &ctx, &home);
        tap.start(&path, spec.append, spec.raw)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(path)
    }

    pub fn set_log_prefs(&mut self, config: &Config) {
        self.log_prefs = (config.log_dir(), config.log_template.clone());
        self.download =
            (config.zmodem_auto && crate::menu::has_tool("rz")).then(|| config.download_dir());
        for tap in self.taps.lock().unwrap_or_else(|e| e.into_inner()).values() {
            tap.set_auto_receive(self.download.clone());
        }
    }

    /// Log Session: the active tab's log on / off (a started one by the session's own template,
    /// else the one of Global Options). `Err`: why it could not start.
    pub fn toggle_log(&mut self) -> Result<(), String> {
        let Some(id) = self.active_id() else {
            return Ok(());
        };
        let Some(tab) = self.tabs.get(&id) else {
            return Ok(());
        };
        let Some(t) = tab.terminal() else {
            return Ok(());
        };
        let tap = t.lock().unwrap_or_else(|e| e.into_inner()).log.clone();
        if tap.active() {
            tap.stop();
            return Ok(());
        }
        self.log_on(id, None).map(|_| ())
    }

    /// Starts the log of tab `id` (`append` overrides the session's): the session's own name
    /// template, else the one of Global Options; the port and user are the session's either way.
    fn log_on(&self, id: TabId, append: Option<bool>) -> Result<std::path::PathBuf, String> {
        let tab = self.tabs.get(&id).ok_or("no such tab")?;
        let t = tab.terminal().ok_or("not a terminal tab")?;
        let tap = t.lock().unwrap_or_else(|e| e.into_inner()).log.clone();
        let mut spec = tab.launch.log.clone().unwrap_or_default();
        if spec.file.is_empty() {
            spec.file = self.log_prefs.1.clone();
        }
        if let Some(a) = append {
            spec.append = a;
        }
        self.begin_log(&tap, &tab.launch, &spec)
    }

    /// Scripts: `Session.Log(on[, append])`.
    pub fn script_log(&self, id: TabId, on: bool, append: Option<bool>) -> Result<(), String> {
        let tap = self.tap_of(id).ok_or("no such tab")?;
        match on {
            true if tap.active() => Ok(()),
            true => self.log_on(id, append).map(|_| ()),
            false => {
                tap.stop();
                Ok(())
            }
        }
    }

    /// Scripts: `Session.LogText(text)`.
    pub fn script_log_text(&self, id: TabId, text: &str) -> Result<(), String> {
        let tap = self.tap_of(id).ok_or("no such tab")?;
        tap.log_text(text).map_err(|e| e.to_string())
    }

    /// Scripts: the file of the tab's log ("" when it is off).
    pub fn log_file(&self, id: TabId) -> String {
        self.tap_of(id)
            .and_then(|t| t.path())
            .map_or_else(String::new, |p| p.display().to_string())
    }

    fn tap_of(&self, id: TabId) -> Option<term::log::Tap> {
        let t = self.terminal(id)?;
        Some(t.lock().unwrap_or_else(|e| e.into_inner()).log.clone())
    }

    /// Scripts: what a terminal tab shows.
    pub fn screen(&self, id: TabId) -> Option<term::Screen> {
        self.terminal(id).map(term::screen)
    }

    /// Scripts: wipes a terminal tab's screen and scrollback (the program is not told).
    pub fn clear_screen(&self, id: TabId) -> Option<()> {
        term::print(self.terminal(id)?, "\x1b[2J\x1b[3J\x1b[H");
        Some(())
    }

    /// The active tab is being recorded as a script.
    fn recording(&self) -> bool {
        self.active_id()
            .and_then(|id| self.tap_of(id))
            .is_some_and(|t| t.recording())
    }

    /// Script → Start / Stop Recording on the active terminal. Stopping gives the script text
    /// and the tab's name.
    pub fn toggle_recording(&mut self) -> Option<(String, shagshell_core::recorder::Recorder)> {
        let id = self.active_id()?;
        let tap = self.tap_of(id)?;
        let title = self
            .tabs
            .get(&id)
            .map(|t| t.launch.title.clone())
            .unwrap_or_default();
        if tap.recording() {
            return tap.stop_recording().map(|r| (title, r));
        }
        // The prompt that is on the screen now is the first step's.
        let line = self.screen(id).map(|s| {
            let row = s.rows.get(s.row).map(String::as_str).unwrap_or_default();
            row.chars().take(s.col).collect::<String>()
        });
        tap.start_recording(&line.unwrap_or_default());
        None
    }

    /// The recordings of tabs that were closed meanwhile, once.
    pub fn take_closed_recordings(&mut self) -> Vec<(String, shagshell_core::recorder::Recorder)> {
        std::mem::take(&mut self.closed_recordings)
    }

    /// The active tab is being logged.
    fn logging(&self) -> bool {
        self.active_id()
            .and_then(|id| self.terminal(id))
            .is_some_and(|t| t.lock().unwrap_or_else(|e| e.into_inner()).log.active())
    }

    /// What a log said that the user should see: it could not start, or stopped (disk full).
    pub fn take_notice(&self) -> Option<String> {
        let own = self.notice.lock().unwrap_or_else(|e| e.into_inner()).take();
        own.or_else(|| {
            self.tabs.values().filter_map(Tab::terminal).find_map(|t| {
                t.lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .log
                    .take_failure()
            })
        })
    }

    /// Start the program again in a tab whose program ended (SecureCRT's Reconnect / Enter).
    /// Under a new id: the old program's late events (Exit after ChildExit) find no tab.
    /// Disconnect: the program (or the SFTP connection) ends, the tab stays (✗, Enter restarts).
    fn disconnect(&mut self, id: TabId) {
        let Some(tab) = self.tabs.get_mut(&id) else {
            return;
        };
        match &mut tab.body {
            Body::Terminal(t) if tab.alive => {
                t.get_mut().unwrap_or_else(|e| e.into_inner()).shutdown();
                term::print(t, &format!("\r\n[{}]\r\n", fl!("term-disconnected")));
                tab.alive = false;
                tab.stopped = true;
            }
            Body::Sftp(v) => v.disconnect(),
            Body::Note(_) | Body::Console => {}
            // Already down, maybe waiting to reconnect by itself: that wait ends.
            Body::Terminal(_) => tab.stopped = true,
        }
        self.retitle(id);
    }

    /// A saved session's terminal tab and its title; `None` for the rest (SFTP, notes, plain shells).
    pub fn saved_session(&self, id: TabId) -> Option<(String, String)> {
        let l = &self.tabs.get(&id)?.launch;
        let sid = l
            .session
            .clone()
            .filter(|_| !l.sftp && l.note.is_none() && !l.console)?;
        Some((sid, l.title.clone()))
    }

    /// Reconnect / Clone of such a tab go through the app first (the session's pre-script).
    fn gate(&self, cmd: TabCmd, id: TabId) -> Option<Task<Message>> {
        (matches!(cmd, TabCmd::Reconnect | TabCmd::Clone) && self.saved_session(id).is_some())
            .then(|| Task::done(cosmic::Action::App(Message::PreGate(cmd, id))))
    }

    /// The gated command, once the app lets it through.
    pub fn run_gated(&mut self, cmd: TabCmd, id: TabId, look: &Look) -> Task<Message> {
        match cmd {
            TabCmd::Reconnect => self.reconnect(id, look),
            TabCmd::Clone => {
                let pane = (self.grid.iter().flat_map(|g| g.iter()))
                    .find(|(_, g)| entity_of(&g.model, id).is_some())
                    .map(|(p, _)| *p);
                if pane.is_some() {
                    self.focus = pane; // its group
                }
                match self.tabs.get(&id).map(|t| t.launch.clone()) {
                    Some(launch) => self.open(launch, look),
                    None => Task::none(),
                }
            }
            _ => Task::none(),
        }
    }

    /// Reconnect: a live tab is disconnected first.
    fn reconnect(&mut self, id: TabId, look: &Look) -> Task<Message> {
        self.disconnect(id);
        self.restart(id, look)
    }

    fn restart(&mut self, id: TabId, look: &Look) -> Task<Message> {
        if self
            .tabs
            .get(&id)
            .is_none_or(|t| t.live() || matches!(t.body, Body::Note(_) | Body::Console))
        {
            return Task::none();
        }
        // SFTP: a new connection in the same tab (its old workers' events are told apart),
        // with the session as it is now (`sessions_changed` keeps `launch` current).
        if let Some(tab) = self.tabs.get_mut(&id)
            && let Body::Sftp(v) = &mut tab.body
        {
            v.set_connect(Connect {
                program: tab.launch.program.clone(),
                args: tab.launch.args.clone(),
                env: tab.launch.env.clone(),
            });
            v.reconnect();
            self.retitle(id);
            return Task::none();
        }
        let new_id = self.next_id;
        let launch = self.tabs.get(&id).map(|t| t.launch.clone());
        let Some(terminal) = launch.and_then(|l| self.spawn(new_id, &l, look)) else {
            return Task::none();
        };
        self.next_id += 1;
        let Some(mut tab) = self.tabs.remove(&id) else {
            return Task::none();
        };
        self.forget_tap(id);
        (tab.body, tab.alive) = (terminal, true);
        (tab.started, tab.stopped) = (std::time::Instant::now(), false);
        (tab.seen, tab.unread, tab.quiet) = (false, false, false);
        tab.program_title.clear();
        let widget_id = tab.widget_id.clone();
        self.tabs.insert(new_id, tab);
        for (_, group) in self.grid.iter_mut().flat_map(|g| g.iter_mut()) {
            if let Some(e) = entity_of(&group.model, id) {
                group.model.data_set(e, new_id);
            }
        }
        // The tab of a window of its own keeps its window.
        for (_, tab_id) in self.detached.iter_mut().filter(|(_, t)| *t == id) {
            *tab_id = new_id;
        }
        self.retitle(new_id);
        focus(widget_id)
    }

    /// A dragged tab (the one pressed last) goes to group `dst`: the same tab, still running.
    fn move_pressed_to(&mut self, dst: pane_grid::Pane) {
        let Some((src, e)) = self.drag_src.take().or(self.pressed.take()) else {
            return;
        };
        let Some(grid) = self.grid.as_mut() else {
            return;
        };
        if src == dst {
            return; // inside its own bar it is a reorder
        }
        let moved = grid.get(src).and_then(|g| {
            Some((
                g.model.data::<TabId>(e).copied()?,
                g.model.text(e).map(str::to_string)?,
            ))
        });
        let (Some((id, title)), true) = (moved, grid.get(dst).is_some()) else {
            return;
        };
        let emptied = grid.get_mut(src).is_some_and(|g| detach(&mut g.model, e));
        if let Some(g) = grid.get_mut(dst) {
            g.model.insert().text(title).data(id).closable().activate();
        }
        if emptied && grid.len() > 1 {
            grid.close(src);
        }
        self.focus = Some(dst);
        self.retitle(id);
    }

    /// The tab into a window of its own: it leaves its group, the program goes on.
    fn detach(&mut self, (pane, e, id): (pane_grid::Pane, Entity, TabId)) -> Task<Message> {
        let Some(grid) = self.grid.as_mut() else {
            return Task::none();
        };
        let emptied = grid.get_mut(pane).is_some_and(|g| detach(&mut g.model, e));
        if emptied {
            match grid.len() > 1 {
                true => {
                    if let Some((_, sibling)) = grid.close(pane) {
                        self.focus = Some(sibling);
                    }
                }
                false => (self.grid, self.focus) = (None, None),
            }
        }
        let (window, open) = iced::window::open(iced::window::Settings {
            size: iced::Size::new(900.0, 600.0),
            min_size: Some(iced::Size::new(400.0, 240.0)),
            decorations: false,
            exit_on_close_request: false,
            ..Default::default()
        });
        self.detached.push((window, id));
        let tab = self.tabs.get(&id);
        self.new_windows
            .push((window, tab.map(Tab::title).unwrap_or_default()));
        let widget_id = tab.map(|t| t.widget_id.clone());
        let focus = widget_id.map_or_else(Task::none, focus);
        Task::batch([open.then(|_| Task::none()), focus])
    }

    /// The windows just opened for tabs and their titles, once.
    pub fn take_new_windows(&mut self) -> Vec<(iced::window::Id, String)> {
        std::mem::take(&mut self.new_windows)
    }

    /// A tab's window that is to come to the front, once.
    pub fn take_raise(&mut self) -> Option<iced::window::Id> {
        self.raise.take()
    }

    /// The tab of `window`, back into the focused group; the window closes.
    fn reattach(&mut self, window: iced::window::Id) -> Task<Message> {
        let Some(i) = self.detached.iter().position(|(w, _)| *w == window) else {
            return Task::none();
        };
        let (_, id) = self.detached.remove(i);
        let title = self.tabs.get(&id).map(Tab::title).unwrap_or_default();
        let pane = match (&mut self.grid, self.focus) {
            (Some(_), Some(p)) => p,
            _ => {
                let (grid, pane) = pane_grid::State::new(Group::default());
                (self.grid, self.focus) = (Some(grid), Some(pane));
                pane
            }
        };
        if let Some(group) = self.grid.as_mut().and_then(|g| g.get_mut(pane)) {
            group
                .model
                .insert()
                .text(title)
                .data(id)
                .closable()
                .activate();
        }
        self.retitle(id);
        Task::batch([iced::window::close(window), self.focus_active()])
    }

    /// A tab command about a tab in a window of its own (no group: splitting, moving and
    /// closing mean bringing it back).
    fn cmd_in_window(&mut self, cmd: TabCmd, id: TabId, look: &Look) -> Task<Message> {
        if let Some(task) = self.gate(cmd, id).or_else(|| self.own_cmd(cmd, id)) {
            return task;
        }
        match cmd {
            TabCmd::Reconnect => self.reconnect(id, look),
            TabCmd::Disconnect => {
                self.disconnect(id);
                Task::none()
            }
            TabCmd::Clone => match (self.tabs.get(&id).map(|tab| tab.launch.clone()))
                .filter(|l| l.note.is_none() && !l.console)
            {
                Some(launch) => self.open(launch, look),
                None => Task::none(),
            },
            TabCmd::Sftp => match self.tabs.get(&id).and_then(|tab| tab.launch.sftp()) {
                Some(launch) => self.open(launch, look),
                None => Task::none(),
            },
            TabCmd::Close | TabCmd::Detach | TabCmd::CloseOthers | TabCmd::CloseAll => {
                match self.window_of(id) {
                    Some(w) => self.reattach(w),
                    None => Task::none(),
                }
            }
            _ => Task::none(),
        }
    }

    /// Lock and the note's commands: the same in a group or a window of its own.
    fn own_cmd(&mut self, cmd: TabCmd, id: TabId) -> Option<Task<Message>> {
        use crate::notes::NoteMsg;
        let note = |m: NoteMsg| Some(m);
        let msg = match cmd {
            TabCmd::Lock => {
                let unlock = self.user_locks.contains_key(&id);
                let field = widget::Id::unique();
                self.lock_ask = Some(LockAsk {
                    id,
                    unlock,
                    pw: String::new(),
                    pw2: String::new(),
                    error: None,
                    field: field.clone(),
                });
                return Some(widget::text_input::focus(field));
            }
            TabCmd::NoteSave => note(NoteMsg::Save),
            TabCmd::NoteWrap => note(NoteMsg::Wrap),
            TabCmd::NoteNumbers => note(NoteMsg::LineNumbers),
            TabCmd::NoteRename => note(NoteMsg::RenameStart),
            TabCmd::NoteSaveAs => {
                let dir = self.note_path(id)?.parent()?.to_path_buf();
                return Some(Task::perform(
                    crate::notes::save_as(fl!("note-save-as"), dir),
                    move |p| cosmic::Action::App(Message::Note(id, NoteMsg::SaveAs(p))),
                ));
            }
            _ => None,
        }?;
        Some(self.on_note(id, msg).1)
    }

    fn note_path(&self, id: TabId) -> Option<std::path::PathBuf> {
        match &self.tabs.get(&id)?.body {
            Body::Note(v) => Some(v.path.clone()),
            _ => None,
        }
    }

    /// Lock Session on (or off): a terminal drops what is typed, a note is read only.
    fn set_user_lock(&mut self, id: TabId, on: bool) {
        let Some(tab) = self.tabs.get_mut(&id) else {
            return;
        };
        match &mut tab.body {
            Body::Note(v) => v.locked = on,
            // A script's own lock stays until that script lets go.
            Body::Terminal(t) if !self.locks.contains_key(&id) => {
                t.get_mut()
                    .unwrap_or_else(|e| e.into_inner())
                    .set_locked(on);
            }
            _ => {}
        }
        self.retitle(id);
    }

    fn window_of(&self, id: TabId) -> Option<iced::window::Id> {
        self.detached
            .iter()
            .find(|(_, t)| *t == id)
            .map(|(w, _)| *w)
    }

    /// The tab shown in `window`, if it is one of ours.
    pub fn tab_in(&self, window: iced::window::Id) -> Option<TabId> {
        self.detached
            .iter()
            .find(|(w, _)| *w == window)
            .map(|(_, t)| *t)
    }

    /// A tab's own window: a title bar with «back» and the tab.
    pub fn window_view<'a>(
        &'a self,
        window: iced::window::Id,
        config: &Config,
        focused: bool,
        keys: bool,
        scripts: &'a crate::script::Scripts,
    ) -> Option<Element<'a, Message>> {
        let id = self.tab_in(window)?;
        let tab = self.tabs.get(&id)?;
        let m = |x| Message::Tabs(x);
        let bar = widget::header_bar()
            .title(tab.title())
            .start(
                widget::button::text(fl!("tab-window-back")).on_press(m(TabsMsg::Reattach(window))),
            )
            .on_close(m(TabsMsg::Reattach(window)))
            .on_drag(m(TabsMsg::DragWindow(window)))
            .focused(focused);
        let body = self.body(
            Some(id),
            config.copy_on_select,
            config.paste_on_middle_click,
            keys,
            scripts,
        );
        let content = widget::column::with_children(vec![bar.into(), body]);
        Some(
            widget::container(content)
                .width(Length::Fill)
                .height(Length::Fill)
                .class(cosmic::theme::Container::WindowBackground)
                .into(),
        )
    }

    /// What a tab shows: its terminal or its SFTP view (empty: no tab).
    fn body<'a>(
        &'a self,
        id: Option<TabId>,
        copy: bool,
        middle: bool,
        keys: bool,
        scripts: &'a crate::script::Scripts,
    ) -> Element<'a, Message> {
        match id.and_then(|id| self.tabs.get(&id)) {
            Some(Tab {
                body: Body::Console,
                ..
            }) => scripts.console_view(),
            Some(Tab {
                body: Body::Sftp(v),
                ..
            }) => v.view(),
            Some(
                tab @ Tab {
                    body: Body::Note(v),
                    ..
                },
            ) => v.view(
                id.unwrap_or_default(),
                tab.widget_id.clone(),
                &self.note_buttons,
            ),
            Some(
                tab @ Tab {
                    body: Body::Terminal(terminal),
                    ..
                },
            ) => term::terminal_box::terminal_box(terminal, is_app_key)
                .id(tab.widget_id.clone())
                .copy_on_select(copy)
                .paste_on_middle(middle)
                // An ended program takes no keys (its PTY is gone): Enter reaches the app.
                // Nor does any while a dialog is up.
                .ended(!tab.alive || !keys)
                .padding(4)
                .into(),
            None => widget::Space::new()
                .width(Length::Fill)
                .height(Length::Fill)
                .into(),
        }
    }

    fn forget_tap(&self, id: TabId) {
        self.taps
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&id);
    }

    /// The terminal tabs in the order they are shown (groups as laid out, tabs left to right).
    pub fn order(&self) -> Vec<TabId> {
        let ids = self.grid.iter().flat_map(|g| g.iter()).flat_map(|(_, g)| {
            g.model
                .iter()
                .filter_map(|e| g.model.data::<TabId>(e).copied())
        });
        let detached = self.detached.iter().map(|(_, id)| *id);
        ids.chain(detached)
            .filter(|id| self.terminal(*id).is_some())
            .collect()
    }

    /// The active tab's id, if it is a terminal.
    pub fn active_terminal(&self) -> Option<TabId> {
        let id = self.active_id()?;
        self.terminal(id).map(|_| id)
    }

    /// The saved session a tab was opened from.
    pub fn session_of(&self, id: TabId) -> Option<String> {
        match &self.tabs.get(&id)?.launch.protocol {
            Protocol::Ssh { session, .. }
            | Protocol::Telnet { session, .. }
            | Protocol::Serial { session, .. } => session.clone(),
            Protocol::Local => None,
        }
    }

    /// Shown now: the active tab of its group, or alone in a window of its own.
    fn in_front(&self, id: TabId) -> bool {
        self.detached.iter().any(|(_, t)| *t == id)
            || (self.grid.iter().flat_map(|g| g.iter()))
                .any(|(_, g)| g.model.active_data::<TabId>() == Some(&id))
    }

    /// The tabs in front have been seen: their «new output» goes. The active one is the most
    /// recent.
    fn mark_read(&mut self) {
        if let Some(a) = self.active_id().filter(|a| self.recent.first() != Some(a)) {
            let tabs = &self.tabs;
            rules::touch(&mut self.recent, a, |id| tabs.contains_key(&id));
        }
        let ids: Vec<TabId> = (self.tabs.iter())
            .filter(|(_, t)| t.unread)
            .map(|(id, _)| *id)
            .filter(|id| self.in_front(*id))
            .collect();
        for id in ids {
            if let Some(t) = self.tabs.get_mut(&id) {
                (t.unread, t.quiet) = (false, false);
            }
            self.retitle(id);
        }
    }

    /// A tab's title.
    pub fn caption(&self, id: TabId) -> Option<String> {
        self.tabs.get(&id).map(Tab::title)
    }

    /// Every tab's text again (a session's label changed).
    pub fn retitle_all(&mut self) {
        let ids: Vec<TabId> = self.tabs.keys().copied().collect();
        for id in ids {
            self.retitle(id);
        }
    }

    /// Transfer → …: lrzsz's `cmd` on the active terminal's connection.
    pub fn start_transfer(
        &self,
        id: TabId,
        cmd: &shagshell_core::zmodem::Command,
    ) -> Result<(), String> {
        let tap = (self.taps.lock().unwrap_or_else(|e| e.into_inner()))
            .get(&id)
            .cloned()
            .ok_or_else(|| fl!("zm-no-tab"))?;
        tap.begin(cmd).map_err(|e| e.to_string())
    }

    /// Transfer → Cancel Transfer.
    pub fn cancel_transfer(&self) {
        if let Some(id) = self.active_terminal()
            && let Some(tap) = (self.taps.lock().unwrap_or_else(|e| e.into_inner())).get(&id)
        {
            tap.cancel_transfer();
        }
    }

    pub fn disconnect_tab(&mut self, id: TabId) {
        self.disconnect(id);
    }

    /// `crt.Session.Lock / Unlock` from run `run` (a locked tab drops what the user types).
    pub fn set_lock(&mut self, id: TabId, on: bool, run: u64) -> Result<(), String> {
        let Some(t) = self.terminal(id) else {
            self.locks.remove(&id);
            return Err("no such terminal tab".into());
        };
        // Held by another run: that one's to release.
        match self.locks.get(&id) {
            Some(owner) if *owner != run => {
                return Err("the tab is locked by another script".into());
            }
            _ => {}
        }
        // The user's own lock stays when the script lets go.
        let user = self.user_locks.contains_key(&id);
        t.lock()
            .unwrap_or_else(|e| e.into_inner())
            .set_locked(on || user);
        match on {
            true => self.locks.insert(id, run),
            false => self.locks.remove(&id),
        };
        self.retitle(id);
        Ok(())
    }

    pub fn locked(&self, id: TabId) -> bool {
        self.terminal(id)
            .is_some_and(|t| t.lock().unwrap_or_else(|e| e.into_inner()).locked())
    }

    /// A run ended: the tabs it locked are free.
    pub fn unlock_run(&mut self, run: u64) {
        let ids: Vec<TabId> = (self.locks.iter())
            .filter(|(_, r)| **r == run)
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            let _ = self.set_lock(id, false, run);
        }
    }

    /// What `crt.FileTransfer` runs for this tab: its ssh as an SFTP subsystem; `None` for a tab
    /// that is not an ssh one.
    pub fn sftp_launch(&self, id: TabId) -> Option<Launch> {
        let mut l = self.tabs.get(&id)?.launch.sftp()?;
        l.env.push(("SHAGSHELL_TAB".into(), id.to_string()));
        Some(l)
    }

    fn retitle(&mut self, id: TabId) {
        let Some((title, icon)) = self.tabs.get(&id).map(|t| (t.title(), t.icon())) else {
            return;
        };
        let title = match self.starred.contains(&id) {
            true => format!("* {title}"),
            false => title,
        };
        for (_, group) in self.grid.iter_mut().flat_map(|g| g.iter_mut()) {
            if let Some(e) = entity_of(&group.model, id) {
                group.model.text_set(e, title.clone());
                group.model.icon_set(e, icon.clone());
            }
        }
    }

    /// «Close others» (all but `keep`) / «Close all»: asks once when a live tab is among them.
    fn close_many(&mut self, keep: Option<(pane_grid::Pane, Entity)>) -> Task<Message> {
        let mut all = Vec::new();
        for (pane, group) in self.grid.iter().flat_map(|g| g.iter()) {
            all.extend(group.model.iter().map(|e| (*pane, e)));
        }
        all.retain(|t| Some(*t) != keep);
        let live = all.iter().any(|(pane, e)| {
            (self.grid.as_ref())
                .and_then(|g| g.get(*pane)?.model.data::<TabId>(*e).copied())
                .and_then(|id| self.tabs.get(&id))
                .is_some_and(|t| t.live())
        });
        match (live, all.is_empty()) {
            (_, true) => Task::none(),
            (true, _) => {
                self.confirm_many = Some(all);
                Task::none()
            }
            _ => self.close_list(all),
        }
    }

    fn close_list(&mut self, list: Vec<(pane_grid::Pane, Entity)>) -> Task<Message> {
        // Panes and entities are stable ids: closing one does not renumber the others.
        Task::batch(list.into_iter().map(|(p, e)| self.close_now(p, e)))
    }

    /// Close without asking.
    fn close_now(&mut self, pane: pane_grid::Pane, e: Entity) -> Task<Message> {
        let Some(grid) = self.grid.as_mut() else {
            return Task::none();
        };
        let Some(group) = grid.get_mut(pane) else {
            return Task::none();
        };
        if let Some(id) = group.model.data::<TabId>(e).copied() {
            let tap = self
                .tabs
                .get(&id)
                .and_then(Tab::terminal)
                .map(|t| t.lock().unwrap_or_else(|e| e.into_inner()).log.clone());
            if let Some(r) = tap.and_then(|t| t.stop_recording()) {
                let title = self
                    .tabs
                    .get(&id)
                    .map(|t| t.launch.title.clone())
                    .unwrap_or_default();
                self.closed_recordings.push((title, r));
            }
            if let Some(Tab {
                body: Body::Note(v),
                ..
            }) = self.tabs.get_mut(&id)
            {
                v.save();
            }
            self.user_locks.remove(&id);
            self.tabs.remove(&id); // its Drop ends the program
            self.taps
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&id);
        }
        match detach(&mut group.model, e) {
            false => return self.focus_active(),
            true if grid.len() > 1 => {
                if let Some((_, sibling)) = grid.close(pane) {
                    self.focus = Some(sibling);
                }
            }
            true => (self.grid, self.focus) = (None, None),
        }
        self.focus_active()
    }

    pub fn focus_active(&self) -> Task<Message> {
        match self.active_id().and_then(|id| self.tabs.get(&id)) {
            Some(t) => focus(t.widget_id.clone()),
            None => Task::none(),
        }
    }

    /// The Script Console's tab.
    fn console_id(&self) -> Option<TabId> {
        (self.tabs.iter()).find_map(|(id, t)| matches!(t.body, Body::Console).then_some(*id))
    }

    /// The Script Console as a tab in the group at the right (a new one beside the focused when
    /// there is only one); there already: shown when `raise`. The focus stays where it was.
    pub fn open_console(&mut self, raise: bool, look: &Look) -> Task<Message> {
        if let Some(id) = self.console_id() {
            if raise {
                self.show(id);
            }
            return Task::none();
        }
        let back = self.focus;
        // The right-most group from the focused one.
        if let (Some(grid), Some(mut p)) = (&self.grid, self.focus) {
            while let Some(next) = grid.adjacent(p, pane_grid::Direction::Right) {
                p = next;
            }
            self.focus = Some(p);
        }
        // What the target group shows now: an automatic open must not hide it.
        let shown = (self.grid.as_ref().zip(self.focus))
            .and_then(|(g, p)| g.get(p))
            .map(|g| (self.focus, g.model.active()));
        let mut launch = Launch::local_shell();
        launch.title = fl!("console-title");
        launch.console = true;
        let task = self.open(launch, look);
        let Some(id) = self.console_id() else {
            return task;
        };
        // Alone in the only group: a group of its own, at its right.
        let alone_group = self.grid.as_ref().is_some_and(|g| g.len() == 1);
        let at = self.grid.as_ref().and_then(|g| {
            g.iter()
                .find_map(|(pane, group)| entity_of(&group.model, id).map(|e| (*pane, e, id)))
        });
        let split = match at {
            Some(at) if alone_group => self.split(at, pane_grid::Axis::Vertical),
            _ => Task::none(),
        };
        if raise {
            return Task::batch([task, split]);
        }
        if let (Some((Some(p), e)), Some(grid)) = (shown, self.grid.as_mut())
            && !alone_group
            && let Some(g) = grid.get_mut(p)
        {
            g.model.activate(e);
        }
        self.focus = back.filter(|p| self.grid.as_ref().is_some_and(|g| g.get(*p).is_some()));
        // Not `task` / `split`: their focus would take the keyboard from the terminal.
        Task::none()
    }

    /// A note as a tab in the group at the right (a new one beside the focused when there is only
    /// one), where the Script Console lives too; the note is shown and takes the focus.
    pub fn open_note_right(&mut self, launch: Launch, look: &Look) -> Task<Message> {
        let note = launch.note.clone();
        if let (Some(grid), Some(mut p)) = (&self.grid, self.focus) {
            while let Some(next) = grid.adjacent(p, pane_grid::Direction::Right) {
                p = next;
            }
            self.focus = Some(p);
        }
        let alone_group = self.grid.as_ref().is_some_and(|g| g.len() == 1);
        let task = self.open(launch, look);
        let id = note.and_then(|n| self.note_tab(&n));
        let at = id.and_then(|id| {
            self.grid.as_ref().and_then(|g| {
                g.iter()
                    .find_map(|(pane, group)| entity_of(&group.model, id).map(|e| (*pane, e, id)))
            })
        });
        let split = match at {
            Some(at) if alone_group => self.split(at, pane_grid::Axis::Vertical),
            _ => Task::none(),
        };
        Task::batch([task, split])
    }

    /// That tab into a new group beside its own; refused when it is alone in its group.
    fn split(
        &mut self,
        (pane, e, id): (pane_grid::Pane, Entity, TabId),
        axis: pane_grid::Axis,
    ) -> Task<Message> {
        let Some(grid) = self.grid.as_mut() else {
            return Task::none();
        };
        let title = self.tabs.get(&id).map(Tab::title).unwrap_or_default();
        let Some(group) = grid.get_mut(pane) else {
            return Task::none();
        };
        if group.model.len() < 2 {
            return Task::none();
        }
        detach(&mut group.model, e);
        let mut new = Group::default();
        new.model
            .insert()
            .text(title)
            .data(id)
            .closable()
            .activate();
        if let Some((new_pane, _)) = grid.split(axis, pane, new) {
            self.focus = Some(new_pane);
        }
        self.retitle(id);
        self.focus_active()
    }

    /// A tab command on tab `t` (right-clicked, or the active one).
    fn cmd(
        &mut self,
        cmd: TabCmd,
        t: (pane_grid::Pane, Entity, TabId),
        look: &Look,
    ) -> Task<Message> {
        if let Some(task) = self.gate(cmd, t.2).or_else(|| self.own_cmd(cmd, t.2)) {
            return task;
        }
        match cmd {
            TabCmd::Reconnect => self.reconnect(t.2, look),
            TabCmd::Disconnect => {
                self.disconnect(t.2);
                Task::none()
            }
            TabCmd::Clone => {
                self.focus = Some(t.0); // its group
                match (self.tabs.get(&t.2).map(|tab| tab.launch.clone()))
                    .filter(|l| l.note.is_none() && !l.console)
                {
                    Some(launch) => self.open(launch, look),
                    None => Task::none(),
                }
            }
            TabCmd::Sftp => {
                self.focus = Some(t.0);
                match self.tabs.get(&t.2).and_then(|tab| tab.launch.sftp()) {
                    Some(launch) => self.open(launch, look),
                    None => Task::none(),
                }
            }
            TabCmd::SplitRight => self.split(t, pane_grid::Axis::Vertical),
            TabCmd::SplitDown => self.split(t, pane_grid::Axis::Horizontal),
            TabCmd::MoveTo(side) => {
                let to = self
                    .grid
                    .as_ref()
                    .and_then(|g| g.adjacent(t.0, side.direction()));
                if let Some(to) = to {
                    self.pressed = Some((t.0, t.1));
                    self.move_pressed_to(to);
                }
                self.focus_active()
            }
            TabCmd::Detach => self.detach(t),
            TabCmd::Close => self.update(TabsMsg::Close(t.0, t.1), look),
            TabCmd::CloseOthers => self.close_many(Some((t.0, t.1))),
            TabCmd::CloseAll => self.close_many(None),
            _ => Task::none(),
        }
    }

    /// Actions about tabs from keys and the menu bar: on the active tab. `None`: not ours.
    pub fn act(&mut self, action: Action, look: &Look) -> Option<Task<Message>> {
        let task = self.act_inner(action, look);
        self.mark_read();
        task
    }

    fn act_inner(&mut self, action: Action, look: &Look) -> Option<Task<Message>> {
        let on_active = |this: &mut Self, cmd| {
            if let Some(id) = this.front {
                return this.cmd_in_window(cmd, id, look);
            }
            match this.active() {
                Some(t) => this.cmd(cmd, t, look),
                None => Task::none(),
            }
        };
        let task = match action {
            Action::NewLocalShell => self.open(Launch::local_shell(), look),
            Action::CloseTab => on_active(self, TabCmd::Close),
            Action::Reconnect => on_active(self, TabCmd::Reconnect),
            Action::ZoomIn | Action::ZoomOut | Action::ZoomReset => {
                if let Some(t) = self.active_terminal().and_then(|id| self.terminal(id)) {
                    let mut t = t.lock().unwrap_or_else(|e| e.into_inner());
                    let by = match action {
                        Action::ZoomIn => 1,
                        Action::ZoomOut => -1,
                        _ => -t.zoom_adj(),
                    };
                    t.zoom(by);
                }
                Task::none()
            }
            Action::Disconnect => on_active(self, TabCmd::Disconnect),
            Action::ToggleLog => {
                if let Err(e) = self.toggle_log() {
                    *self.notice.lock().unwrap_or_else(|e| e.into_inner()) = Some(e);
                }
                Task::none()
            }
            Action::ConnectSftp => on_active(self, TabCmd::Sftp),
            Action::DetachTab => on_active(self, TabCmd::Detach),
            Action::ReattachAll => {
                let windows: Vec<_> = self.detached.iter().map(|(w, _)| *w).collect();
                Task::batch(windows.into_iter().map(|w| self.reattach(w)))
            }
            Action::NextTab | Action::PrevTab | Action::SelectTab(_) if self.front.is_some() => {
                Task::none() // a tab's own window has no tabs to go through
            }
            Action::NextTab | Action::PrevTab if self.mru => {
                if let Some(&prev) =
                    (self.recent.iter().skip(1)).find(|id| self.tabs.contains_key(id))
                {
                    self.show(prev);
                }
                self.focus_active()
            }
            Action::NextTab | Action::PrevTab => {
                if let Some(group) = self.focused_group_mut() {
                    let len = group.model.len();
                    let pos = group.model.position(group.model.active()).unwrap_or(0) as usize;
                    let next = rules::cycle(pos, len, action == Action::NextTab);
                    group.model.activate_position(next as u16);
                }
                self.focus_active()
            }
            Action::SelectTab(n) => {
                if let Some(group) = self.focused_group_mut()
                    && usize::from(n) < group.model.len()
                {
                    group.model.activate_position(u16::from(n));
                }
                self.focus_active()
            }
            Action::SplitRight => on_active(self, TabCmd::SplitRight),
            Action::SplitDown => on_active(self, TabCmd::SplitDown),
            Action::Copy => {
                let text = self.active_id().and_then(|id| {
                    let t = self.terminal(id)?.lock().unwrap_or_else(|e| e.into_inner());
                    t.term.lock().selection_to_string()
                });
                match text {
                    Some(text) => cosmic::iced::clipboard::write(text),
                    None => Task::none(),
                }
            }
            Action::Paste => match self.active_id() {
                Some(id) => cosmic::iced::clipboard::read()
                    .map(move |t| cosmic::Action::App(Message::Tabs(TabsMsg::Paste(id, t)))),
                None => Task::none(),
            },
            Action::SelectAll => {
                if let Some(id) = self.active_id()
                    && let Some(t) = self.terminal(id)
                {
                    t.lock().unwrap_or_else(|e| e.into_inner()).select_all();
                }
                Task::none()
            }
            _ => return None,
        };
        Some(task)
    }

    fn focused_group_mut(&mut self) -> Option<&mut Group> {
        let pane = self.focus?;
        self.grid.as_mut()?.get_mut(pane)
    }

    /// Enter on a tab whose program ended: start it again. `false`: not such a tab.
    /// A «Close tab?» dialog is up.
    pub fn confirming(&self) -> bool {
        self.confirm.is_some() || self.confirm_many.is_some()
    }

    /// The active tab is an ssh one: an SFTP tab can use its connection.
    pub fn can_sftp(&self) -> bool {
        (self.active_id())
            .and_then(|id| self.tabs.get(&id))
            .is_some_and(|t| t.launch.sftp().is_some())
    }

    pub fn enter_on_dead(&self) -> bool {
        self.active_id()
            .and_then(|id| self.tabs.get(&id))
            .is_some_and(|t| !t.live() && t.terminal().is_some())
    }

    pub fn on_term(&mut self, id: TabId, event: alacritty_terminal::event::Event) -> Task<Message> {
        let Some(tab) = self.tabs.get_mut(&id) else {
            return Task::none();
        };
        let Body::Terminal(terminal) = &tab.body else {
            return Task::none();
        };
        match term::on_event(terminal, event) {
            Outcome::None => {}
            Outcome::Output => {
                let unread = !self.in_front(id);
                let tab = self.tabs.get_mut(&id).expect("looked up above");
                tab.last_output = std::time::Instant::now();
                if !tab.seen || (unread && !tab.unread) || tab.quiet {
                    (tab.seen, tab.unread) = (true, tab.unread || unread);
                    tab.quiet = false;
                    self.retitle(id);
                }
            }
            Outcome::Copy(text) => return cosmic::iced::clipboard::write(text),
            Outcome::Title(title) => {
                tab.program_title = title;
                self.retitle(id);
            }
            // ChildExit then Exit: told once.
            Outcome::Exit(code) if tab.alive => {
                tab.alive = false;
                let line = match code {
                    Some(code) => fl!("term-exited-code", code = code.to_string()),
                    None => fl!("term-exited"),
                };
                term::print(terminal, &format!("\r\n[{line}]\r\n"));
                // ssh says 255 when the connection is lost; after it had been up for a while
                // (or while a series of retries goes on) the session connects by itself.
                if let Some(r) = tab.launch.auto_reconnect
                    && let Some(n) = shagshell_core::reconnect::next_attempt(
                        &shagshell_core::reconnect::Ended {
                            code,
                            stopped: tab.stopped,
                            uptime: tab.started.elapsed(),
                            attempts: tab.attempts,
                        },
                        r.tries,
                    )
                {
                    tab.attempts = n;
                    let line = fl!(
                        "term-reconnecting",
                        seconds = i64::from(r.delay),
                        n = i64::from(n),
                        max = i64::from(r.tries)
                    );
                    term::print(terminal, &format!("[{line}]\r\n"));
                    self.retitle(id);
                    let wait = std::time::Duration::from_secs(u64::from(r.delay));
                    return Task::perform(tokio::time::sleep(wait), move |()| {
                        cosmic::Action::App(Message::Tabs(TabsMsg::AutoReconnect(id)))
                    });
                }
                self.retitle(id);
            }
            Outcome::Exit(_) => {}
        }
        Task::none()
    }

    pub fn restyle(&self, look: &Look) {
        let (style, dark, bg) = look;
        for tab in self.tabs.values() {
            if let Some(t) = tab.terminal() {
                term::restyle(t, style, *dark, *bg, self.scheme_of(&tab.launch));
            }
        }
    }

    /// The scheme of a tab: its session's, else the global one.
    fn scheme_of<'a>(&'a self, launch: &'a Launch) -> &'a str {
        match launch.scheme.is_empty() {
            true => &self.scheme,
            false => &launch.scheme,
        }
    }

    /// Global Options: the keyword rules (none when highlighting is off).
    pub fn set_highlights(&mut self, config: &Config, rules: &shagshell_core::highlight::Rules) {
        self.highlights = match config.highlight_enabled {
            true => rules.rules.clone(),
            false => Vec::new(),
        };
        for tab in self.tabs.values() {
            if let Some(t) = tab.terminal() {
                t.lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .set_highlights(self.highlights.clone());
            }
        }
    }

    /// Global Options: the colour scheme and the keyword rules.
    pub fn set_looks(
        &mut self,
        config: &Config,
        rules: &shagshell_core::highlight::Rules,
        look: &Look,
    ) {
        self.scheme = config.color_scheme.clone();
        self.double_click = config.tab_double_click;
        if !config.cursor_blink {
            self.show_cursors(None);
        }
        self.mru = config.ctrl_tab_recent;
        self.cursor = config.cursor();
        for tab in self.tabs.values() {
            if let Some(t) = tab.terminal() {
                (t.lock().unwrap_or_else(|e| e.into_inner())).set_cursor_shape(self.cursor);
            }
        }
        self.set_highlights(config, rules);
        self.restyle(look);
    }

    pub fn update(&mut self, msg: TabsMsg, look: &Look) -> Task<Message> {
        let task = self.update_inner(msg, look);
        self.mark_read();
        task
    }

    fn update_inner(&mut self, msg: TabsMsg, look: &Look) -> Task<Message> {
        match msg {
            TabsMsg::Cmd(cmd) => {
                let target = self.context.take().and_then(|(p, e)| self.tab_at(p, e));
                if let Some(t) = target {
                    return self.cmd(cmd, t, look);
                }
            }
            TabsMsg::Activate(pane, e) => {
                self.focus = Some(pane);
                self.drag_src = None; // a click, not a drag
                self.pressed = Some((pane, e));
                if let Some(group) = self.grid.as_mut().and_then(|g| g.get_mut(pane)) {
                    group.model.activate(e);
                }
                let now = std::time::Instant::now();
                let double = self.last_click.is_some_and(|(at, t)| {
                    at == e && now.duration_since(t) < std::time::Duration::from_millis(400)
                });
                self.last_click = (!double).then_some((e, now));
                let cmd = match (double, self.double_click) {
                    (true, 1) => Some(TabCmd::Close),
                    (true, 2) => Some(TabCmd::Clone),
                    (true, 3) => Some(TabCmd::Reconnect),
                    (true, 4) => Some(TabCmd::Detach),
                    _ => None,
                };
                if let (Some(cmd), Some(t)) = (cmd, self.tab_at(pane, e)) {
                    self.pressed = None; // not the start of a drag
                    return self.cmd(cmd, t, look);
                }
                return self.focus_active();
            }
            TabsMsg::Close(pane, e) => {
                let alive = (self.grid.as_ref())
                    .and_then(|g| g.get(pane)?.model.data::<TabId>(e).copied())
                    .and_then(|id| self.tabs.get(&id))
                    .is_some_and(|t| t.live());
                if alive {
                    self.confirm = Some((pane, e));
                } else {
                    return self.close_now(pane, e);
                }
            }
            TabsMsg::CloseConfirmed => {
                if let Some(list) = self.confirm_many.take() {
                    return self.close_list(list);
                }
                if let Some((pane, e)) = self.confirm.take() {
                    return self.close_now(pane, e);
                }
            }
            TabsMsg::CloseCancelled => {
                self.confirm = None;
                self.confirm_many = None;
                return self.focus_active();
            }
            TabsMsg::Context(pane, e) => self.context = Some((pane, e)),
            TabsMsg::LockPw(v) => {
                if let Some(a) = &mut self.lock_ask {
                    a.pw = v;
                    a.error = None;
                }
            }
            TabsMsg::LockPw2(v) => {
                if let Some(a) = &mut self.lock_ask {
                    a.pw2 = v;
                    a.error = None;
                }
            }
            TabsMsg::LockCancel => {
                self.lock_ask = None;
                return self.focus_active();
            }
            TabsMsg::LockOk => {
                use shagshell_core::tablock::{Lock, LockError};
                let Some(a) = &mut self.lock_ask else {
                    return Task::none();
                };
                let id = a.id;
                if a.unlock {
                    match self.user_locks.get(&id) {
                        Some(l) if !l.opens(&a.pw) => {
                            a.error = Some(fl!("lock-wrong"));
                            a.pw.clear();
                            return Task::none();
                        }
                        _ => {}
                    }
                    self.user_locks.remove(&id);
                    self.lock_ask = None;
                    self.set_user_lock(id, false);
                } else {
                    match Lock::new(&a.pw, &a.pw2) {
                        Ok(l) => {
                            self.user_locks.insert(id, l);
                            self.lock_ask = None;
                            self.set_user_lock(id, true);
                        }
                        Err(LockError::Empty) => {
                            a.error = Some(fl!("lock-empty"));
                            return Task::none();
                        }
                        Err(LockError::Mismatch) => {
                            a.error = Some(fl!("lock-mismatch"));
                            a.pw2.clear();
                            return Task::none();
                        }
                    }
                }
                return self.focus_active();
            }
            TabsMsg::Reorder(pane, ev) => {
                if let Some(group) = self.grid.as_mut().and_then(|g| g.get_mut(pane)) {
                    group.model.reorder(ev.dragged, ev.target, ev.position);
                }
            }
            TabsMsg::DropHint(pane, hint) => {
                if let (None, Some((e, _))) = (self.drag_src, hint) {
                    self.drag_src = Some((pane, e));
                }
            }
            TabsMsg::Drop(pane) => {
                self.move_pressed_to(pane);
                return self.focus_active();
            }
            TabsMsg::PaneClicked(pane) => {
                self.focus = Some(pane);
                return self.focus_active();
            }
            TabsMsg::Resized(ev) => {
                if let Some(grid) = self.grid.as_mut() {
                    grid.resize(ev.split, ev.ratio);
                }
            }
            TabsMsg::Paste(id, Some(text)) => {
                if let Some(t) = self.terminal(id) {
                    t.lock().unwrap_or_else(|e| e.into_inner()).paste(text);
                }
            }
            TabsMsg::Paste(_, None) => {}
            TabsMsg::Reattach(w) => return self.reattach(w),
            TabsMsg::DragWindow(w) => return iced::window::drag(w),
            TabsMsg::Send(ids, pieces) => return self.send(ids, pieces),
            TabsMsg::AutoReconnect(id) => {
                // Still there, still down, not ended by the user meanwhile (Enter, Close).
                if self.tabs.get(&id).is_some_and(|t| !t.alive && !t.stopped) {
                    return self.restart(id, look);
                }
            }
        }
        Task::none()
    }

    /// Asks before closing a live tab, as SecureCRT's "Confirm disconnect".
    pub fn dialog(&self) -> Option<Element<'_, Message>> {
        if let Some(a) = &self.lock_ask {
            let msg = |m| Message::Tabs(m);
            let name = self.tabs.get(&a.id).map(Tab::title).unwrap_or_default();
            let mut col = widget::column::with_capacity(4).spacing(8).push(
                widget::secure_input(fl!("lock-password"), a.pw.clone(), None, true)
                    .id(a.field.clone())
                    .on_input(|v| Message::Tabs(TabsMsg::LockPw(v)))
                    .on_submit(move |_| msg(TabsMsg::LockOk)),
            );
            if !a.unlock {
                col = col.push(
                    widget::secure_input(fl!("lock-again"), a.pw2.clone(), None, true)
                        .on_input(|v| Message::Tabs(TabsMsg::LockPw2(v)))
                        .on_submit(move |_| msg(TabsMsg::LockOk)),
                );
            }
            if let Some(e) = &a.error {
                col = col.push(widget::text::body(e.clone()).class(cosmic::theme::Text::Accent));
            }
            let (title, ok) = match a.unlock {
                true => (fl!("lock-unlock-title", name = name), fl!("lock-unlock")),
                false => (fl!("lock-title", name = name), fl!("lock-lock")),
            };
            return Some(
                widget::dialog()
                    .icon(crate::dialog_icons::Kind::Lock.icon(48))
                    .title(title)
                    .body(match a.unlock {
                        true => fl!("lock-unlock-body"),
                        false => fl!("lock-body"),
                    })
                    .control(col)
                    .primary_action(widget::button::suggested(ok).on_press(msg(TabsMsg::LockOk)))
                    .secondary_action(
                        widget::button::standard(fl!("cancel")).on_press(msg(TabsMsg::LockCancel)),
                    )
                    .into(),
            );
        }
        if let Some(list) = &self.confirm_many {
            let msg = |m| Message::Tabs(m);
            return Some(
                widget::dialog()
                    .icon(crate::dialog_icons::Kind::Warning.icon(48))
                    .title(fl!("close-many-title", n = list.len()))
                    .body(fl!("close-tab-body"))
                    .primary_action(
                        widget::button::destructive(fl!("close"))
                            .on_press(msg(TabsMsg::CloseConfirmed)),
                    )
                    .secondary_action(
                        widget::button::standard(fl!("cancel"))
                            .on_press(msg(TabsMsg::CloseCancelled)),
                    )
                    .into(),
            );
        }
        let Some((pane, e)) = self.confirm else {
            // The active SFTP tab's own (copy, rename, overwrite…).
            let id = self.active_sftp()?;
            let Body::Sftp(v) = &self.tabs.get(&id)?.body else {
                return None;
            };
            return v.dialog();
        };
        let id = *self.grid.as_ref()?.get(pane)?.model.data::<TabId>(e)?;
        let name = self.tabs.get(&id)?.title();
        let msg = |m| Message::Tabs(m);
        Some(
            widget::dialog()
                .icon(crate::dialog_icons::Kind::Warning.icon(48))
                .title(fl!("close-tab-title", name = name))
                .body(fl!("close-tab-body"))
                .primary_action(
                    widget::button::destructive(fl!("close"))
                        .on_press(msg(TabsMsg::CloseConfirmed)),
                )
                .secondary_action(
                    widget::button::standard(fl!("cancel")).on_press(msg(TabsMsg::CloseCancelled)),
                )
                .into(),
        )
    }

    /// `keys`: false while a dialog is up — its Enter / Esc must not go to the program.
    pub fn view<'a>(
        &'a self,
        config: &Config,
        window: iced::window::Id,
        keys: bool,
        scripts: &'a crate::script::Scripts,
    ) -> Option<Element<'a, Message>> {
        let grid = self.grid.as_ref()?;
        let focus = self.focus;
        let many = grid.len() > 1;
        let copy = config.copy_on_select;
        let middle = config.paste_on_middle_click;
        let pg = pane_grid::PaneGrid::new(grid, move |pane, group, _| {
            // The tab bar speaks `cosmic::Action`: at this libcosmic rev it boxes its menu popup's
            // view as `Element<its Message>`, and the runtime only shows one of
            // `Element<Action<Message>>` («Invalid view for subsurface view», no menu).
            let app = |m: Message| cosmic::Action::App(m);
            let m = move |f: fn(pane_grid::Pane, Entity) -> TabsMsg| {
                move |e| app(Message::Tabs(f(pane, e)))
            };
            // The sides that have a group beside this one: the tab can move there.
            let neighbours = Side::ALL.map(|s| grid.adjacent(pane, s.direction()).is_some());
            let mut bar = tab_bar::horizontal(&group.model)
                .button_spacing(6)
                // As wide as their text (SecureCRT's «fit to label»), a long name cut at the end.
                .width(Length::Shrink)
                .maximum_button_width(240)
                .on_activate(m(TabsMsg::Activate))
                .on_close(m(TabsMsg::Close))
                .on_middle_press(m(TabsMsg::Close))
                .on_context(m(TabsMsg::Context))
                .context_menu(Some(tab_menu(
                    &group.model,
                    &self.tabs,
                    &self.user_locks,
                    neighbours,
                )))
                .window_id(window)
                .on_surface_action(move |a| app(Message::Surface(a)))
                .enable_tab_drag(TAB_MIME.to_string())
                .on_reorder(move |ev| app(Message::Tabs(TabsMsg::Reorder(pane, ev))))
                .on_drop_hint(move |h| app(Message::Tabs(TabsMsg::DropHint(pane, h))))
                .on_dnd_drop::<TabDrop>(move |_, _, _| app(Message::Tabs(TabsMsg::Drop(pane))));
            if let Some(h) = config.tab_px() {
                bar = bar.button_height(h);
            }
            let bar = Element::from(bar).map(|a| match a {
                cosmic::Action::App(m) => m,
                other => Message::Cosmic(Box::new(other)),
            });
            let body = self.body(group.active_id(), copy, middle, keys, scripts);
            // A tab dropped on the body comes here too.
            let body: Element<'_, Message> =
                cosmic::widget::dnd_destination::DndDestination::for_data::<TabDrop>(
                    body,
                    move |_, _| Message::Tabs(TabsMsg::Drop(pane)),
                )
                .into();
            let active = many && focus == Some(pane);
            // One tab, one group: no tab bar if so asked (SecureCRT's «only when more than one»).
            let bar_shown = !(config.tabs_hide_single && !many && group.model.len() == 1);
            let parts = match (bar_shown, config.tabs_bottom) {
                (false, _) => vec![body],
                (true, true) => vec![body, bar],
                (true, false) => vec![bar, body],
            };
            let content = widget::container(widget::column::with_children(parts))
                .padding(1)
                .class(cosmic::theme::Container::custom(move |theme| {
                    let mut s = widget::container::Style::default();
                    if active {
                        s.border.color = theme.cosmic().accent_color().into();
                        s.border.width = 1.0;
                    }
                    s
                }));
            pane_grid::Content::new(content)
        })
        .on_click(|p| Message::Tabs(TabsMsg::PaneClicked(p)))
        .on_resize(10, |ev| Message::Tabs(TabsMsg::Resized(ev)))
        .spacing(4);
        Some(pg.into())
    }
}

fn entity_of(model: &SingleSelectModel, id: TabId) -> Option<Entity> {
    model.iter().find(|&e| model.data::<TabId>(e) == Some(&id))
}

fn focus(id: widget::Id) -> Task<Message> {
    cosmic::iced::runtime::widget::operation::focus(id)
}

/// Remove `e` from its group; the active tab moves only if it was `e` (to its right
/// neighbour, else the left one). `true`: the group is now empty.
fn detach(model: &mut SingleSelectModel, e: Entity) -> bool {
    let was_active = model.is_active(e);
    let pos = model.position(e).unwrap_or(0) as usize;
    let len = model.len();
    model.remove(e);
    match rules::after_close(pos, len) {
        Some(next) if was_active => {
            model.activate_position(next as u16);
            false
        }
        Some(_) => false,
        None => true,
    }
}

/// Right click on a tab: about that tab (`TabsMsg::Context` came first). libcosmic wants one
/// root whose children are the tabs' menus, one per tab in order.
fn tab_menu(
    model: &SingleSelectModel,
    tabs: &HashMap<TabId, Tab>,
    user_locks: &HashMap<TabId, shagshell_core::tablock::Lock>,
    neighbours: [bool; 4],
) -> Vec<menu::Tree<cosmic::Action<Message>>> {
    use menu::Item::{Button, ButtonDisabled, CheckBox, Divider};
    let can_split = model.len() > 1;
    let on = |yes: bool, label: String, cmd| match yes {
        true => Button(label, None, cmd),
        false => ButtonDisabled(label, None, cmd),
    };
    let one = |e: Entity| {
        let id = model.data::<TabId>(e).copied();
        let tab = id.and_then(|id| tabs.get(&id));
        let live = tab.is_some_and(Tab::live);
        let sftp = tab.is_some_and(|t| t.launch.sftp().is_some());
        let locked = id.is_some_and(|id| user_locks.contains_key(&id));
        let lock = match locked {
            true => Button(fl!("menu-unlock"), None, TabCmd::Lock),
            false => Button(fl!("menu-lock"), None, TabCmd::Lock),
        };
        let mut items = match tab.map(|t| &t.body) {
            // A note: its editor's commands (SecureCRT's scratchpad menu).
            Some(Body::Note(v)) => vec![
                Button(fl!("note-save"), None, TabCmd::NoteSave),
                Button(fl!("note-save-as"), None, TabCmd::NoteSaveAs),
                Divider,
                CheckBox(fl!("note-wrap"), None, v.wrap, TabCmd::NoteWrap),
                CheckBox(fl!("note-numbers"), None, v.numbers, TabCmd::NoteNumbers),
                Divider,
                on(!v.locked, fl!("note-rename"), TabCmd::NoteRename),
                lock,
            ],
            _ => vec![
                Button(fl!("menu-reconnect"), None, TabCmd::Reconnect),
                on(live, fl!("menu-disconnect"), TabCmd::Disconnect),
                Button(fl!("menu-clone"), None, TabCmd::Clone),
                on(sftp, fl!("menu-connect-sftp"), TabCmd::Sftp),
                lock,
            ],
        };
        items.extend([
            Divider,
            on(can_split, fl!("menu-split-right"), TabCmd::SplitRight),
            on(can_split, fl!("menu-split-down"), TabCmd::SplitDown),
        ]);
        // Into a group already there (dragging the tab onto it does the same).
        for (side, has) in Side::ALL.into_iter().zip(neighbours) {
            if has {
                items.push(Button(side.label(), None, TabCmd::MoveTo(side)));
            }
        }
        items.push(Button(fl!("menu-tab-window"), None, TabCmd::Detach));
        items.extend([
            Divider,
            Button(fl!("menu-close-tab"), None, TabCmd::Close),
            on(
                model.len() > 1 || neighbours.contains(&true),
                fl!("menu-close-others"),
                TabCmd::CloseOthers,
            ),
            Button(fl!("menu-close-all"), None, TabCmd::CloseAll),
        ]);
        let blank = Element::from(widget::Space::new());
        menu::Tree::with_children(blank, menu::items(&HashMap::new(), items))
    };
    let blank = Element::from(widget::Space::new());
    vec![menu::Tree::with_children(
        blank,
        model.iter().map(one).collect::<Vec<_>>(),
    )]
}

/// Keys the terminal leaves to the app: exactly those `app::key_action` acts on (by physical
/// key, any layout). Everything else, every Ctrl+letter and Alt+letter, is the terminal's.
/// What a tab drag carries: nothing but «a tab» (libcosmic sends one byte, not which).
pub struct TabDrop;

impl cosmic::iced::clipboard::mime::AllowedMimeTypes for TabDrop {
    fn allowed() -> std::borrow::Cow<'static, [String]> {
        std::borrow::Cow::Owned(vec![TAB_MIME.to_string()])
    }
}

impl TryFrom<(Vec<u8>, String)> for TabDrop {
    type Error = ();
    fn try_from(_: (Vec<u8>, String)) -> Result<Self, ()> {
        Ok(TabDrop)
    }
}

pub fn is_app_key(mods: Modifiers, _: &Key, key: &Physical) -> bool {
    crate::app::key_action(key, mods).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic::widget::menu::Action as _;

    /// The tab menu is a popup: libcosmic shows it only if its items speak `cosmic::Action`.
    #[test]
    fn regression_tab_menu_items_are_cosmic_actions() {
        assert!(matches!(
            TabCmd::Clone.message(),
            cosmic::Action::App(Message::Tabs(TabsMsg::Cmd(TabCmd::Clone)))
        ));
    }
}
