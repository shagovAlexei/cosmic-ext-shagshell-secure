//! Help → About, a dialog: the support note on top, then the name, version with
//! the update check right under it, what the program is, and buttons to the latest release and
//! the issues.

use crate::app::{APP_ID, Message};
use crate::fl;
use cosmic::Element;
use cosmic::iced::Alignment;
use cosmic::widget;

const REPO: &str = "https://github.com/shagovAlexei/cosmic-ext-shagshell-secure";

/// Room the dialog's title, Close row and paddings take beside the scrolled content.
const CHROME: f32 = 200.0;

/// Sized to the window: in a small one the content scrolls and Close stays in sight.
pub fn view(note: Option<&str>) -> Element<'_, Message> {
    widget::responsive(move |size| {
        widget::container(dialog(note, (size.height - CHROME).max(80.0)))
            .center(cosmic::iced::Length::Fill)
            .into()
    })
    .into()
}

fn dialog(note: Option<&str>, max_height: f32) -> Element<'_, Message> {
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
            widget::row::with_children(vec![
                widget::button::standard(fl!("menu-check-updates"))
                    .on_press(Message::Action(Action::CheckUpdates))
                    .into(),
                widget::button::standard(fl!("menu-changelog"))
                    .on_press(Message::Action(Action::Changelog))
                    .into(),
            ])
            .spacing(8),
        )
        .push_maybe(note.map(widget::text::body))
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
    widget::dialog()
        .title(fl!("menu-about"))
        .control(
            widget::container(widget::scrollable(content).width(cosmic::iced::Length::Fill))
                .max_height(max_height),
        )
        .primary_action(widget::button::standard(fl!("close")).on_press(Message::CloseAbout))
        .into()
}

/// Help → Changelog: the newest releases of `CHANGELOG.md` (built in), each with what it brought.
pub fn changelog() -> Element<'static, Message> {
    use cosmic::iced::Length;
    static TEXT: &str = include_str!("../../../CHANGELOG.md");
    let mut col = widget::column::with_capacity(64).spacing(4);
    for (head, items) in shagshell_core::update::releases(TEXT, 40) {
        col = col.push(widget::text::heading(head));
        for item in items {
            col = col.push(widget::text::body(format!("• {}", item.replace('`', ""))));
        }
        col = col.push(widget::Space::new().height(Length::Fixed(8.0)));
    }
    col = col.push(widget::text::caption(fl!("changelog-older")));
    widget::dialog()
        .title(fl!("menu-changelog"))
        .width(Length::Fixed(760.0))
        .max_width(760.0)
        .control(widget::scrollable(col.padding([0, 12, 0, 0])).height(Length::Fixed(480.0)))
        .primary_action(
            widget::button::suggested(fl!("close"))
                .on_press(Message::Action(crate::menu::Action::Changelog)),
        )
        .into()
}
