//! The application: state and routing only; each screen area lives in its own module.

use crate::config::{self, Config};
use crate::menu::Action;
use crate::tabs::{self, Tabs, TabsMsg};
use crate::{about, fl, menu, options, session_manager, status_bar, toolbar};
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
        let app = Self {
            core,
            config: config::read(config_handler.as_ref()),
            config_handler,
            about: about::about(),
            drawer: None,
            tabs: Tabs::default(),
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
            Message::Tabs(m) => return self.tabs.update(m, &look()),
            Message::TermTx(tx) => self.tabs.set_sender(tx),
            Message::Term(id, e) => return self.tabs.on_term(id, e),
            Message::EnterKey if self.tabs.enter_on_dead() => return self.act(Action::Reconnect),
            Message::EnterKey => {}
            Message::Surface(a) => {
                return cosmic::task::message(cosmic::Action::Cosmic(
                    cosmic::app::Action::Surface(a),
                ));
            }
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            // Not a key a focused widget took (the terminal takes all but `tabs::is_app_key`),
            // not a held key's repeat.
            event::listen_with(|e, status, _| match e {
                cosmic::iced::Event::Keyboard(keyboard::Event::KeyPressed {
                    physical_key,
                    modifiers,
                    repeat: false,
                    ..
                }) if status == event::Status::Ignored => route_key(&physical_key, modifiers),
                _ => None,
            }),
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
        self.tabs.dialog()
    }

    fn header_start(&self) -> Vec<Element<'_, Message>> {
        vec![menu::bar(
            self.config.session_manager_visible,
            self.tabs.menu_state(),
        )]
    }

    fn view(&self) -> Element<'_, Message> {
        let center = match self.tabs.view(
            &self.config,
            self.core
                .main_window_id()
                .unwrap_or(cosmic::iced::window::Id::RESERVED),
        ) {
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
        let mut middle = widget::row::with_capacity(2).spacing(8);
        if self.config.session_manager_visible {
            middle = middle.push(session_manager::view());
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
            Action::About => self.toggle_drawer(Drawer::About),
            Action::GlobalOptions => self.toggle_drawer(Drawer::Options),
            _ => {}
        }
        Task::none()
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

/// Keys reaching the app (the terminal took none of them): Enter restarts an ended tab.
fn route_key(key: &Physical, mods: Modifiers) -> Option<Message> {
    match (key, mods) {
        (Physical::Code(Code::Enter | Code::NumpadEnter), m) if m.is_empty() => {
            Some(Message::EnterKey)
        }
        _ => key_action(key, mods).map(Message::Action),
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
