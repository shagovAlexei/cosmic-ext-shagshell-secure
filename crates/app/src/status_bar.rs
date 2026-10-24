//! Status bar at the bottom: the active tab's state, protocol and cols×rows.

use crate::app::Message;
use crate::fl;
use cosmic::Element;
use cosmic::widget;

/// A tab's state: connected, protocol, a terminal's cols × rows.
pub type Status = (bool, String, Option<(usize, usize)>, bool, bool);

/// `tab`: the active tab's (connected, protocol, a terminal's cols × rows); none open:
/// «Not connected».
pub fn view(tab: Option<Status>, notice: Option<&str>) -> Element<'_, Message> {
    let text = match tab {
        None => fl!("status-disconnected"),
        Some((alive, protocol, size, logging, recording)) => {
            let state = match alive {
                true => fl!("status-connected"),
                false => fl!("status-ended"),
            };
            let mut text = match size {
                Some((cols, rows)) => format!("{state} · {protocol} · {cols}×{rows}"),
                None => format!("{state} · {protocol}"),
            };
            if logging {
                text.push_str(" · ● ");
                text.push_str(&fl!("status-logging"));
            }
            if recording {
                text.push_str(" · ● ");
                text.push_str(&fl!("status-recording"));
            }
            text
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
