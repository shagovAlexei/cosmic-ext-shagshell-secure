//! Header menu bar as in SecureCRT, and `Action`: every user action, whatever triggers it
//! (menu, key, toolbar, later Button Bar "Menu Function").

use crate::app::Message;
use crate::fl;
use cosmic::Element;
use cosmic::iced::keyboard::Key;
use cosmic::widget::menu::key_bind::Modifier;
use cosmic::widget::menu::{self, ItemHeight, ItemWidth, KeyBind};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    Exit,
    ToggleSessionManager,
    About,
    // Shown, not working yet (later phases).
    Connect,
    QuickConnect,
    Reconnect,
    Disconnect,
    ConnectSftp,
    SessionOptions,
    GlobalOptions,
    Pending,
}

impl menu::Action for Action {
    type Message = Message;
    fn message(&self) -> Message {
        Message::Action(*self)
    }
}

/// The keys the menu shows. Only Alt+M works in this phase (`app::route_key`).
fn key_binds() -> HashMap<KeyBind, Action> {
    HashMap::from([(
        KeyBind {
            modifiers: vec![Modifier::Alt],
            key: Key::Character("m".into()),
        },
        Action::ToggleSessionManager,
    )])
}

pub fn bar(session_manager: bool) -> Element<'static, Message> {
    use menu::Item::{Button, ButtonDisabled, CheckBox, Divider};
    let off = |label: String| ButtonDisabled(label, None, Action::Pending);
    let roots = [
        (
            fl!("menu-file"),
            vec![
                ButtonDisabled(fl!("menu-connect"), None, Action::Connect),
                ButtonDisabled(fl!("menu-quick-connect"), None, Action::QuickConnect),
                ButtonDisabled(fl!("menu-reconnect"), None, Action::Reconnect),
                ButtonDisabled(fl!("menu-disconnect"), None, Action::Disconnect),
                Divider,
                Button(fl!("menu-exit"), None, Action::Exit),
            ],
        ),
        (
            fl!("menu-edit"),
            vec![
                off(fl!("menu-copy")),
                off(fl!("menu-paste")),
                off(fl!("menu-select-all")),
                off(fl!("menu-find")),
            ],
        ),
        (
            fl!("menu-view"),
            vec![
                CheckBox(
                    fl!("menu-session-manager"),
                    None,
                    session_manager,
                    Action::ToggleSessionManager,
                ),
                off(fl!("menu-toolbar")),
                off(fl!("menu-status-bar")),
            ],
        ),
        (
            fl!("menu-options"),
            vec![
                ButtonDisabled(fl!("menu-session-options"), None, Action::SessionOptions),
                ButtonDisabled(fl!("menu-global-options"), None, Action::GlobalOptions),
            ],
        ),
        (
            fl!("menu-transfer"),
            vec![
                ButtonDisabled(fl!("menu-connect-sftp"), None, Action::ConnectSftp),
                off(fl!("menu-send-file")),
            ],
        ),
        (
            fl!("menu-script"),
            vec![off(fl!("menu-script-run")), off(fl!("menu-script-cancel"))],
        ),
        (
            fl!("menu-tools"),
            vec![off(fl!("menu-create-key")), off(fl!("menu-keymap"))],
        ),
        (
            fl!("menu-window"),
            vec![
                off(fl!("menu-tile-h")),
                off(fl!("menu-tile-v")),
                off(fl!("menu-next-tab")),
                off(fl!("menu-prev-tab")),
            ],
        ),
        (
            fl!("menu-help"),
            vec![Button(fl!("menu-about"), None, Action::About)],
        ),
    ];
    let binds = key_binds();
    let roots = roots
        .into_iter()
        .map(|(title, items)| {
            menu::Tree::with_children(Element::from(menu::root(title)), menu::items(&binds, items))
        })
        .collect();
    menu::bar(roots)
        .item_height(ItemHeight::Dynamic(40))
        .item_width(ItemWidth::Uniform(280))
        .spacing(4.0)
        .into()
}
