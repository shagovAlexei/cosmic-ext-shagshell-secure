//! Header menu bar as in SecureCRT, and `Action`: every user action, whatever triggers it
//! (menu, key, toolbar, tab menu, later Button Bar "Menu Function").

use crate::app::Message;
use crate::fl;
use cosmic::Element;
use cosmic::iced::keyboard::Key;
use cosmic::iced::keyboard::key::Named;
use cosmic::widget::menu::key_bind::Modifier;
use cosmic::widget::menu::{self, ItemHeight, ItemWidth, KeyBind};
use shagshell_core::menu::{Item as MenuItem, Menu};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

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
    /// Tools → Customize Menu….
    CustomizeMenu,
    /// A program / script item of the user's own in the menu: the n-th (`menu::own_item`).
    OwnItem(u16),
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

/// Every function a key, a Button Bar button or the menu can call, by its name (`buttonbars.toml`,
/// `keymap.toml`, `Config.menus`): the stock menu's (`core::menu::STOCK`, in menu order; not the
/// script submenus). `entry` is the one table of names.
pub static FUNCTIONS: std::sync::LazyLock<Vec<&'static str>> = std::sync::LazyLock::new(|| {
    (shagshell_core::menu::STOCK.iter())
        .flat_map(|(_, items)| items.iter().copied())
        .filter(|c| Action::named(c).is_some())
        .collect()
});

/// A function's label in the current language (the name itself if unknown).
pub fn function_label(name: &str) -> String {
    entry(name).map_or_else(|| name.to_string(), |e| e.1)
}

/// A function as a list offers it: «label — menu» (File's / View's Connect Bar share a label).
pub fn function_choice(name: &str) -> String {
    let menu = (shagshell_core::menu::STOCK.iter())
        .find(|(_, items)| items.contains(&name))
        .map(|(id, _)| {
            title(&Menu {
                id: (*id).into(),
                ..Default::default()
            })
        });
    match menu {
        Some(m) => format!("{} — {m}", function_label(name)),
        None => function_label(name),
    }
}

impl Action {
    /// A function by its name (`FUNCTIONS`).
    pub fn named(name: &str) -> Option<Action> {
        entry(name).map(|e| e.0).filter(|a| *a != Action::Pending)
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
        Action::CustomizeMenu => "open-menu-symbolic",
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

/// A stock item's function and label (`core::menu::STOCK` names; the submenus `ScriptsList` /
/// `RecentScripts` are drawn by `bar` itself).
pub fn entry(cmd: &str) -> Option<(Action, String)> {
    use shagshell_core::zmodem::Proto;
    Some(match cmd {
        "Connect" => (Action::Connect, fl!("menu-connect")),
        "QuickConnect" => (Action::QuickConnect, fl!("menu-quick-connect")),
        "ConnectBar" => (Action::ConnectBar, fl!("menu-connect-bar")),
        "NewLocalShell" => (Action::NewLocalShell, fl!("menu-new-local-shell")),
        "OpenNote" => (Action::OpenNote, fl!("menu-note-open")),
        "Reconnect" => (Action::Reconnect, fl!("menu-reconnect")),
        "Disconnect" => (Action::Disconnect, fl!("menu-disconnect")),
        "ToggleLog" => (Action::ToggleLog, fl!("menu-log-session")),
        "CloseTab" => (Action::CloseTab, fl!("menu-close-tab")),
        "ImportSshConfig" => (Action::ImportSshConfig, fl!("menu-import-ssh-config")),
        "ImportSecureCrt" => (Action::ImportSecureCrt, fl!("menu-import-securecrt")),
        "Exit" => (Action::Exit, fl!("menu-exit")),
        "Copy" => (Action::Copy, fl!("menu-copy")),
        "Paste" => (Action::Paste, fl!("menu-paste")),
        "SelectAll" => (Action::SelectAll, fl!("menu-select-all")),
        "Find" => (Action::Find, fl!("menu-find")),
        "ToggleSessionManager" => (Action::ToggleSessionManager, fl!("menu-session-manager")),
        "ToggleCommandWindow" => (Action::ToggleCommandWindow, fl!("menu-command-window")),
        "ToggleCommandManager" => (Action::ToggleCommandManager, fl!("menu-command-manager")),
        "ToggleButtonBar" => (Action::ToggleButtonBar, fl!("menu-button-bar")),
        "ToggleConnectBar" => (Action::ToggleConnectBar, fl!("menu-connect-bar")),
        "ToggleMenuBar" => (Action::ToggleMenuBar, fl!("menu-menu-bar")),
        "Notes" => (Action::Notes, fl!("menu-notes")),
        "ZoomIn" => (Action::ZoomIn, fl!("menu-zoom-in")),
        "ZoomOut" => (Action::ZoomOut, fl!("menu-zoom-out")),
        "ZoomReset" => (Action::ZoomReset, fl!("menu-zoom-reset")),
        "FullScreen" => (Action::FullScreen, fl!("menu-full-screen")),
        "SessionOptions" => (Action::SessionOptions, fl!("menu-session-options")),
        "EditDefault" => (Action::EditDefault, fl!("menu-edit-default")),
        "ConnectSftp" => (Action::ConnectSftp, fl!("menu-connect-sftp")),
        "SendZmodem" => (
            Action::Transfer(Proto::Zmodem, true),
            fl!("menu-send-zmodem"),
        ),
        "SendYmodem" => (
            Action::Transfer(Proto::Ymodem, true),
            fl!("menu-send-ymodem"),
        ),
        "SendXmodem" => (
            Action::Transfer(Proto::Xmodem, true),
            fl!("menu-send-xmodem"),
        ),
        "ReceiveYmodem" => (
            Action::Transfer(Proto::Ymodem, false),
            fl!("menu-receive-ymodem"),
        ),
        "ReceiveXmodem" => (
            Action::Transfer(Proto::Xmodem, false),
            fl!("menu-receive-xmodem"),
        ),
        "CancelTransfer" => (Action::CancelTransfer, fl!("menu-cancel-transfer")),
        "RunScript" => (Action::RunScript, fl!("menu-script-run")),
        "CancelScript" => (Action::CancelScript, fl!("menu-script-cancel")),
        "RerunScript" => (Action::RerunScript, fl!("menu-script-rerun")),
        "ScriptConsole" => (Action::ScriptConsole, fl!("menu-script-console")),
        "Scheduler" => (Action::Scheduler, fl!("menu-script-scheduler")),
        "ScriptBuilder" => (Action::ScriptBuilder, fl!("menu-script-builder")),
        "OpenInBuilder" => (Action::OpenInBuilder, fl!("menu-script-open-builder")),
        "ToggleRecording" => (Action::ToggleRecording, fl!("menu-script-record")),
        "ScriptsList" => (Action::Pending, fl!("menu-script-folder")),
        "RecentScripts" => (Action::Pending, fl!("menu-script-recent")),
        "ScriptsFolder" => (Action::ScriptsFolder, fl!("menu-script-choose-folder")),
        "ScriptsRefresh" => (Action::ScriptsRefresh, fl!("menu-script-refresh")),
        "GlobalOptions" => (Action::GlobalOptions, fl!("menu-global-options")),
        "Credentials" => (Action::Credentials, fl!("menu-credentials")),
        "CreateKey" => (Action::CreateKey, fl!("menu-create-key")),
        "UploadKey" => (Action::UploadKey, fl!("menu-upload-key")),
        "KeyMap" => (Action::KeyMap, fl!("menu-keymap")),
        "CustomizeMenu" => (Action::CustomizeMenu, fl!("menu-customize")),
        "SplitRight" => (Action::SplitRight, fl!("menu-split-right")),
        "SplitDown" => (Action::SplitDown, fl!("menu-split-down")),
        "NextTab" => (Action::NextTab, fl!("menu-next-tab")),
        "PrevTab" => (Action::PrevTab, fl!("menu-prev-tab")),
        "DetachTab" => (Action::DetachTab, fl!("menu-tab-window")),
        "ReattachAll" => (Action::ReattachAll, fl!("menu-reattach-all")),
        "AlwaysOnTop" => (Action::AlwaysOnTop, fl!("menu-on-top")),
        "CheckUpdates" => (Action::CheckUpdates, fl!("menu-check-updates")),
        "Donate" => (Action::Donate, fl!("menu-donate")),
        "About" => (Action::About, fl!("menu-about")),
        _ => return None,
    })
}

/// A menu's name on the bar: the user's, or the stock one.
pub fn title(m: &Menu) -> String {
    if !m.title.is_empty() {
        return m.title.clone();
    }
    match m.id.as_str() {
        "File" => fl!("menu-file"),
        "Edit" => fl!("menu-edit"),
        "View" => fl!("menu-view"),
        "Options" => fl!("menu-options"),
        "Transfer" => fl!("menu-transfer"),
        "Script" => fl!("menu-script"),
        "Tools" => fl!("menu-tools"),
        "Window" => fl!("menu-window"),
        "Help" => fl!("menu-help"),
        _ => fl!("me-new-menu"),
    }
}

/// An item's label: the user's, the function's, or a program's command.
pub fn label(it: &MenuItem) -> String {
    match () {
        _ if !it.label.is_empty() => it.label.clone(),
        _ if it.is_sep() => "────────".into(),
        _ if it.cmd.is_empty() => fl!("me-new-program"),
        _ => entry(&it.cmd).map_or_else(|| it.cmd.clone(), |e| e.1),
    }
}

/// `core::menu::merged(menus)`, made again only when `menus` changes: the bar is drawn on every
/// frame.
fn drawn(menus: &[Menu]) -> Rc<Vec<Menu>> {
    thread_local! {
        static CACHE: RefCell<(Vec<Menu>, Rc<Vec<Menu>>)> =
            RefCell::new((Vec::new(), Rc::new(shagshell_core::menu::merged(&[]))));
    }
    CACHE.with_borrow_mut(|(saved, out)| {
        if saved.as_slice() != menus {
            *saved = menus.to_vec();
            *out = Rc::new(shagshell_core::menu::merged(menus));
        }
        out.clone()
    })
}

/// The n-th program / script item of the user's (hidden ones counted), as `Action::OwnItem`.
pub fn own_item(menus: &[Menu], n: u16) -> Option<MenuItem> {
    (drawn(menus).iter())
        .flat_map(|m| &m.items)
        .filter(|i| i.is_own())
        .nth(usize::from(n))
        .cloned()
}

/// What the menus' check marks and greyed items depend on.
pub struct State<'a> {
    pub session_manager: bool,
    pub command_window: bool,
    pub button_bar: bool,
    pub command_manager: bool,
    pub connect_bar: bool,
    /// `Tabs::menu_state`: any tab open; the active one connected; its group has more than one
    /// tab; logging; recording.
    pub tabs: (bool, bool, bool, bool, bool),
    /// Script → Scripts / Recent.
    pub folder: &'a [String],
    pub recent: &'a [String],
    pub scripting: bool,
    pub full_screen: bool,
}

/// The menu bar, as `menus` (`Config.menus`, through `core::menu::merged`) arranges it.
pub fn bar(s: &State, menus: &[Menu]) -> Element<'static, Message> {
    use menu::Item::{Button, ButtonDisabled, CheckBox, Divider, Folder};
    use shagshell_core::zmodem::Proto;
    let (tabs, live, split, logging, recording) = s.tabs;
    // File transfer in a terminal needs a terminal tab and lrzsz.
    let xfer = |a: Action| {
        let tool = match a {
            Action::Transfer(Proto::Zmodem, true) => "sz",
            Action::Transfer(Proto::Ymodem, true) => "sb",
            Action::Transfer(Proto::Xmodem, true) => "sx",
            Action::Transfer(Proto::Ymodem, false) => "rb",
            Action::Transfer(Proto::Xmodem, false) => "rx",
            _ => "sz",
        };
        tabs && has_tool(tool)
    };
    let enabled = |a: Action| match a {
        Action::Reconnect
        | Action::CloseTab
        | Action::Copy
        | Action::Paste
        | Action::SelectAll
        | Action::Find
        | Action::ZoomIn
        | Action::ZoomOut
        | Action::ZoomReset
        | Action::NextTab
        | Action::PrevTab
        | Action::DetachTab
        | Action::ConnectSftp => tabs,
        Action::Disconnect => live,
        Action::SplitRight | Action::SplitDown => split,
        Action::CancelScript => s.scripting,
        Action::Transfer(..) | Action::CancelTransfer => xfer(a),
        _ => true,
    };
    let checked = |a: Action| match a {
        Action::ToggleSessionManager => Some(s.session_manager),
        Action::ToggleCommandWindow => Some(s.command_window),
        Action::ToggleCommandManager => Some(s.command_manager),
        Action::ToggleButtonBar => Some(s.button_bar),
        Action::ToggleConnectBar => Some(s.connect_bar),
        // Seen only while the menu is there: this hides it, its key brings it back.
        Action::ToggleMenuBar => Some(true),
        Action::FullScreen => Some(s.full_screen),
        Action::ToggleLog => Some(logging),
        Action::ToggleRecording => Some(recording),
        _ => None,
    };
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
        Folder(title, items)
    };
    let mut own = 0u16;
    let mut roots = Vec::new();
    for m in drawn(menus).iter() {
        let mut items = Vec::new();
        for it in &m.items {
            let n = own;
            if it.is_own() {
                own = own.saturating_add(1);
            }
            if !it.shown || !m.shown {
                continue;
            }
            let found = (!it.is_own() && !it.is_sep())
                .then(|| entry(&it.cmd))
                .flatten();
            let text = match (&found, it.label.is_empty()) {
                (Some((_, l)), true) => l.clone(),
                _ => label(it),
            };
            let item = match it.cmd.as_str() {
                _ if it.is_sep() => match items.last() {
                    None | Some(Divider) => continue,
                    _ => Divider,
                },
                _ if it.is_own() && it.cmd.is_empty() => continue,
                _ if it.is_own() => Button(text, None, Action::OwnItem(n)),
                "ScriptsList" => list(text, s.folder, 0),
                "RecentScripts" => list(text, s.recent, s.folder.len()),
                "Donate" if crate::updates::DONATE_URL.is_none() => continue,
                _ => {
                    let Some((a, _)) = found else { continue };
                    match (checked(a), enabled(a)) {
                        (Some(on), _) => CheckBox(text, None, on, a),
                        (None, true) => Button(text, None, a),
                        (None, false) => ButtonDisabled(text, None, a),
                    }
                }
            };
            items.push(item);
        }
        if matches!(items.last(), Some(Divider)) {
            items.pop();
        }
        if !items.is_empty() {
            roots.push((title(m), items));
        }
    }
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
    fn every_stock_menu_name_is_drawn() {
        for (_, items) in shagshell_core::menu::STOCK {
            for cmd in items.iter().filter(|c| **c != shagshell_core::toolbar::SEP) {
                assert!(entry(cmd).is_some(), "{cmd}");
            }
        }
    }

    #[test]
    fn every_builtin_key_has_its_function() {
        for (id, _) in shagshell_core::keymap::BUILTIN {
            assert!(Action::named(id).is_some(), "{id}");
        }
    }

    #[test]
    fn menu_cache_follows_the_saved_menu() {
        let stock = drawn(&[]);
        assert_eq!(*stock, shagshell_core::menu::default_menus());
        assert!(Rc::ptr_eq(&stock, &drawn(&[])), "made once");
        let mut saved = shagshell_core::menu::default_menus();
        saved[0].shown = false;
        assert!(!drawn(&saved)[0].shown);
        assert!(drawn(&[])[0].shown);
    }

    #[test]
    fn regression_script_functions_can_be_bound() {
        // Run / Cancel / the last script again were in the Script menu only: no key, Button Bar
        // button or toolbar button could call them.
        for name in ["RunScript", "CancelScript", "RerunScript"] {
            assert!(FUNCTIONS.contains(&name), "{name} in FUNCTIONS");
            assert!(
                crate::toolbar::TOOLS.iter().any(|t| t.0 == name),
                "{name} on the toolbar"
            );
        }
    }
}
