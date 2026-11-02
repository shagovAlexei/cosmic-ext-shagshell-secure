//! Status bar at the bottom: the active tab's state, protocol, cols×rows, cursor, time up, log.

use crate::app::Message;
use crate::fl;
use cosmic::Element;
use cosmic::iced::Length;
use cosmic::iced::alignment::Horizontal;
use cosmic::widget;
use shagshell_core::status_items::{Item, Layout, clock_text, up_text};

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

/// `tab`: the active tab's; none open: «Not connected». `layout`: the items to show, in order.
pub fn view<'a>(
    tab: Option<Status>,
    layout: &Layout,
    notice: Option<&'a str>,
) -> Element<'a, Message> {
    let text = match &tab {
        None if !layout.items.contains(&Item::Clock) => fl!("status-disconnected"),
        _ => layout
            .items
            .iter()
            .filter_map(|i| item_text(*i, tab.as_ref(), layout.seconds))
            .collect::<Vec<_>>()
            .join(" · "),
    };
    let mut row = widget::row::with_capacity(2)
        .spacing(24)
        .push(widget::text::body(text));
    // What the last action had to say (an error, why it did nothing), as the tree used to.
    if let Some(n) = notice {
        row = row.push(widget::text::body(n.to_string()).class(cosmic::theme::Text::Accent));
    }
    let bar = widget::container(row).padding([6, 6]);
    match layout.right {
        true => bar.width(Length::Fill).align_x(Horizontal::Right).into(),
        false => bar.into(),
    }
}

/// One item's text; none when it has nothing to say (no log, no size, ...).
fn item_text(item: Item, tab: Option<&Status>, seconds: bool) -> Option<String> {
    if item == Item::Clock {
        return Some(clock_text(seconds));
    }
    let s = tab?;
    match item {
        Item::State => Some(match s.alive {
            true => fl!("status-connected"),
            false => fl!("status-ended"),
        }),
        Item::Protocol => Some(s.protocol.clone()),
        Item::Size => s.size.map(|(cols, rows)| format!("{cols}×{rows}")),
        Item::Cursor => s
            .cursor
            .map(|(line, col)| fl!("status-cursor", line = line, col = col)),
        Item::Up => s.up.map(|up| format!("⏱ {}", up_text(up, seconds))),
        Item::Log => s
            .log
            .as_ref()
            .map(|l| format!("● {} {l}", fl!("status-logging"))),
        Item::Recording => s
            .recording
            .then(|| format!("● {}", fl!("status-recording"))),
        Item::Clock | Item::Unknown => None,
    }
}
