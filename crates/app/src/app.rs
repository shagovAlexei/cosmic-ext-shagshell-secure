//! The application: state and routing only; each screen area lives in its own module.

use crate::config::{self, Config};
use crate::menu::Action;
use crate::quick_connect::{QcMsg, QuickConnect};
use crate::session_manager::{SessionManager, SmMsg, TreeKey};
use crate::tabs::{self, Tabs, TabsMsg};
use crate::{about, fl, menu, options, status_bar, toolbar};
use cosmic::app::{Core, Task};
use cosmic::cosmic_config::CosmicConfigEntry;
use cosmic::iced::keyboard::{self, Modifiers, key::Code, key::Physical};
use cosmic::iced::{Length, Subscription, event};
use cosmic::widget::{self, about::About};
use cosmic::{Application, Element};

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
    /// ssh's ControlMaster sockets (`launch::ensure_control_dir`).
    control_dir: std::path::PathBuf,
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
    Sessions(SmMsg),
    QuickConnect(QcMsg),
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
        core.window.header_title = fl!("app-title");
        let config_handler = config::handler();
        let config: Config = config::read(config_handler.as_ref());
        let control_dir = shagshell_core::launch::ensure_control_dir()
            .inspect_err(|e| log::warn!("ssh control dir: {e}"))
            .unwrap_or_else(|_| shagshell_core::launch::control_dir());
        let store =
            shagshell_core::session::Store::new(shagshell_core::session::store::default_root());
        let sessions = SessionManager::new(store, &config.expanded, config.session_manager_width);
        let app = Self {
            core,
            config,
            config_handler,
            about: about::about(),
            drawer: None,
            tabs: Tabs::default(),
            sessions,
            quick: None,
            control_dir,
        };
        (app, Task::none())
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Action(a) => return self.act(a),
            Message::Config(c) => self.config = c,
            Message::SetConfig(c) => {
                self.config = c;
                self.save_config();
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
                return self.tabs.update(m, &look());
            }
            Message::TermTx(tx) => self.tabs.set_sender(tx),
            Message::Term(id, e) => return self.tabs.on_term(id, e),
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
                    tasks.push(self.quick_connect(Some(folder)));
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
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            // Not a held key's repeat. The app's keys win wherever focus is (a text field would
            // take Alt+Q as a «q»; the terminal lets them through anyway); Enter and the tree's
            // keys only when no widget took them.
            event::listen_with(|e, status, _| match e {
                cosmic::iced::Event::Keyboard(keyboard::Event::KeyPressed {
                    physical_key,
                    modifiers,
                    repeat: false,
                    ..
                }) => match key_action(&physical_key, modifiers) {
                    Some(a) => Some(Message::Action(a)),
                    None if status == event::Status::Ignored => route_key(&physical_key, modifiers),
                    None => None,
                },
                _ => None,
            }),
            // Ctrl / Shift for clicks in the session tree.
            event::listen_with(|e, _, _| match e {
                cosmic::iced::Event::Keyboard(keyboard::Event::ModifiersChanged(m)) => {
                    Some(Message::Sessions(SmMsg::Mods(m)))
                }
                _ => None,
            }),
            // A row (drag) or the splitter is held: where the pointer goes, where it comes up.
            match self.sessions.holding() {
                true => event::listen_with(|e, _, _| {
                    use cosmic::iced::mouse::{Button, Event as M};
                    match e {
                        cosmic::iced::Event::Mouse(M::ButtonReleased(Button::Left)) => {
                            Some(Message::Sessions(SmMsg::Release))
                        }
                        cosmic::iced::Event::Mouse(M::CursorMoved { position }) => {
                            Some(Message::Sessions(SmMsg::Cursor(position.x)))
                        }
                        _ => None,
                    }
                }),
                false => Subscription::none(),
            },
            crate::term::subscription(),
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
            Drawer::Options => Some(options::view(&self.config)),
        }
    }

    /// Popups (menus) are windows too; libcosmic draws them itself, only asking here.
    fn view_window(&self, _: cosmic::iced::window::Id) -> Element<'_, Message> {
        widget::Space::new().into()
    }

    fn dialog(&self) -> Option<Element<'_, Message>> {
        (self.tabs.dialog())
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
        let center = match self.tabs.view(&self.config, window) {
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
            Action::Exit => return cosmic::iced::exit(),
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
            Action::QuickConnect => return self.quick_connect(None),
            Action::ImportSshConfig => self.import_ssh_config(),
            Action::About => self.toggle_drawer(Drawer::About),
            Action::GlobalOptions => self.toggle_drawer(Drawer::Options),
            _ => {}
        }
        Task::none()
    }

    /// Each session in a tab of its own; one that can't start says why under the tree.
    fn open_sessions(&mut self, ids: &[String]) -> Task<Message> {
        let mut tasks = Vec::new();
        for id in ids {
            match self.sessions.store.get(id) {
                Ok(s) => {
                    let name = id.rsplit_once('/').map_or(id.as_str(), |(_, n)| n);
                    match self.ssh_launch(&s, name.to_string()) {
                        Ok(l) => tasks.push(self.tabs.open(l, &look())),
                        Err(e) => self.sessions.notice = Some(e),
                    }
                }
                Err(e) => self.sessions.notice = Some(format!("{id}: {e}")),
            }
        }
        Task::batch(tasks)
    }

    fn ssh_launch(
        &self,
        s: &shagshell_core::session::Session,
        title: String,
    ) -> Result<tabs::Launch, String> {
        let home = std::env::home_dir().unwrap_or_default();
        let mut argv = shagshell_core::launch::ssh_argv(s, &self.control_dir, &home)
            .map_err(|_| fl!("bad-extra"))?;
        let program = argv.remove(0);
        Ok(tabs::Launch {
            program,
            args: argv,
            title,
            protocol: tabs::Protocol::Ssh {
                host: s.host.clone(),
            },
        })
    }

    fn quick_connect(&mut self, folder: Option<String>) -> Task<Message> {
        let q = QuickConnect::new(folder);
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
        if let Some((folder, name)) = &req.save_as {
            let store = &self.sessions.store;
            let saved = store
                .create(folder, name, &req.session.host)
                .and_then(|id| {
                    let mut s = store.get(&id)?;
                    (s.port, s.user) = (req.session.port, req.session.user.clone());
                    store.save(&id, &s).map(|()| id)
                });
            match saved {
                Ok(id) => {
                    self.sessions.select(&id);
                    title = name.clone();
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
        match self.ssh_launch(&req.session, title) {
            Ok(l) => self.tabs.open(l, &look()),
            Err(e) => {
                self.sessions.notice = Some(e);
                Task::none()
            }
        }
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

/// The terminals' look: cosmic-term's style, the theme's darkness and window background.
fn look() -> tabs::Look {
    let theme = cosmic::theme::active();
    let c = theme.cosmic();
    let bg = c.bg_color();
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    let bg = 0xFF00_0000 | byte(bg.red) << 16 | byte(bg.green) << 8 | byte(bg.blue);
    (Default::default(), c.is_dark, bg)
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
