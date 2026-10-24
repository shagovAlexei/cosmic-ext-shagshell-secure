//! Toolbar under the header, as in SecureCRT. A button without an action waits for a later phase.

use crate::app::Message;
use crate::fl;
use crate::menu::Action;
use cosmic::Element;
use cosmic::widget::{self, tooltip};

pub fn view() -> Element<'static, Message> {
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
    widget::row::with_children(row.collect::<Vec<_>>())
        .spacing(4)
        .padding([4, 8])
        .into()
}
