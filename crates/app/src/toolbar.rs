//! Toolbar under the header, as in SecureCRT. All buttons wait for later phases (no `on_press`).

use crate::app::Message;
use crate::fl;
use cosmic::Element;
use cosmic::widget::{self, tooltip};

pub fn view() -> Element<'static, Message> {
    let buttons = [
        ("network-server-symbolic", fl!("menu-connect")),
        ("media-playback-start-symbolic", fl!("menu-quick-connect")),
        ("view-refresh-symbolic", fl!("menu-reconnect")),
        ("process-stop-symbolic", fl!("menu-disconnect")),
        ("folder-remote-symbolic", fl!("menu-connect-sftp")),
        ("emblem-system-symbolic", fl!("menu-session-options")),
    ];
    let row = buttons
        .into_iter()
        .map(|(icon, tip)| -> Element<'static, Message> {
            let b = widget::button::icon(widget::icon::from_name(icon));
            widget::tooltip(b, widget::text(tip), tooltip::Position::Bottom).into()
        });
    widget::row::with_children(row.collect::<Vec<_>>())
        .spacing(4)
        .padding([4, 8])
        .into()
}
