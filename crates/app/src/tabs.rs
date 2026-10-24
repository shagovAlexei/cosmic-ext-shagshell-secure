//! Tabs and tab groups (SecureCRT "tab groups"): a `pane_grid` of groups, each a tab strip over
//! its active tab's terminal. Tabs are kept by id; a group's model holds the ids in order.

use crate::app::Message;
use crate::config::Config;
use crate::fl;
use crate::menu::Action;
use crate::term::{self, Events, Outcome, TabId, terminal::Terminal};
use alacritty_terminal::grid::Dimensions;
use cosmic::app::Task;
use cosmic::iced::Length;
use cosmic::iced::keyboard::{Key, Modifiers, key::Physical};
use cosmic::widget::segmented_button::{Entity, ReorderEvent, SingleSelectModel};
use cosmic::widget::{self, menu, pane_grid, tab_bar};
use cosmic::{Element, iced};
use shagshell_core::tabs as rules;
use std::collections::HashMap;
use std::sync::Mutex;

const TAB_MIME: &str = "application/x-shagshell-tab";

/// Who a tab talks to: the status bar's protocol.
#[derive(Clone, Debug)]
pub enum Protocol {
    Local,
    /// `session`: the saved one it was opened from (Quick Connect without saving: none).
    Ssh {
        host: String,
        session: Option<String>,
    },
}

/// What a tab runs (again on Reconnect).
#[derive(Clone, Debug)]
pub struct Launch {
    pub program: String,
    pub args: Vec<String>,
    /// Ours (a session's name, «Local Shell»), numbered if taken.
    pub title: String,
    pub protocol: Protocol,
}

impl Launch {
    pub fn local_shell() -> Self {
        let shell = shagshell_core::shell::pick(std::env::var_os("SHELL").as_deref());
        Self {
            program: shell.to_string_lossy().into_owned(),
            args: Vec::new(),
            title: fl!("tab-local-shell"),
            protocol: Protocol::Local,
        }
    }

    fn label(&self) -> String {
        match &self.protocol {
            Protocol::Local => fl!("tab-local-shell"),
            Protocol::Ssh { host, .. } => format!("SSH · {host}"),
        }
    }
}

pub struct Tab {
    pub terminal: Mutex<Terminal>,
    launch: Launch,
    /// «web-01 (2)»: ours, shown while the program sets none.
    base: String,
    /// OSC 0 / 2 from the program; empty: none.
    program_title: String,
    pub alive: bool,
    widget_id: widget::Id,
}

impl Tab {
    fn title(&self) -> String {
        let t = match self.program_title.is_empty() {
            true => &self.base,
            false => &self.program_title,
        };
        format!("{} {t}", if self.alive { "✓" } else { "✗" })
    }
}

#[derive(Default)]
pub struct Group {
    model: SingleSelectModel,
}

impl Group {
    fn active_id(&self) -> Option<TabId> {
        self.model.data::<TabId>(self.model.active()).copied()
    }
}

/// How a terminal looks: text style, dark theme or not, window background (0xAARRGGBB).
pub type Look = (term::style::Style, bool, u32);

/// The tab menu's commands: about the right-clicked tab (keys and the menu bar: the active one).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TabCmd {
    Reconnect,
    Clone,
    SplitRight,
    SplitDown,
    Close,
}

impl menu::Action for TabCmd {
    type Message = Message;
    fn message(&self) -> Message {
        Message::Tabs(TabsMsg::Cmd(*self))
    }
}

#[derive(Clone, Debug)]
pub enum TabsMsg {
    Cmd(TabCmd),
    Activate(pane_grid::Pane, Entity),
    Close(pane_grid::Pane, Entity),
    CloseConfirmed,
    CloseCancelled,
    Context(pane_grid::Pane, Entity),
    Reorder(pane_grid::Pane, ReorderEvent),
    PaneClicked(pane_grid::Pane),
    Resized(pane_grid::ResizeEvent),
    Paste(Option<String>),
}

#[derive(Default)]
pub struct Tabs {
    grid: Option<pane_grid::State<Group>>,
    focus: Option<pane_grid::Pane>,
    tabs: HashMap<TabId, Tab>,
    next_id: TabId,
    tx: Option<Events>,
    /// A live tab asked to close: waits for the dialog.
    confirm: Option<(pane_grid::Pane, Entity)>,
    /// The tab right-clicked last: `TabCmd`s are about it.
    context: Option<(pane_grid::Pane, Entity)>,
}

impl Tabs {
    pub fn set_sender(&mut self, tx: Events) {
        self.tx = Some(tx);
    }

    /// Active tab: (connected, protocol label, cols, rows).
    pub fn status(&self) -> Option<(bool, String, usize, usize)> {
        let tab = self.tabs.get(&self.active()?.2)?;
        let t = tab.terminal.lock().unwrap_or_else(|e| e.into_inner());
        let term = t.term.lock();
        let label = tab.launch.label();
        Some((tab.alive, label, term.columns(), term.screen_lines()))
    }

    /// What every tab runs (again on Reconnect / Clone).
    pub fn launches_mut(&mut self) -> impl Iterator<Item = &mut Launch> {
        self.tabs.values_mut().map(|t| &mut t.launch)
    }

    /// The saved session the active tab was opened from.
    pub fn active_session(&self) -> Option<String> {
        match &self.tabs.get(&self.active()?.2)?.launch.protocol {
            Protocol::Ssh { session, .. } => session.clone(),
            Protocol::Local => None,
        }
    }

    /// Focused group, its active entity and that tab's id.
    fn active(&self) -> Option<(pane_grid::Pane, Entity, TabId)> {
        let pane = self.focus?;
        let group = self.grid.as_ref()?.get(pane)?;
        Some((pane, group.model.active(), group.active_id()?))
    }

    /// For the menu bar: (any tab, the active one ended, its group can be split).
    pub fn menu_state(&self) -> (bool, bool, bool) {
        let Some((pane, _, id)) = self.active() else {
            return (false, false, false);
        };
        let ended = self.tabs.get(&id).is_some_and(|t| !t.alive);
        let many = (self.grid.as_ref())
            .and_then(|g| g.get(pane))
            .is_some_and(|g| g.model.len() > 1);
        (true, ended, many)
    }

    fn tab_at(&self, pane: pane_grid::Pane, e: Entity) -> Option<(pane_grid::Pane, Entity, TabId)> {
        let id = *self.grid.as_ref()?.get(pane)?.model.data::<TabId>(e)?;
        Some((pane, e, id))
    }

    fn terminal(&self, id: TabId) -> Option<&Mutex<Terminal>> {
        self.tabs.get(&id).map(|t| &t.terminal)
    }

    /// A new tab running `launch` in the focused group (the first group if none).
    pub fn open(&mut self, launch: Launch, look: &Look) -> Task<Message> {
        let taken: Vec<&str> = self.tabs.values().map(|t| t.base.as_str()).collect();
        let base = rules::numbered(&launch.title, &taken);
        let id = self.next_id;
        let Some(terminal) = self.spawn(id, &launch, look) else {
            return Task::none();
        };
        self.next_id += 1;
        let widget_id = widget::Id::unique();
        let tab = Tab {
            terminal: Mutex::new(terminal),
            launch,
            base,
            program_title: String::new(),
            alive: true,
            widget_id: widget_id.clone(),
        };
        let title = tab.title();
        self.tabs.insert(id, tab);
        let pane = match (&mut self.grid, self.focus) {
            (Some(_), Some(p)) => p,
            _ => {
                let (grid, pane) = pane_grid::State::new(Group::default());
                (self.grid, self.focus) = (Some(grid), Some(pane));
                pane
            }
        };
        if let Some(group) = self.grid.as_mut().and_then(|g| g.get_mut(pane)) {
            group
                .model
                .insert()
                .text(title)
                .data(id)
                .closable()
                .activate();
        }
        focus(widget_id)
    }

    fn spawn(&self, id: TabId, launch: &Launch, look: &Look) -> Option<Terminal> {
        let tx = self.tx.clone()?;
        let (style, dark, bg) = look;
        term::start(id, tx, &launch.program, &launch.args, *dark, *bg, style)
            .inspect_err(|e| log::error!("start {}: {e}", launch.program))
            .ok()
    }

    /// Start the program again in a tab whose program ended (SecureCRT's Reconnect / Enter).
    /// Under a new id: the old program's late events (Exit after ChildExit) find no tab.
    fn restart(&mut self, id: TabId, look: &Look) -> Task<Message> {
        if self.tabs.get(&id).is_none_or(|t| t.alive) {
            return Task::none();
        }
        let new_id = self.next_id;
        let launch = self.tabs.get(&id).map(|t| t.launch.clone());
        let Some(terminal) = launch.and_then(|l| self.spawn(new_id, &l, look)) else {
            return Task::none();
        };
        self.next_id += 1;
        let Some(mut tab) = self.tabs.remove(&id) else {
            return Task::none();
        };
        (tab.terminal, tab.alive) = (Mutex::new(terminal), true);
        tab.program_title.clear();
        let widget_id = tab.widget_id.clone();
        self.tabs.insert(new_id, tab);
        for (_, group) in self.grid.iter_mut().flat_map(|g| g.iter_mut()) {
            if let Some(e) = entity_of(&group.model, id) {
                group.model.data_set(e, new_id);
            }
        }
        self.retitle(new_id);
        focus(widget_id)
    }

    fn retitle(&mut self, id: TabId) {
        let Some(title) = self.tabs.get(&id).map(Tab::title) else {
            return;
        };
        for (_, group) in self.grid.iter_mut().flat_map(|g| g.iter_mut()) {
            if let Some(e) = entity_of(&group.model, id) {
                group.model.text_set(e, title.clone());
            }
        }
    }

    /// Close without asking.
    fn close_now(&mut self, pane: pane_grid::Pane, e: Entity) -> Task<Message> {
        let Some(grid) = self.grid.as_mut() else {
            return Task::none();
        };
        let Some(group) = grid.get_mut(pane) else {
            return Task::none();
        };
        if let Some(id) = group.model.data::<TabId>(e).copied() {
            self.tabs.remove(&id); // its Drop ends the program
        }
        match detach(&mut group.model, e) {
            false => return self.focus_active(),
            true if grid.len() > 1 => {
                if let Some((_, sibling)) = grid.close(pane) {
                    self.focus = Some(sibling);
                }
            }
            true => (self.grid, self.focus) = (None, None),
        }
        self.focus_active()
    }

    fn focus_active(&self) -> Task<Message> {
        match self.active().and_then(|(_, _, id)| self.tabs.get(&id)) {
            Some(t) => focus(t.widget_id.clone()),
            None => Task::none(),
        }
    }

    /// That tab into a new group beside its own; refused when it is alone in its group.
    fn split(
        &mut self,
        (pane, e, id): (pane_grid::Pane, Entity, TabId),
        axis: pane_grid::Axis,
    ) -> Task<Message> {
        let Some(grid) = self.grid.as_mut() else {
            return Task::none();
        };
        let title = self.tabs.get(&id).map(Tab::title).unwrap_or_default();
        let Some(group) = grid.get_mut(pane) else {
            return Task::none();
        };
        if group.model.len() < 2 {
            return Task::none();
        }
        detach(&mut group.model, e);
        let mut new = Group::default();
        new.model
            .insert()
            .text(title)
            .data(id)
            .closable()
            .activate();
        if let Some((new_pane, _)) = grid.split(axis, pane, new) {
            self.focus = Some(new_pane);
        }
        self.focus_active()
    }

    /// A tab command on tab `t` (right-clicked, or the active one).
    fn cmd(
        &mut self,
        cmd: TabCmd,
        t: (pane_grid::Pane, Entity, TabId),
        look: &Look,
    ) -> Task<Message> {
        match cmd {
            TabCmd::Reconnect => self.restart(t.2, look),
            TabCmd::Clone => {
                self.focus = Some(t.0); // its group
                match self.tabs.get(&t.2).map(|tab| tab.launch.clone()) {
                    Some(launch) => self.open(launch, look),
                    None => Task::none(),
                }
            }
            TabCmd::SplitRight => self.split(t, pane_grid::Axis::Vertical),
            TabCmd::SplitDown => self.split(t, pane_grid::Axis::Horizontal),
            TabCmd::Close => self.update(TabsMsg::Close(t.0, t.1), look),
        }
    }

    /// Actions about tabs from keys and the menu bar: on the active tab. `None`: not ours.
    pub fn act(&mut self, action: Action, look: &Look) -> Option<Task<Message>> {
        let on_active = |this: &mut Self, cmd| match this.active() {
            Some(t) => this.cmd(cmd, t, look),
            None => Task::none(),
        };
        let task = match action {
            Action::NewLocalShell => self.open(Launch::local_shell(), look),
            Action::CloseTab => on_active(self, TabCmd::Close),
            Action::Reconnect => on_active(self, TabCmd::Reconnect),
            Action::NextTab | Action::PrevTab => {
                if let Some(group) = self.focused_group_mut() {
                    let len = group.model.len();
                    let pos = group.model.position(group.model.active()).unwrap_or(0) as usize;
                    let next = rules::cycle(pos, len, action == Action::NextTab);
                    group.model.activate_position(next as u16);
                }
                self.focus_active()
            }
            Action::SelectTab(n) => {
                if let Some(group) = self.focused_group_mut()
                    && usize::from(n) < group.model.len()
                {
                    group.model.activate_position(u16::from(n));
                }
                self.focus_active()
            }
            Action::SplitRight => on_active(self, TabCmd::SplitRight),
            Action::SplitDown => on_active(self, TabCmd::SplitDown),
            Action::Copy => {
                let text = self.active().and_then(|(_, _, id)| {
                    let t = self.terminal(id)?.lock().unwrap_or_else(|e| e.into_inner());
                    t.term.lock().selection_to_string()
                });
                match text {
                    Some(text) => cosmic::iced::clipboard::write(text),
                    None => Task::none(),
                }
            }
            Action::Paste => cosmic::iced::clipboard::read()
                .map(|t| cosmic::Action::App(Message::Tabs(TabsMsg::Paste(t)))),
            Action::SelectAll => {
                if let Some((_, _, id)) = self.active()
                    && let Some(t) = self.terminal(id)
                {
                    t.lock().unwrap_or_else(|e| e.into_inner()).select_all();
                }
                Task::none()
            }
            _ => return None,
        };
        Some(task)
    }

    fn focused_group_mut(&mut self) -> Option<&mut Group> {
        let pane = self.focus?;
        self.grid.as_mut()?.get_mut(pane)
    }

    /// Enter on a tab whose program ended: start it again. `false`: not such a tab.
    pub fn enter_on_dead(&self) -> bool {
        self.active()
            .and_then(|(_, _, id)| self.tabs.get(&id))
            .is_some_and(|t| !t.alive)
    }

    pub fn on_term(&mut self, id: TabId, event: alacritty_terminal::event::Event) -> Task<Message> {
        let Some(tab) = self.tabs.get_mut(&id) else {
            return Task::none();
        };
        match term::on_event(&tab.terminal, event) {
            Outcome::None => {}
            Outcome::Copy(text) => return cosmic::iced::clipboard::write(text),
            Outcome::Title(title) => {
                tab.program_title = title;
                self.retitle(id);
            }
            // ChildExit then Exit: told once.
            Outcome::Exit(code) if tab.alive => {
                tab.alive = false;
                let line = match code {
                    Some(code) => fl!("term-exited-code", code = code.to_string()),
                    None => fl!("term-exited"),
                };
                term::print(&tab.terminal, &format!("\r\n[{line}]\r\n"));
                self.retitle(id);
            }
            Outcome::Exit(_) => {}
        }
        Task::none()
    }

    pub fn restyle(&self, look: &Look) {
        let (style, dark, bg) = look;
        for t in self.tabs.values() {
            term::restyle(&t.terminal, style, *dark, *bg);
        }
    }

    pub fn update(&mut self, msg: TabsMsg, look: &Look) -> Task<Message> {
        match msg {
            TabsMsg::Cmd(cmd) => {
                let target = self.context.take().and_then(|(p, e)| self.tab_at(p, e));
                if let Some(t) = target {
                    return self.cmd(cmd, t, look);
                }
            }
            TabsMsg::Activate(pane, e) => {
                self.focus = Some(pane);
                if let Some(group) = self.grid.as_mut().and_then(|g| g.get_mut(pane)) {
                    group.model.activate(e);
                }
                return self.focus_active();
            }
            TabsMsg::Close(pane, e) => {
                let alive = (self.grid.as_ref())
                    .and_then(|g| g.get(pane)?.model.data::<TabId>(e).copied())
                    .and_then(|id| self.tabs.get(&id))
                    .is_some_and(|t| t.alive);
                if alive {
                    self.confirm = Some((pane, e));
                } else {
                    return self.close_now(pane, e);
                }
            }
            TabsMsg::CloseConfirmed => {
                if let Some((pane, e)) = self.confirm.take() {
                    return self.close_now(pane, e);
                }
            }
            TabsMsg::CloseCancelled => {
                self.confirm = None;
                return self.focus_active();
            }
            TabsMsg::Context(pane, e) => self.context = Some((pane, e)),
            TabsMsg::Reorder(pane, ev) => {
                if let Some(group) = self.grid.as_mut().and_then(|g| g.get_mut(pane)) {
                    group.model.reorder(ev.dragged, ev.target, ev.position);
                }
            }
            TabsMsg::PaneClicked(pane) => {
                self.focus = Some(pane);
                return self.focus_active();
            }
            TabsMsg::Resized(ev) => {
                if let Some(grid) = self.grid.as_mut() {
                    grid.resize(ev.split, ev.ratio);
                }
            }
            TabsMsg::Paste(Some(text)) => {
                if let Some((_, _, id)) = self.active()
                    && let Some(t) = self.terminal(id)
                {
                    t.lock().unwrap_or_else(|e| e.into_inner()).paste(text);
                }
            }
            TabsMsg::Paste(None) => {}
        }
        Task::none()
    }

    /// Asks before closing a live tab, as SecureCRT's "Confirm disconnect".
    pub fn dialog(&self) -> Option<Element<'_, Message>> {
        let (pane, e) = self.confirm?;
        let id = *self.grid.as_ref()?.get(pane)?.model.data::<TabId>(e)?;
        let name = self.tabs.get(&id)?.title();
        let name = name.trim_start_matches(['✓', '✗', ' ']).to_string();
        let msg = |m| Message::Tabs(m);
        Some(
            widget::dialog()
                .title(fl!("close-tab-title", name = name))
                .body(fl!("close-tab-body"))
                .primary_action(
                    widget::button::destructive(fl!("close"))
                        .on_press(msg(TabsMsg::CloseConfirmed)),
                )
                .secondary_action(
                    widget::button::standard(fl!("cancel")).on_press(msg(TabsMsg::CloseCancelled)),
                )
                .into(),
        )
    }

    pub fn view(&self, config: &Config, window: iced::window::Id) -> Option<Element<'_, Message>> {
        let grid = self.grid.as_ref()?;
        let focus = self.focus;
        let many = grid.len() > 1;
        let copy = config.copy_on_select;
        let middle = config.paste_on_middle_click;
        let pg = pane_grid::PaneGrid::new(grid, move |pane, group, _| {
            let m =
                move |f: fn(pane_grid::Pane, Entity) -> TabsMsg| move |e| Message::Tabs(f(pane, e));
            let bar = tab_bar::horizontal(&group.model)
                .on_activate(m(TabsMsg::Activate))
                .on_close(m(TabsMsg::Close))
                .on_middle_press(m(TabsMsg::Close))
                .on_context(m(TabsMsg::Context))
                .context_menu(Some(tab_menu(&group.model, &self.tabs)))
                .window_id(window)
                .on_surface_action(Message::Surface)
                .enable_tab_drag(TAB_MIME.to_string())
                .on_reorder(move |ev| Message::Tabs(TabsMsg::Reorder(pane, ev)));
            let body: Element<'_, Message> =
                match group.active_id().and_then(|id| self.tabs.get(&id)) {
                    Some(tab) => term::terminal_box::terminal_box(&tab.terminal, is_app_key)
                        .id(tab.widget_id.clone())
                        .copy_on_select(copy)
                        .paste_on_middle(middle)
                        // An ended program takes no keys (its PTY is gone): Enter reaches the app.
                        .ended(!tab.alive)
                        .padding(4)
                        .into(),
                    None => widget::Space::new()
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .into(),
                };
            let active = many && focus == Some(pane);
            let content = widget::container(widget::column::with_children(vec![bar.into(), body]))
                .padding(1)
                .class(cosmic::theme::Container::custom(move |theme| {
                    let mut s = widget::container::Style::default();
                    if active {
                        s.border.color = theme.cosmic().accent_color().into();
                        s.border.width = 1.0;
                    }
                    s
                }));
            pane_grid::Content::new(content)
        })
        .on_click(|p| Message::Tabs(TabsMsg::PaneClicked(p)))
        .on_resize(10, |ev| Message::Tabs(TabsMsg::Resized(ev)))
        .spacing(4);
        Some(pg.into())
    }
}

fn entity_of(model: &SingleSelectModel, id: TabId) -> Option<Entity> {
    model.iter().find(|&e| model.data::<TabId>(e) == Some(&id))
}

fn focus(id: widget::Id) -> Task<Message> {
    cosmic::iced::runtime::widget::operation::focus(id)
}

/// Remove `e` from its group; the active tab moves only if it was `e` (to its right
/// neighbour, else the left one). `true`: the group is now empty.
fn detach(model: &mut SingleSelectModel, e: Entity) -> bool {
    let was_active = model.is_active(e);
    let pos = model.position(e).unwrap_or(0) as usize;
    let len = model.len();
    model.remove(e);
    match rules::after_close(pos, len) {
        Some(next) if was_active => {
            model.activate_position(next as u16);
            false
        }
        Some(_) => false,
        None => true,
    }
}

/// Right click on a tab: about that tab (`TabsMsg::Context` came first). libcosmic wants one
/// root whose children are the tabs' menus, one per tab in order.
fn tab_menu(model: &SingleSelectModel, tabs: &HashMap<TabId, Tab>) -> Vec<menu::Tree<Message>> {
    use menu::Item::{Button, ButtonDisabled, Divider};
    let can_split = model.len() > 1;
    let on = |yes: bool, label: String, cmd| match yes {
        true => Button(label, None, cmd),
        false => ButtonDisabled(label, None, cmd),
    };
    let one = |e: Entity| {
        let ended = (model.data::<TabId>(e))
            .and_then(|id| tabs.get(id))
            .is_some_and(|t| !t.alive);
        let items = vec![
            on(ended, fl!("menu-reconnect"), TabCmd::Reconnect),
            Button(fl!("menu-clone"), None, TabCmd::Clone),
            Divider,
            on(can_split, fl!("menu-split-right"), TabCmd::SplitRight),
            on(can_split, fl!("menu-split-down"), TabCmd::SplitDown),
            Divider,
            Button(fl!("menu-close-tab"), None, TabCmd::Close),
        ];
        let blank = Element::from(widget::Space::new());
        menu::Tree::with_children(blank, menu::items(&HashMap::new(), items))
    };
    let blank = Element::from(widget::Space::new());
    vec![menu::Tree::with_children(
        blank,
        model.iter().map(one).collect::<Vec<_>>(),
    )]
}

/// Keys the terminal leaves to the app: exactly those `app::key_action` acts on (by physical
/// key, any layout). Everything else, every Ctrl+letter and Alt+letter, is the terminal's.
pub fn is_app_key(mods: Modifiers, _: &Key, key: &Physical) -> bool {
    crate::app::key_action(key, mods).is_some()
}
