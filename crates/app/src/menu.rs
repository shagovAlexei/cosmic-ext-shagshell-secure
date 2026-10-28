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
    ToggleCommandManager,
    /// View → Button Bar.
    ToggleButtonBar,
    ToggleConnectBar,
    /// View → Menu Bar (Ctrl+Shift+M): the main menu hidden / shown.
    ToggleMenuBar,
    /// Window → Always on Top…: X11 sets it; on Wayland only the compositor can, so its window
    /// menu opens (COSMIC: «Sticky window»).
    AlwaysOnTop,
    /// File → Log Session.
    ToggleLog,
    About,
    /// Window → the active tab into a window of its own / every such tab back.
    DetachTab,
    ReattachAll,
    /// Script → Record Script (start / stop).
    ToggleRecording,
    /// Help → Check for Updates…
    CheckUpdates,
    /// Help → Support the Project.
    Donate,
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
    /// Tools → Create Public Key….
    CreateKey,
    /// Tools → Upload Public Key….
    UploadKey,
    /// Tools → Keymap Editor….
    KeyMap,
    /// A key of the user's own (`keymap.toml`): the n-th binding.
    Bound(u16),
    /// Transfer → Zmodem / Ymodem / Xmodem: send or receive, or stop.
    Transfer(shagshell_core::zmodem::Proto, bool),
    CancelTransfer,
    /// Script → Run….
    RunScript,
    /// Script → Cancel.
    CancelScript,
    /// Script → Run the last script again (SecureCRT's «Recent Script» button): with its arguments.
    RerunScript,
    /// Script → Console.
    ScriptConsole,
    /// View → Notes: the main note's tab.
    Notes,
    /// Script → Script Builder… (a new tape) / Open in Builder… (a `.py` as blocks).
    ScriptBuilder,
    OpenInBuilder,
    /// File → Open Note….
    OpenNote,
    /// Script → Scheduler….
    Scheduler,
    /// Script → Scripts / Recent: the n-th entry of the app's list (folder files, then recents).
    Listed(u16),
    /// Script → Choose scripts folder….
    ScriptsFolder,
    /// Script → Refresh the list.
    ScriptsRefresh,
    /// View → Zoom In / Out / Reset: the active terminal's font.
    ZoomIn,
    ZoomOut,
    ZoomReset,
    /// View → Full Screen (F11).
    FullScreen,
    /// Edit → Find… (Ctrl+Shift+F).
    Find,
    /// File → Connect Bar (Ctrl+Shift+R): the toolbar's field takes the keys.
    ConnectBar,
    // Shown, not working yet (later phases).
    Disconnect,
    ConnectSftp,
    Pending,
}

/// The functions a Button Bar button can call: the name in `buttonbars.toml` and its label.
pub type NamedFn = (&'static str, fn() -> String);
pub const NAMED: [NamedFn; 33] = [
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
    ("ToggleCommandManager", || fl!("menu-command-manager")),
    ("ToggleRecording", || fl!("menu-script-record")),
    ("GlobalOptions", || fl!("menu-global-options")),
    ("Notes", || fl!("menu-notes")),
    ("ScriptBuilder", || fl!("menu-script-builder")),
    ("OpenInBuilder", || fl!("menu-script-open-builder")),
    ("ZoomIn", || fl!("menu-zoom-in")),
    ("ZoomOut", || fl!("menu-zoom-out")),
    ("ZoomReset", || fl!("menu-zoom-reset")),
    ("FullScreen", || fl!("menu-full-screen")),
    ("Copy", || fl!("menu-copy")),
    ("Paste", || fl!("menu-paste")),
    ("NextTab", || fl!("menu-next-tab")),
    ("PrevTab", || fl!("menu-prev-tab")),
    ("Find", || fl!("menu-find")),
    ("ConnectBar", || fl!("menu-connect-bar")),
    ("RunScript", || fl!("menu-script-run")),
    ("CancelScript", || fl!("menu-script-cancel")),
    ("RerunScript", || fl!("menu-script-rerun")),
    ("ToggleMenuBar", || fl!("menu-menu-bar")),
    ("AlwaysOnTop", || fl!("menu-on-top")),
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
            "ToggleCommandManager" => Action::ToggleCommandManager,
            "ToggleRecording" => Action::ToggleRecording,
            "GlobalOptions" => Action::GlobalOptions,
            "ScriptConsole" => Action::ScriptConsole,
            "Scheduler" => Action::Scheduler,
            "Notes" => Action::Notes,
            "ScriptBuilder" => Action::ScriptBuilder,
            "OpenInBuilder" => Action::OpenInBuilder,
            "ZoomIn" => Action::ZoomIn,
            "ZoomOut" => Action::ZoomOut,
            "ZoomReset" => Action::ZoomReset,
            "FullScreen" => Action::FullScreen,
            "Copy" => Action::Copy,
            "Paste" => Action::Paste,
            "NextTab" => Action::NextTab,
            "PrevTab" => Action::PrevTab,
            "Find" => Action::Find,
            "ConnectBar" => Action::ConnectBar,
            "RunScript" => Action::RunScript,
            "CancelScript" => Action::CancelScript,
            "RerunScript" => Action::RerunScript,
            "ToggleMenuBar" => Action::ToggleMenuBar,
            "AlwaysOnTop" => Action::AlwaysOnTop,
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

/// The keys the menu shows (the chords in force, `keymap_editor::builtin_keys`);
/// `app::route_key` is what acts on them.
fn key_binds() -> HashMap<KeyBind, Action> {
    use Modifier::{Alt, Ctrl, Shift};
    crate::keymap_editor::builtin_keys()
        .into_iter()
        .filter_map(|(chord, id)| {
            let key = menu_key(&chord.code)?;
            let modifiers = [(chord.ctrl, Ctrl), (chord.alt, Alt), (chord.shift, Shift)]
                .into_iter()
                .filter_map(|(on, m)| on.then_some(m))
                .collect();
            Some((KeyBind { modifiers, key }, Action::named(id)?))
        })
        .collect()
}

/// A physical key's name as the menu draws it.
fn menu_key(code: &str) -> Option<Key> {
    let named = match code {
        "Tab" => Named::Tab,
        "Enter" => Named::Enter,
        "ArrowUp" => Named::ArrowUp,
        "ArrowDown" => Named::ArrowDown,
        "ArrowLeft" => Named::ArrowLeft,
        "ArrowRight" => Named::ArrowRight,
        "Home" => Named::Home,
        "End" => Named::End,
        "PageUp" => Named::PageUp,
        "PageDown" => Named::PageDown,
        "F1" => Named::F1,
        "F2" => Named::F2,
        "F3" => Named::F3,
        "F4" => Named::F4,
        "F5" => Named::F5,
        "F6" => Named::F6,
        "F7" => Named::F7,
        "F8" => Named::F8,
        "F9" => Named::F9,
        "F10" => Named::F10,
        "F11" => Named::F11,
        "F12" => Named::F12,
        "Equal" => return Some(Key::Character("=".into())),
        "Minus" => return Some(Key::Character("-".into())),
        c => {
            let one = c.strip_prefix("Key").or_else(|| c.strip_prefix("Digit"))?;
            return (one.len() == 1).then(|| Key::Character(one.to_lowercase().into()));
        }
    };
    Some(Key::Named(named))
}

/// The icon of a menu item (symbolic, from the icon theme), SecureCRT style: most items have one.
fn icon_of(a: Action) -> Option<cosmic::widget::icon::Handle> {
    let name = match a {
        Action::Connect => "network-server-symbolic",
        Action::QuickConnect => "media-playback-start-symbolic",
        Action::NewLocalShell => "utilities-terminal-symbolic",
        Action::OpenNote | Action::Notes => "accessories-text-editor-symbolic",
        Action::Reconnect => "view-refresh-symbolic",
        Action::Disconnect => "process-stop-symbolic",
        Action::CloseTab => "window-close-symbolic",
        Action::ImportSshConfig | Action::ImportSecureCrt => "document-open-symbolic",
        Action::Exit => "application-exit-symbolic",
        Action::Copy => "edit-copy-symbolic",
        Action::Paste => "edit-paste-symbolic",
        Action::SelectAll => "edit-select-all-symbolic",
        Action::SessionOptions => "document-properties-symbolic",
        Action::EditDefault => "document-edit-symbolic",
        Action::GlobalOptions => "emblem-system-symbolic",
        Action::ConnectSftp => "folder-remote-symbolic",
        Action::Transfer(_, true) => "network-transmit-symbolic",
        Action::Transfer(_, false) => "network-receive-symbolic",
        Action::CancelTransfer | Action::CancelScript => "media-playback-stop-symbolic",
        Action::RunScript => "system-run-symbolic",
        Action::RerunScript => "media-playlist-repeat-symbolic",
        Action::ScriptConsole => "utilities-terminal-symbolic",
        Action::Scheduler => "alarm-symbolic",
        Action::ScriptsFolder => "folder-open-symbolic",
        Action::ScriptsRefresh => "view-refresh-symbolic",
        Action::Credentials => "system-users-symbolic",
        Action::CreateKey | Action::UploadKey => "dialog-password-symbolic",
        Action::KeyMap => "input-keyboard-symbolic",
        Action::SplitRight => "view-dual-symbolic",
        Action::SplitDown => "view-paged-symbolic",
        Action::NextTab => "go-next-symbolic",
        Action::PrevTab => "go-previous-symbolic",
        Action::DetachTab => "window-new-symbolic",
        Action::ReattachAll => "view-restore-symbolic",
        Action::CheckUpdates => "software-update-available-symbolic",
        Action::Donate => "emblem-favorite-symbolic",
        Action::About => "help-about-symbolic",
        Action::Pending | Action::Find => "edit-find-symbolic",
        Action::ConnectBar => "system-search-symbolic",
        Action::ZoomIn => "zoom-in-symbolic",
        Action::ZoomOut => "zoom-out-symbolic",
        Action::ZoomReset => "zoom-original-symbolic",
        Action::FullScreen => "view-fullscreen-symbolic",
        _ => return None,
    };
    Some(cosmic::widget::icon::from_name(name).handle())
}

/// Every plain item gets its icon (a check box keeps its check mark: same indent).
fn with_icons(items: Vec<menu::Item<Action, String>>) -> Vec<menu::Item<Action, String>> {
    use menu::Item::{Button, ButtonDisabled, Folder};
    items
        .into_iter()
        .map(|i| match i {
            Button(l, None, a) => Button(l, icon_of(a), a),
            ButtonDisabled(l, None, a) => ButtonDisabled(l, icon_of(a), a),
            Folder(l, children) => Folder(l, with_icons(children)),
            other => other,
        })
        .collect()
}

/// `(tabs, live, splittable)`: any tab open; the active one connected; its group has
/// more than one tab (`Tabs::menu_state`). `scripting`: a script runs.
pub fn bar(
    (session_manager, command_window, button_bar, command_manager, connect_bar): (
        bool,
        bool,
        bool,
        bool,
        bool,
    ),
    (tabs, live, split, logging, recording): (bool, bool, bool, bool, bool),
    (folder, recent): (&[String], &[String]),
    scripting: bool,
    full_screen: bool,
) -> Element<'static, Message> {
    use menu::Item::{Button, ButtonDisabled, CheckBox, Divider};
    let on = |yes: bool, label: String, a| match yes {
        true => Button(label, None, a),
        false => ButtonDisabled(label, None, a),
    };
    let tab = |label: String, a| on(tabs, label, a);
    // File transfer in a terminal needs a terminal tab and lrzsz.
    let xfer = |label: String, a: Action| {
        let tool = match a {
            Action::Transfer(Proto::Zmodem, true) => "sz",
            Action::Transfer(Proto::Ymodem, true) => "sb",
            Action::Transfer(Proto::Xmodem, true) => "sx",
            Action::Transfer(Proto::Ymodem, false) => "rb",
            Action::Transfer(Proto::Xmodem, false) => "rx",
            _ => "sz",
        };
        on(tabs && has_tool(tool), label, a)
    };
    use shagshell_core::zmodem::Proto;
    // A submenu of scripts: their entries are numbered from `first` in the app's list.
    let list = |title: String, names: &[String], first: usize| {
        let items: Vec<_> = match names.is_empty() {
            true => vec![ButtonDisabled(
                fl!("menu-script-none"),
                None,
                Action::Pending,
            )],
            false => (names.iter().enumerate())
                .map(|(i, n)| {
                    let at = u16::try_from(first + i).unwrap_or(u16::MAX);
                    Button(n.clone(), None, Action::Listed(at))
                })
                .collect(),
        };
        menu::Item::Folder(title, items)
    };
    let roots = [
        (
            fl!("menu-file"),
            vec![
                Button(fl!("menu-connect"), None, Action::Connect),
                Button(fl!("menu-quick-connect"), None, Action::QuickConnect),
                Button(fl!("menu-connect-bar"), None, Action::ConnectBar),
                Button(fl!("menu-new-local-shell"), None, Action::NewLocalShell),
                Button(fl!("menu-note-open"), None, Action::OpenNote),
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
                tab(fl!("menu-find"), Action::Find),
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
                    fl!("menu-command-manager"),
                    None,
                    command_manager,
                    Action::ToggleCommandManager,
                ),
                CheckBox(
                    fl!("menu-button-bar"),
                    None,
                    button_bar,
                    Action::ToggleButtonBar,
                ),
                CheckBox(
                    fl!("menu-connect-bar"),
                    None,
                    connect_bar,
                    Action::ToggleConnectBar,
                ),
                // Seen only while the menu is there: this hides it, its key brings it back.
                CheckBox(fl!("menu-menu-bar"), None, true, Action::ToggleMenuBar),
                Button(fl!("menu-notes"), None, Action::Notes),
                Divider,
                tab(fl!("menu-zoom-in"), Action::ZoomIn),
                tab(fl!("menu-zoom-out"), Action::ZoomOut),
                tab(fl!("menu-zoom-reset"), Action::ZoomReset),
                CheckBox(
                    fl!("menu-full-screen"),
                    None,
                    full_screen,
                    Action::FullScreen,
                ),
            ],
        ),
        (
            fl!("menu-options"),
            vec![
                Button(fl!("menu-session-options"), None, Action::SessionOptions),
                Button(fl!("menu-edit-default"), None, Action::EditDefault),
            ],
        ),
        (
            fl!("menu-transfer"),
            vec![
                tab(fl!("menu-connect-sftp"), Action::ConnectSftp),
                Divider,
                xfer(
                    fl!("menu-send-zmodem"),
                    Action::Transfer(Proto::Zmodem, true),
                ),
                xfer(
                    fl!("menu-send-ymodem"),
                    Action::Transfer(Proto::Ymodem, true),
                ),
                xfer(
                    fl!("menu-send-xmodem"),
                    Action::Transfer(Proto::Xmodem, true),
                ),
                xfer(
                    fl!("menu-receive-ymodem"),
                    Action::Transfer(Proto::Ymodem, false),
                ),
                xfer(
                    fl!("menu-receive-xmodem"),
                    Action::Transfer(Proto::Xmodem, false),
                ),
                xfer(fl!("menu-cancel-transfer"), Action::CancelTransfer),
            ],
        ),
        (
            fl!("menu-script"),
            vec![
                Button(fl!("menu-script-run"), None, Action::RunScript),
                on(scripting, fl!("menu-script-cancel"), Action::CancelScript),
                Button(fl!("menu-script-rerun"), None, Action::RerunScript),
                Button(fl!("menu-script-console"), None, Action::ScriptConsole),
                Button(fl!("menu-script-scheduler"), None, Action::Scheduler),
                Button(fl!("menu-script-builder"), None, Action::ScriptBuilder),
                Button(fl!("menu-script-open-builder"), None, Action::OpenInBuilder),
                Divider,
                CheckBox(
                    fl!("menu-script-record"),
                    None,
                    recording,
                    Action::ToggleRecording,
                ),
                Divider,
                list(fl!("menu-script-folder"), folder, 0),
                list(fl!("menu-script-recent"), recent, folder.len()),
                Divider,
                Button(
                    fl!("menu-script-choose-folder"),
                    None,
                    Action::ScriptsFolder,
                ),
                Button(fl!("menu-script-refresh"), None, Action::ScriptsRefresh),
            ],
        ),
        (
            fl!("menu-tools"),
            vec![
                Button(fl!("menu-global-options"), None, Action::GlobalOptions),
                Divider,
                Button(fl!("menu-credentials"), None, Action::Credentials),
                Button(fl!("menu-create-key"), None, Action::CreateKey),
                Button(fl!("menu-upload-key"), None, Action::UploadKey),
                Button(fl!("menu-keymap"), None, Action::KeyMap),
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
                Divider,
                tab(fl!("menu-tab-window"), Action::DetachTab),
                Button(fl!("menu-reattach-all"), None, Action::ReattachAll),
                Divider,
                Button(fl!("menu-on-top"), None, Action::AlwaysOnTop),
            ],
        ),
        (fl!("menu-help"), {
            let mut help = vec![Button(
                fl!("menu-check-updates"),
                None,
                Action::CheckUpdates,
            )];
            if crate::updates::DONATE_URL.is_some() {
                help.push(Button(fl!("menu-donate"), None, Action::Donate));
            }
            help.push(Divider);
            help.push(Button(fl!("menu-about"), None, Action::About));
            help
        }),
    ];
    let binds = key_binds();
    let roots = roots
        .into_iter()
        .map(|(title, items)| {
            let items = with_icons(items);
            menu::Tree::with_children(Element::from(menu::root(title)), menu::items(&binds, items))
        })
        .collect();
    menu::bar(roots)
        .item_height(ItemHeight::Dynamic(40))
        .item_width(ItemWidth::Uniform(320))
        .spacing(4.0)
        .into()
}

/// This lrzsz program is installed. Looked for once (after installing lrzsz, restart).
pub fn has_tool(name: &str) -> bool {
    static FOUND: std::sync::OnceLock<Vec<(String, bool)>> = std::sync::OnceLock::new();
    let found = FOUND.get_or_init(|| {
        let path = std::env::var_os("PATH");
        shagshell_core::check::probe(&shagshell_core::zmodem::programs(), path.as_deref())
    });
    found.iter().any(|(n, ok)| n == name && *ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regression_script_functions_can_be_bound() {
        // Run / Cancel / the last script again were in the Script menu only: no key, Button Bar
        // button or toolbar button could call them.
        for (name, _) in NAMED {
            assert!(Action::named(name).is_some(), "{name}");
        }
        for name in ["RunScript", "CancelScript", "RerunScript"] {
            assert!(NAMED.iter().any(|(n, _)| *n == name), "{name} in NAMED");
            assert!(
                crate::toolbar::TOOLS.iter().any(|t| t.0 == name),
                "{name} on the toolbar"
            );
        }
    }
}
