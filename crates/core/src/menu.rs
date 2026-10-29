//! The main menu as data (spec 37): menus of items, each a stock function by name, a divider,
//! or a program / `.py` of the user's own (the toolbar's conventions). `STOCK` is the menu as
//! shipped; the user's arrangement (`Config.menus`, empty = stock) is drawn through `merged`,
//! and `MenuEdit` is the editor's state.

use crate::toolbar::SEP;
use serde::{Deserialize, Serialize};

/// Tools → Customize Menu…: never hidden, so the editor stays reachable.
pub const CUSTOMIZE: &str = "CustomizeMenu";

/// The menus as shipped: (menu id, item names in order).
pub const STOCK: &[(&str, &[&str])] = &[
    (
        "File",
        &[
            "Connect",
            "QuickConnect",
            "ConnectBar",
            "NewLocalShell",
            "OpenNote",
            "Reconnect",
            "Disconnect",
            "ToggleLog",
            "CloseTab",
            SEP,
            "ImportSshConfig",
            "ImportSecureCrt",
            "ExportSettings",
            "ImportSettings",
            SEP,
            "Exit",
        ],
    ),
    ("Edit", &["Copy", "Paste", "SelectAll", "Find"]),
    (
        "View",
        &[
            "ToggleSessionManager",
            "ToggleCommandWindow",
            "ToggleCommandManager",
            "ToggleButtonBar",
            "ToggleConnectBar",
            "ToggleMenuBar",
            "Notes",
            SEP,
            "ZoomIn",
            "ZoomOut",
            "ZoomReset",
            "FullScreen",
        ],
    ),
    ("Options", &["SessionOptions", "EditDefault"]),
    (
        "Transfer",
        &[
            "ConnectSftp",
            SEP,
            "SendZmodem",
            "SendYmodem",
            "SendXmodem",
            "ReceiveYmodem",
            "ReceiveXmodem",
            "CancelTransfer",
        ],
    ),
    (
        "Script",
        &[
            "RunScript",
            "CancelScript",
            "RerunScript",
            "ScriptConsole",
            "Scheduler",
            "ScriptBuilder",
            "OpenInBuilder",
            SEP,
            "ToggleRecording",
            SEP,
            "ScriptsList",
            "RecentScripts",
            SEP,
            "ScriptsFolder",
            "ScriptsRefresh",
        ],
    ),
    (
        "Tools",
        &[
            "GlobalOptions",
            SEP,
            "Credentials",
            "CreateKey",
            "UploadKey",
            "KeyMap",
            CUSTOMIZE,
        ],
    ),
    (
        "Window",
        &[
            "SplitRight",
            "SplitDown",
            SEP,
            "NextTab",
            "PrevTab",
            SEP,
            "DetachTab",
            "ReattachAll",
            SEP,
            "AlwaysOnTop",
        ],
    ),
    ("Help", &["CheckUpdates", "Donate", SEP, "About"]),
];

/// A stock function's name (not a divider, not a program of the user's).
pub fn is_stock(cmd: &str) -> bool {
    cmd != SEP && STOCK.iter().any(|(_, items)| items.contains(&cmd))
}

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Item {
    /// A stock function's name, `SEP`, a program, or a `.py`.
    pub cmd: String,
    /// A program's / script's arguments.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub params: String,
    /// The label; empty: the function's own (a program: its command).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub label: String,
    /// A program / script of the user's own (not a function, whatever `cmd` says).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub program: bool,
    #[serde(default = "yes")]
    pub shown: bool,
}

impl Default for Item {
    fn default() -> Self {
        Self {
            cmd: String::new(),
            params: String::new(),
            label: String::new(),
            program: false,
            shown: true,
        }
    }
}

impl Item {
    pub fn function(cmd: &str) -> Self {
        Self {
            cmd: cmd.into(),
            ..Default::default()
        }
    }

    pub fn is_sep(&self) -> bool {
        self.cmd == SEP
    }

    /// A program or script of the user's own.
    pub fn is_own(&self) -> bool {
        self.program
    }

    pub fn program() -> Self {
        Self {
            program: true,
            ..Default::default()
        }
    }

    pub fn is_script(&self) -> bool {
        self.cmd.ends_with(".py")
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Menu {
    /// A stock id (`STOCK`) or `own-N`.
    pub id: String,
    /// The name; empty: the stock one.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub title: String,
    #[serde(default = "yes")]
    pub shown: bool,
    pub items: Vec<Item>,
}

impl Default for Menu {
    fn default() -> Self {
        Self {
            id: String::new(),
            title: String::new(),
            shown: true,
            items: Vec::new(),
        }
    }
}

impl Menu {
    pub fn is_own(&self) -> bool {
        self.id.starts_with("own-")
    }

    fn has_customize(&self) -> bool {
        self.items.iter().any(|i| i.cmd == CUSTOMIZE && !i.program)
    }
}

pub fn default_menus() -> Vec<Menu> {
    STOCK
        .iter()
        .map(|(id, items)| Menu {
            id: (*id).into(),
            items: items.iter().map(|c| Item::function(c)).collect(),
            ..Default::default()
        })
        .collect()
}

/// What is drawn: the stock menu when nothing is saved; else the saved one, with every stock
/// function it lacks (a newer version's, or one whose own menu was deleted) appended to its
/// stock menu, so the editor can always show it again; Customize Menu… kept in sight.
pub fn merged(saved: &[Menu]) -> Vec<Menu> {
    if saved.is_empty() {
        return default_menus();
    }
    let mut out = saved.to_vec();
    for (id, items) in STOCK {
        for cmd in items.iter().filter(|c| **c != SEP) {
            if out.iter().flat_map(|m| &m.items).any(|i| i.cmd == *cmd) {
                continue;
            }
            let at = match out.iter().position(|m| m.id == *id) {
                Some(at) => at,
                None => {
                    out.push(Menu {
                        id: (*id).into(),
                        ..Default::default()
                    });
                    out.len() - 1
                }
            };
            out[at].items.push(Item::function(cmd));
        }
    }
    keep_customize(&mut out);
    out
}

/// Every Customize Menu… item shown, and the menus holding one: the editor stays reachable.
fn keep_customize(menus: &mut [Menu]) {
    for m in menus.iter_mut().filter(|m| m.has_customize()) {
        m.shown = true;
        for it in m
            .items
            .iter_mut()
            .filter(|i| i.cmd == CUSTOMIZE && !i.program)
        {
            it.shown = true;
        }
    }
}

#[derive(Clone, Debug)]
pub enum MenuMsg {
    SelectMenu(usize),
    SelectItem(usize),
    ShowMenu(usize, bool),
    MenuUp,
    MenuDown,
    MenuTitle(String),
    AddMenu,
    DeleteMenu,
    ShowItem(usize, bool),
    ItemUp,
    ItemDown,
    /// The chosen item to the end of this menu.
    MoveTo(usize),
    Label(String),
    Cmd(String),
    Params(String),
    AddFunction(String),
    AddProgram,
    AddSep,
    DeleteItem,
    Reset,
}

/// The editor's state: the menus being edited, the chosen menu and item.
#[derive(Clone, Debug)]
pub struct MenuEdit {
    pub list: Vec<Menu>,
    pub menu: usize,
    pub item: Option<usize>,
}

impl MenuEdit {
    pub fn new(saved: &[Menu]) -> Self {
        Self {
            list: merged(saved),
            menu: 0,
            item: None,
        }
    }

    /// What to store: empty when it is the stock menu, so later stock changes reach it.
    pub fn ok(self) -> Vec<Menu> {
        match self.list == default_menus() {
            true => Vec::new(),
            false => self.list,
        }
    }

    fn items(&self) -> &[Item] {
        self.list.get(self.menu).map_or(&[], |m| &m.items)
    }

    fn chosen(&self) -> Option<&Item> {
        self.item.and_then(|i| self.items().get(i))
    }

    /// This menu's check box may be cleared (it doesn't hold Customize Menu…).
    pub fn can_hide_menu(&self, m: usize) -> bool {
        self.list.get(m).is_some_and(|m| !m.has_customize())
    }

    /// This item of the chosen menu may be hidden.
    pub fn can_hide_item(&self, i: usize) -> bool {
        self.items().get(i).is_some_and(|i| i.cmd != CUSTOMIZE)
    }

    /// The chosen menu is the user's own.
    pub fn can_delete_menu(&self) -> bool {
        self.list.get(self.menu).is_some_and(Menu::is_own)
    }

    /// The chosen item is the user's own, a divider, or a stock function standing twice.
    pub fn can_delete_item(&self) -> bool {
        self.chosen().is_some_and(|it| {
            it.is_own()
                || it.is_sep()
                || (self.list.iter().flat_map(|m| &m.items))
                    .filter(|i| !i.program && i.cmd == it.cmd)
                    .count()
                    > 1
        })
    }

    pub fn update(&mut self, m: MenuMsg) {
        let n = self.list.len();
        match m {
            MenuMsg::SelectMenu(i) if i < n => {
                self.menu = i;
                self.item = None;
            }
            MenuMsg::SelectItem(i) if i < self.items().len() => self.item = Some(i),
            MenuMsg::ShowMenu(i, on) if on || self.can_hide_menu(i) => {
                if let Some(m) = self.list.get_mut(i) {
                    m.shown = on;
                }
            }
            MenuMsg::MenuUp if self.menu > 0 && self.menu < n => {
                self.list.swap(self.menu, self.menu - 1);
                self.menu -= 1;
            }
            MenuMsg::MenuDown if self.menu + 1 < n => {
                self.list.swap(self.menu, self.menu + 1);
                self.menu += 1;
            }
            MenuMsg::MenuTitle(t) => {
                if let Some(m) = self.list.get_mut(self.menu) {
                    m.title = t;
                }
            }
            MenuMsg::AddMenu => {
                let next = (self.list.iter())
                    .filter_map(|m| m.id.strip_prefix("own-")?.parse::<u32>().ok())
                    .max()
                    .map_or(1, |x| x + 1);
                let at = (self.menu + 1).min(n);
                self.list.insert(
                    at,
                    Menu {
                        id: format!("own-{next}"),
                        ..Default::default()
                    },
                );
                self.menu = at;
                self.item = None;
            }
            MenuMsg::DeleteMenu if self.can_delete_menu() => {
                self.list.remove(self.menu);
                self.list = merged(&self.list);
                self.menu = self.menu.min(self.list.len().saturating_sub(1));
                self.item = None;
            }
            MenuMsg::ShowItem(i, on) if on || self.can_hide_item(i) => {
                if let Some(it) = self.item_mut(i) {
                    it.shown = on;
                }
            }
            MenuMsg::ItemUp => {
                if let Some(i) = self.item.filter(|&i| i > 0 && i < self.items().len()) {
                    self.list[self.menu].items.swap(i, i - 1);
                    self.item = Some(i - 1);
                }
            }
            MenuMsg::ItemDown => {
                if let Some(i) = self.item.filter(|&i| i + 1 < self.items().len()) {
                    self.list[self.menu].items.swap(i, i + 1);
                    self.item = Some(i + 1);
                }
            }
            MenuMsg::MoveTo(to) if to < n && to != self.menu => {
                if let Some(i) = self.item.filter(|&i| i < self.items().len()) {
                    let it = self.list[self.menu].items.remove(i);
                    if it.cmd == CUSTOMIZE {
                        self.list[to].shown = true;
                    }
                    self.list[to].items.push(it);
                    self.menu = to;
                    self.item = Some(self.list[to].items.len() - 1);
                }
            }
            MenuMsg::Label(t) => self.edit(|it| it.label = t),
            MenuMsg::Cmd(t) => self.edit(|it| {
                if it.is_own() {
                    it.cmd = t
                }
            }),
            MenuMsg::Params(t) => self.edit(|it| {
                if it.is_own() {
                    it.params = t
                }
            }),
            MenuMsg::AddFunction(cmd) if is_stock(&cmd) => self.insert(Item::function(&cmd)),
            MenuMsg::AddProgram => self.insert(Item::program()),
            MenuMsg::AddSep => self.insert(Item::function(SEP)),
            MenuMsg::DeleteItem if self.can_delete_item() => {
                if let Some(i) = self.item {
                    self.list[self.menu].items.remove(i);
                    let len = self.items().len();
                    self.item = (len > 0).then(|| i.min(len - 1));
                }
            }
            MenuMsg::Reset => *self = Self::new(&[]),
            _ => {}
        }
        keep_customize(&mut self.list);
    }

    fn item_mut(&mut self, i: usize) -> Option<&mut Item> {
        self.list.get_mut(self.menu)?.items.get_mut(i)
    }

    fn edit(&mut self, f: impl FnOnce(&mut Item)) {
        if let Some(i) = self.item
            && let Some(it) = self.item_mut(i)
        {
            f(it)
        }
    }

    /// After the chosen item, or at the end; then chosen.
    fn insert(&mut self, it: Item) {
        let Some(m) = self.list.get_mut(self.menu) else {
            return;
        };
        let at = self
            .item
            .map_or(m.items.len(), |i| (i + 1).min(m.items.len()));
        m.items.insert(at, it);
        self.item = Some(at);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(e: &MenuEdit, id: &str) -> usize {
        e.list.iter().position(|m| m.id == id).unwrap()
    }

    fn cmds(m: &Menu) -> Vec<&str> {
        m.items.iter().map(|i| i.cmd.as_str()).collect()
    }

    #[test]
    fn stock_names_are_unique() {
        let all: Vec<_> = STOCK
            .iter()
            .flat_map(|(_, i)| *i)
            .filter(|c| **c != SEP)
            .collect();
        let mut sorted = all.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(all.len(), sorted.len());
    }

    #[test]
    fn nothing_saved_draws_the_stock_menu() {
        assert_eq!(merged(&[]), default_menus());
        assert_eq!(default_menus()[6].id, "Tools");
        assert_eq!(default_menus()[6].items[0].cmd, "GlobalOptions");
    }

    #[test]
    fn a_new_stock_function_reaches_a_saved_menu() {
        let mut saved = default_menus();
        let tools = saved.iter_mut().find(|m| m.id == "Tools").unwrap();
        tools.items.retain(|i| i.cmd != "KeyMap");
        saved[0].items[0].shown = false;
        let out = merged(&saved);
        let tools = out.iter().find(|m| m.id == "Tools").unwrap();
        assert_eq!(cmds(tools).last(), Some(&"KeyMap"));
        assert!(!out[0].items[0].shown, "a hidden item stays hidden");
    }

    #[test]
    fn moving_an_item_to_another_menu() {
        let mut e = MenuEdit::new(&[]);
        e.update(MenuMsg::SelectMenu(at(&e, "Tools")));
        e.update(MenuMsg::SelectItem(0));
        let options = at(&e, "Options");
        e.update(MenuMsg::MoveTo(options));
        assert_eq!(e.menu, options);
        assert_eq!(cmds(&e.list[options]).last(), Some(&"GlobalOptions"));
        assert!(!cmds(&e.list[at(&e, "Tools")]).contains(&"GlobalOptions"));
        let saved = e.ok();
        assert!(!saved.is_empty());
        assert_eq!(merged(&saved), saved, "nothing appended twice");
    }

    #[test]
    fn up_and_down_stop_at_the_ends() {
        let mut e = MenuEdit::new(&[]);
        e.update(MenuMsg::SelectItem(0));
        e.update(MenuMsg::ItemUp);
        assert_eq!(e.item, Some(0));
        e.update(MenuMsg::ItemDown);
        assert_eq!(e.item, Some(1));
        assert_eq!(cmds(&e.list[0])[..2], ["QuickConnect", "Connect"]);
        e.update(MenuMsg::MenuUp);
        assert_eq!(e.menu, 0);
        e.update(MenuMsg::MenuDown);
        assert_eq!((e.menu, e.list[0].id.as_str()), (1, "Edit"));
    }

    #[test]
    fn a_stock_function_is_hidden_not_deleted() {
        let mut e = MenuEdit::new(&[]);
        e.update(MenuMsg::SelectItem(0));
        assert!(!e.can_delete_item());
        e.update(MenuMsg::DeleteItem);
        assert_eq!(e.list[0].items[0].cmd, "Connect");
        e.update(MenuMsg::ShowItem(0, false));
        assert!(!e.list[0].items[0].shown);
        // Standing twice, one may go.
        e.update(MenuMsg::AddFunction("Connect".into()));
        assert!(e.can_delete_item());
        e.update(MenuMsg::DeleteItem);
        assert_eq!(
            e.list[0]
                .items
                .iter()
                .filter(|i| i.cmd == "Connect")
                .count(),
            1
        );
    }

    #[test]
    fn customize_menu_and_its_menu_stay_shown() {
        let mut e = MenuEdit::new(&[]);
        let tools = at(&e, "Tools");
        e.update(MenuMsg::ShowMenu(tools, false));
        assert!(e.list[tools].shown);
        e.update(MenuMsg::SelectMenu(tools));
        let c = e.list[tools]
            .items
            .iter()
            .position(|i| i.cmd == CUSTOMIZE)
            .unwrap();
        e.update(MenuMsg::ShowItem(c, false));
        assert!(e.list[tools].items[c].shown);
        // Moved into a hidden menu: that menu shows.
        let help = at(&e, "Help");
        e.update(MenuMsg::ShowMenu(help, false));
        e.update(MenuMsg::SelectItem(c));
        e.update(MenuMsg::MoveTo(help));
        assert!(e.list[help].shown);
        e.update(MenuMsg::ShowMenu(tools, false));
        assert!(!e.list[tools].shown, "Tools no longer holds it");
    }

    #[test]
    fn own_menu_and_items() {
        let mut e = MenuEdit::new(&[]);
        e.update(MenuMsg::AddMenu);
        assert_eq!((e.menu, e.list[1].id.as_str()), (1, "own-1"));
        e.update(MenuMsg::MenuTitle("Мои".into()));
        e.update(MenuMsg::AddProgram);
        e.update(MenuMsg::Cmd("htop".into()));
        e.update(MenuMsg::Params("-d 10".into()));
        e.update(MenuMsg::Label("Процессы".into()));
        e.update(MenuMsg::AddSep);
        assert_eq!(cmds(&e.list[1]), ["htop", SEP]);
        assert!(e.list[1].items[0].is_own());
        // A stock function into the own menu, then the menu deleted: it goes back.
        e.update(MenuMsg::SelectMenu(0));
        e.update(MenuMsg::SelectItem(0));
        e.update(MenuMsg::MoveTo(1));
        e.update(MenuMsg::AddMenu);
        assert_eq!(e.list[2].id, "own-2");
        e.update(MenuMsg::SelectMenu(1));
        assert!(e.can_delete_menu());
        e.update(MenuMsg::DeleteMenu);
        assert!(e.list.iter().all(|m| m.id != "own-1"));
        assert_eq!(cmds(&e.list[0]).last(), Some(&"Connect"));
        e.update(MenuMsg::SelectMenu(0));
        assert!(!e.can_delete_menu(), "a stock menu is never deleted");
    }

    #[test]
    fn regression_customize_menu_never_ends_up_hidden() {
        // A second copy added to a hidden menu (then the first could be deleted).
        let mut e = MenuEdit::new(&[]);
        let help = at(&e, "Help");
        e.update(MenuMsg::ShowMenu(help, false));
        e.update(MenuMsg::SelectMenu(help));
        e.update(MenuMsg::AddFunction(CUSTOMIZE.into()));
        assert!(e.list[help].shown);
        // An own menu holding it deleted: it goes back to Tools, which shows again.
        let mut e = MenuEdit::new(&[]);
        e.update(MenuMsg::AddMenu);
        let own = e.menu;
        let tools = at(&e, "Tools");
        e.update(MenuMsg::SelectMenu(tools));
        let c = (e.list[tools].items.iter())
            .position(|i| i.cmd == CUSTOMIZE)
            .unwrap();
        e.update(MenuMsg::SelectItem(c));
        e.update(MenuMsg::MoveTo(own));
        e.update(MenuMsg::ShowMenu(tools, false));
        assert!(!e.list[tools].shown);
        e.update(MenuMsg::SelectMenu(own));
        e.update(MenuMsg::DeleteMenu);
        let tools = at(&e, "Tools");
        assert!(e.list[tools].shown && e.list[tools].has_customize());
    }

    #[test]
    fn regression_program_command_may_spell_a_function_name() {
        // Typing «Connect.py» passed through «Connect», which was refused.
        let mut e = MenuEdit::new(&[]);
        e.update(MenuMsg::AddProgram);
        for t in ["C", "Connect", "Connect.py"] {
            e.update(MenuMsg::Cmd(t.into()));
        }
        let it = &e.list[0].items[e.item.unwrap()];
        assert_eq!(it.cmd, "Connect.py");
        assert!(it.is_own() && it.is_script());
        e.update(MenuMsg::Cmd("Connect".into()));
        assert!(e.list[0].items[e.item.unwrap()].is_own(), "still a program");
    }

    #[test]
    fn stock_function_command_is_fixed() {
        let mut e = MenuEdit::new(&[]);
        e.update(MenuMsg::SelectItem(0));
        e.update(MenuMsg::Cmd("rm".into()));
        e.update(MenuMsg::Label("Подключиться".into()));
        assert_eq!(e.list[0].items[0].cmd, "Connect");
        assert_eq!(e.list[0].items[0].label, "Подключиться");
    }

    #[test]
    fn reset_and_ok_of_the_stock_menu_store_nothing() {
        let mut e = MenuEdit::new(&[]);
        e.update(MenuMsg::AddMenu);
        e.update(MenuMsg::Reset);
        assert_eq!(e.list, default_menus());
        assert!(e.ok().is_empty());
    }

    #[test]
    fn toml_round_trip_keeps_hidden_flags() {
        #[derive(Serialize, Deserialize)]
        struct W {
            menus: Vec<Menu>,
        }
        let mut m = default_menus();
        m[0].shown = false;
        m[0].items[1].shown = false;
        let text = toml::to_string(&W { menus: m.clone() }).unwrap();
        let back: W = toml::from_str(&text).unwrap();
        assert_eq!(back.menus, m);
    }
}
