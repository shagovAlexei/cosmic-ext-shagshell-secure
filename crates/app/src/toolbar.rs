//! Toolbar under the header, as in SecureCRT. A button without an action waits for a later phase.

use crate::app::Message;
use crate::fl;
use crate::menu::Action;
use cosmic::Element;
use cosmic::widget::{self, tooltip};

/// The Connect Bar's field (focused through it, so the terminal lets go of the keys).
pub fn bar_id() -> widget::Id {
    static ID: std::sync::LazyLock<widget::Id> = std::sync::LazyLock::new(widget::Id::unique);
    ID.clone()
}

/// `bar`: what is typed in the Connect Bar.
pub fn view(bar: &str) -> Element<'static, Message> {
    let buttons = [
        (
            "network-server-symbolic",
            fl!("menu-connect"),
            Some(Action::Connect),
        ),
        (
            "media-playback-start-symbolic",
            fl!("menu-quick-connect"),
            Some(Action::QuickConnect),
        ),
        (
            "view-refresh-symbolic",
            fl!("menu-reconnect"),
            Some(Action::Reconnect),
        ),
        (
            "process-stop-symbolic",
            fl!("menu-disconnect"),
            Some(Action::Disconnect),
        ),
        (
            "folder-remote-symbolic",
            fl!("menu-connect-sftp"),
            Some(Action::ConnectSftp),
        ),
        (
            "emblem-system-symbolic",
            fl!("menu-session-options"),
            Some(Action::SessionOptions),
        ),
    ];
    let row = buttons
        .into_iter()
        .map(|(icon, tip, action)| -> Element<'static, Message> {
            let b = widget::button::icon(widget::icon::from_name(icon))
                .on_press_maybe(action.map(Message::Action));
            widget::tooltip(b, widget::text(tip), tooltip::Position::Bottom).into()
        });
    let mut items = row.collect::<Vec<_>>();
    // SecureCRT's Connect Bar: a session's name or user@host:port, Enter connects.
    items.push(widget::Space::new().width(12.0).into());
    items.push(
        widget::search_input(fl!("connect-bar-hint"), bar.to_string())
            .id(bar_id())
            .on_focus(Message::FocusField(bar_id()))
            .on_input(Message::ConnectBar)
            .on_clear(Message::ConnectBar(String::new()))
            .on_submit(|_| Message::ConnectBarGo)
            .width(cosmic::iced::Length::Fixed(300.0))
            .into(),
    );
    widget::row::with_children(items)
        .spacing(4)
        .padding([4, 8])
        .align_y(cosmic::iced::Alignment::Center)
        .into()
}

/// An icon button with a hint on hover: for strips that are often narrow (a note, a queue, a
/// console), where a row of text buttons does not fit. `msg == None`: the button is disabled.
pub fn tip_button<'a>(
    icon: &'static str,
    tip: String,
    msg: Option<crate::app::Message>,
) -> cosmic::Element<'a, crate::app::Message> {
    use cosmic::widget;
    widget::tooltip(
        widget::button::icon(widget::icon::from_name(icon)).on_press_maybe(msg),
        widget::text::caption(tip),
        widget::tooltip::Position::Bottom,
    )
    .into()
}
