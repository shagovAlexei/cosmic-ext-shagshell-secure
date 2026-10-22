//! Status bar at the bottom: the active tab's state, protocol and cols×rows.

use crate::app::Message;
use crate::fl;
use cosmic::Element;
use cosmic::widget;

/// `tab`: the active tab's (connected, cols, rows); none open: «Not connected».
pub fn view(tab: Option<(bool, usize, usize)>) -> Element<'static, Message> {
    let text = match tab {
        None => fl!("status-disconnected"),
        Some((alive, cols, rows)) => {
            let state = match alive {
                true => fl!("status-connected"),
                false => fl!("status-ended"),
            };
            format!("{state} · {} · {cols}×{rows}", fl!("tab-local-shell"))
        }
    };
    widget::container(widget::text::caption(text))
        .padding([2, 8])
        .into()
}
