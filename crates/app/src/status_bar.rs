//! Status bar at the bottom: the active tab's state, protocol and cols×rows.

use crate::app::Message;
use crate::fl;
use cosmic::Element;
use cosmic::widget;

/// A tab's state: connected, protocol, a terminal's cols × rows.
pub type Status = (bool, String, Option<(usize, usize)>);

/// `tab`: the active tab's (connected, protocol, a terminal's cols × rows); none open:
/// «Not connected».
pub fn view(tab: Option<Status>) -> Element<'static, Message> {
    let text = match tab {
        None => fl!("status-disconnected"),
        Some((alive, protocol, size)) => {
            let state = match alive {
                true => fl!("status-connected"),
                false => fl!("status-ended"),
            };
            match size {
                Some((cols, rows)) => format!("{state} · {protocol} · {cols}×{rows}"),
                None => format!("{state} · {protocol}"),
            }
        }
    };
    widget::container(widget::text::caption(text))
        .padding([2, 8])
        .into()
}
