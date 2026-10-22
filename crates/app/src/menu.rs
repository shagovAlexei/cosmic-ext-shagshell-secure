//! Header menu bar as in SecureCRT, and `Action`: every user action, whatever triggers it
//! (menu, key, toolbar, tab menu, later Button Bar "Menu Function").

use crate::app::Message;
use crate::fl;
use cosmic::Element;
use cosmic::iced::keyboard::Key;
use cosmic::iced::keyboard::key::Named;
use cosmic::widget::menu::key_bind::Modifier;
use cosmic::widget::menu::{self, ItemHeight, ItemWidth, KeyBind};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    Exit,
    ToggleSessionManager,
    About,
    GlobalOptions,
    NewLocalShell,
    CloseTab,
    Clone,
    NextTab,
    PrevTab,
    /// Alt+1…9: the n-th tab (0-based) of the focused group.
    SelectTab(u8),
    SplitRight,
    SplitDown,
    Copy,
    Paste,
    SelectAll,
    /// The tab's program ended: start it again.
    Reconnect,
    // Shown, not working yet (later phases).
    Connect,
    QuickConnect,
    Disconnect,
    ConnectSftp,
    SessionOptions,
    Pending,
}

impl menu::Action for Action {
    type Message = Message;
    fn message(&self) -> Message {
        Message::Action(*self)
    }
}

/// The keys the menu shows; `app::route_key` is what acts on them.
fn key_binds() -> HashMap<KeyBind, Action> {
    use Modifier::{Alt, Ctrl, Shift};
    let bind = |modifiers: Vec<Modifier>, key: Key, a| (KeyBind { modifiers, key }, a);
    let c = |s: &str| Key::Character(s.into());
    HashMap::from([
        bind(vec![Alt], c("m"), Action::ToggleSessionManager),
        bind(vec![Ctrl, Shift], c("t"), Action::NewLocalShell),
        bind(vec![Ctrl, Shift], c("w"), Action::CloseTab),
        bind(vec![Ctrl, Shift], c("c"), Action::Copy),
        bind(vec![Ctrl, Shift], c("v"), Action::Paste),
        bind(vec![Ctrl], Key::Named(Named::Tab), Action::NextTab),
        bind(vec![Ctrl, Shift], Key::Named(Named::Tab), Action::PrevTab),
    ])
}

/// `tabs`: any tab open (tab items enabled).
pub fn bar(session_manager: bool, tabs: bool) -> Element<'static, Message> {
    use menu::Item::{Button, ButtonDisabled, CheckBox, Divider};
    let off = |label: String| ButtonDisabled(label, None, Action::Pending);
    let tab = |label: String, a| match tabs {
        true => Button(label, None, a),
        false => ButtonDisabled(label, None, a),
    };
    let roots = [
        (
            fl!("menu-file"),
            vec![
                ButtonDisabled(fl!("menu-connect"), None, Action::Connect),
                ButtonDisabled(fl!("menu-quick-connect"), None, Action::QuickConnect),
                Button(fl!("menu-new-local-shell"), None, Action::NewLocalShell),
                tab(fl!("menu-reconnect"), Action::Reconnect),
                ButtonDisabled(fl!("menu-disconnect"), None, Action::Disconnect),
                tab(fl!("menu-close-tab"), Action::CloseTab),
                Divider,
                Button(fl!("menu-exit"), None, Action::Exit),
            ],
        ),
        (
            fl!("menu-edit"),
            vec![
                tab(fl!("menu-copy"), Action::Copy),
                tab(fl!("menu-paste"), Action::Paste),
                tab(fl!("menu-select-all"), Action::SelectAll),
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
                Button(fl!("menu-global-options"), None, Action::GlobalOptions),
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
                tab(fl!("menu-split-right"), Action::SplitRight),
                tab(fl!("menu-split-down"), Action::SplitDown),
                Divider,
                tab(fl!("menu-next-tab"), Action::NextTab),
                tab(fl!("menu-prev-tab"), Action::PrevTab),
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
        .item_width(ItemWidth::Uniform(320))
        .spacing(4.0)
        .into()
}
