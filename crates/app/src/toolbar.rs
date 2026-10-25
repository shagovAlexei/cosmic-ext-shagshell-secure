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

/// The buttons the toolbar can show, in their order: (name in the config, icon, action, label).
pub type Tool = (&'static str, &'static str, Action, fn() -> String);
pub const TOOLS: [Tool; 16] = [
    (
        "SessionManager",
        "view-list-symbolic",
        Action::ToggleSessionManager,
        || fl!("menu-session-manager"),
    ),
    (
        "Connect",
        "network-server-symbolic",
        Action::Connect,
        || fl!("menu-connect"),
    ),
    (
        "QuickConnect",
        "media-playback-start-symbolic",
        Action::QuickConnect,
        || fl!("menu-quick-connect"),
    ),
    (
        "LocalShell",
        "utilities-terminal-symbolic",
        Action::NewLocalShell,
        || fl!("menu-new-local-shell"),
    ),
    (
        "Reconnect",
        "view-refresh-symbolic",
        Action::Reconnect,
        || fl!("menu-reconnect"),
    ),
    (
        "Disconnect",
        "process-stop-symbolic",
        Action::Disconnect,
        || fl!("menu-disconnect"),
    ),
    (
        "Sftp",
        "folder-remote-symbolic",
        Action::ConnectSftp,
        || fl!("menu-connect-sftp"),
    ),
    (
        "SessionOptions",
        "emblem-system-symbolic",
        Action::SessionOptions,
        || fl!("menu-session-options"),
    ),
    ("Copy", "edit-copy-symbolic", Action::Copy, || {
        fl!("menu-copy")
    }),
    ("Paste", "edit-paste-symbolic", Action::Paste, || {
        fl!("menu-paste")
    }),
    (
        "CommandWindow",
        "input-keyboard-symbolic",
        Action::ToggleCommandWindow,
        || fl!("menu-command-window"),
    ),
    (
        "RunScript",
        "system-run-symbolic",
        Action::RunScript,
        || fl!("menu-script-run"),
    ),
    (
        "KeyMap",
        "preferences-desktop-keyboard-shortcuts-symbolic",
        Action::KeyMap,
        || fl!("menu-keymap"),
    ),
    (
        "CreateKey",
        "dialog-password-symbolic",
        Action::CreateKey,
        || fl!("menu-create-key"),
    ),
    (
        "Notes",
        "accessories-text-editor-symbolic",
        Action::Notes,
        || fl!("menu-notes"),
    ),
    (
        "GlobalOptions",
        "preferences-system-symbolic",
        Action::GlobalOptions,
        || fl!("menu-global-options"),
    ),
];

/// The toolbar until the user picks: as it always was.
pub const DEFAULT: [&str; 6] = [
    "Connect",
    "QuickConnect",
    "Reconnect",
    "Disconnect",
    "Sftp",
    "SessionOptions",
];

/// The names shown: the config's list (empty: `DEFAULT`), in `TOOLS` order.
pub fn shown(config: &[String]) -> Vec<&'static str> {
    (TOOLS.iter())
        .map(|t| t.0)
        .filter(|n| match config.is_empty() {
            true => DEFAULT.contains(n),
            false => config.iter().any(|c| c == n),
        })
        .collect()
}

/// `bar`: what is typed in the Connect Bar, `list`: the sessions it matches (shown under it);
/// `tools`: the config's toolbar list.
pub fn view(
    bar: &str,
    list: &[String],
    picked: Option<usize>,
    tools: &[String],
) -> Element<'static, Message> {
    let shown = shown(tools);
    let row = (TOOLS.iter()).filter(|t| shown.contains(&t.0)).map(
        |(_, icon, action, tip)| -> Element<'static, Message> {
            let b = widget::button::icon(widget::icon::from_name(*icon))
                .on_press(Message::Action(*action));
            widget::tooltip(b, widget::text(tip()), tooltip::Position::Bottom).into()
        },
    );
    let mut items = row.collect::<Vec<_>>();
    // SecureCRT's Connect Bar: a session's name or user@host:port, Enter connects.
    items.push(widget::Space::new().width(12.0).into());
    let input = widget::search_input(fl!("connect-bar-hint"), bar.to_string())
        .id(bar_id())
        .on_focus(Message::FocusField(bar_id()))
        .on_input(Message::ConnectBar)
        .on_clear(Message::ConnectBar(String::new()))
        .on_submit(|_| Message::ConnectBarGo)
        .width(cosmic::iced::Length::Fixed(300.0));
    // Always a popover (with or without its list): a field that changed parents would lose its
    // focus, and the next letter with it.
    let mut input = widget::popover(input)
        .position(widget::popover::Position::Bottom)
        .on_close(Message::ConnectBarClose);
    if !list.is_empty() {
        let rows = list.iter().enumerate().map(|(i, id)| {
            widget::button::custom(widget::text::body(id.clone()))
                // The row picked with the arrows: filled, as a hovered one.
                .class(match picked == Some(i) {
                    true => cosmic::theme::Button::Standard,
                    false => cosmic::theme::Button::MenuItem,
                })
                .width(cosmic::iced::Length::Fill)
                .on_press(Message::ConnectBarOpen(id.clone()))
                .into()
        });
        input = input.popup(
            widget::container(
                widget::column::with_children(rows.collect::<Vec<Element<Message>>>())
                    .spacing(2)
                    .width(cosmic::iced::Length::Fixed(300.0)),
            )
            .padding(4)
            .class(cosmic::theme::Container::Dropdown),
        );
    }
    let input: Element<'static, Message> = input.into();
    items.push(input);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_toolbar_shows_the_picked_buttons_in_their_order() {
        assert_eq!(shown(&[]), DEFAULT.to_vec());
        let picked = ["Paste".to_string(), "LocalShell".to_string()];
        assert_eq!(shown(&picked), vec!["LocalShell", "Paste"]);
        assert!(shown(&["none".to_string()]).is_empty()); // all turned off
    }
}
