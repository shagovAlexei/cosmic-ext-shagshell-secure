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
    /// View → Command Window.
    ToggleCommandWindow,
    /// View → Button Bar.
    ToggleButtonBar,
    /// File → Log Session.
    ToggleLog,
    About,
    GlobalOptions,
    NewLocalShell,
    CloseTab,
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
    /// Alt+C: the session tree, filter focused (SecureCRT's Connect dialog).
    Connect,
    QuickConnect,
    ImportSshConfig,
    /// File → Import from SecureCRT….
    ImportSecureCrt,
    SessionOptions,
    /// Options → Edit Default Session…: the template new sessions copy.
    EditDefault,
    /// Tools → Credentials Manager….
    Credentials,
    /// Script → Run….
    RunScript,
    /// Script → Cancel.
    CancelScript,
    // Shown, not working yet (later phases).
    Disconnect,
    ConnectSftp,
    Pending,
}

/// The functions a Button Bar button can call: the name in `buttonbars.toml` and its label.
pub type NamedFn = (&'static str, fn() -> String);
pub const NAMED: [NamedFn; 13] = [
    ("Connect", || fl!("menu-connect")),
    ("QuickConnect", || fl!("menu-quick-connect")),
    ("NewLocalShell", || fl!("menu-new-local-shell")),
    ("Reconnect", || fl!("menu-reconnect")),
    ("Disconnect", || fl!("menu-disconnect")),
    ("CloseTab", || fl!("menu-close-tab")),
    ("ConnectSftp", || fl!("menu-connect-sftp")),
    ("SessionOptions", || fl!("menu-session-options")),
    ("SplitRight", || fl!("menu-split-right")),
    ("SplitDown", || fl!("menu-split-down")),
    ("ToggleSessionManager", || fl!("menu-session-manager")),
    ("ToggleCommandWindow", || fl!("menu-command-window")),
    ("GlobalOptions", || fl!("menu-global-options")),
];

impl Action {
    /// A function by the name `NAMED` gives it.
    pub fn named(name: &str) -> Option<Action> {
        Some(match name {
            "Connect" => Action::Connect,
            "QuickConnect" => Action::QuickConnect,
            "NewLocalShell" => Action::NewLocalShell,
            "Reconnect" => Action::Reconnect,
            "Disconnect" => Action::Disconnect,
            "CloseTab" => Action::CloseTab,
            "ConnectSftp" => Action::ConnectSftp,
            "SessionOptions" => Action::SessionOptions,
            "SplitRight" => Action::SplitRight,
            "SplitDown" => Action::SplitDown,
            "ToggleSessionManager" => Action::ToggleSessionManager,
            "ToggleCommandWindow" => Action::ToggleCommandWindow,
            "GlobalOptions" => Action::GlobalOptions,
            _ => return None,
        })
    }
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
        bind(vec![Alt], c("q"), Action::QuickConnect),
        bind(vec![Alt], c("c"), Action::Connect),
        bind(vec![Alt], c("p"), Action::ConnectSftp),
        bind(vec![Ctrl, Shift], c("t"), Action::NewLocalShell),
        bind(vec![Ctrl, Shift], c("w"), Action::CloseTab),
        bind(vec![Ctrl, Shift], c("c"), Action::Copy),
        bind(vec![Ctrl, Shift], c("v"), Action::Paste),
        bind(vec![Ctrl], Key::Named(Named::Tab), Action::NextTab),
        bind(vec![Ctrl, Shift], Key::Named(Named::Tab), Action::PrevTab),
    ])
}

/// `(tabs, live, splittable)`: any tab open; the active one connected; its group has
/// more than one tab (`Tabs::menu_state`).
pub fn bar(
    (session_manager, command_window, button_bar): (bool, bool, bool),
    (tabs, live, split, logging): (bool, bool, bool, bool),
) -> Element<'static, Message> {
    use menu::Item::{Button, ButtonDisabled, CheckBox, Divider};
    let off = |label: String| ButtonDisabled(label, None, Action::Pending);
    let on = |yes: bool, label: String, a| match yes {
        true => Button(label, None, a),
        false => ButtonDisabled(label, None, a),
    };
    let tab = |label: String, a| on(tabs, label, a);
    let roots = [
        (
            fl!("menu-file"),
            vec![
                Button(fl!("menu-connect"), None, Action::Connect),
                Button(fl!("menu-quick-connect"), None, Action::QuickConnect),
                Button(fl!("menu-new-local-shell"), None, Action::NewLocalShell),
                tab(fl!("menu-reconnect"), Action::Reconnect),
                on(live, fl!("menu-disconnect"), Action::Disconnect),
                CheckBox(fl!("menu-log-session"), None, logging, Action::ToggleLog),
                tab(fl!("menu-close-tab"), Action::CloseTab),
                Divider,
                Button(fl!("menu-import-ssh-config"), None, Action::ImportSshConfig),
                Button(fl!("menu-import-securecrt"), None, Action::ImportSecureCrt),
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
                CheckBox(
                    fl!("menu-command-window"),
                    None,
                    command_window,
                    Action::ToggleCommandWindow,
                ),
                CheckBox(
                    fl!("menu-button-bar"),
                    None,
                    button_bar,
                    Action::ToggleButtonBar,
                ),
                off(fl!("menu-toolbar")),
                off(fl!("menu-status-bar")),
            ],
        ),
        (
            fl!("menu-options"),
            vec![
                Button(fl!("menu-session-options"), None, Action::SessionOptions),
                Button(fl!("menu-edit-default"), None, Action::EditDefault),
                Button(fl!("menu-global-options"), None, Action::GlobalOptions),
            ],
        ),
        (
            fl!("menu-transfer"),
            vec![
                tab(fl!("menu-connect-sftp"), Action::ConnectSftp),
                off(fl!("menu-send-file")),
            ],
        ),
        (
            fl!("menu-script"),
            vec![
                Button(fl!("menu-script-run"), None, Action::RunScript),
                Button(fl!("menu-script-cancel"), None, Action::CancelScript),
            ],
        ),
        (
            fl!("menu-tools"),
            vec![
                Button(fl!("menu-credentials"), None, Action::Credentials),
                off(fl!("menu-create-key")),
                off(fl!("menu-keymap")),
            ],
        ),
        (
            fl!("menu-window"),
            vec![
                on(split, fl!("menu-split-right"), Action::SplitRight),
                on(split, fl!("menu-split-down"), Action::SplitDown),
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
