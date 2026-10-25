//! Status bar at the bottom: the active tab's state, protocol, cols×rows, cursor, time up, log.

use crate::app::Message;
use crate::fl;
use cosmic::Element;
use cosmic::widget;

/// What the status bar says about the active tab.
pub struct Status {
    pub alive: bool,
    pub protocol: String,
    /// A terminal's cols × rows.
    pub size: Option<(usize, usize)>,
    /// A terminal's cursor: line, column (from 1).
    pub cursor: Option<(usize, usize)>,
    /// Seconds since it connected, while it is.
    pub up: Option<u64>,
    /// The log file's name, while logging.
    pub log: Option<String>,
    pub recording: bool,
}

/// `tab`: the active tab's; none open: «Not connected».
pub fn view(tab: Option<Status>, notice: Option<&str>) -> Element<'_, Message> {
    let text = match tab {
        None => fl!("status-disconnected"),
        Some(s) => {
            let mut parts = vec![
                match s.alive {
                    true => fl!("status-connected"),
                    false => fl!("status-ended"),
                },
                s.protocol,
            ];
            if let Some((cols, rows)) = s.size {
                parts.push(format!("{cols}×{rows}"));
            }
            if let Some((line, col)) = s.cursor {
                parts.push(fl!("status-cursor", line = line, col = col));
            }
            if let Some(up) = s.up {
                parts.push(format!("⏱ {}", shagshell_core::panel::duration_text(up)));
            }
            if let Some(log) = s.log {
                parts.push(format!("● {} {log}", fl!("status-logging")));
            }
            if s.recording {
                parts.push(format!("● {}", fl!("status-recording")));
            }
            parts.join(" · ")
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
