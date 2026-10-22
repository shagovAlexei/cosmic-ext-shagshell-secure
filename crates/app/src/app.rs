//! The application: state and routing only; each screen area lives in its own module.

use crate::config::{self, Config};
use crate::menu::Action;
use crate::{about, fl, menu, session_manager, status_bar, toolbar};
use cosmic::app::{Core, Task};
use cosmic::cosmic_config::CosmicConfigEntry;
use cosmic::iced::keyboard::{self, key::Code, key::Physical};
use cosmic::iced::{Length, Subscription, event};
use cosmic::widget::{self, about::About};
use cosmic::{Application, Element};

pub const APP_ID: &str = "io.github.shagovalexei.ShagShellSecure";

pub struct App {
    core: Core,
    config: Config,
    config_handler: Option<cosmic::cosmic_config::Config>,
    about: About,
}

#[derive(Clone, Debug)]
pub enum Message {
    Action(Action),
    Config(Config),
    OpenUrl(String),
    CloseDrawer,
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
        };
        (app, Task::none())
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Action(Action::Exit) => return cosmic::iced::exit(),
            Message::Action(Action::ToggleSessionManager) => self.toggle_session_manager(),
            Message::Action(Action::About) => self.core.window.show_context = true,
            Message::Action(_) => {}
            Message::Config(c) => self.config = c,
            Message::OpenUrl(url) => {
                match std::process::Command::new("xdg-open").arg(url).spawn() {
                    // Reaped off the UI thread, or each click leaves a zombie until exit.
                    Ok(mut child) => drop(std::thread::spawn(move || child.wait())),
                    Err(e) => log::warn!("xdg-open: {e}"),
                }
            }
            Message::CloseDrawer => self.core.window.show_context = false,
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            // Not a key a focused widget took (the terminal's Meta-m later), not a held key's repeat.
            event::listen_with(|e, status, _| match e {
                cosmic::iced::Event::Keyboard(keyboard::Event::KeyPressed {
                    physical_key,
                    modifiers,
                    repeat: false,
                    ..
                }) if status == event::Status::Ignored => {
                    route_key(physical_key, modifiers).map(Message::Action)
                }
                _ => None,
            }),
            self.core()
                .watch_config::<Config>(APP_ID)
                .map(|u| Message::Config(u.config)),
        ])
    }

    fn context_drawer(&self) -> Option<cosmic::app::context_drawer::ContextDrawer<'_, Message>> {
        self.core
            .window
            .show_context
            .then(|| about::view(&self.about))
    }

    fn header_start(&self) -> Vec<Element<'_, Message>> {
        vec![menu::bar(self.config.session_manager_visible)]
    }

    fn view(&self) -> Element<'_, Message> {
        let placeholder = widget::container(
            widget::column::with_children(vec![
                widget::text::title3(fl!("no-sessions")).into(),
                widget::text(fl!("no-sessions-hint")).into(),
            ])
            .spacing(8)
            .align_x(cosmic::iced::Alignment::Center),
        )
        .center(Length::Fill);
        let mut middle = widget::row::with_capacity(2).spacing(8);
        if self.config.session_manager_visible {
            middle = middle.push(session_manager::view());
        }
        widget::column::with_children(vec![
            toolbar::view(),
            middle.push(placeholder).height(Length::Fill).into(),
            status_bar::view(),
        ])
        .into()
    }
}

impl App {
    fn toggle_session_manager(&mut self) {
        self.config.session_manager_visible = !self.config.session_manager_visible;
        if let Some(h) = &self.config_handler
            && let Err(e) = self.config.write_entry(h)
        {
            log::warn!("config: {e}");
        }
    }
}

/// App keys, by physical key so they work in any layout (Alt+M in the Russian one too).
fn route_key(key: Physical, mods: keyboard::Modifiers) -> Option<Action> {
    match key {
        Physical::Code(Code::KeyM) if mods == keyboard::Modifiers::ALT => {
            Some(Action::ToggleSessionManager)
        }
        _ => None,
    }
}
