//! Help → About, in the context drawer: the support note on top, then the name, version with
//! the update check right under it, what the program is, and buttons to the latest release and
//! the issues.

use crate::app::{APP_ID, Message};
use crate::fl;
use cosmic::app::context_drawer::{self, ContextDrawer};
use cosmic::iced::Alignment;
use cosmic::widget;

const REPO: &str = "https://github.com/shagovAlexei/cosmic-ext-shagshell-secure";

pub fn view<'a>() -> ContextDrawer<'a, Message> {
    use crate::menu::Action;
    let mut col = widget::column::with_capacity(12)
        .spacing(16)
        .align_x(Alignment::Center);
    if crate::updates::DONATE_URL.is_some() {
        col = col
            .push(widget::text::body(fl!("up-support")))
            .push(
                widget::button::suggested(fl!("up-donate"))
                    .on_press(Message::Action(Action::Donate)),
            )
            .push(widget::divider::horizontal::default());
    }
    let button = |label: String, icon: &'static str, url: String| {
        widget::button::standard(label)
            .leading_icon(widget::icon::from_name(icon).size(16))
            .on_press(Message::OpenUrl(url))
    };
    let content = col
        .push(widget::icon::from_name(APP_ID).size(96))
        .push(widget::text::title3(fl!("app-title")))
        .push(widget::text::body(format!(
            "{} {}",
            fl!("about-version"),
            env!("CARGO_PKG_VERSION")
        )))
        .push(
            widget::button::standard(fl!("menu-check-updates"))
                .on_press(Message::Action(Action::CheckUpdates)),
        )
        .push(widget::text::body(fl!("about-comments")))
        .push(widget::text::caption(format!(
            "{} Shagov Alexei · GPL-3.0-only",
            fl!("about-author")
        )))
        .push(
            widget::row::with_children(vec![
                button(
                    fl!("about-release"),
                    "software-update-available-symbolic",
                    format!("{REPO}/releases/latest"),
                )
                .into(),
                button(
                    fl!("about-issues"),
                    "dialog-warning-symbolic",
                    format!("{REPO}/issues"),
                )
                .into(),
            ])
            .spacing(8),
        );
    context_drawer::context_drawer(
        widget::scrollable(content).width(cosmic::iced::Length::Fill),
        Message::CloseDrawer,
    )
    .title(fl!("menu-about"))
}
