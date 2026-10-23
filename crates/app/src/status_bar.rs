//! Status bar at the bottom: the active tab's state, protocol and cols×rows.

use crate::app::Message;
use crate::fl;
use cosmic::Element;
use cosmic::widget;

/// A tab's state: connected, protocol, a terminal's cols × rows.
pub type Status = (bool, String, Option<(usize, usize)>);

/// `tab`: the active tab's (connected, protocol, a terminal's cols × rows); none open:
/// «Not connected».
pub fn view(tab: Option<Status>, notice: Option<&str>) -> Element<'_, Message> {
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
    let mut row = widget::row::with_capacity(2)
        .spacing(24)
        .push(widget::text::body(text));
    // What the last action had to say (an error, why it did nothing), as the tree used to.
    if let Some(n) = notice {
        row = row.push(widget::text::body(n.to_string()).class(cosmic::theme::Text::Accent));
    }
    widget::container(row).padding([6, 12]).into()
}
