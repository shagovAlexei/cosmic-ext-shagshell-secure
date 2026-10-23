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
use cosmic::widget::{self, about::About};
use cosmic::{Application, Element};
use shagshell_core::credentials::{self, Credentials, PutError};
use shagshell_core::keyring::{Owner, Secret};
use shagshell_core::session::store::DEFAULT;

pub const APP_ID: &str = "io.github.shagovalexei.ShagShellSecure";

/// What the context drawer shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drawer {
    About,
    Options,
}

pub struct App {
    core: Core,
    config: Config,
    config_handler: Option<cosmic::cosmic_config::Config>,
    about: About,
    drawer: Option<Drawer>,
    tabs: Tabs,
    sessions: SessionManager,
    quick: Option<QuickConnect>,
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
}

#[derive(Clone, Debug)]
pub enum Message {
    Action(Action),
    Config(Config),
    /// Global Options: a changed value, written to the config.
    SetConfig(Config),
    OpenUrl(String),
    CloseDrawer,
    Tabs(TabsMsg),
    TermTx(crate::term::Events),
    Term(crate::term::TabId, alacritty_terminal::event::Event),
    /// Enter while the active tab's program has ended.
    EnterKey,
    Surface(cosmic::surface::Action),
    /// The theme was switched in Global Options: terminals take its colors.
    Restyle,
    /// Anything else a widget speaking `cosmic::Action` says (the tab bar): to the runtime.
    Cosmic(Box<cosmic::Action<Message>>),
    Sessions(SmMsg),
    QuickConnect(QcMsg),
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
    /// Global Options: the master-password keyring wanted on / off.
    SetSeparate(bool),
    /// …and how it is now (or why it could not be changed).
    Separate(Result<bool, String>),
}

impl Application for App {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = APP_ID;

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(mut core: Core, _: ()) -> (Self, Task<Message>) {
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
        let sessions = SessionManager::new(store, &config.expanded, config.session_manager_width);
        let asks = crate::askpass::Asks::new(askpass_sock(&control_dir));
        let app = Self {
            core,
            config,
            config_handler,
            about: about::about(),
            drawer: None,
            tabs: Tabs::default(),
            sessions,
            quick: None,
            options: None,
            creds_win: None,
            separate: None,
            control_dir,
            asks,
        };
        (app, theme.unwrap_or_else(Task::none))
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Action(a) => return self.act(a),
            Message::Config(c) => self.config = c,
            Message::SetConfig(c) => {
                let (lang, theme) = (
                    c.language != self.config.language,
                    c.app_theme != self.config.app_theme,
                );
                self.config = c;
                self.save_config();
                let mut tasks = Vec::new();
                if lang {
                    crate::i18n::init(&config::languages(
                        &self.config.language,
                        i18n_embed::DesktopLanguageRequester::requested_languages(),
                    ));
                    self.core.window.header_title = fl!("app-title");
                    self.about = about::about();
                }
                if theme {
                    tasks.push(
                        cosmic::command::set_theme(self.config.app_theme.theme())
                            .chain(cosmic::task::message(Message::Restyle)),
                    );
                }
                return Task::batch(tasks);
            }
            Message::OpenUrl(url) => {
                match std::process::Command::new("xdg-open").arg(url).spawn() {
                    // Reaped off the UI thread, or each click leaves a zombie until exit.
                    Ok(mut child) => drop(std::thread::spawn(move || child.wait())),
                    Err(e) => log::warn!("xdg-open: {e}"),
                }
            }
            Message::CloseDrawer => self.set_drawer(None),
            Message::Tabs(m) => {
                self.sessions.focused = false; // a terminal is used: Enter is its again
                let task = self.tabs.update(m, &look());
                return Task::batch([task, self.prune_asks()]);
            }
            Message::TermTx(tx) => self.tabs.set_sender(tx),
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
            Message::EnterKey if self.sessions.focused => {
                let ids = self.sessions.connect();
                return self.open_sessions(&ids);
            }
            Message::EnterKey if self.tabs.enter_on_dead() => return self.act(Action::Reconnect),
            Message::EnterKey => {}
            Message::Surface(a) => {
                return cosmic::task::message(cosmic::Action::Cosmic(
                    cosmic::app::Action::Surface(a),
                ));
            }
            Message::Restyle => self.tabs.restyle(&look()),
            Message::Cosmic(a) => return cosmic::task::message(*a),
            Message::Sessions(m) => {
                let out = self.sessions.update(m);
                if out.save {
                    self.config.expanded = self.sessions.expanded.iter().cloned().collect();
                    self.config.session_manager_width = self.sessions.width as u16;
                    self.save_config();
                }
                let mut tasks: Vec<_> = out.task.into_iter().collect();
                tasks.push(self.open_sessions(&out.open));
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
            Message::Input(..) => {}
            Message::CloseRequested(w) if self.options.as_ref().is_some_and(|o| o.window == w) => {
                return self.session_options(SoMsg::Cancel);
            }
            Message::CloseRequested(w)
                if self.creds_win.as_ref().is_some_and(|c| c.window == w) =>
            {
                return self.credentials(CwMsg::Close);
            }
            Message::CloseRequested(_) => {}
            Message::Credentials(m) => return self.credentials(m),
            Message::Key(k, m) => {
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
                                Some(Message::Sessions(SmMsg::Cursor(position.x)))
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
            crate::askpass::subscription(askpass_sock(&self.control_dir)),
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
        self.tabs.restyle(&look());
        Task::none()
    }

    fn system_theme_mode_update(
        &mut self,
        _: &[&'static str],
        _: &cosmic::cosmic_theme::ThemeMode,
    ) -> Task<Message> {
        self.tabs.restyle(&look());
        Task::none()
    }

    fn context_drawer(&self) -> Option<cosmic::app::context_drawer::ContextDrawer<'_, Message>> {
        match self.drawer? {
            Drawer::About => Some(about::view(&self.about)),
            Drawer::Options => Some(options::view(&self.config, self.separate)),
        }
    }

    /// Session Options; popups (menus) are windows too, libcosmic draws them itself, only
    /// asking here.
    fn view_window(&self, id: cosmic::iced::window::Id) -> Element<'_, Message> {
        let focused = self.core.focused_window() == Some(id);
        if let Some(o) = self.options.as_ref().filter(|o| o.window == id) {
            return o.view(focused);
        }
        match self.creds_win.as_ref().filter(|c| c.window == id) {
            Some(c) => c.view(focused),
            None => widget::Space::new().into(),
        }
    }

    fn dialog(&self) -> Option<Element<'_, Message>> {
        (self.asks.dialog(|t| self.tabs.title(t)))
            .or_else(|| self.tabs.dialog())
            .or_else(|| self.quick.as_ref().map(QuickConnect::view))
            .or_else(|| self.sessions.dialog())
    }

    fn header_start(&self) -> Vec<Element<'_, Message>> {
        vec![menu::bar(
            self.config.session_manager_visible,
            self.tabs.menu_state(),
        )]
    }

    fn view(&self) -> Element<'_, Message> {
        let window = (self.core.main_window_id()).unwrap_or(cosmic::iced::window::Id::RESERVED);
        let center = match self
            .tabs
            .view(&self.config, window, self.dialog().is_none())
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
        let mut middle = widget::row::with_capacity(2).spacing(2);
        if self.config.session_manager_visible {
            middle = middle.push(self.sessions.view(window));
        }
        widget::column::with_children(vec![
            toolbar::view(),
            middle.push(center).height(Length::Fill).into(),
            status_bar::view(self.tabs.status()),
        ])
        .into()
    }
}

impl App {
    fn act(&mut self, action: Action) -> Task<Message> {
        if let Some(task) = self.tabs.act(action, &look()) {
            return task;
        }
        match action {
            Action::Exit => {
                let _ = std::fs::remove_file(askpass_sock(&self.control_dir));
                return cosmic::iced::exit();
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
            Action::About => self.toggle_drawer(Drawer::About),
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

    /// Each session in a tab of its own; one that can't start says why under the tree.
    fn open_sessions(&mut self, ids: &[String]) -> Task<Message> {
        let mut tasks = Vec::new();
        let creds = load_credentials();
        for id in ids {
            match self.sessions.store.get(id) {
                Ok(s) => {
                    let name = shagshell_core::session::store::name_of(id).to_string();
                    if s.protocol == shagshell_core::session::Protocol::Local {
                        let l = tabs::Launch {
                            title: name,
                            ..tabs::Launch::local_shell()
                        };
                        tasks.push(self.tabs.open(l, &look()));
                        continue;
                    }
                    match ssh_launch(&self.control_dir, creds.as_ref(), &s, name, Some(id)) {
                        Ok(l) => tasks.push(self.tabs.open(l, &look())),
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
        if s.protocol == shagshell_core::session::Protocol::Local {
            self.sessions.notice = Some(fl!("so-ssh-only"));
            return Task::none();
        }
        let name = shagshell_core::session::store::name_of(id).to_string();
        let creds = load_credentials();
        match ssh_launch(&self.control_dir, creds.as_ref(), &s, name, Some(id)) {
            Ok(l) => match l.sftp() {
                Some(l) => self.tabs.open(l, &look()),
                None => Task::none(),
            },
            Err(e) => {
                self.sessions.notice = Some(e);
                Task::none()
            }
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
            Ok(l) => self.tabs.open(l, &look()),
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
        let o = SessionOptions::new(window, target, &session, sets);
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
                return Task::batch([
                    keyring_task(move || {
                        use shagshell_core::keyring::{Secret, delete};
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
        let store = &self.sessions.store;
        let io = |e: std::io::Error| crate::session_manager::io_text(&e);
        match &o.target {
            Target::New { folder } => {
                let name = o.name();
                crate::session_manager::check(&name, folder.is_empty())?;
                let id = store.add(folder, &name, s).map_err(io)?;
                self.sessions.select(&id); // reloads the tree
                self.config.expanded = self.sessions.expanded.iter().cloned().collect();
                self.save_config();
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
                if renamed {
                    self.sessions.select(&new);
                    return Ok(keyring_task(move || {
                        shagshell_core::keyring::retarget(&id, &new)
                    }));
                }
            }
            Target::Default => store.save(DEFAULT, s).map_err(io)?,
        }
        Ok(Task::none())
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
    })
}

/// A key or pointer event, tagged with the window it came from (`Message::Input`).
fn input(w: cosmic::iced::window::Id, m: Option<Message>) -> Option<Message> {
    m.map(|m| Message::Input(w, Box::new(m)))
}

/// The terminals' look: cosmic-term's style, the theme's darkness and window background.
fn look() -> tabs::Look {
    let theme = cosmic::theme::active();
    let c = theme.cosmic();
    let bg = c.bg_color();
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    let bg = 0xFF00_0000 | byte(bg.red) << 16 | byte(bg.green) << 8 | byte(bg.blue);
    (Default::default(), c.is_dark, bg)
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
