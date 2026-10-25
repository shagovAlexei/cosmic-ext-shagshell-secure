//! Toolbar under the header, as in SecureCRT. A button without an action waits for a later phase.

use crate::app::Message;
use crate::fl;
use crate::menu::Action;
use cosmic::Element;
use cosmic::widget::{self, tooltip};
use shagshell_core::toolbar::{ToolButton, ToolEdit, ToolMsg, matching};

/// The Connect Bar's field (focused through it, so the terminal lets go of the keys).
pub fn bar_id() -> widget::Id {
    static ID: std::sync::LazyLock<widget::Id> = std::sync::LazyLock::new(widget::Id::unique);
    ID.clone()
}

/// The app functions a toolbar button can run: (name in the config, icon, action, label).
pub type Tool = (&'static str, &'static str, Action, fn() -> String);
pub const TOOLS: [Tool; 26] = [
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
        "document-properties-symbolic",
        Action::SessionOptions,
        || fl!("menu-session-options"),
    ),
    ("Copy", "edit-copy-symbolic", Action::Copy, || {
        fl!("menu-copy")
    }),
    ("Paste", "edit-paste-symbolic", Action::Paste, || {
        fl!("menu-paste")
    }),
    ("Find", "edit-find-symbolic", Action::Find, || {
        fl!("menu-find")
    }),
    (
        "CommandWindow",
        "input-keyboard-symbolic",
        Action::ToggleCommandWindow,
        || fl!("menu-command-window"),
    ),
    (
        "CommandManager",
        "view-list-bullet-symbolic",
        Action::ToggleCommandManager,
        || fl!("menu-command-manager"),
    ),
    (
        "RunScript",
        "system-run-symbolic",
        Action::RunScript,
        || fl!("menu-script-run"),
    ),
    (
        "ScriptConsole",
        "utilities-system-monitor-symbolic",
        Action::ScriptConsole,
        || fl!("menu-script-console"),
    ),
    ("Scheduler", "alarm-symbolic", Action::Scheduler, || {
        fl!("menu-script-scheduler")
    }),
    (
        "SplitRight",
        "view-dual-symbolic",
        Action::SplitRight,
        || fl!("menu-split-right"),
    ),
    (
        "NewWindow",
        "window-new-symbolic",
        Action::DetachTab,
        || fl!("menu-tab-window"),
    ),
    ("ZoomIn", "zoom-in-symbolic", Action::ZoomIn, || {
        fl!("menu-zoom-in")
    }),
    ("ZoomOut", "zoom-out-symbolic", Action::ZoomOut, || {
        fl!("menu-zoom-out")
    }),
    (
        "FullScreen",
        "view-fullscreen-symbolic",
        Action::FullScreen,
        || fl!("menu-full-screen"),
    ),
    (
        "Credentials",
        "system-users-symbolic",
        Action::Credentials,
        || fl!("menu-credentials"),
    ),
    (
        "CreateKey",
        "dialog-password-symbolic",
        Action::CreateKey,
        || fl!("menu-create-key"),
    ),
    ("KeyMap", "input-keyboard-symbolic", Action::KeyMap, || {
        fl!("menu-keymap")
    }),
    (
        "Notes",
        "accessories-text-editor-symbolic",
        Action::Notes,
        || fl!("menu-notes"),
    ),
    (
        "GlobalOptions",
        "emblem-system-symbolic",
        Action::GlobalOptions,
        || fl!("menu-global-options"),
    ),
];

pub fn function(name: &str) -> Option<&'static Tool> {
    TOOLS.iter().find(|t| t.0 == name.trim())
}

/// The toolbar until the user changes it: SecureCRT's groups.
pub fn default_bar() -> Vec<ToolButton> {
    let f = ToolButton::function;
    vec![
        f("SessionManager"),
        ToolButton::sep(),
        f("Connect"),
        f("QuickConnect"),
        f("LocalShell"),
        ToolButton::sep(),
        f("Reconnect"),
        f("Disconnect"),
        ToolButton::sep(),
        f("Sftp"),
        f("SessionOptions"),
    ]
}

/// The bar: the config's list, else the default one.
pub fn buttons(config: &[ToolButton]) -> Vec<ToolButton> {
    match config.is_empty() {
        true => default_bar(),
        false => config.to_vec(),
    }
}

/// A button's icon: its own, else its function's (a program: the generic one).
pub fn icon(b: &ToolButton) -> String {
    match (b.icon.as_str(), function(&b.cmd)) {
        ("", Some(t)) => t.1.to_string(),
        ("", None) => "application-x-executable-symbolic".into(),
        (i, _) => i.to_string(),
    }
}

/// A button's hint: its own, else its function's name (a program: its command).
pub fn tip(b: &ToolButton) -> String {
    match (b.tip.as_str(), function(&b.cmd)) {
        ("", Some(t)) => (t.3)(),
        ("", None) => b.cmd.clone(),
        (t, _) => t.to_string(),
    }
}

/// `bar`: what is typed in the Connect Bar, `list`: the sessions it matches (shown under it),
/// `picked`: the row chosen with the arrows; `tools`: the bar's buttons. A right click on the bar
/// opens its editor.
pub fn view(
    bar: &str,
    list: &[String],
    picked: Option<usize>,
    tools: &[ToolButton],
) -> Element<'static, Message> {
    let row = (tools.iter().enumerate()).map(|(i, b)| -> Element<'static, Message> {
        if b.is_sep() {
            return widget::Space::new().width(10.0).into();
        }
        let button = widget::button::icon(widget::icon::from_name(icon(b)))
            .on_press(Message::Toolbar(TbMsg::Run(i)));
        widget::tooltip(button, widget::text(tip(b)), tooltip::Position::Bottom).into()
    });
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
    let row = widget::row::with_children(items)
        .spacing(4)
        .padding([4, 8])
        .width(cosmic::iced::Length::Fill)
        .align_y(cosmic::iced::Alignment::Center);
    widget::mouse_area(row)
        .on_right_press(Message::Toolbar(TbMsg::Open))
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
    fn the_default_bar_runs_known_functions() {
        let bar = default_bar();
        assert_eq!(bar.iter().filter(|b| b.is_sep()).count(), 3);
        assert!(bar.iter().all(|b| b.is_sep() || function(&b.cmd).is_some()));
        let mut seen = std::collections::HashSet::new();
        assert!(TOOLS.iter().all(|t| seen.insert(t.0)), "names are unique");
        assert_eq!(buttons(&[]), bar);
        let program = ToolButton::function("xterm");
        assert_eq!(icon(&program), "application-x-executable-symbolic");
        assert_eq!(tip(&program), "xterm");
    }
}

/// The toolbar's messages: a button pressed, its editor (opened by a right click or Global
/// Options).
#[derive(Clone, Debug)]
pub enum TbMsg {
    Run(usize),
    Open,
    Edit(ToolMsg),
    Ok,
    Cancel,
}

/// The icon picker draws at most this many icons; the filter narrows the rest down.
const PICKER_MAX: usize = 240;
const PICKER_COLS: usize = 12;

/// The symbolic icons of the current theme and its usual fallbacks. Slow (every name goes
/// through the icon lookup): off the UI thread, once.
// ponytail: kept for the process; a theme switch needs a restart to show its own set.
pub fn icon_names() -> Vec<String> {
    static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    NAMES
        .get_or_init(|| {
            let home = std::env::home_dir();
            let data_home = std::env::var_os("XDG_DATA_HOME")
                .map(std::path::PathBuf::from)
                .or_else(|| home.as_ref().map(|h| h.join(".local/share")));
            let data_dirs = std::env::var("XDG_DATA_DIRS")
                .ok()
                .filter(|d| !d.is_empty())
                .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
            let roots: Vec<_> = (home.map(|h| h.join(".icons")).into_iter())
                .chain(data_home.map(|d| d.join("icons")))
                .chain(
                    data_dirs
                        .split(':')
                        .map(|d| std::path::Path::new(d).join("icons")),
                )
                .collect();
            let theme = cosmic::icon_theme::default();
            let mut names =
                shagshell_core::icons::symbolic(&roots, &[&theme, "Adwaita", "hicolor"]);
            // Only what the icon loader finds (a file outside its lookup chain draws blank).
            names.retain(|n| widget::icon::from_name(n.as_str()).path().is_some());
            names
        })
        .clone()
}

fn picker_view<'a>(all: &'a [String], filter: &'a str) -> Element<'a, Message> {
    use cosmic::iced::widget::{column, row};
    let msg = |m| Message::Toolbar(TbMsg::Edit(m));
    let (shown, total) = matching(all, filter, PICKER_MAX);
    let cell = |name: &'a str| -> Element<'a, Message> {
        widget::tooltip(
            widget::button::icon(widget::icon::from_name(name).size(24))
                .on_press(msg(ToolMsg::Pick(name.into()))),
            widget::text(name).size(12),
            tooltip::Position::Top,
        )
        .into()
    };
    // Rows by hand: flex_row in a scrollable lays everything out on one line.
    let grid = column(
        shown
            .chunks(PICKER_COLS)
            .map(|r| row(r.iter().map(|&n| cell(n))).spacing(4).into()),
    )
    .spacing(4);
    widget::dialog()
        .title(fl!("toolbar-pick-title"))
        .control(
            column![
                widget::text_input(fl!("toolbar-pick-filter"), filter)
                    .on_input(move |s| msg(ToolMsg::PickFilter(s))),
                widget::container(
                    widget::scrollable(grid).height(cosmic::iced::Length::Fixed(250.0))
                )
                .padding(6)
                .width(cosmic::iced::Length::Fill)
                .class(cosmic::theme::Container::Card),
                widget::text::caption(match all.is_empty() {
                    true => fl!("toolbar-pick-loading"),
                    false => fl!("toolbar-pick-count", shown = shown.len(), total = total),
                }),
            ]
            .spacing(8)
            .width(cosmic::iced::Length::Fill),
        )
        .secondary_action(
            widget::button::standard(fl!("toolbar-pick-back")).on_press(msg(ToolMsg::PickClose)),
        )
        .into()
}

/// The «Toolbar» window (shagoff-commander's «Change button bar»): the buttons on the left,
/// what to do with them beside, the selected one's fields under.
pub fn editor<'a>(t: &'a ToolEdit, labels: &'a [String]) -> Element<'a, Message> {
    use cosmic::iced::widget::{column, row};
    use cosmic::iced::{Alignment, Length};
    let msg = |m| Message::Toolbar(TbMsg::Edit(m));
    if let Some((all, filter)) = &t.picker {
        return picker_view(all, filter);
    }
    let mut list = column![].spacing(1);
    for (i, b) in t.list.iter().enumerate() {
        let selected = t.sel == Some(i);
        let line: Element<'a, Message> = match b.is_sep() {
            true => widget::divider::horizontal::default().into(),
            false => row![
                widget::icon::from_name(icon(b)).size(16),
                widget::text::body(tip(b)),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
        };
        list = list.push(
            widget::button::custom(widget::container(line).padding([2, 4]).width(Length::Fill))
                .class(match selected {
                    true => cosmic::theme::Button::Standard,
                    false => cosmic::theme::Button::MenuItem,
                })
                .width(Length::Fill)
                .on_press(msg(ToolMsg::Select(i))),
        );
    }
    let list = widget::container(widget::scrollable(list).height(Length::Fixed(220.0)))
        .padding(4)
        .width(Length::Fill)
        .class(cosmic::theme::Container::Card);
    let sel = t.sel.filter(|&i| i < t.list.len());
    let btn = |label: String, m: ToolMsg, on: bool| {
        widget::button::standard(label)
            .width(Length::Fixed(170.0))
            .on_press_maybe(on.then(|| msg(m)))
    };
    let arrow = |name: &'static str, m: ToolMsg, on: bool| {
        widget::button::icon(widget::icon::from_name(name).size(16))
            .on_press_maybe(on.then(|| msg(m)))
    };
    let buttons = column![
        btn(fl!("toolbar-add"), ToolMsg::Add, true),
        btn(fl!("toolbar-add-sep"), ToolMsg::AddSep, true),
        btn(fl!("toolbar-delete"), ToolMsg::Delete, sel.is_some()),
        row![
            arrow("go-up-symbolic", ToolMsg::Up, sel.is_some_and(|i| i > 0)),
            arrow(
                "go-down-symbolic",
                ToolMsg::Down,
                sel.is_some_and(|i| i + 1 < t.list.len())
            ),
        ]
        .spacing(6),
    ]
    .spacing(6);
    let editable = sel.map(|i| &t.list[i]).filter(|b| !b.is_sep());
    let field = |label: String, hint: String, value: &'a str, on: Option<fn(String) -> ToolMsg>| {
        let mut input = widget::text_input(hint, value);
        if let Some(f) = on {
            input = input.on_input(move |s| msg(f(s)));
        }
        row![widget::text::body(label).width(Length::Fixed(110.0)), input]
            .spacing(6)
            .align_y(Alignment::Center)
    };
    let on = |f: fn(String) -> ToolMsg| editable.map(|_| f);
    let b = editable.cloned().unwrap_or_default();
    let internal = editable.and_then(|b| TOOLS.iter().position(|t| t.0 == b.cmd));
    let (cmd, params, icon_name, tip_text) = match editable {
        Some(b) => (&*b.cmd, &*b.params, &*b.icon, &*b.tip),
        None => ("", "", "", ""),
    };
    let fields = column![
        row![
            field(
                fl!("toolbar-command"),
                fl!("toolbar-command-hint"),
                cmd,
                on(ToolMsg::Cmd)
            ),
            widget::dropdown(labels, internal, move |i| {
                msg(ToolMsg::Function(TOOLS[i].0.to_string()))
            })
            .width(Length::Fixed(220.0)),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
        field(
            fl!("toolbar-params"),
            fl!("toolbar-params-hint"),
            params,
            on(ToolMsg::Params).filter(|_| internal.is_none())
        ),
        row![
            field(fl!("toolbar-icon"), icon(&b), icon_name, on(ToolMsg::Icon)),
            widget::button::standard(fl!("toolbar-pick"))
                .leading_icon(widget::icon::from_name(icon(&b)).size(16))
                .on_press_maybe(editable.map(|_| msg(ToolMsg::PickOpen))),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
        field(fl!("toolbar-tip"), tip(&b), tip_text, on(ToolMsg::Tip)),
    ]
    .spacing(6);
    widget::dialog()
        .title(fl!("toolbar-title"))
        .control(
            column![row![list, buttons].spacing(12), fields]
                .spacing(10)
                .width(Length::Fill),
        )
        .primary_action(widget::button::suggested(fl!("ok")).on_press(Message::Toolbar(TbMsg::Ok)))
        .secondary_action(
            widget::button::standard(fl!("cancel")).on_press(Message::Toolbar(TbMsg::Cancel)),
        )
        .tertiary_action(
            widget::button::standard(fl!("toolbar-reset")).on_press(msg(ToolMsg::Reset)),
        )
        .into()
}
