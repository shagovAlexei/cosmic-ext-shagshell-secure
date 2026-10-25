//! The application: state and routing only; each screen area lives in its own module.

use crate::askpass::Hidden;
use crate::config::{self, Config};
use crate::credentials_window::{CredWindow, CwMsg, Edit};
use crate::menu::Action;
use crate::quick_connect::{QcMsg, QuickConnect};
use crate::session_manager::{SessionManager, SmMsg, TreeKey};
use crate::session_options::{SessionOptions, SoMsg, Target};
use crate::tabs::{self, Tabs, TabsMsg};
use crate::{about, fl, menu, options, status_bar, toolbar};
use cosmic::app::{Core, Task};
use cosmic::cosmic_config::CosmicConfigEntry;
use cosmic::iced::keyboard::{self, Modifiers, key::Code, key::Physical};
use cosmic::iced::{Length, Subscription, event};
use cosmic::widget;
use cosmic::{Application, Element};
use shagshell_core::credentials::{self, Credentials, PutError};
use shagshell_core::keyring::{Owner, Secret};
use shagshell_core::session::store::DEFAULT;

pub const APP_ID: &str = "io.github.shagovalexei.ShagShellSecure";

/// What the context drawer shows.
/// What follows a pre-script that ended well.
enum AfterPre {
    /// A new tab for the session, and the session's own script on it.
    Open(Box<tabs::Launch>, String),
    /// Reconnect / Clone of this tab.
    Cmd(tabs::TabCmd, crate::term::TabId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drawer {
    About,
    Options,
}

pub struct App {
    core: Core,
    config: Config,
    config_handler: Option<cosmic::cosmic_config::Config>,
    drawer: Option<Drawer>,
    tabs: Tabs,
    sessions: SessionManager,
    quick: Option<QuickConnect>,
    /// Tools → Create / Upload Public Key…, while its dialog is open.
    keys: Option<Keys>,
    /// View → Command Manager.
    commands: crate::command_manager::CommandManager,
    /// Tools → Keymap Editor…, while its dialog is open.
    keymap: Option<crate::keymap_editor::Editor>,
    /// The user's keys (`keymap.toml`); a file that could not be read is never written over.
    user_keys: shagshell_core::keymap::Keymap,
    user_keys_broken: bool,
    /// The Session Options window, when open (one at a time).
    options: Option<SessionOptions>,
    /// The Credentials Manager window, when open.
    creds_win: Option<crate::credentials_window::CredWindow>,
    /// Passwords in a keyring of our own (the master password); `None`: not known yet, or being
    /// changed (the checkbox waits).
    separate: Option<bool>,
    /// ssh's ControlMaster sockets (`launch::ensure_control_dir`).
    control_dir: std::path::PathBuf,
    /// ssh's prompts waiting for an answer (`askpass`).
    asks: crate::askpass::Asks,
    /// View → Command Window.
    command: crate::command_window::CommandWindow,
    buttons: crate::button_bar::ButtonBar,
    /// File → Import from SecureCRT…, while its dialog is open.
    sc_import: Option<crate::securecrt_import::ScImport>,
    /// Keyword highlighting rules (`highlights.toml`); a file that could not be read is never
    /// written over.
    hl: shagshell_core::highlight::Rules,
    hl_broken: bool,
    /// Script → Run…: the scripts running and their dialogs.
    scripts: crate::script::Scripts,
    /// A newer release (its tag) the window is about.
    update: Option<String>,
    /// `--script`: runs once the terminals can start (their event channel is up).
    startup: Option<(std::path::PathBuf, Vec<String>)>,
    /// The `*.py` files of the Script menu's folder (read at start, and when asked).
    script_files: Vec<std::path::PathBuf>,
    /// Sessions waiting for their pre-script: run id → what to do when it ended well.
    pre_runs: std::collections::HashMap<u64, AfterPre>,
    /// The scheduled scripts (`schedule.toml`); a broken file is never written over.
    schedule: shagshell_core::schedule::Schedule,
    schedule_broken: bool,
    /// Per job name: when it last ran (local minutes), and the run that is still going.
    sched_state: std::collections::HashMap<String, (i64, Option<u64>)>,
    /// Script → Scheduler…, while open.
    sched_win: Option<crate::scheduler::SchedWindow>,
}

/// The open dialog of Tools → Create / Upload Public Key….
enum Keys {
    Gen(crate::keygen::KeyGen),
    Upload(crate::keygen::Upload),
}

impl Keys {
    fn view(&self) -> Element<'_, Message> {
        match self {
            Keys::Gen(g) => g.view(),
            Keys::Upload(u) => u.view(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Message {
    Action(Action),
    Config(Config),
    /// Global Options: a changed value, written to the config.
    SetConfig(Config),
    OpenUrl(String),
    /// A recorded script and where the user put it (none: given up).
    RecordSaved(String, Option<std::path::PathBuf>),
    /// Script → Choose scripts folder…
    ScriptsFolder(Option<std::path::PathBuf>),
    CloseDrawer,
    Tabs(TabsMsg),
    /// Reconnect / Clone of a saved session's tab: its pre-script runs first.
    PreGate(tabs::TabCmd, crate::term::TabId),
    TermTx(crate::term::Events),
    Term(crate::term::TabId, alacritty_terminal::event::Event),
    /// Enter while the active tab's program has ended.
    EnterKey,
    Surface(cosmic::surface::Action),
    /// The theme was switched in Global Options: terminals take its colors.
    Restyle,
    /// Every 2 s while a server file is open in an editor.
    EditTick,
    /// Anything else a widget speaking `cosmic::Action` says (the tab bar): to the runtime.
    Cosmic(Box<cosmic::Action<Message>>),
    Sessions(SmMsg),
    CommandWindow(crate::command_window::CwMsg),
    ButtonBar(crate::button_bar::BbMsg),
    /// Global Options: the keyword highlighting rules changed.
    SetRules(shagshell_core::highlight::Rules),
    SecureCrt(crate::securecrt_import::ScMsg),
    Script(crate::script::ScMsg),
    Update(crate::updates::UpMsg),
    /// A transfer picked in the Transfer menu, ready to run.
    Transfer(crate::term::TabId, shagshell_core::zmodem::Command),
    QuickConnect(QcMsg),
    KeyGen(crate::keygen::KgMsg),
    CommandManager(crate::command_manager::CmMsg),
    KeyMap(crate::keymap_editor::KmMsg),
    SessionOptions(SoMsg),
    /// Keys and the pointer: acted on only when from the main window (not Session Options).
    Input(cosmic::iced::window::Id, Box<Message>),
    CloseRequested(cosmic::iced::window::Id),
    Ask(crate::askpass::AskMsg),
    /// A key no widget took (not one of the app's): the tree's, Enter, an SFTP tab's.
    Key(Physical, Modifiers),
    SftpTx(crate::sftp_worker::Events),
    Sftp(crate::term::TabId, crate::sftp_view::SftpMsg),
    Credentials(crate::credentials_window::CwMsg),
    Scheduler(crate::scheduler::SchMsg),
    Note(crate::term::TabId, crate::notes::NoteMsg),
    NoteTick,
    /// The note chooser answered.
    NotePicked(Option<std::path::PathBuf>),
    /// A «Browse…» beside a script field was pressed / its pick came back.
    ScriptFile(crate::script::Slot),
    ScriptFileDone(crate::script::Slot, Option<std::path::PathBuf>),
    /// The scheduler's clock.
    SchedTick,
    /// Global Options: the master-password keyring wanted on / off.
    SetSeparate(bool),
    /// …and how it is now (or why it could not be changed).
    Separate(Result<bool, String>),
}

impl Application for App {
    type Executor = cosmic::executor::Default;
    /// `--script file args…`: opens a local shell and runs the script on it.
    type Flags = Option<(std::path::PathBuf, Vec<String>)>;
    type Message = Message;
    const APP_ID: &'static str = APP_ID;

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(mut core: Core, startup: Self::Flags) -> (Self, Task<Message>) {
        let config_handler = config::handler();
        let config: Config = config::read(config_handler.as_ref());
        // The language is chosen in main() from the system; a configured one replaces it.
        if !config.language.is_empty() {
            crate::i18n::init(&config::languages(&config.language, Vec::new()));
        }
        core.window.header_title = fl!("app-title");
        let theme = (config.app_theme != config::AppTheme::System)
            .then(|| cosmic::command::set_theme(config.app_theme.theme()));
        let control_dir = shagshell_core::launch::ensure_control_dir()
            .inspect_err(|e| log::warn!("ssh control dir: {e}"))
            .unwrap_or_else(|_| shagshell_core::launch::control_dir());
        let store =
            shagshell_core::session::Store::new(shagshell_core::session::store::default_root());
        let mut sessions =
            SessionManager::new(store, &config.expanded, config.session_manager_width);
        sessions.right = config.session_manager_side == crate::config::Side::Right;
        sessions.cmd_width = f32::from(config.command_manager_width).clamp(
            crate::session_manager::MIN_WIDTH,
            crate::session_manager::MAX_WIDTH,
        );
        sessions.cmd_right = config.command_manager_side == crate::config::Side::Right;
        sessions.split = (f32::from(config.panes_split) / 1000.0).clamp(0.15, 0.85);
        let asks = crate::askpass::Asks::new(askpass_sock(&control_dir));
        let hl_path = shagshell_core::highlight::default_path();
        let (hl, hl_error) = match shagshell_core::highlight::Rules::load(&hl_path) {
            Ok(r) => (r, None),
            Err(e) => (
                Default::default(),
                Some(format!("{}: {e}", hl_path.display())),
            ),
        };
        let hl_broken = hl_error.is_some();
        let mut tabs = Tabs::default();
        tabs.set_sftp_prefs(&config);
        tabs.set_log_prefs(&config);
        let scripts = crate::script::Scripts::new(tabs.taps.clone(), control_dir.clone());
        scripts
            .debug
            .store(config.script_debug, std::sync::atomic::Ordering::SeqCst);
        let (user_keys, keys_error) = match shagshell_core::keymap::Keymap::load(
            &shagshell_core::keymap::Keymap::default_path(),
        ) {
            Ok(k) => (k, None),
            Err(e) => (Default::default(), Some(format!("keymap.toml: {e}"))),
        };
        let user_keys_broken = keys_error.is_some();
        crate::keymap_editor::set_bindings(&user_keys);
        let mut app = Self {
            core,
            config,
            config_handler,
            drawer: None,
            tabs,
            sessions,
            quick: None,
            keys: None,
            commands: crate::command_manager::CommandManager::new(),
            keymap: None,
            user_keys,
            user_keys_broken,
            options: None,
            creds_win: None,
            separate: None,
            control_dir,
            asks,
            command: crate::command_window::CommandWindow::new(),
            buttons: crate::button_bar::ButtonBar::new(),
            sc_import: None,
            hl,
            hl_broken,
            scripts,
            update: None,
            startup: None,
            script_files: Vec::new(),
            pre_runs: Default::default(),
            schedule: Default::default(),
            schedule_broken: false,
            sched_state: Default::default(),
            sched_win: None,
        };
        let mut sched_error = None;
        match shagshell_core::schedule::Schedule::load(
            &shagshell_core::schedule::Schedule::default_path(),
        ) {
            Ok(mut s) => {
                s.unique_names();
                app.schedule = s;
            }
            Err(e) => {
                app.schedule_broken = true;
                sched_error = Some(format!("schedule.toml: {e}"));
            }
        }
        // Counting starts now: what passed while the app was closed is not made up for.
        let now = shagshell_core::schedule::local_minutes();
        for j in &app.schedule.jobs {
            app.sched_state.insert(j.name.clone(), (now, None));
        }
        // A broken buttonbars.toml is said, and left alone.
        app.sessions.notice = (app.buttons.broken().map(String::from))
            .or(hl_error)
            .or(keys_error)
            .or(sched_error);
        app.tabs.set_looks(&app.config, &app.hl, &look(&app.config));
        app.startup = startup.or_else(|| {
            let p = shagshell_core::script::expand_home(std::path::Path::new(
                &app.config.startup_script,
            ));
            (!app.config.startup_script.is_empty() && p.is_file()).then_some((p, Vec::new()))
        });
        app.refresh_scripts();
        let mut note_tasks = Vec::new();
        if app.config.tabs_on_start {
            let ids = app.config.tabs_open.clone();
            note_tasks.push(app.open_sessions(&ids, true));
        }
        if app.config.notes_on_start {
            for f in app.config.notes_open.clone() {
                note_tasks.push(app.open_note(std::path::Path::new(&f)));
            }
        }
        let check = match app.config.update_check {
            true => check_updates(false),
            false => Task::none(),
        };
        note_tasks.extend([theme.unwrap_or_else(Task::none), check]);
        (app, Task::batch(note_tasks))
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        self.sessions.right = self.config.session_manager_side == crate::config::Side::Right;
        self.sessions.cmd_right = self.config.command_manager_side == crate::config::Side::Right;
        let tabs_changed = matches!(
            message,
            Message::Tabs(_) | Message::Note(_, crate::notes::NoteMsg::RenameDo)
        );
        let mut task = self.handle(message);
        if tabs_changed {
            self.remember_notes();
        }
        self.remember_sessions();
        // A tab went into a window of its own: titled.
        for (window, title) in self.tabs.take_new_windows() {
            task = Task::batch([
                task,
                cosmic::ApplicationExt::set_window_title(self, title, window),
            ]);
        }
        if let Some(window) = self.tabs.take_raise() {
            task = Task::batch([task, cosmic::iced::window::gain_focus(window)]);
        }
        // The tab being recorded was closed: what was recorded is still offered.
        for (title, rec) in self.tabs.take_closed_recordings() {
            task = Task::batch([task, self.save_recording(&title, rec)]);
        }
        // A log that could not start, or stopped by itself (disk full): said in the status bar.
        if let Some(n) = self.tabs.take_notice() {
            self.sessions.notice.get_or_insert(n);
        }
        task
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            // Not a held key's repeat. Keys no widget took: all of the app's (the terminal lets
            // them through). Taken by a text field: only the Alt / Ctrl+Tab ones (Alt+Q would be a
            // «q» there), never Ctrl+Shift+C / V / W — the field's paste must not paste into the
            // terminal too. Enter and the tree's keys only when no widget took them.
            event::listen_with(|e, status, w| {
                input(
                    w,
                    match e {
                        cosmic::iced::Event::Keyboard(keyboard::Event::KeyPressed {
                            physical_key,
                            modifiers,
                            repeat: false,
                            ..
                        }) if crate::keymap_editor::capturing() => {
                            // Alt on its own is the start of Alt+Q, not the chord.
                            (!crate::keymap_editor::is_modifier(&physical_key)).then(|| {
                                Message::KeyMap(crate::keymap_editor::KmMsg::Captured(
                                    physical_key,
                                    modifiers,
                                ))
                            })
                        }
                        cosmic::iced::Event::Keyboard(keyboard::Event::KeyPressed {
                            physical_key,
                            modifiers,
                            repeat: false,
                            ..
                        }) => match key_action(&physical_key, modifiers) {
                            Some(a) if status == event::Status::Ignored || over_fields(a) => {
                                Some(Message::Action(a))
                            }
                            Some(_) => None,
                            None if status == event::Status::Ignored => {
                                Some(Message::Key(physical_key, modifiers))
                            }
                            None => None,
                        },
                        _ => None,
                    },
                )
            }),
            // Ctrl / Shift for clicks in the session tree; from any window, or a Ctrl let go in
            // Session Options would stay held for the tree.
            event::listen_with(|e, _, _| match e {
                cosmic::iced::Event::Keyboard(keyboard::Event::ModifiersChanged(m)) => {
                    Some(Message::Sessions(SmMsg::Mods(m)))
                }
                _ => None,
            }),
            // A row (drag) or the splitter is held: where the pointer goes, where it comes up.
            match self.sessions.holding() {
                true => event::listen_with(|e, _, w| {
                    use cosmic::iced::mouse::{Button, Event as M};
                    input(
                        w,
                        match e {
                            cosmic::iced::Event::Mouse(M::ButtonReleased(Button::Left)) => {
                                Some(Message::Sessions(SmMsg::Release))
                            }
                            cosmic::iced::Event::Mouse(M::CursorMoved { position }) => {
                                Some(Message::Sessions(SmMsg::Cursor(position.x, position.y)))
                            }
                            _ => None,
                        },
                    )
                }),
                false => Subscription::none(),
            },
            // Session Options' × / Alt+F4: Cancel.
            cosmic::iced::window::close_requests().map(Message::CloseRequested),
            crate::term::subscription(),
            crate::sftp_view::subscription(),
            match self.tabs.has_edits() {
                true => cosmic::iced::time::every(std::time::Duration::from_secs(2))
                    .map(|_| Message::EditTick),
                false => Subscription::none(),
            },
            crate::askpass::subscription(askpass_sock(&self.control_dir)),
            self.scripts.subscription(),
            match self.tabs.has_unsaved_notes() {
                true => cosmic::iced::time::every(std::time::Duration::from_millis(500))
                    .map(|_| Message::NoteTick),
                false => Subscription::none(),
            },
            match self.schedule.jobs.iter().any(|j| j.enabled) {
                true => cosmic::iced::time::every(std::time::Duration::from_secs(15))
                    .map(|_| Message::SchedTick),
                false => Subscription::none(),
            },
            self.core()
                .watch_config::<Config>(APP_ID)
                .map(|u| Message::Config(u.config)),
        ])
    }

    fn system_theme_update(
        &mut self,
        _: &[&'static str],
        _: &cosmic::cosmic_theme::Theme,
    ) -> Task<Message> {
        self.tabs.restyle(&look(&self.config));
        Task::none()
    }

    fn system_theme_mode_update(
        &mut self,
        _: &[&'static str],
        _: &cosmic::cosmic_theme::ThemeMode,
    ) -> Task<Message> {
        self.tabs.restyle(&look(&self.config));
        Task::none()
    }

    fn context_drawer(&self) -> Option<cosmic::app::context_drawer::ContextDrawer<'_, Message>> {
        match self.drawer? {
            Drawer::About => Some(about::view()),
            Drawer::Options => Some(options::view(&self.config, self.separate, &self.hl)),
        }
    }

    /// Session Options; popups (menus) are windows too, libcosmic draws them itself, only
    /// asking here.
    fn view_window(&self, id: cosmic::iced::window::Id) -> Element<'_, Message> {
        let focused = self.core.focused_window() == Some(id);
        if let Some(o) = self.options.as_ref().filter(|o| o.window == id) {
            return o.view(focused);
        }
        if self.scripts.win == Some(id)
            && let Some(v) = self.scripts.window_view(focused)
        {
            return v;
        }
        if let Some(v) = self.tabs.window_view(
            id,
            &self.config,
            focused,
            self.dialog().is_none(),
            &self.scripts,
        ) {
            return v;
        }
        if let Some(s) = self.sched_win.as_ref().filter(|s| s.window == id) {
            let last = |i: usize| self.sched_last_text(i);
            return s.view(focused, &last);
        }
        match self.creds_win.as_ref().filter(|c| c.window == id) {
            Some(c) => c.view(focused),
            None => widget::Space::new().into(),
        }
    }

    fn dialog(&self) -> Option<Element<'_, Message>> {
        (self.asks.dialog(|t| self.tabs.title(t)))
            .or_else(|| self.keys.as_ref().map(Keys::view))
            .or_else(|| self.commands.dialog())
            .or_else(|| self.keymap.as_ref().map(crate::keymap_editor::Editor::view))
            .or_else(|| {
                self.sc_import
                    .as_ref()
                    .map(crate::securecrt_import::ScImport::view)
            })
            .or_else(|| self.buttons.dialog())
            .or_else(|| self.tabs.dialog())
            .or_else(|| self.quick.as_ref().map(QuickConnect::view))
            .or_else(|| self.sessions.dialog())
            // Last: a check that ends while another dialog is up does not push it away.
            .or_else(|| self.update.as_deref().map(crate::updates::dialog))
    }

    fn header_start(&self) -> Vec<Element<'_, Message>> {
        vec![menu::bar(
            (
                self.config.session_manager_visible,
                self.config.command_window_visible,
                self.config.button_bar_visible,
                self.config.command_manager_visible,
            ),
            self.tabs.menu_state(),
            (
                &self.script_labels(&self.script_files),
                &self.script_labels(&self.recent_files()),
            ),
            self.scripts.running(),
        )]
    }

    fn view(&self) -> Element<'_, Message> {
        let window = (self.core.main_window_id()).unwrap_or(cosmic::iced::window::Id::RESERVED);
        let center =
            match self
                .tabs
                .view(&self.config, window, self.dialog().is_none(), &self.scripts)
            {
                Some(tabs) => tabs,
                None => widget::container(
                    widget::column::with_children(vec![
                        widget::text::title3(fl!("no-sessions")).into(),
                        widget::text(fl!("no-sessions-hint")).into(),
                    ])
                    .spacing(8)
                    .align_x(cosmic::iced::Alignment::Center),
                )
                .center(Length::Fill)
                .into(),
            };
        let (left, right) = crate::config::panels(
            (
                self.config.session_manager_side,
                self.config.session_manager_visible,
            ),
            (
                self.config.command_manager_side,
                self.config.command_manager_visible,
            ),
            self.config.commands_above,
        );
        let mut middle = widget::row::with_capacity(3).spacing(2);
        if let Some(c) = self.side_column(&left, window, false) {
            middle = middle.push(c);
        }
        middle = middle.push(center);
        if let Some(c) = self.side_column(&right, window, true) {
            middle = middle.push(c);
        }
        let mut col = widget::column::with_capacity(4)
            .push(toolbar::view())
            .push(middle.height(Length::Fill));
        if self.config.command_window_visible {
            col = col.push(self.command.view());
        }
        col = col.push(status_bar::view(
            self.tabs.status(),
            self.sessions.notice.as_deref(),
        ));
        if self.config.button_bar_visible {
            col = col.push(
                self.buttons
                    .view(self.shown_bar(), window, self.config.button_px()),
            );
        }
        col.into()
    }
}

impl App {
    /// The panes of one side stacked in a column (and the Session Manager's splitter beside it).
    fn side_column(
        &self,
        panes: &[crate::config::Panel],
        window: cosmic::iced::window::Id,
        right: bool,
    ) -> Option<Element<'_, Message>> {
        use crate::config::Panel;
        if panes.is_empty() {
            return None;
        }
        let stacked = panes.len() > 1;
        let mut col = widget::column::with_capacity(2).spacing(2);
        for (at, p) in panes.iter().enumerate() {
            let (title, collapsed) = match p {
                Panel::Sessions => (
                    fl!("menu-session-manager"),
                    self.config.session_manager_collapsed,
                ),
                Panel::Commands => (
                    fl!("menu-command-manager"),
                    self.config.command_manager_collapsed,
                ),
            };
            let change = |f: fn(&mut crate::config::Config)| {
                let mut c = self.config.clone();
                f(&mut c);
                Message::SetConfig(c)
            };
            let toggle = match p {
                Panel::Sessions => change(|c| c.session_manager_collapsed ^= true),
                Panel::Commands => change(|c| c.command_manager_collapsed ^= true),
            };
            let tip = |text: String, icon: &'static str, msg: Message| {
                widget::tooltip(
                    widget::button::icon(widget::icon::from_name(icon))
                        .padding(2)
                        .on_press(msg),
                    widget::text::caption(text),
                    widget::tooltip::Position::Bottom,
                )
            };
            let mut head = widget::row::with_capacity(3)
                .align_y(cosmic::iced::Alignment::Center)
                .padding([4, 8, 0, 8])
                .push(widget::text::heading(title).width(Length::Fill));
            if stacked {
                let (icon, text) = match at {
                    0 => ("go-down-symbolic", fl!("panel-down")),
                    _ => ("go-up-symbolic", fl!("panel-up")),
                };
                head = head.push(tip(text, icon, change(|c| c.commands_above ^= true)));
            }
            head = head.push(tip(
                if collapsed {
                    fl!("panel-expand")
                } else {
                    fl!("panel-collapse")
                },
                if collapsed {
                    "pan-end-symbolic"
                } else {
                    "pan-down-symbolic"
                },
                toggle,
            ));
            let mut pane = widget::column::with_capacity(2).push(head);
            if !collapsed {
                pane = pane.push(match p {
                    Panel::Sessions => self.sessions.view(window),
                    Panel::Commands => self.commands.view(),
                });
            }
            col = col.push(
                widget::container(pane)
                    .width(Length::Fill)
                    .height(match (collapsed, stacked) {
                        (true, _) => Length::Shrink,
                        (false, false) => Length::Fill,
                        // The two share the column as the strip between them was dragged.
                        (false, true) => {
                            let top = (self.sessions.split * 1000.0) as u16;
                            Length::FillPortion(if at == 0 { top } else { 1000 - top })
                        }
                    })
                    .class(cosmic::theme::Container::Card),
            );
            // Between two open panes: the strip that resizes them.
            if at == 0
                && stacked
                && !collapsed
                && !self.config.command_manager_collapsed
                && !self.config.session_manager_collapsed
            {
                col = col.push(self.sessions.splitter_v());
            }
        }
        let col = cosmic::iced::widget::sensor(col)
            .on_show(|s| Message::Sessions(SmMsg::ColHeight(s.height)))
            .on_resize(|s| Message::Sessions(SmMsg::ColHeight(s.height)));
        let sessions = panes.contains(&Panel::Sessions);
        let width = if sessions {
            self.sessions.width
        } else {
            self.sessions.cmd_width
        };
        let col: Element<'_, Message> = widget::container(col)
            .width(width)
            .height(Length::Fill)
            .into();
        let mut parts = vec![col];
        parts.push(match sessions {
            true => self.sessions.splitter(),
            false => self.sessions.splitter_cmd(),
        });
        if right {
            parts.reverse();
        }
        Some(widget::row::with_children(parts).into())
    }

    fn handle(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Action(a) => return self.act(a),
            Message::Config(c) => self.config = c,
            Message::SetConfig(c) => {
                let (lang, theme) = (
                    c.language != self.config.language,
                    c.app_theme != self.config.app_theme,
                );
                let folder = c.scripts_folder != self.config.scripts_folder;
                self.config = c;
                self.save_config();
                if folder {
                    self.refresh_scripts();
                }
                self.tabs.set_sftp_prefs(&self.config);
                self.tabs.set_log_prefs(&self.config);
                self.tabs
                    .set_looks(&self.config, &self.hl, &look(&self.config));
                let mut tasks = Vec::new();
                if lang {
                    crate::i18n::init(&config::languages(
                        &self.config.language,
                        i18n_embed::DesktopLanguageRequester::requested_languages(),
                    ));
                    self.core.window.header_title = fl!("app-title");
                    self.command.relabel();
                    self.buttons.relabel();
                }
                if theme {
                    tasks.push(
                        cosmic::command::set_theme(self.config.app_theme.theme())
                            .chain(cosmic::task::message(Message::Restyle)),
                    );
                }
                return Task::batch(tasks);
            }
            Message::ScriptsFolder(Some(dir)) => {
                let mut c = self.config.clone();
                c.scripts_folder = dir.display().to_string();
                self.config = c;
                self.save_config();
                self.refresh_scripts();
            }
            Message::ScriptsFolder(None) => {}
            Message::RecordSaved(text, path) => {
                if let Some(path) = path {
                    self.remember_script_dir(&path);
                    self.refresh_scripts();
                    self.sessions.notice = Some(match std::fs::write(&path, text) {
                        Ok(()) => fl!("rec-saved", path = path.display().to_string()),
                        Err(e) => fl!("rec-failed", error = e.to_string()),
                    });
                }
            }
            Message::OpenUrl(url) => {
                match std::process::Command::new("xdg-open").arg(url).spawn() {
                    // Reaped off the UI thread, or each click leaves a zombie until exit.
                    Ok(mut child) => drop(std::thread::spawn(move || child.wait())),
                    Err(e) => log::warn!("xdg-open: {e}"),
                }
            }
            Message::CloseDrawer => self.set_drawer(None),
            Message::PreGate(cmd, id) => return self.pre_gate(cmd, id),
            Message::Tabs(m) => {
                self.sessions.focused = false; // a terminal is used: Enter is its again
                let task = self.tabs.update(m, &look(&self.config));
                return Task::batch([task, self.prune_asks()]);
            }
            Message::TermTx(tx) => {
                self.tabs.set_sender(tx);
                if let Some((path, args)) = self.startup.take() {
                    // The shell is the active tab at once; what it prints waits in the script's buffer.
                    let open = self
                        .tabs
                        .open(tabs::Launch::local_shell(), &look(&self.config));
                    return Task::batch([
                        open,
                        self.script(crate::script::ScMsg::Start(path, args)),
                    ]);
                }
            }
            Message::Term(id, e) => {
                use alacritty_terminal::event::Event;
                // Only an ended program can leave a prompt without its ssh (not every output).
                let ended = matches!(e, Event::ChildExit(_) | Event::Exit);
                let task = self.tabs.on_term(id, e);
                return match ended {
                    true => Task::batch([task, self.prune_asks()]),
                    false => task,
                };
            }
            Message::Ask(crate::askpass::AskMsg::Stored(r)) => {
                if let Err(e) = r {
                    self.sessions.notice = Some(fl!("ask-keyring-failed", error = e));
                }
            }
            Message::Ask(m) => {
                let out = self.asks.update(m);
                // A keyring answer may come back after its ssh ended.
                return Task::batch([self.ask_outcome(out), self.prune_asks()]);
            }
            // A prompt's dialog is up: Enter (no field took it: the host key) and Esc are its.
            Message::EnterKey if self.asks.active() => {
                return self.update(Message::Ask(crate::askpass::AskMsg::Ok));
            }
            Message::Sessions(SmMsg::Key(TreeKey::Clear)) if self.asks.active() => {
                return self.update(Message::Ask(crate::askpass::AskMsg::Cancel));
            }
            Message::Sessions(SmMsg::Key(TreeKey::Clear)) if self.update.is_some() => {
                return self.updates(crate::updates::UpMsg::Later);
            }
            Message::Sessions(SmMsg::Key(TreeKey::Clear)) if self.keys.is_some() => {
                return self.key_dialog(crate::keygen::KgMsg::Close);
            }
            Message::Sessions(SmMsg::Key(TreeKey::Clear)) if self.keymap.is_some() => {
                let editing = self.keymap.as_ref().is_some_and(|k| k.editing());
                return self.keymap_dialog(match editing {
                    true => crate::keymap_editor::KmMsg::Cancel,
                    false => crate::keymap_editor::KmMsg::Close,
                });
            }
            Message::Sessions(SmMsg::Key(TreeKey::Clear))
                if self.commands.has_dialog() && !self.asks.active() && self.keys.is_none() =>
            {
                return self.command_manager(crate::command_manager::CmMsg::Cancel);
            }
            Message::EnterKey if self.sessions.focused => {
                let ids = self.sessions.connect();
                return self.open_sessions(&ids, true);
            }
            Message::EnterKey if self.tabs.enter_on_dead() => return self.act(Action::Reconnect),
            Message::EnterKey => {}
            Message::Surface(a) => {
                return cosmic::task::message(cosmic::Action::Cosmic(
                    cosmic::app::Action::Surface(a),
                ));
            }
            Message::EditTick => self.tabs.edit_tick(),
            Message::CommandWindow(m) => {
                // A click takes the keys from the terminal (it takes them from every widget).
                if matches!(
                    m,
                    crate::command_window::CwMsg::Edit(widget::text_editor::Action::Click(_))
                ) {
                    let focus = widget::text_input::focus(self.command.input_id.clone());
                    self.command.update(m);
                    return focus;
                }
                if let Some((to, pieces)) = self.command.update(m) {
                    let ids = self.tabs.send_targets(to);
                    return self.tabs.send(ids, pieces);
                }
            }
            Message::ButtonBar(m) => {
                let opens = matches!(
                    m,
                    crate::button_bar::BbMsg::AddButton
                        | crate::button_bar::BbMsg::Edit(_)
                        | crate::button_bar::BbMsg::NewBar
                        | crate::button_bar::BbMsg::RenameBar
                );
                if matches!(m, crate::button_bar::BbMsg::Select(_)) {
                    self.buttons.picked_for = Some(self.tabs.active_session());
                }
                let (run, notice) = self.buttons.update(m, self.shown_bar());
                if notice.is_some() {
                    self.sessions.notice = notice;
                }
                // A bar renamed or deleted: the sessions that named it follow.
                if let Some((old, new)) = self.buttons.relink.take() {
                    match self.sessions.store.relink_bar(&old, new.as_deref()) {
                        Ok(_) => self.sessions.reload(),
                        Err(e) => {
                            self.sessions.notice = Some(crate::session_manager::io_text(&e));
                        }
                    }
                }
                if let Some(b) = run {
                    return self.run_button(b);
                }
                if opens && self.buttons.has_dialog() {
                    return widget::text_input::focus(self.buttons.first_field());
                }
            }
            Message::Script(m) => return self.script(m),
            Message::Update(m) => return self.updates(m),
            Message::Transfer(tab, cmd) => {
                // Started: nothing to say (the result is said when it ends).
                if let Err(e) = self.tabs.start_transfer(tab, &cmd) {
                    self.sessions.notice = Some(e);
                }
                return self.tabs.focus_active();
            }
            Message::SecureCrt(m) => {
                use crate::securecrt_import::{ReportView, ScMsg};
                match &m {
                    ScMsg::Close => {
                        self.sc_import = None;
                        return self.tabs.focus_active();
                    }
                    ScMsg::Browse => {
                        let title = fl!("sc-folder");
                        return Task::perform(crate::securecrt_import::browse(title), |p| {
                            cosmic::Action::App(Message::SecureCrt(ScMsg::Browsed(p)))
                        });
                    }
                    ScMsg::Done(Ok(v)) => {
                        // The new bars into the Button Bar (and its file); the tree reloaded.
                        let v: &ReportView = v;
                        let notice = self.buttons.merge(v.bars.clone());
                        if notice.is_some() {
                            self.sessions.notice = notice;
                        }
                        let added = self.commands.merge(v.commands.clone());
                        if added > 0
                            && self.commands.broken.is_none()
                            && let Err(e) = self
                                .commands
                                .tree
                                .save(&shagshell_core::commands::Folder::default_path())
                        {
                            self.sessions.notice = Some(format!("commands.toml: {e}"));
                        }
                        self.sessions.reload();
                    }
                    ScMsg::Done(Err(_)) => self.sessions.reload(),
                    _ => {}
                }
                let Some(d) = &mut self.sc_import else {
                    return Task::none();
                };
                if d.update(m) {
                    let (root, (sessions, bars, commands)) = (d.path(), d.wants());
                    let store_root = shagshell_core::session::store::default_root();
                    let have = self.buttons.names();
                    return off_ui(
                        move || {
                            let store = shagshell_core::session::Store::new(store_root);
                            shagshell_core::securecrt::import(
                                &root, &store, &have, sessions, bars, commands,
                            )
                            .map(ReportView::of)
                            .map_err(|e| e.to_string())
                        },
                        |r| Message::SecureCrt(ScMsg::Done(r)),
                    );
                }
            }
            Message::SetRules(r) => {
                self.hl = r;
                match self.hl_broken {
                    // Not written over a file that could not be read: said, every time.
                    true => self.sessions.notice = Some(fl!("hl-broken")),
                    false => {
                        if let Err(e) = self.hl.save(&shagshell_core::highlight::default_path()) {
                            self.sessions.notice = Some(e.to_string());
                        }
                    }
                }
                // Only the words changed: no need to restyle every terminal.
                self.tabs.set_highlights(&self.config, &self.hl);
            }
            Message::Restyle => self.tabs.restyle(&look(&self.config)),
            Message::Cosmic(a) => return cosmic::task::message(*a),
            Message::Sessions(m) => {
                let out = self.sessions.update(m);
                if out.save {
                    self.config.expanded = self.sessions.expanded.iter().cloned().collect();
                    self.config.session_manager_width = self.sessions.width as u16;
                    self.config.command_manager_width = self.sessions.cmd_width as u16;
                    self.config.panes_split = (self.sessions.split * 1000.0) as u16;
                    self.save_config();
                }
                let mut tasks: Vec<_> = out.task.into_iter().collect();
                tasks.push(self.open_sessions(&out.open, true));
                if let Some(folder) = out.new_session {
                    tasks.push(self.open_options(Target::New { folder }));
                }
                if let Some((old, new)) = &out.moved {
                    self.sessions_changed(old, new);
                    let (old, new) = (old.clone(), new.clone());
                    tasks.push(keyring_task(move || {
                        shagshell_core::keyring::retarget(&old, &new)
                    }));
                }
                for id in &out.sftp {
                    tasks.push(self.open_sftp(id));
                }
                if let Some(id) = out.deleted.clone() {
                    tasks.push(keyring_task(move || {
                        shagshell_core::keyring::forget_under(&id)
                    }));
                }
                if let Some(id) = out.edit {
                    tasks.push(self.open_options(Target::Edit { id }));
                }
                return Task::batch(tasks);
            }
            Message::KeyGen(m) => return self.key_dialog(m),
            Message::CommandManager(m) => return self.command_manager(m),
            Message::KeyMap(m) => return self.keymap_dialog(m),
            Message::QuickConnect(QcMsg::Cancel) => self.quick = None,
            Message::QuickConnect(m) => {
                let Some(req) = self.quick.as_mut().and_then(|q| q.update(m)) else {
                    return Task::none();
                };
                return self.quick_connected(req);
            }
            Message::SessionOptions(m) => return self.session_options(m),
            Message::Input(w, m) if Some(w) == self.core.main_window_id() => {
                return self.update(*m);
            }
            // Esc in Session Options (no field took it): Cancel, as SecureCRT's dialogs.
            Message::Input(w, m) if self.options.as_ref().is_some_and(|o| o.window == w) => {
                if is_escape(&m) {
                    return self.session_options(SoMsg::Cancel);
                }
            }
            // Esc in the Credentials Manager: Close.
            Message::Input(w, m) if self.creds_win.as_ref().is_some_and(|c| c.window == w) => {
                if is_escape(&m) {
                    return self.credentials(CwMsg::Close);
                }
            }
            Message::Input(w, m) if self.sched_win.as_ref().is_some_and(|s| s.window == w) => {
                if is_escape(&m) {
                    return self.scheduler(crate::scheduler::SchMsg::Close);
                }
            }
            // A tab's own window: what happens there is about that tab.
            Message::Input(w, m) if self.tabs.tab_in(w).is_some() => {
                self.tabs.front = self.tabs.tab_in(w);
                let task = self.handle(*m);
                self.tabs.front = None;
                return task;
            }
            // A script's dialog window: Enter (a message box) and Esc.
            Message::Input(w, m) if self.scripts.win == Some(w) => {
                if is_escape(&m) {
                    return self.script(crate::script::ScMsg::Cancel);
                }
                if matches!(*m, Message::EnterKey) {
                    return self.script(crate::script::ScMsg::Ok);
                }
            }
            Message::Input(..) => {}
            Message::CloseRequested(w) if self.tabs.tab_in(w).is_some() => {
                return self.update(Message::Tabs(crate::tabs::TabsMsg::Reattach(w)));
            }
            Message::CloseRequested(w) if self.scripts.win == Some(w) => {
                return self.script(crate::script::ScMsg::Cancel);
            }
            Message::CloseRequested(w) if self.options.as_ref().is_some_and(|o| o.window == w) => {
                return self.session_options(SoMsg::Cancel);
            }
            Message::CloseRequested(w)
                if self.creds_win.as_ref().is_some_and(|c| c.window == w) =>
            {
                return self.credentials(CwMsg::Close);
            }
            Message::CloseRequested(w)
                if self.sched_win.as_ref().is_some_and(|s| s.window == w) =>
            {
                return self.scheduler(crate::scheduler::SchMsg::Close);
            }
            Message::CloseRequested(_) => {}
            Message::Credentials(m) => return self.credentials(m),
            Message::Scheduler(m) => return self.scheduler(m),
            Message::SchedTick => return self.sched_tick(),
            Message::NoteTick => self.tabs.notes_tick(),
            Message::Note(id, m) => {
                self.sessions.focused = false;
                let (open, task) = self.tabs.on_note(id, m);
                if open {
                    return Task::perform(
                        crate::notes::pick(fl!("note-open-title"), self.note_start_dir()),
                        |p| cosmic::Action::App(Message::NotePicked(p)),
                    );
                }
                return task;
            }
            Message::ScriptFile(slot) => {
                return Task::perform(
                    crate::script::pick(fl!("script-pick"), self.script_start_dir()),
                    move |p| cosmic::Action::App(Message::ScriptFileDone(slot, p)),
                );
            }
            Message::ScriptFileDone(slot, Some(p)) => {
                use crate::script::Slot;
                let s = p.to_string_lossy().into_owned();
                self.remember_script_dir(&p);
                return match slot {
                    Slot::Button => {
                        self.update(Message::ButtonBar(crate::button_bar::BbMsg::Arg(s)))
                    }
                    Slot::Sched => self.scheduler(crate::scheduler::SchMsg::Script(s)),
                    Slot::Pre => self
                        .session_options(SoMsg::Field(crate::session_options::Field::PreScript, s)),
                    Slot::Open => {
                        self.session_options(SoMsg::Field(crate::session_options::Field::Script, s))
                    }
                };
            }
            Message::ScriptFileDone(_, None) => {}
            Message::NotePicked(p) => {
                if let Some(p) = p {
                    return self.open_note(&p);
                }
            }
            Message::Key(k, m) => {
                // «Close tab?»: Enter is Close, Esc Cancel.
                if self.tabs.confirming()
                    && !self.asks.active()
                    && m.is_empty()
                    && let Physical::Code(code) = k
                {
                    match code {
                        Code::Enter | Code::NumpadEnter => {
                            return self
                                .tabs
                                .update(TabsMsg::CloseConfirmed, &look(&self.config));
                        }
                        Code::Escape => {
                            return self
                                .tabs
                                .update(TabsMsg::CloseCancelled, &look(&self.config));
                        }
                        _ => {}
                    }
                }
                // An SFTP dialog with no field (delete, overwrite): Enter is its OK, Esc Cancel.
                if let Some(id) = self.tabs.sftp_dialog()
                    && !self.asks.active()
                    && m.is_empty()
                    && let Physical::Code(code) = k
                {
                    use crate::sftp_view::SftpMsg;
                    match code {
                        Code::Enter | Code::NumpadEnter => {
                            return self.tabs.on_sftp(id, SftpMsg::DialogOk);
                        }
                        Code::Escape => return self.tabs.on_sftp(id, SftpMsg::DialogCancel),
                        _ => {}
                    }
                }
                // The active SFTP tab's commander keys, unless the tree or a dialog has them.
                if let Some(id) = self.tabs.active_sftp()
                    && !self.sessions.focused
                    && self.dialog().is_none()
                    && let Some(key) = crate::sftp_view::key(&k, m)
                {
                    return self.tabs.on_sftp(id, crate::sftp_view::SftpMsg::Key(key));
                }
                if let Some(msg) = route_key(&k, m) {
                    return self.update(msg);
                }
            }
            Message::SftpTx(tx) => self.tabs.set_sftp_sender(tx),
            Message::Sftp(id, m) => {
                use crate::sftp_view::SftpMsg;
                let m = match m {
                    // The click's Ctrl / Shift: as the tree has them.
                    SftpMsg::Click(p, i, _) => {
                        self.sessions.focused = false;
                        SftpMsg::Click(p, i, self.sessions.mods())
                    }
                    m => m,
                };
                return self.tabs.on_sftp(id, m);
            }
            Message::SetSeparate(on) => {
                self.separate = None;
                return separate_task(move || {
                    shagshell_core::keyring::set_separate(on)?;
                    shagshell_core::keyring::separate()
                });
            }
            Message::Separate(r) => match r {
                Ok(on) => self.separate = Some(on),
                Err(e) => {
                    self.sessions.notice = Some(fl!("ask-keyring-failed", error = e));
                    return separate_task(shagshell_core::keyring::separate);
                }
            },
        }
        Task::none()
    }

    /// The bar on screen: the active session's own, else the one picked on the bar.
    fn shown_bar(&self) -> Option<usize> {
        let active = self.tabs.active_session();
        // A bar picked on the bar itself wins over the session's own while that tab is up.
        if self.buttons.picked_for.as_ref() == Some(&active) {
            return self.buttons.shown(None);
        }
        let own = active
            .and_then(|id| self.sessions.store.get(&id).ok())
            .and_then(|s| s.button_bar);
        self.buttons.shown(own.as_deref())
    }

    /// A Button Bar button pressed.
    fn run_button(&mut self, b: shagshell_core::buttonbar::Button) -> Task<Message> {
        use shagshell_core::buttonbar::{Kind, Target};
        match b.kind {
            Kind::Send => {
                let to = match b.target {
                    Target::Current => crate::command_window::SendTo::Current,
                    Target::All => crate::command_window::SendTo::All,
                };
                let ids = self.tabs.send_targets(to);
                self.tabs.send(ids, shagshell_core::macros::parse(&b.arg))
            }
            Kind::Script => {
                let Some(args) = shlex::split(&b.args) else {
                    self.sessions.notice = Some(fl!("script-bad-args"));
                    return Task::none();
                };
                self.script(crate::script::ScMsg::Start(b.arg.into(), args))
            }
            Kind::Menu => match Action::named(&b.arg) {
                Some(a) => self.act(a),
                None => {
                    self.sessions.notice = Some(fl!("bb-unknown-function", name = b.arg));
                    Task::none()
                }
            },
            Kind::Launch => {
                self.launch_app(&b.arg);
                Task::none()
            }
        }
    }

    /// «Launch App»: `%H %P %U %S` from the active session, no shell (the words as split).
    fn launch_app(&mut self, cmd: &str) {
        let id = self.tabs.active_session();
        let s = id.as_ref().and_then(|id| self.sessions.store.get(id).ok());
        // Split first, then fill in: a host or user with a space or a leading dash stays one
        // word of its own and is never taken for an option.
        let port = (s.as_ref().and_then(|s| s.port)).map_or_else(String::new, |p| p.to_string());
        let Some(words) = shlex::split(cmd).filter(|w| !w.is_empty()) else {
            self.sessions.notice = Some(fl!("bb-bad-command"));
            return;
        };
        let words: Vec<String> = words
            .iter()
            .map(|w| {
                shagshell_core::buttonbar::substitute(
                    w,
                    s.as_ref().map_or("", |s| s.host.as_str()),
                    &port,
                    s.as_ref().and_then(|s| s.user.as_deref()).unwrap_or(""),
                    id.as_deref()
                        .map_or("", shagshell_core::session::store::name_of),
                )
            })
            .collect();
        match std::process::Command::new(&words[0])
            .args(&words[1..])
            .spawn()
        {
            // Reaped off the UI thread, or each start leaves a zombie until exit.
            Ok(mut child) => drop(std::thread::spawn(move || child.wait())),
            Err(e) => self.sessions.notice = Some(format!("{}: {e}", words[0])),
        }
    }

    fn act(&mut self, action: Action) -> Task<Message> {
        // SFTP: the active ssh tab's connection, else the session selected in the tree.
        if action == Action::ConnectSftp && !self.tabs.can_sftp() {
            return match self.sessions.one_session() {
                Some(id) => self.open_sftp(&id),
                None => {
                    self.sessions.notice = Some(fl!("sftp-needs-ssh"));
                    Task::none()
                }
            };
        }
        if let Some(task) = self.tabs.act(action, &look(&self.config)) {
            return task;
        }
        match action {
            Action::Exit => {
                self.tabs.save_notes();
                let _ = std::fs::remove_file(askpass_sock(&self.control_dir));
                return cosmic::iced::exit();
            }
            Action::ToggleButtonBar => {
                self.config.button_bar_visible = !self.config.button_bar_visible;
                self.save_config();
            }
            Action::ToggleCommandWindow => {
                self.config.command_window_visible = !self.config.command_window_visible;
                self.save_config();
                if self.config.command_window_visible {
                    return widget::text_input::focus(self.command.input_id.clone());
                }
                return self.tabs.focus_active();
            }
            Action::ToggleCommandManager => {
                self.config.command_manager_visible = !self.config.command_manager_visible;
                self.save_config();
                if self.config.command_manager_visible {
                    return widget::text_input::focus(self.commands.filter_id());
                }
                return self.tabs.focus_active();
            }
            Action::ToggleSessionManager => {
                self.config.session_manager_visible = !self.config.session_manager_visible;
                self.save_config();
            }
            // Alt+C, as SecureCRT's Connect dialog: the tree, with the filter focused.
            Action::Connect => {
                if !self.config.session_manager_visible {
                    self.config.session_manager_visible = true;
                    self.save_config();
                }
                return widget::text_input::focus(self.sessions.filter_id.clone());
            }
            Action::QuickConnect if self.quick.is_some() => {} // open: keep what is typed
            Action::QuickConnect => return self.quick_connect(),
            Action::ImportSshConfig => self.import_ssh_config(),
            Action::ImportSecureCrt => {
                let d = crate::securecrt_import::ScImport::new();
                let focus = widget::text_input::focus(d.input_id.clone());
                self.sc_import = Some(d);
                return focus;
            }
            // The active tab's session, else the tree's one selected.
            Action::SessionOptions => {
                match self
                    .tabs
                    .active_session()
                    .or_else(|| self.sessions.one_session())
                {
                    Some(id) => return self.open_options(Target::Edit { id }),
                    None => self.sessions.notice = Some(fl!("so-nothing")),
                }
            }
            Action::EditDefault => return self.open_options(Target::Default),
            Action::Credentials => return self.open_credentials(),
            Action::KeyMap if self.user_keys_broken => {
                self.sessions.notice = Some(fl!("km-broken"));
            }
            Action::KeyMap => {
                if self.keymap.is_none() {
                    self.keymap = Some(crate::keymap_editor::Editor::new(self.user_keys.clone()));
                }
                return widget::text_input::focus(widget::Id::unique());
            }
            Action::Bound(i) => {
                if let Some(b) = crate::keymap_editor::binding(usize::from(i)) {
                    return self.run_button(b);
                }
            }
            Action::CreateKey => {
                let k = crate::keygen::KeyGen::new();
                let focus = k.focus();
                self.keys = Some(Keys::Gen(k));
                return focus;
            }
            Action::UploadKey => {
                let session = (self.tabs.active_session()).or_else(|| self.sessions.one_session());
                match session {
                    None => self.sessions.notice = Some(fl!("so-nothing")),
                    Some(id) => match crate::keygen::Upload::new(id) {
                        Some(u) => self.keys = Some(Keys::Upload(u)),
                        None => self.sessions.notice = Some(fl!("ku-no-keys")),
                    },
                }
            }
            Action::RunScript => {
                return Task::perform(
                    crate::script::pick(fl!("script-pick"), self.script_start_dir()),
                    |p| cosmic::Action::App(Message::Script(crate::script::ScMsg::Picked(p))),
                );
            }
            Action::Listed(i) => {
                let all = [self.script_files.clone(), self.recent_files()].concat();
                if let Some(path) = all.get(usize::from(i)).cloned() {
                    return self.script(crate::script::ScMsg::Picked(Some(path)));
                }
            }
            Action::ScriptsRefresh => self.refresh_scripts(),
            Action::Scheduler => return self.open_scheduler(),
            Action::Notes => {
                let russian = crate::i18n::russian();
                return match shagshell_core::notes::ensure_default(
                    &shagshell_core::notes::dir(),
                    russian,
                ) {
                    Ok(p) => self.open_note(&p),
                    Err(e) => {
                        self.sessions.notice = Some(format!("notes: {e}"));
                        Task::none()
                    }
                };
            }
            Action::OpenNote => {
                return Task::perform(
                    crate::notes::pick(fl!("note-open-title"), self.note_start_dir()),
                    |p| cosmic::Action::App(Message::NotePicked(p)),
                );
            }
            Action::ScriptsFolder => {
                return Task::perform(
                    crate::script::pick_folder(fl!("script-folder-title"), self.scripts_dir()),
                    |p| cosmic::Action::App(Message::ScriptsFolder(p)),
                );
            }
            Action::ScriptConsole => return self.open_console(true),
            Action::CancelScript => {
                self.scripts.cancel();
                return self.script_window();
            }
            Action::CancelTransfer => self.tabs.cancel_transfer(),
            Action::Transfer(proto, send) => {
                let dir = self.config.download_dir();
                // The tab it was asked in, not the one in front when the chooser is closed.
                let Some(tab) = self.tabs.active_terminal() else {
                    self.sessions.notice = Some(fl!("zm-no-tab"));
                    return Task::none();
                };
                return Task::perform(
                    crate::transfer_pick::pick(proto, send, dir),
                    move |c| match c {
                        Some(c) => cosmic::Action::App(Message::Transfer(tab, c)),
                        None => cosmic::Action::None,
                    },
                );
            }
            Action::About => self.toggle_drawer(Drawer::About),
            Action::CheckUpdates => return check_updates(true),
            Action::ToggleRecording => {
                if let Some((title, rec)) = self.tabs.toggle_recording() {
                    return self.save_recording(&title, rec);
                }
            }
            Action::Donate => {
                if let Some(url) = crate::updates::DONATE_URL {
                    return self.update(Message::OpenUrl(url.into()));
                }
            }
            Action::GlobalOptions => {
                self.toggle_drawer(Drawer::Options);
                if self.drawer == Some(Drawer::Options) {
                    return separate_task(shagshell_core::keyring::separate);
                }
            }
            _ => {}
        }
        Task::none()
    }

    /// Script → Run…, and what running scripts ask of the window.
    fn script(&mut self, m: crate::script::ScMsg) -> Task<Message> {
        use crate::script::ScMsg;
        let (start, m) = match m {
            ScMsg::Picked(p) => {
                if let Some(p) = &p {
                    self.remember_script(p);
                }
                (Some(p.map(|p| (None, p, Vec::new()))), None)
            }
            ScMsg::Start(p, a) => (Some(Some((None, p, a))), None),
            ScMsg::StartOn(t, p, a) => (Some(Some((Some(t), p, a))), None),
            m => (None, Some(m)),
        };
        if let Some(start) = start {
            let Some((on, path, args)) = start else {
                return self.tabs.focus_active();
            };
            let path = shagshell_core::script::expand_home(&path);
            // A session's script says nothing when it starts, and leaves the focus to the tab.
            let quiet = on.is_some();
            match on.or_else(|| self.tabs.active_terminal()) {
                None => self.sessions.notice = Some(fl!("script-no-tab")),
                Some(tab) => match self.scripts.run(&path, &args, tab) {
                    Ok(_) if quiet => {}
                    Ok(_) => {
                        let name = path
                            .file_name()
                            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                        self.sessions.notice = Some(fl!("script-running", name = name));
                    }
                    Err(e) => self.sessions.notice = Some(e),
                },
            }
            return match quiet {
                true => Task::none(),
                false => self.tabs.focus_active(),
            };
        }
        let Some(m) = m else {
            return Task::none();
        };
        if let crate::script::ScMsg::ConsoleDebug(on) = &m {
            let mut c = self.config.clone();
            c.script_debug = *on;
            self.config = c;
            self.save_config();
        }
        if matches!(m, crate::script::ScMsg::Drag) {
            return self
                .scripts
                .win
                .map_or_else(Task::none, cosmic::iced::window::drag);
        }
        let out = self.scripts.update(m, &mut self.tabs);
        if out.notice.is_some() {
            self.sessions.notice = out.notice.clone();
        }
        // After the run's own notice: that the session was not opened is what matters.
        let mut pre_task = Task::none();
        if let Some((run, _)) = out.ended {
            self.tabs.unlock_run(run);
            for st in self.sched_state.values_mut().filter(|s| s.1 == Some(run)) {
                st.1 = None;
            }
        }
        if let Some((run, ok)) = out.ended
            && let Some(then) = self.pre_runs.remove(&run)
        {
            let name = match &then {
                AfterPre::Open(launch, _) => launch.title.clone(),
                AfterPre::Cmd(_, id) => self
                    .tabs
                    .saved_session(*id)
                    .map(|t| t.1)
                    .unwrap_or_default(),
            };
            match (ok, then) {
                (true, AfterPre::Open(launch, post)) => pre_task = self.open_tab(*launch, &post),
                (true, AfterPre::Cmd(cmd, id)) => {
                    pre_task = self.tabs.run_gated(cmd, id, &look(&self.config))
                }
                (false, _) => self.sessions.notice = Some(fl!("pre-script-failed", name = name)),
            }
        }
        let mut tasks = vec![pre_task];
        if out.focus {
            tasks.push(widget::text_input::focus(self.scripts.input_id.clone()));
        }
        if out.refocus {
            tasks.push(self.tabs.focus_active());
        }
        for (set, reply) in out.clip {
            match set {
                Some(text) => {
                    tasks.push(cosmic::iced::clipboard::write(text));
                    reply.send(Ok(serde_json::Value::Null));
                }
                None => tasks.push(cosmic::iced::clipboard::read().map(move |t| {
                    cosmic::Action::App(Message::Script(crate::script::ScMsg::Clip(
                        reply.clone(),
                        t.unwrap_or_default(),
                    )))
                })),
            }
        }
        for (call, save) in out.files {
            let reply = call.reply.clone();
            tasks.push(Task::perform(
                crate::script::Scripts::choose(call.args, save),
                move |p| {
                    cosmic::Action::App(Message::Script(crate::script::ScMsg::Chosen(
                        reply.clone(),
                        p,
                    )))
                },
            ));
        }
        tasks.push(self.script_window());
        if out.output.is_some() && self.config.script_console_auto {
            tasks.push(self.open_console(false));
        }
        if let Some(text) = out.copy {
            tasks.push(cosmic::iced::clipboard::write(text));
        }
        if let Some((name, reply)) = out.connect {
            // Their scripts do not run: a script that connects its own session would loop.
            tasks.push(self.open_sessions(&[name], false));
            reply.send(Ok(serde_json::Value::Null)); // the tab is there now
        }
        if out.raise
            && let Some(w) = self.core.main_window_id()
        {
            tasks.push(cosmic::iced::window::gain_focus(w));
        }
        if let Some((n, reply)) = out.create {
            reply.send(self.script_create(&n));
        }
        Task::batch(tasks)
    }

    /// `Session.Create`: a new SSH session from the Default template, no secrets; its path.
    fn script_create(
        &mut self,
        n: &shagshell_core::script::NewSession,
    ) -> Result<serde_json::Value, String> {
        crate::session_manager::check(&n.name, n.folder.is_empty())
            .map_err(|e| format!("Session.Create: {e}"))?;
        let store = &self.sessions.store;
        let mut s = store.template();
        s.host = n.host.clone();
        // Only what the script said: the rest is the Default template's.
        s.user = n.user.clone().or(s.user);
        s.port = n.port.or(s.port);
        let id =
            (store.add(&n.folder, &n.name, &s)).map_err(|e| crate::session_manager::io_text(&e))?;
        self.sessions.select(&id);
        self.config.expanded = self.sessions.expanded.iter().cloned().collect();
        self.save_config();
        Ok(serde_json::json!(id))
    }

    /// The folder the Script menu lists.
    fn scripts_dir(&self) -> std::path::PathBuf {
        match self.config.scripts_folder.as_str() {
            "" => shagshell_core::script::default_dir(),
            f => shagshell_core::script::expand_home(std::path::Path::new(f)),
        }
    }

    /// Where Run… and Save Recording open: the folder of the last script, else the menu's folder.
    fn script_start_dir(&self) -> std::path::PathBuf {
        match self.config.script_dir.as_str() {
            "" => self.scripts_dir(),
            d => std::path::PathBuf::from(d),
        }
    }

    fn refresh_scripts(&mut self) {
        self.script_files = shagshell_core::script::list_dir(&self.scripts_dir());
    }

    fn recent_files(&self) -> Vec<std::path::PathBuf> {
        (self.config.recent_scripts.iter())
            .map(std::path::PathBuf::from)
            .filter(|p| p.is_file())
            .collect()
    }

    fn script_labels(&self, files: &[std::path::PathBuf]) -> Vec<String> {
        files
            .iter()
            .map(|p| {
                p.file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
            })
            .collect()
    }

    fn remember_script_dir(&mut self, path: &std::path::Path) {
        if let Some(d) = path.parent() {
            self.config.script_dir = d.display().to_string();
            self.save_config();
        }
    }

    /// A script picked or run from the menu: its folder is where the next chooser opens, and it
    /// is the newest of the recent ones.
    fn remember_script(&mut self, path: &std::path::Path) {
        self.config.recent_scripts = shagshell_core::script::recent_push(
            &self.config.recent_scripts,
            &path.display().to_string(),
            10,
        );
        self.remember_script_dir(path);
    }

    /// Script → Console: a tab in the right-hand group; `raise`: shown when it is there already
    /// (a script's first output only opens it).
    fn open_console(&mut self, raise: bool) -> Task<Message> {
        self.tabs.open_console(raise, &look(&self.config))
    }

    /// The window of the script dialog that is up: opened with the first ask, gone with the last.
    fn script_window(&mut self) -> Task<Message> {
        match (self.scripts.active(), self.scripts.win) {
            (true, None) => {
                let (window, open) = cosmic::iced::window::open(self.scripts.settings());
                self.scripts.win = Some(window);
                self.scripts.win_size = self.scripts.size();
                let id = self.scripts.input_id.clone();
                Task::batch([
                    open.then(move |_| widget::text_input::focus(id.clone())),
                    cosmic::ApplicationExt::set_window_title(self, fl!("script-title"), window),
                ])
            }
            (false, Some(w)) => {
                self.scripts.win = None;
                cosmic::iced::window::close(w)
            }
            (true, Some(w)) if self.scripts.win_size != self.scripts.size() => {
                self.scripts.win_size = self.scripts.size();
                cosmic::iced::window::resize(w, self.scripts.win_size)
            }
            _ => Task::none(),
        }
    }

    /// The end of a recording: the save dialog (nothing recorded: a notice).
    fn save_recording(
        &mut self,
        title: &str,
        rec: shagshell_core::recorder::Recorder,
    ) -> Task<Message> {
        if rec.is_empty() {
            self.sessions.notice = Some(fl!("rec-empty"));
            return Task::none();
        }
        let when = shagshell_core::logfile::now().strftime("%Y-%m-%d %H:%M");
        let text = rec.script(&format!(
            "Recorded by ShagShell Secure from «{title}», {when}"
        ));
        Task::perform(
            crate::script::save_script(fl!("rec-save-title"), self.script_start_dir()),
            move |p| cosmic::Action::App(Message::RecordSaved(text.clone(), p)),
        )
    }

    /// The update window and the answers of the release check.
    fn updates(&mut self, m: crate::updates::UpMsg) -> Task<Message> {
        use crate::updates::UpMsg;
        use shagshell_core::update::Status;
        match m {
            UpMsg::Checked(Status::Available(tag), manual) => {
                if manual || self.config.skipped_version != tag {
                    self.update = Some(tag);
                }
            }
            UpMsg::Checked(Status::Latest, true) => self.sessions.notice = Some(fl!("up-latest")),
            UpMsg::Checked(Status::Failed(error), true) => {
                self.sessions.notice = Some(fl!("up-failed", error = error));
            }
            UpMsg::Checked(..) => {}
            UpMsg::Update => {
                if let Some(tag) = self.update.take() {
                    self.sessions.notice = Some(fl!("up-downloading", tag = tag.clone()));
                    let dir = crate::updates::cache_dir();
                    return off_ui(
                        move || shagshell_core::update::download(&tag, &dir),
                        |r| Message::Update(UpMsg::Downloaded(r)),
                    );
                }
            }
            // The hash matched: apt installs it (polkit asks for the password).
            UpMsg::Downloaded(Ok(deb)) => {
                self.sessions.notice = Some(fl!("up-verified"));
                return off_ui(
                    move || {
                        let out = std::process::Command::new("pkexec")
                            .args(["apt-get", "install", "-y", "--"])
                            .arg(&deb)
                            .output()
                            .map_err(|e| format!("pkexec: {e}"))?;
                        match out.status.success() {
                            true => Ok(()),
                            false => Err(String::from_utf8_lossy(&out.stderr).trim().to_string()),
                        }
                    },
                    |r| Message::Update(UpMsg::Installed(r)),
                );
            }
            UpMsg::Downloaded(Err(error)) => {
                self.sessions.notice = Some(fl!("up-update-failed", error = error));
            }
            UpMsg::Installed(Ok(())) => self.sessions.notice = Some(fl!("up-installed")),
            UpMsg::Installed(Err(error)) => {
                self.sessions.notice = Some(fl!("up-update-failed", error = error));
            }
            UpMsg::Skip => {
                if let Some(tag) = self.update.take() {
                    self.config.skipped_version = tag;
                    self.save_config();
                }
            }
            UpMsg::Later => self.update = None,
            UpMsg::Donate => {
                if let Some(url) = crate::updates::DONATE_URL {
                    return self.update(Message::OpenUrl(url.into()));
                }
            }
        }
        Task::none()
    }

    /// Tools → Keymap Editor…: the editor's messages; a change is saved and put in force.
    fn keymap_dialog(&mut self, m: crate::keymap_editor::KmMsg) -> Task<Message> {
        use crate::keymap_editor::KmMsg;
        let Some(ed) = self.keymap.as_mut() else {
            return Task::none();
        };
        let builtin = match &m {
            KmMsg::Captured(k, mods) => builtin_action(k, *mods).is_some(),
            _ => false,
        };
        let out = ed.update(m, builtin);
        if out.save {
            if self.user_keys_broken {
                self.sessions.notice = Some(fl!("km-broken"));
            } else {
                self.user_keys = ed.keymap.clone();
                crate::keymap_editor::set_bindings(&self.user_keys);
                if let Err(e) = self
                    .user_keys
                    .save(&shagshell_core::keymap::Keymap::default_path())
                {
                    self.sessions.notice = Some(format!("keymap.toml: {e}"));
                }
            }
        }
        if out.close {
            self.keymap = None;
            return self.tabs.focus_active();
        }
        match out.unfocus {
            true => widget::text_input::focus(widget::Id::unique()),
            false => Task::none(),
        }
    }

    /// The Command Manager's messages: a command is sent, a change is saved.
    fn command_manager(&mut self, m: crate::command_manager::CmMsg) -> Task<Message> {
        let out = self.commands.update(m);
        let mut tasks = Vec::new();
        if let Some(c) = out.send {
            let to = match c.target {
                shagshell_core::buttonbar::Target::Current => {
                    crate::command_window::SendTo::Current
                }
                shagshell_core::buttonbar::Target::All => crate::command_window::SendTo::All,
            };
            let ids = self.tabs.send_targets(to);
            tasks.push(self.tabs.send(ids, c.to_send()));
            // The keys go back to the terminal that got it.
            tasks.push(self.tabs.focus_active());
        }
        if out.save {
            match &self.commands.broken {
                Some(_) => self.sessions.notice = Some(fl!("cm-broken")),
                None => {
                    if let Err(e) = self
                        .commands
                        .tree
                        .save(&shagshell_core::commands::Folder::default_path())
                    {
                        self.sessions.notice = Some(format!("commands.toml: {e}"));
                    }
                }
            }
        }
        if let Some(id) = out.focus {
            tasks.push(widget::text_input::focus(id));
        }
        Task::batch(tasks)
    }

    /// Create / Upload Public Key: what the dialogs ask for.
    fn key_dialog(&mut self, m: crate::keygen::KgMsg) -> Task<Message> {
        use crate::keygen::KgMsg;
        match (&mut self.keys, m) {
            // A key is being made: its result has to land in this dialog.
            (Some(Keys::Gen(g)), KgMsg::Close) if g.busy() => {}
            (_, KgMsg::Close) => {
                self.keys = None;
                return self.tabs.focus_active();
            }
            (Some(Keys::Gen(g)), KgMsg::Copy) => {
                if let Some(r) = &g.result {
                    return cosmic::iced::clipboard::write(r.public.clone());
                }
            }
            (Some(Keys::Gen(g)), m) => {
                if let Some(spec) = g.update(m) {
                    let exe = std::env::current_exe().unwrap_or_else(|_| "shagshell".into());
                    return off_ui(
                        move || shagshell_core::keygen::generate(&spec, &exe),
                        |r| Message::KeyGen(KgMsg::Done(r)),
                    );
                }
            }
            (Some(Keys::Upload(u)), KgMsg::Key(i)) => u.key = i,
            (Some(Keys::Upload(u)), KgMsg::Upload) => {
                let (id, key) = (u.session.clone(), u.chosen().cloned());
                self.keys = None;
                if let Some(key) = key {
                    return self.upload_key(&id, &key);
                }
            }
            _ => {}
        }
        Task::none()
    }

    /// `ssh-copy-id` for a session in a tab of its own: the password, if asked, goes through
    /// the usual askpass dialog.
    fn upload_key(&mut self, id: &str, key: &std::path::Path) -> Task<Message> {
        let s = match self.sessions.store.get(id) {
            Ok(s) if s.protocol == shagshell_core::session::Protocol::Ssh2 => s,
            Ok(_) => {
                self.sessions.notice = Some(fl!("sftp-needs-ssh"));
                return Task::none();
            }
            Err(e) => {
                self.sessions.notice = Some(format!("{id}: {e}"));
                return Task::none();
            }
        };
        let name = shagshell_core::session::store::name_of(id).to_string();
        let creds = load_credentials();
        let mut l = match ssh_launch(
            &self.control_dir,
            creds.as_ref(),
            &s,
            name.clone(),
            Some(id),
        ) {
            Ok(l) => l,
            Err(e) => {
                self.sessions.notice = Some(e);
                return Task::none();
            }
        };
        let s = creds.unwrap_or_default().resolve(&s);
        // ssh-copy-id has no `--`: a host that looks like an option would be one.
        if s.host.starts_with('-') {
            self.sessions.notice = Some(fl!("ku-bad-host"));
            return Task::none();
        }
        // Its own tab, not the session's: a rebuilt launch, Clone or SFTP must not take it for ssh.
        l.protocol = tabs::Protocol::Local;
        l.program = "ssh-copy-id".into();
        l.args = shagshell_core::keygen::copy_id_args(
            key,
            &s.host,
            s.user.as_deref().unwrap_or_default(),
            s.port,
            &s.jump,
            s.identity.as_deref().unwrap_or_default(),
            &self.control_dir,
        );
        l.title = fl!("ku-tab", name = name);
        (l.log, l.logon, l.auto_reconnect) = (None, None, None);
        self.tabs.open(l, &look(&self.config))
    }

    /// A session's tab: after its pre-script (when it has one and scripts are wanted) finished
    /// well, else at once.
    fn open_session_tab(
        &mut self,
        mut launch: tabs::Launch,
        id: &str,
        s: &shagshell_core::session::Session,
        scripts: bool,
    ) -> Task<Message> {
        launch.session = Some(id.to_string());
        let post = if scripts { s.script.as_str() } else { "" };
        let pre = s.pre_script.trim();
        if !scripts || pre.is_empty() {
            return self.open_tab(launch, post);
        }
        self.run_pre(
            s,
            &launch.title.clone(),
            AfterPre::Open(Box::new(launch), post.to_string()),
        )
    }

    /// Runs the session's pre-script (no tab); `then` follows when it ended well.
    fn run_pre(
        &mut self,
        s: &shagshell_core::session::Session,
        title: &str,
        then: AfterPre,
    ) -> Task<Message> {
        let path = shagshell_core::script::expand_home(std::path::Path::new(s.pre_script.trim()));
        let args = vec![
            title.to_string(),
            s.host.clone(),
            s.user.clone().unwrap_or_default(),
        ];
        // No tab: an id nothing has, so `crt.Screen` calls fail instead of hitting tab 0.
        match self.scripts.run(&path, &args, u64::MAX) {
            Ok(run) => {
                self.pre_runs.insert(run, then);
            }
            Err(e) => self.sessions.notice = Some(e),
        }
        Task::none()
    }

    /// Reconnect / Clone of a saved session's tab: after its pre-script when it has one.
    fn pre_gate(&mut self, cmd: tabs::TabCmd, id: crate::term::TabId) -> Task<Message> {
        let look = look(&self.config);
        let session = self.tabs.saved_session(id).and_then(|(sid, title)| {
            let s = self.sessions.store.get(&sid).ok()?;
            (!s.pre_script.trim().is_empty()).then_some((s, title))
        });
        match session {
            Some((s, title)) => self.run_pre(&s, &title, AfterPre::Cmd(cmd, id)),
            None => self.tabs.run_gated(cmd, id, &look),
        }
    }

    /// A tab for `launch`; the session's `script` (if any) runs on it.
    fn open_tab(&mut self, launch: tabs::Launch, script: &str) -> Task<Message> {
        let open = self.tabs.open(launch, &look(&self.config));
        match (self.tabs.last_opened(), script.trim()) {
            (Some(tab), s) if !s.is_empty() => Task::batch([
                open,
                self.script(crate::script::ScMsg::StartOn(tab, s.into(), Vec::new())),
            ]),
            _ => open,
        }
    }

    /// Each session in a tab of its own; one that can't start says why under the tree.
    fn open_sessions(&mut self, ids: &[String], scripts: bool) -> Task<Message> {
        let mut tasks = Vec::new();
        let creds = load_credentials();
        for id in ids {
            match self.sessions.store.get(id) {
                Ok(s) => {
                    let name = shagshell_core::session::store::name_of(id).to_string();
                    use shagshell_core::session::Protocol as P;
                    match s.protocol {
                        P::Local => {
                            let l = tabs::Launch {
                                title: name,
                                log: Some(log_spec(&s)),
                                logon: logon_spec(&s, Some(id)),
                                scheme: s.color_scheme.clone(),
                                label: s.label.clone(),
                                ..tabs::Launch::local_shell()
                            };
                            tasks.push(self.open_session_tab(l, id, &s, scripts));
                            continue;
                        }
                        P::Telnet | P::Serial => {
                            match program_launch(&s, name, Some(id)) {
                                Ok(l) => tasks.push(self.open_session_tab(l, id, &s, scripts)),
                                Err(e) => self.sessions.notice = Some(e),
                            }
                            continue;
                        }
                        P::Ssh2 => {}
                    }
                    match ssh_launch(&self.control_dir, creds.as_ref(), &s, name, Some(id)) {
                        Ok(l) => tasks.push(self.open_session_tab(l, id, &s, scripts)),
                        Err(e) => self.sessions.notice = Some(e),
                    }
                }
                Err(e) => self.sessions.notice = Some(format!("{id}: {e}")),
            }
        }
        Task::batch(tasks)
    }

    /// The tree's «Открыть SFTP»: an SFTP tab for the session (ssh connects, asking as a
    /// terminal would).
    fn open_sftp(&mut self, id: &str) -> Task<Message> {
        let s = match self.sessions.store.get(id) {
            Ok(s) => s,
            Err(e) => {
                self.sessions.notice = Some(format!("{id}: {e}"));
                return Task::none();
            }
        };
        if s.protocol != shagshell_core::session::Protocol::Ssh2 {
            self.sessions.notice = Some(fl!("sftp-needs-ssh"));
            return Task::none();
        }
        let name = shagshell_core::session::store::name_of(id).to_string();
        let creds = load_credentials();
        match ssh_launch(&self.control_dir, creds.as_ref(), &s, name, Some(id)) {
            Ok(l) => match l.sftp() {
                Some(l) => self.tabs.open(l, &look(&self.config)),
                None => Task::none(),
            },
            Err(e) => {
                self.sessions.notice = Some(e);
                Task::none()
            }
        }
    }

    /// Where the note chooser starts: the folder of the note in front, else the notes folder.
    fn note_start_dir(&self) -> std::path::PathBuf {
        let dir = shagshell_core::notes::dir();
        self.tabs
            .note_paths()
            .last()
            .and_then(|p| p.parent().map(std::path::Path::to_path_buf))
            .unwrap_or(dir)
    }

    /// A note tab for `path`: the one that has it is shown, else a new one.
    fn open_note(&mut self, path: &std::path::Path) -> Task<Message> {
        if let Some(id) = self.tabs.note_tab(path) {
            self.tabs.show(id);
            return Task::none();
        }
        if let Err(e) = crate::notes::read(path) {
            self.sessions.notice = Some(e);
            return Task::none();
        }
        let mut launch = tabs::Launch::local_shell();
        launch.title =
            (path.file_name()).map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        launch.note = Some(path.to_path_buf());
        let task = self.tabs.open_note_right(launch, &look(&self.config));
        self.remember_notes();
        task
    }

    /// The sessions with a tab open, for the next start.
    fn remember_sessions(&mut self) {
        let open = self.tabs.session_ids();
        if open != self.config.tabs_open {
            self.config.tabs_open = open;
            self.save_config();
        }
    }

    /// The open notes, for the next start.
    fn remember_notes(&mut self) {
        let open: Vec<String> = (self.tabs.note_paths().iter())
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        if open != self.config.notes_open {
            self.config.notes_open = open;
            self.save_config();
        }
    }

    /// Script → Scheduler…; open already: raised.
    fn open_scheduler(&mut self) -> Task<Message> {
        if let Some(s) = &self.sched_win {
            return cosmic::iced::window::gain_focus(s.window);
        }
        let (window, open) = cosmic::iced::window::open(crate::scheduler::settings());
        let s = crate::scheduler::SchedWindow::new(window, &self.schedule);
        let first = s.name_field();
        self.sched_win = Some(s);
        Task::batch([
            open.then(move |_| widget::text_input::focus(first.clone())),
            cosmic::ApplicationExt::set_window_title(self, fl!("sch-title"), window),
        ])
    }

    fn scheduler(&mut self, msg: crate::scheduler::SchMsg) -> Task<Message> {
        use crate::scheduler::SchMsg;
        let Some(w) = &mut self.sched_win else {
            return Task::none();
        };
        match msg {
            SchMsg::Close => {
                let id = w.window;
                self.sched_win = None;
                return cosmic::iced::window::close(id);
            }
            SchMsg::Drag => return cosmic::iced::window::drag(w.window),
            SchMsg::Save => self.sched_save(),
            SchMsg::Delete => {
                if let Some(i) = w.selected
                    && i < self.schedule.jobs.len()
                {
                    let old = self.schedule.jobs.remove(i);
                    self.sched_state.remove(&old.name);
                    self.sched_write();
                    let next = i.min(self.schedule.jobs.len().saturating_sub(1));
                    let w = self.sched_win.as_mut().expect("open");
                    w.reload(
                        &self.schedule,
                        (!self.schedule.jobs.is_empty()).then_some(next),
                    );
                }
            }
            SchMsg::RunNow => {
                if let Some(job) = w.selected.and_then(|i| self.schedule.jobs.get(i)).cloned() {
                    self.sched_run(&job);
                }
            }
            m => w.update(m, &self.schedule),
        }
        Task::none()
    }

    /// Save: the fields checked, the name not another job's; counting starts again.
    fn sched_save(&mut self) {
        let Some(w) = &mut self.sched_win else {
            return;
        };
        let job = match w.job() {
            Ok(j) => j,
            Err(e) => {
                w.error = Some(e);
                return;
            }
        };
        let at = w.selected.filter(|&i| i < self.schedule.jobs.len());
        if (self.schedule.jobs.iter().enumerate()).any(|(i, j)| j.name == job.name && Some(i) != at)
        {
            w.error = Some(fl!("sch-name-taken"));
            return;
        }
        let now = shagshell_core::schedule::local_minutes();
        // Counting goes on when only the name / script / arguments changed.
        let (last, running) = match at {
            Some(i) => {
                let old = std::mem::replace(&mut self.schedule.jobs[i], job.clone());
                let (last, running) = self.sched_state.remove(&old.name).unwrap_or((now, None));
                (if old.every == job.every { last } else { now }, running)
            }
            None => {
                self.schedule.jobs.push(job.clone());
                (now, None)
            }
        };
        self.sched_state.insert(job.name.clone(), (last, running));
        self.sched_write();
        let i = at.unwrap_or(self.schedule.jobs.len() - 1);
        if let Some(w) = &mut self.sched_win {
            w.reload(&self.schedule, Some(i));
        }
    }

    fn sched_write(&mut self) {
        if self.schedule_broken {
            self.sessions.notice = Some(fl!("sch-broken"));
            return;
        }
        if let Err(e) = self
            .schedule
            .save(&shagshell_core::schedule::Schedule::default_path())
        {
            self.sessions.notice = Some(format!("schedule.toml: {e}"));
        }
    }

    /// The clock: what is due starts, one run of a job at a time.
    fn sched_tick(&mut self) -> Task<Message> {
        let now = shagshell_core::schedule::local_minutes();
        let due: Vec<_> = (self.schedule.jobs.iter())
            .filter(|j| {
                let (last, running) = self
                    .sched_state
                    .get(&j.name)
                    .copied()
                    .unwrap_or((now, None));
                running.is_none() && shagshell_core::schedule::due(j, last, now)
            })
            .cloned()
            .collect();
        for j in due {
            self.sched_run(&j);
        }
        Task::none()
    }

    /// Starts `job` (no tab: the id nothing has, as with a pre-script); a run still going stops it.
    fn sched_run(&mut self, job: &shagshell_core::schedule::Job) {
        let st = self
            .sched_state
            .entry(job.name.clone())
            .or_insert((0, None));
        if st.1.is_some() {
            return;
        }
        st.0 = shagshell_core::schedule::local_minutes();
        let path = shagshell_core::script::expand_home(std::path::Path::new(&job.script));
        match self.scripts.run(&path, &job.args, u64::MAX) {
            Ok(run) => st.1 = Some(run),
            Err(e) => self.sessions.notice = Some(format!("{}: {e}", job.name)),
        }
    }

    /// «Last run» of the i-th job for its window.
    fn sched_last_text(&self, i: usize) -> String {
        let Some(job) = self.schedule.jobs.get(i) else {
            return String::new();
        };
        match self.sched_state.get(&job.name) {
            Some((_, Some(_))) => fl!("sch-running"),
            Some((m, _)) if job.enabled => {
                let ago = shagshell_core::schedule::local_minutes() - m;
                fl!("sch-counted", min = ago)
            }
            _ => String::new(),
        }
    }

    /// Tools → Credentials Manager…; open already: raised.
    fn open_credentials(&mut self) -> Task<Message> {
        if let Some(c) = &self.creds_win {
            return cosmic::iced::window::gain_focus(c.window);
        }
        let creds = match Credentials::load(&credentials::default_path()) {
            Ok(c) => c,
            Err(e) => {
                self.sessions.notice = Some(format!("credentials.toml: {e}"));
                return Task::none();
            }
        };
        let (window, open) = cosmic::iced::window::open(crate::credentials_window::settings());
        let c = CredWindow::new(window, creds);
        let first = c.name_field();
        let mut tasks = vec![
            open.then(move |_| widget::text_input::focus(first.clone())),
            cosmic::ApplicationExt::set_window_title(self, fl!("cw-title"), window),
        ];
        if let Some(name) = c.selected.clone() {
            tasks.push(has_password_task(window, name));
        }
        self.creds_win = Some(c);
        Task::batch(tasks)
    }

    fn credentials(&mut self, msg: CwMsg) -> Task<Message> {
        let Some(c) = &mut self.creds_win else {
            return Task::none();
        };
        match msg {
            CwMsg::Close => {
                let w = c.window;
                self.creds_win = None;
                return cosmic::iced::window::close(w);
            }
            CwMsg::Drag => return cosmic::iced::window::drag(c.window),
            CwMsg::Save => return self.save_credential(),
            CwMsg::Renamed(edit, Ok(())) => return self.commit_credential(edit),
            CwMsg::Renamed(_, Err(e)) => c.error = Some(fl!("ask-keyring-failed", error = e)),
            CwMsg::Delete => {
                if let Some(name) = &c.selected {
                    c.confirm = Some(self.sessions.store.using(name).len());
                }
            }
            CwMsg::DeleteConfirmed => return self.delete_credential(),
            m => {
                if let Some(name) = c.update(m) {
                    return has_password_task(c.window, name);
                }
            }
        }
        Task::none()
    }

    /// «Сохранить»: the set checked; a rename moves its password in the keyring first (failed:
    /// nothing written), then `commit_credential`.
    fn save_credential(&mut self) -> Task<Message> {
        let Some(c) = &mut self.creds_win else {
            return Task::none();
        };
        if c.confirm.is_some() {
            return Task::none(); // Enter in a field while the delete is asked
        }
        let (name, user) = (c.name.trim().to_string(), c.user.trim().to_string());
        let old = c.selected.clone();
        if let Err(e) = c.creds.clone().put(old.as_deref(), &name, &user) {
            c.error = Some(match e {
                PutError::Taken => fl!("name-taken"),
                PutError::Name(e) => crate::session_manager::name_text(e),
            });
            return Task::none();
        }
        let password = Hidden(std::mem::take(&mut c.password));
        let edit = Edit {
            old,
            name,
            user,
            password,
        };
        match edit.old.clone().filter(|o| *o != edit.name) {
            Some(o) => {
                let to = edit.name.clone();
                off_ui(
                    move || {
                        let (from, to) = (Owner::Credential(o), Owner::Credential(to));
                        shagshell_core::keyring::rename(&from, &to).map_err(|e| e.to_string())
                    },
                    move |r| Message::Credentials(CwMsg::Renamed(edit.clone(), r)),
                )
            }
            None => self.commit_credential(edit),
        }
    }

    /// The set written; its sessions (renamed: relinked) and their tabs follow; a typed password
    /// stored, then looked up for the field.
    fn commit_credential(&mut self, edit: Edit) -> Task<Message> {
        let Some(c) = &mut self.creds_win else {
            return Task::none();
        };
        let mut creds = c.creds.clone();
        let saved = (creds.put(edit.old.as_deref(), &edit.name, &edit.user))
            .map_err(|_| fl!("name-taken"))
            .and_then(|()| {
                (creds.save(&credentials::default_path()))
                    .map_err(|e| crate::session_manager::io_text(&e))
            });
        if let Err(e) = saved {
            c.error = Some(e);
            return Task::none();
        }
        c.creds = creds;
        c.reload(Some(&edit.name));
        let window = c.window;
        if let Some(o) = edit.old.as_ref().filter(|o| **o != edit.name)
            && let Err(e) = self.sessions.store.relink(o, Some(&edit.name), "")
        {
            self.sessions.notice = Some(crate::session_manager::io_text(&e));
        }
        // Its sessions' tabs: the new name / user for Reconnect and Clone.
        for id in self.sessions.store.using(&edit.name) {
            self.sessions_changed(&id, &id);
        }
        let lookup = has_password_task(window, edit.name.clone());
        let Hidden(password) = edit.password;
        if password.is_empty() {
            return lookup;
        }
        let to = Owner::Credential(edit.name);
        keyring_task(move || shagshell_core::keyring::store(&to, Secret::Password, &password))
            // Looked up once written (in a batch the lookup could come first).
            .chain(lookup)
    }

    /// Delete confirmed: its sessions keep its user, its password goes.
    fn delete_credential(&mut self) -> Task<Message> {
        let Some(c) = &mut self.creds_win else {
            return Task::none();
        };
        let Some(name) = c.selected.clone() else {
            return Task::none();
        };
        let mut creds = c.creds.clone();
        let user = creds.0.remove(&name).map(|s| s.user).unwrap_or_default();
        if let Err(e) = creds.save(&credentials::default_path()) {
            c.error = Some(crate::session_manager::io_text(&e));
            return Task::none();
        }
        c.creds = creds;
        let first = c.creds.0.keys().next().cloned();
        let shown = c.reload(first.as_deref());
        let window = c.window;
        match self.sessions.store.relink(&name, None, &user) {
            Ok(ids) => {
                for id in ids {
                    self.sessions_changed(&id, &id);
                }
            }
            Err(e) => self.sessions.notice = Some(crate::session_manager::io_text(&e)),
        }
        let delete = keyring_task(move || {
            shagshell_core::keyring::delete(&Owner::Credential(name), Secret::Password)
        });
        match shown {
            Some(n) => delete.chain(has_password_task(window, n)),
            None => delete,
        }
    }

    /// After a tab closed or its program ended: its prompts go.
    fn prune_asks(&mut self) -> Task<Message> {
        let tabs = &self.tabs;
        let out = self.asks.prune(|t| tabs.alive(t));
        self.ask_outcome(out)
    }

    fn ask_outcome(&mut self, out: crate::askpass::Outcome) -> Task<Message> {
        use crate::askpass::{AskMsg, Hidden};
        let mut tasks = Vec::new();
        if let Some((inc, owner, s)) = out.lookup {
            let who = owner.clone();
            tasks.push(off_ui(
                move || shagshell_core::keyring::lookup(&who, s),
                move |r| {
                    let r = r.map(|v| v.map(Hidden)).map_err(|e| e.to_string());
                    Message::Ask(AskMsg::Saved(inc.clone(), owner.clone(), r))
                },
            ));
        }
        if let Some((session, s, Hidden(value))) = out.store {
            tasks.push(keyring_task(move || {
                shagshell_core::keyring::store(&session, s, &value)
            }));
        }
        if let Some((session, s)) = out.forget {
            tasks.push(keyring_task(move || {
                shagshell_core::keyring::delete(&session, s)
            }));
        }
        if out.refocus {
            tasks.push(self.tabs.focus_active());
        }
        if let Some((tab, field)) = out.shown {
            if let Some(t) = tab {
                self.tabs.show(t);
            }
            tasks.push(widget::text_input::focus(field));
        }
        Task::batch(tasks)
    }

    /// Session files changed (`old` saved, or renamed / moved to `new`): the tabs opened from
    /// it follow, and Reconnect / Clone run it as saved now (SecureCRT re-reads the session).
    fn sessions_changed(&mut self, old: &str, new: &str) {
        let store = &self.sessions.store;
        let creds = load_credentials();
        for l in self.tabs.launches_mut() {
            // Telnet and Serial tabs: run the session as saved now, under its new name.
            if let tabs::Protocol::Telnet {
                session: Some(id), ..
            }
            | tabs::Protocol::Serial {
                session: Some(id), ..
            } = &l.protocol
                && let Some(id) = shagshell_core::session::store::retarget(id, old, new)
                && let Ok(s) = store.get(&id)
                && let Ok(n) = program_launch(&s, l.title.clone(), Some(&id))
            {
                *l = n;
                continue;
            }
            let tabs::Protocol::Ssh {
                session: Some(id), ..
            } = &l.protocol
            else {
                continue;
            };
            let Some(id) = shagshell_core::session::store::retarget(id, old, new) else {
                continue;
            };
            let relaunch = store.get(&id).ok().and_then(|s| {
                let title = l.title.clone();
                let n = ssh_launch(
                    &self.control_dir,
                    creds.as_ref(),
                    &s,
                    title.clone(),
                    Some(&id),
                );
                // An SFTP tab stays one, under its own title.
                match l.sftp {
                    true => n.ok()?.sftp().map(|n| tabs::Launch { title, ..n }),
                    false => n.ok(),
                }
            });
            match relaunch {
                Some(n) => *l = n,
                None => {
                    if let tabs::Protocol::Ssh { session, .. } = &mut l.protocol {
                        *session = Some(id);
                    }
                }
            }
        }
        self.tabs.retitle_all();
    }

    fn quick_connect(&mut self) -> Task<Message> {
        let q = QuickConnect::new();
        let focus = q.focus();
        self.quick = Some(q);
        focus
    }

    /// Quick Connect's Connect: save if asked (from the Default template), then open.
    fn quick_connected(&mut self, req: crate::quick_connect::Request) -> Task<Message> {
        let mut title = match &req.session.user {
            Some(u) => format!("{u}@{}", req.session.host),
            None => req.session.host.clone(),
        };
        let mut session = req.session.clone();
        let mut saved_id = None;
        if let Some((folder, name)) = &req.save_as {
            if let Err(e) = crate::session_manager::check(name, folder.is_empty()) {
                if let Some(q) = &mut self.quick {
                    q.error = Some(e);
                }
                return Task::none();
            }
            let store = &self.sessions.store;
            let mut s = store.template();
            s.host = req.session.host.clone();
            (s.port, s.user) = (req.session.port, req.session.user.clone());
            let saved = store.add(folder, name, &s).map(|id| (id, s));
            match saved {
                // Opened as saved: the Default template's key / keepalive / options too.
                Ok((id, s)) => {
                    self.sessions.select(&id);
                    self.config.expanded = self.sessions.expanded.iter().cloned().collect();
                    self.save_config();
                    title = name.clone();
                    session = s;
                    saved_id = Some(id);
                }
                Err(e) => {
                    if let Some(q) = &mut self.quick {
                        q.error = Some(crate::session_manager::io_text(&e));
                    }
                    return Task::none();
                }
            }
        }
        self.quick = None;
        let creds = load_credentials();
        match ssh_launch(
            &self.control_dir,
            creds.as_ref(),
            &session,
            title,
            saved_id.as_deref(),
        ) {
            Ok(l) => self.tabs.open(l, &look(&self.config)),
            Err(e) => {
                self.sessions.notice = Some(e);
                Task::none()
            }
        }
    }

    /// Session Options in its window; open already: raised, what is typed there kept.
    fn open_options(&mut self, target: Target) -> Task<Message> {
        if let Some(o) = &self.options {
            return cosmic::iced::window::gain_focus(o.window);
        }
        let store = &self.sessions.store;
        let loaded = match &target {
            // As SecureCRT: a new session starts as a copy of Default.
            Target::New { .. } => Ok(store.template()),
            Target::Edit { id } => store.get(id),
            // A broken Default.toml is said, not overwritten with an empty one.
            Target::Default => match store.get(DEFAULT) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Default::default()),
                r => r,
            },
        };
        let session = match loaded {
            Ok(s) => s,
            Err(e) => {
                self.sessions.notice = Some(crate::session_manager::io_text(&e));
                return Task::none();
            }
        };
        let (window, open) = cosmic::iced::window::open(crate::session_options::settings());
        let sets = Credentials::load(&credentials::default_path())
            .unwrap_or_default()
            .0
            .into_iter()
            .map(|(n, s)| (n, s.user))
            .collect();
        let o = SessionOptions::new(window, target, &session, sets, self.buttons.names());
        // The first field focused once the window is there (before, the focus goes nowhere).
        let first = o.first_field();
        let mut tasks = vec![
            open.then(move |_| widget::text_input::focus(first.clone())),
            cosmic::ApplicationExt::set_window_title(self, o.title(), window),
        ];
        if let Target::Edit { id } = &o.target {
            tasks.push(saved_task(window, id.clone()));
        }
        self.options = Some(o);
        Task::batch(tasks)
    }

    fn session_options(&mut self, msg: SoMsg) -> Task<Message> {
        let Some(o) = &mut self.options else {
            return Task::none();
        };
        match msg {
            SoMsg::Cancel => {
                let w = o.window;
                self.options = None;
                return cosmic::iced::window::close(w);
            }
            SoMsg::Drag => return cosmic::iced::window::drag(o.window),
            SoMsg::Forget => {
                let Target::Edit { id } = &o.target else {
                    return Task::none();
                };
                let (w, id) = (o.window, id.clone());
                let forget = shagshell_core::keyring::Owner::Session(id.clone());
                // ssh keeps a login open for a minute (ControlPersist): the next one would
                // not ask. «stop»: no new logins through it, open tabs go on.
                let stop = (self.sessions.store.get(&id).ok())
                    .map(|s| load_credentials().unwrap_or_default().resolve(&s))
                    .and_then(|s| {
                        let home = std::env::home_dir().unwrap_or_default();
                        shagshell_core::launch::ssh_argv(&s, &self.control_dir, &home).ok()
                    });
                return Task::batch([
                    keyring_task(move || {
                        use shagshell_core::keyring::{Secret, delete};
                        if let Some(mut argv) = stop {
                            let at = argv.len() - 2; // before `-- host`
                            argv.splice(at..at, ["-O".into(), "stop".into()]);
                            let _ = std::process::Command::new(argv.remove(0))
                                .args(argv)
                                .stdin(std::process::Stdio::null())
                                .stdout(std::process::Stdio::null())
                                .stderr(std::process::Stdio::null())
                                .status();
                        }
                        delete(&forget, Secret::Password)?;
                        delete(&forget, Secret::Passphrase)
                    }),
                    // The line goes at once; a failed delete is said under the tree.
                    Task::done(cosmic::Action::App(Message::SessionOptions(SoMsg::Saved(
                        w, false,
                    )))),
                ]);
            }
            SoMsg::Browse => {
                let title = fl!("so-identity");
                let w = o.window;
                return Task::perform(crate::session_options::browse(title), move |p| {
                    cosmic::Action::App(Message::SessionOptions(SoMsg::Browsed(w, p)))
                });
            }
            SoMsg::Ok => {
                // What the window doesn't show is kept from the file as it is now.
                let base = match &o.target {
                    Target::Edit { id } => match self.sessions.store.get(id) {
                        Ok(s) => s,
                        // Renamed or deleted meanwhile: said, not written back under the old name.
                        Err(e) => {
                            o.error = Some(crate::session_manager::io_text(&e));
                            return Task::none();
                        }
                    },
                    Target::New { .. } => self.sessions.store.template(),
                    Target::Default => self.sessions.store.get(DEFAULT).unwrap_or_default(),
                };
                let Some(mut s) = o.checked(&base) else {
                    return Task::none();
                };
                // A set renamed or deleted while the window was open: what the file says now.
                let sets = load_credentials().unwrap_or_default();
                if s.credential
                    .as_ref()
                    .is_some_and(|c| !sets.0.contains_key(c))
                {
                    s.credential = base.credential.clone();
                }
                return match self.save_options(&s) {
                    Ok(task) => Task::batch([task, self.session_options(SoMsg::Cancel)]),
                    Err(e) => {
                        if let Some(o) = &mut self.options {
                            o.error = Some(e);
                        }
                        Task::none()
                    }
                };
            }
            m => o.update(m),
        }
        Task::none()
    }

    /// OK: the session written where the window's target says.
    fn save_options(
        &mut self,
        s: &shagshell_core::session::Session,
    ) -> Result<Task<Message>, String> {
        let Some(o) = &self.options else {
            return Ok(Task::none());
        };
        let password = o.password();
        let store = &self.sessions.store;
        let io = |e: std::io::Error| crate::session_manager::io_text(&e);
        let mut tasks = Vec::new();
        let mut moved: Option<Task<Message>> = None;
        // The saved session this ends up as: where a typed Logon password is stored.
        let id: Option<String> = match &o.target {
            Target::New { folder } => {
                let name = o.name();
                crate::session_manager::check(&name, folder.is_empty())?;
                let id = store.add(folder, &name, s).map_err(io)?;
                self.sessions.select(&id); // reloads the tree
                self.config.expanded = self.sessions.expanded.iter().cloned().collect();
                self.save_config();
                Some(id)
            }
            Target::Edit { id } => {
                let (id, name) = (id.clone(), o.name());
                let renamed = name != shagshell_core::session::store::name_of(&id);
                if renamed {
                    crate::session_manager::check(
                        &name,
                        crate::session_manager::parent(&id).is_empty(),
                    )?;
                }
                store.save(&id, s).map_err(io)?;
                let new = match renamed {
                    true => store.rename(&id, &name).map_err(io)?,
                    false => id.clone(),
                };
                self.sessions.reload();
                self.sessions_changed(&id, &new);
                // A scheme chosen for the session shows in its open tabs at once.
                self.tabs.restyle(&look(&self.config));
                if renamed {
                    self.sessions.select(&new);
                    let (from, to) = (id, new.clone());
                    // Before the store below: in order, or the old password could win.
                    moved = Some(keyring_task(move || {
                        shagshell_core::keyring::retarget(&from, &to)
                    }));
                }
                Some(new)
            }
            Target::Default => {
                store.save(DEFAULT, s).map_err(io)?;
                None
            }
        };
        // A password typed in Logon Actions: into the keyring (after a rename moved the old one).
        if let (Some(id), Some(pw)) = (id, password) {
            let store_it = keyring_task(move || {
                use shagshell_core::keyring::{Owner, Secret, store};
                store(&Owner::Session(id), Secret::Password, &pw)
            });
            tasks.push(match moved.take() {
                Some(m) => m.chain(store_it),
                None => store_it,
            });
        }
        tasks.extend(moved);
        Ok(Task::batch(tasks))
    }

    fn import_ssh_config(&mut self) {
        let path = std::env::home_dir().unwrap_or_default().join(".ssh/config");
        let notice = match std::fs::read_to_string(&path) {
            Ok(text) => {
                let (done, skipped) =
                    shagshell_core::sshconfig::import(&self.sessions.store, &text);
                self.sessions.reload();
                fl!(
                    "import-done",
                    done = done.to_string(),
                    skipped = skipped.to_string()
                )
            }
            Err(e) => format!("{}: {e}", path.display()),
        };
        self.sessions.notice = Some(notice);
    }

    fn save_config(&self) {
        if let Some(h) = &self.config_handler
            && let Err(e) = self.config.write_entry(h)
        {
            log::warn!("config: {e}");
        }
    }

    fn set_drawer(&mut self, d: Option<Drawer>) {
        self.core.window.show_context = d.is_some();
        self.drawer = d;
    }

    /// The same item again closes its drawer.
    fn toggle_drawer(&mut self, d: Drawer) {
        let open = (self.drawer != Some(d)).then_some(d);
        self.set_drawer(open);
    }
}

/// This instance's askpass socket (`core::askpass::socket_path`).
fn askpass_sock(control_dir: &std::path::Path) -> std::path::PathBuf {
    shagshell_core::askpass::socket_path(control_dir, std::process::id())
}

/// Asks GitHub for the latest release; `manual`: from the Help menu.
fn check_updates(manual: bool) -> Task<Message> {
    off_ui(
        || shagshell_core::update::check(env!("CARGO_PKG_VERSION")),
        move |s| Message::Update(crate::updates::UpMsg::Checked(s, manual)),
    )
}

/// `f` on a blocking thread (D-Bus, files), its result as a message.
fn off_ui<T: Send + 'static>(
    f: impl FnOnce() -> T + Send + 'static,
    then: impl Fn(T) -> Message + Send + 'static,
) -> Task<Message> {
    // Started when the task runs, not when it is made: `chain` must order the work itself.
    let work = async move { tokio::task::spawn_blocking(f).await };
    Task::perform(work, move |r| match r {
        Ok(v) => cosmic::Action::App(then(v)),
        Err(_) => cosmic::Action::None,
    })
}

/// Session Options: whether the session has a secret saved (its «Забыть» line).
fn saved_task(window: cosmic::iced::window::Id, id: String) -> Task<Message> {
    off_ui(
        move || {
            let owner = shagshell_core::keyring::Owner::Session(id);
            shagshell_core::keyring::exists(&owner).unwrap_or(false)
        },
        move |saved| Message::SessionOptions(SoMsg::Saved(window, saved)),
    )
}

/// The master-password keyring: read (or changed, then read) off the UI thread.
fn separate_task<E: std::fmt::Display + Send + 'static>(
    f: impl FnOnce() -> Result<bool, E> + Send + 'static,
) -> Task<Message> {
    off_ui(f, |r| Message::Separate(r.map_err(|e| e.to_string())))
}

/// Credentials Manager: whether the set has a password saved (no unlock asked).
fn has_password_task(window: cosmic::iced::window::Id, name: String) -> Task<Message> {
    off_ui(
        {
            let name = name.clone();
            move || shagshell_core::keyring::exists(&Owner::Credential(name)).unwrap_or(false)
        },
        move |has| Message::Credentials(CwMsg::HasPassword(window, name.clone(), has)),
    )
}

/// A keyring write in the background; a failure is said under the tree.
fn keyring_task<E: std::fmt::Display + Send + 'static>(
    f: impl FnOnce() -> Result<(), E> + Send + 'static,
) -> Task<Message> {
    off_ui(f, |r| {
        Message::Ask(crate::askpass::AskMsg::Stored(r.map_err(|e| e.to_string())))
    })
}

/// `credentials.toml`, `None` if it can't be read (a session using a set then won't connect).
fn load_credentials() -> Option<Credentials> {
    Credentials::load(&credentials::default_path())
        .inspect_err(|e| log::warn!("credentials.toml: {e}"))
        .ok()
}

/// `creds`: `load_credentials`, once for every launch built together.
fn ssh_launch(
    control_dir: &std::path::Path,
    creds: Option<&Credentials>,
    s: &shagshell_core::session::Session,
    title: String,
    session: Option<&str>,
) -> Result<tabs::Launch, String> {
    let home = std::env::home_dir().unwrap_or_default();
    // A credential set: its user here, its password through askpass (a set that is gone: none).
    let creds = match (creds, &s.credential) {
        (Some(c), _) => c,
        (None, Some(_)) => return Err(fl!("credentials-broken")),
        (None, None) => &Credentials::default(),
    };
    let credential = (s.credential.as_ref()).filter(|c| creds.0.contains_key(*c));
    let s = &creds.resolve(s);
    let mut argv =
        shagshell_core::launch::ssh_argv(s, control_dir, &home).map_err(|_| fl!("bad-extra"))?;
    let program = argv.remove(0);
    let exe = std::env::current_exe().unwrap_or_else(|_| "shagshell".into());
    let sock = askpass_sock(control_dir);
    let env =
        shagshell_core::launch::askpass_env(&exe, &sock, session, credential.map(String::as_str));
    Ok(tabs::Launch {
        program,
        args: argv,
        env,
        title,
        protocol: tabs::Protocol::Ssh {
            host: s.host.clone(),
            session: session.map(str::to_string),
        },
        sftp: false,
        note: None,
        console: false,
        auto_reconnect: s.reconnect.then(|| tabs::AutoReconnect {
            delay: s.reconnect_delay.unwrap_or(5),
            tries: s.reconnect_tries.unwrap_or(10),
        }),
        // An empty file name: no log on connect, the port and user still serve a log by hand.
        log: Some(log_spec(s)),
        logon: logon_spec(s, session),
        scheme: s.color_scheme.clone(),
        label: s.label.clone(),
        session: None,
    })
}

/// The session's log wishes (the file name template, how to write it).
fn log_spec(s: &shagshell_core::session::Session) -> tabs::LogSpec {
    use shagshell_core::session::Protocol as P;
    // `%P` / `%U` mean something to a connection that has a port and a user.
    let net = matches!(s.protocol, P::Ssh2 | P::Telnet);
    tabs::LogSpec {
        file: s.log_file.clone(),
        append: s.log_append,
        raw: s.log_raw,
        port: s.port.filter(|_| net),
        user: (s.user.clone().filter(|_| s.protocol == P::Ssh2)).unwrap_or_default(),
    }
}

/// The session's Logon Actions, if it has any.
fn logon_spec(s: &shagshell_core::session::Session, id: Option<&str>) -> Option<tabs::LogonSpec> {
    (!s.logon.is_empty()).then(|| tabs::LogonSpec {
        steps: s.logon.clone(),
        user: s.user.clone().unwrap_or_default(),
        // The password ssh itself would use: the credential set's, else the session's own.
        owner: match (&s.credential, id) {
            (Some(c), _) => Some(shagshell_core::keyring::Owner::Credential(c.clone())),
            (None, Some(id)) => Some(shagshell_core::keyring::Owner::Session(id.to_string())),
            (None, None) => None,
        },
    })
}

/// A Telnet or Serial session: its program (`telnet`, `picocom`) in the tab; why not if the
/// program is not installed.
fn program_launch(
    s: &shagshell_core::session::Session,
    title: String,
    id: Option<&str>,
) -> Result<tabs::Launch, String> {
    use shagshell_core::session::Protocol as P;
    let (mut argv, protocol) = match s.protocol {
        P::Telnet => (
            shagshell_core::launch::telnet_argv(s),
            tabs::Protocol::Telnet {
                host: s.host.clone(),
                session: id.map(str::to_string),
            },
        ),
        _ => (
            shagshell_core::launch::serial_argv(s),
            tabs::Protocol::Serial {
                device: s.serial.device.clone(),
                session: id.map(str::to_string),
            },
        ),
    };
    let program = argv.remove(0);
    let found = shagshell_core::check::probe(&[&program], std::env::var_os("PATH").as_deref());
    if !found.iter().any(|(_, ok)| *ok) {
        return Err(fl!("proto-missing", program = program));
    }
    Ok(tabs::Launch {
        program,
        args: argv,
        env: Vec::new(),
        title,
        protocol,
        sftp: false,
        note: None,
        console: false,
        log: Some(log_spec(s)),
        auto_reconnect: None,
        logon: logon_spec(s, id),
        scheme: s.color_scheme.clone(),
        label: s.label.clone(),
        session: None,
    })
}

/// A key or pointer event, tagged with the window it came from (`Message::Input`).
fn input(w: cosmic::iced::window::Id, m: Option<Message>) -> Option<Message> {
    m.map(|m| Message::Input(w, Box::new(m)))
}

/// The terminals' look: cosmic-term's style, the theme's darkness and window background.
fn look(config: &Config) -> tabs::Look {
    let theme = cosmic::theme::active();
    let c = theme.cosmic();
    let bg = c.bg_color();
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    let bg = 0xFF00_0000 | byte(bg.red) << 16 | byte(bg.green) << 8 | byte(bg.blue);
    let style = crate::term::style::Style {
        font_size: config.font_px(),
        font_family: config.font_family.clone(),
        ..Default::default()
    };
    (style, c.is_dark, bg)
}

/// App keys that act even when a text field took the key.
fn over_fields(a: Action) -> bool {
    matches!(
        a,
        Action::QuickConnect
            | Action::Connect
            | Action::ToggleSessionManager
            | Action::SelectTab(_)
            | Action::NextTab
            | Action::PrevTab
    )
}

/// An Esc no widget took (`Message::Key`, as the subscription tags it).
fn is_escape(m: &Message) -> bool {
    matches!(m, Message::Key(Physical::Code(Code::Escape), mods) if mods.is_empty())
}

/// Keys no widget took, other than the app's (`key_action`): Enter connects from the tree or
/// restarts an ended tab; the tree's own keys.
fn route_key(key: &Physical, mods: Modifiers) -> Option<Message> {
    match (key, mods) {
        (Physical::Code(Code::Enter | Code::NumpadEnter), m) if m.is_empty() => {
            Some(Message::EnterKey)
        }
        // The tree's own keys: only when nothing else (a terminal, a text field) took them.
        (Physical::Code(code), m) if m.is_empty() => {
            let k = match code {
                Code::ArrowUp => TreeKey::Up,
                Code::ArrowDown => TreeKey::Down,
                Code::Delete => TreeKey::Delete,
                Code::F2 => TreeKey::Rename,
                Code::Escape => TreeKey::Clear,
                _ => return None,
            };
            Some(Message::Sessions(SmMsg::Key(k)))
        }
        _ => None,
    }
}

/// The app's keys, by physical key so they work in any layout. The one list: the terminal
/// lets exactly these through (`tabs::is_app_key`), everything else is the program's.
pub fn key_action(key: &Physical, mods: Modifiers) -> Option<Action> {
    // A chord is being recorded (Keymap Editor): nothing acts.
    if crate::keymap_editor::capturing() {
        return None;
    }
    builtin_action(key, mods).or_else(|| {
        let Physical::Code(code) = *key else {
            return None;
        };
        let i = crate::keymap_editor::lookup(mods, &format!("{code:?}"))?;
        Some(Action::Bound(u16::try_from(i).ok()?))
    })
}

/// The program's own keys (the user's cannot take these).
fn builtin_action(key: &Physical, mods: Modifiers) -> Option<Action> {
    let Physical::Code(code) = *key else {
        return None;
    };
    let ctrl_shift = Modifiers::CTRL | Modifiers::SHIFT;
    let digits = [
        Code::Digit1,
        Code::Digit2,
        Code::Digit3,
        Code::Digit4,
        Code::Digit5,
        Code::Digit6,
        Code::Digit7,
        Code::Digit8,
        Code::Digit9,
    ];
    Some(match (code, mods) {
        (Code::KeyM, Modifiers::ALT) => Action::ToggleSessionManager,
        (Code::KeyQ, Modifiers::ALT) => Action::QuickConnect,
        (Code::KeyC, Modifiers::ALT) => Action::Connect,
        (Code::KeyP, Modifiers::ALT) => Action::ConnectSftp,
        (Code::KeyT, m) if m == ctrl_shift => Action::NewLocalShell,
        (Code::KeyW, m) if m == ctrl_shift => Action::CloseTab,
        (Code::KeyC, m) if m == ctrl_shift => Action::Copy,
        (Code::KeyV, m) if m == ctrl_shift => Action::Paste,
        (Code::Tab, Modifiers::CTRL) => Action::NextTab,
        (Code::Tab, m) if m == ctrl_shift => Action::PrevTab,
        (_, Modifiers::ALT) => Action::SelectTab(digits.iter().position(|d| *d == code)? as u8),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn regression_escape_in_a_secondary_window_is_seen() {
        let esc = Message::Key(Physical::Code(Code::Escape), Modifiers::empty());
        assert!(is_escape(&esc));
        assert!(!is_escape(&Message::Key(
            Physical::Code(Code::Escape),
            Modifiers::CTRL
        )));
    }

    use super::*;

    #[test]
    fn terminal_keys_stay_in_the_terminal() {
        for (code, mods) in [
            (Code::KeyC, Modifiers::CTRL),
            (Code::KeyD, Modifiers::CTRL),
            (Code::KeyW, Modifiers::CTRL),
            (Code::KeyB, Modifiers::ALT),
            (Code::KeyF, Modifiers::ALT),
            (Code::Insert, Modifiers::CTRL),
            (Code::Insert, Modifiers::SHIFT),
            (Code::Digit0, Modifiers::ALT),
            (Code::Enter, Modifiers::empty()),
        ] {
            assert_eq!(
                key_action(&Physical::Code(code), mods),
                None,
                "{code:?} {mods:?}"
            );
        }
    }

    #[test]
    fn app_keys() {
        let k = |code, mods| key_action(&Physical::Code(code), mods);
        let cs = Modifiers::CTRL | Modifiers::SHIFT;
        assert_eq!(k(Code::KeyT, cs), Some(Action::NewLocalShell));
        assert_eq!(k(Code::Digit1, Modifiers::ALT), Some(Action::SelectTab(0)));
        assert_eq!(k(Code::Digit9, Modifiers::ALT), Some(Action::SelectTab(8)));
        assert_eq!(k(Code::Tab, cs), Some(Action::PrevTab));
    }
}
