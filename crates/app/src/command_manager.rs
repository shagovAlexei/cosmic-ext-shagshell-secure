//! View → Command Manager: the saved commands (`core::commands`) in a pane at the right of the
//! tabs, and the dialogs that edit them. A double click sends the command; the app saves the
//! file and does the sending.

use crate::app::Message;
use crate::fl;
use cosmic::Element;
use cosmic::iced::{Alignment, Length};
use cosmic::widget;
use shagshell_core::buttonbar::Target;
use shagshell_core::commands::{Addr, Command, Error, Folder, Row};
use std::collections::HashSet;

#[derive(Clone, Debug)]
pub enum CmMsg {
    Filter(String),
    ClearFilter,
    Select(Addr),
    Toggle(Vec<String>),
    /// A double click, Enter or the Send button: send the command at this address.
    Send(Addr),
    AddCommand,
    AddFolder,
    Edit,
    Delete,
    Shift(isize),
    Label(String),
    Text(String),
    TargetSel(usize),
    Name(String),
    Ok,
    Cancel,
}

enum Dialog {
    Command {
        parent: Vec<usize>,
        at: Option<usize>,
        cmd: Command,
    },
    /// A new folder in `parent`, or (`rename`) this one renamed.
    Folder {
        parent: Vec<usize>,
        rename: Option<Vec<usize>>,
        name: String,
    },
    Delete(Addr, String),
}

pub struct CommandManager {
    pub tree: Folder,
    /// The file could not be read: nothing is saved over it.
    pub broken: Option<String>,
    open: HashSet<Vec<String>>,
    filter: String,
    selected: Option<Addr>,
    dialog: Option<Dialog>,
    error: Option<String>,
    filter_id: widget::Id,
    input_id: widget::Id,
    targets: Vec<String>,
}

/// What the app does after an update.
#[derive(Default)]
pub struct Outcome {
    /// Send this command.
    pub send: Option<Command>,
    /// The tree changed: write the file.
    pub save: bool,
    /// Focus this field.
    pub focus: Option<widget::Id>,
}

impl CommandManager {
    pub fn new() -> Self {
        let (tree, broken) = match Folder::load(&Folder::default_path()) {
            Ok(t) => (t, None),
            Err(e) => (Folder::default(), Some(format!("commands.toml: {e}"))),
        };
        Self {
            tree,
            broken,
            open: HashSet::new(),
            filter: String::new(),
            selected: None,
            dialog: None,
            error: None,
            filter_id: widget::Id::unique(),
            input_id: widget::Id::unique(),
            targets: vec![fl!("cw-current"), fl!("cw-all")],
        }
    }

    pub fn has_dialog(&self) -> bool {
        self.dialog.is_some()
    }

    pub fn filter_id(&self) -> widget::Id {
        self.filter_id.clone()
    }

    /// Commands brought in by an import: merged, the number added.
    pub fn merge(&mut self, other: Folder) -> usize {
        self.tree.merge(other)
    }

    /// The folder the next new thing goes into: the selected folder, or the one a selected
    /// command is in, or the top.
    fn parent(&self) -> Vec<usize> {
        self.selected
            .as_ref()
            .map(|a| a.folders.clone())
            .unwrap_or_default()
    }

    pub fn update(&mut self, msg: CmMsg) -> Outcome {
        let mut out = Outcome::default();
        self.error = None;
        match msg {
            CmMsg::Filter(f) => self.filter = f,
            CmMsg::ClearFilter => self.filter.clear(),
            CmMsg::Select(a) => self.selected = Some(a),
            CmMsg::Toggle(key) => {
                if !self.open.remove(&key) {
                    self.open.insert(key);
                }
            }
            CmMsg::Send(a) => out.send = self.tree.command(&a).cloned(),
            CmMsg::AddCommand => {
                self.dialog = Some(Dialog::Command {
                    parent: self.parent(),
                    at: None,
                    cmd: Command::default(),
                });
                out.focus = Some(self.input_id.clone());
            }
            CmMsg::AddFolder => {
                // Inside the selected folder, not inside a command's.
                let parent = match &self.selected {
                    Some(a) if a.command.is_none() => a.folders.clone(),
                    Some(a) => a.folders.clone(),
                    None => Vec::new(),
                };
                self.dialog = Some(Dialog::Folder {
                    parent,
                    rename: None,
                    name: String::new(),
                });
                out.focus = Some(self.input_id.clone());
            }
            CmMsg::Edit => {
                if let Some(a) = self.selected.clone() {
                    match a.command {
                        Some(i) => {
                            if let Some(c) = self.tree.command(&a).cloned() {
                                self.dialog = Some(Dialog::Command {
                                    parent: a.folders,
                                    at: Some(i),
                                    cmd: c,
                                });
                            }
                        }
                        None => {
                            let name = self
                                .tree
                                .folder(&a.folders)
                                .map(|f| f.name.clone())
                                .unwrap_or_default();
                            self.dialog = Some(Dialog::Folder {
                                parent: Vec::new(),
                                rename: Some(a.folders),
                                name,
                            });
                        }
                    }
                    out.focus = Some(self.input_id.clone());
                }
            }
            CmMsg::Delete => {
                if let Some(a) = self.selected.clone() {
                    let name = match a.command {
                        Some(_) => self.tree.command(&a).map(Command::title),
                        None => self.tree.folder(&a.folders).map(|f| f.name.clone()),
                    };
                    if let Some(name) = name {
                        self.dialog = Some(Dialog::Delete(a, name));
                    }
                }
            }
            CmMsg::Shift(by) => {
                if let Some(a) = self.selected.clone() {
                    self.selected = Some(self.tree.shift(&a, by));
                    out.save = true;
                }
            }
            CmMsg::Label(t) => self.map(|c| c.label = t),
            CmMsg::Text(t) => self.map(|c| c.text = t),
            CmMsg::TargetSel(i) => self.map(|c| {
                c.target = if i == 1 { Target::All } else { Target::Current };
            }),
            CmMsg::Name(n) => {
                if let Some(Dialog::Folder { name, .. }) = &mut self.dialog {
                    *name = n;
                }
            }
            CmMsg::Ok => self.ok(&mut out),
            CmMsg::Cancel => self.dialog = None,
        }
        out
    }

    fn map(&mut self, f: impl FnOnce(&mut Command)) {
        if let Some(Dialog::Command { cmd, .. }) = &mut self.dialog {
            f(cmd);
        }
    }

    fn ok(&mut self, out: &mut Outcome) {
        let Some(d) = self.dialog.take() else { return };
        let err = |e: Error| match e {
            Error::Empty => fl!("name-empty"),
            Error::Taken => fl!("cm-name-taken"),
            Error::NoSuch => fl!("cm-gone"),
        };
        match d {
            Dialog::Command { parent, at, cmd } => {
                match self.tree.put_command(&parent, at, cmd.clone()) {
                    Ok(i) => {
                        self.selected = Some(Addr {
                            folders: parent.clone(),
                            command: Some(i),
                        });
                        self.open_path(&parent);
                        out.save = true;
                    }
                    Err(e) => {
                        self.error = Some(err(e));
                        self.dialog = Some(Dialog::Command { parent, at, cmd });
                    }
                }
            }
            Dialog::Folder {
                parent,
                rename,
                name,
            } => {
                let r = match &rename {
                    Some(path) => self.tree.rename_folder(path, &name).map(|()| path.clone()),
                    None => self
                        .tree
                        .add_folder(&parent, &name)
                        .map(|i| [parent.clone(), vec![i]].concat()),
                };
                match r {
                    Ok(path) => {
                        // Renamed folders lose their «open» (it is kept by name).
                        self.open_path(&path[..path.len().saturating_sub(1)]);
                        self.selected = Some(Addr {
                            folders: path,
                            command: None,
                        });
                        out.save = true;
                    }
                    Err(e) => {
                        self.error = Some(err(e));
                        self.dialog = Some(Dialog::Folder {
                            parent,
                            rename,
                            name,
                        });
                    }
                }
            }
            Dialog::Delete(a, _) => {
                self.tree.remove(&a);
                self.selected = None;
                out.save = true;
            }
        }
    }

    /// The folder at `path` (and above) shown open.
    fn open_path(&mut self, path: &[usize]) {
        let mut key = Vec::new();
        let mut f = &self.tree;
        for &i in path {
            let Some(next) = f.folders.get(i) else { break };
            key.push(next.name.clone());
            self.open.insert(key.clone());
            f = next;
        }
    }

    pub fn dialog(&self) -> Option<Element<'_, Message>> {
        let m = Message::CommandManager;
        let d = self.dialog.as_ref()?;
        let ok = widget::button::suggested(fl!("ok")).on_press(m(CmMsg::Ok));
        let cancel = widget::button::standard(fl!("cancel")).on_press(m(CmMsg::Cancel));
        let mut col = widget::column::with_capacity(5).spacing(10);
        let title = match d {
            Dialog::Command { at, cmd, .. } => {
                col = col
                    .push(field(
                        fl!("bb-label"),
                        widget::text_input("", cmd.label.clone())
                            .on_input(|t| Message::CommandManager(CmMsg::Label(t)))
                            .into(),
                    ))
                    .push(field(
                        fl!("cm-text"),
                        widget::text_input("", cmd.text.clone())
                            .id(self.input_id.clone())
                            .on_input(|t| Message::CommandManager(CmMsg::Text(t)))
                            .on_submit(|_| Message::CommandManager(CmMsg::Ok))
                            .into(),
                    ))
                    .push(widget::text::caption(fl!("bb-string-hint")))
                    .push(field(
                        fl!("bb-target"),
                        widget::dropdown(
                            &self.targets,
                            Some(usize::from(cmd.target == Target::All)),
                            move |i| m(CmMsg::TargetSel(i)),
                        )
                        .into(),
                    ));
                match at {
                    Some(_) => fl!("cm-edit-title"),
                    None => fl!("cm-add-title"),
                }
            }
            Dialog::Folder { rename, name, .. } => {
                col = col.push(field(
                    fl!("cm-name"),
                    widget::text_input("", name.clone())
                        .id(self.input_id.clone())
                        .on_input(|t| Message::CommandManager(CmMsg::Name(t)))
                        .on_submit(|_| Message::CommandManager(CmMsg::Ok))
                        .into(),
                ));
                match rename {
                    Some(_) => fl!("cm-rename-title"),
                    None => fl!("cm-folder-title"),
                }
            }
            Dialog::Delete(_, name) => {
                col = col.push(widget::text::body(fl!(
                    "cm-delete-ask",
                    name = name.clone()
                )));
                fl!("cm-delete-title")
            }
        };
        if let Some(e) = &self.error {
            col = col.push(widget::text::body(e.clone()).class(cosmic::theme::Text::Accent));
        }
        Some(
            widget::dialog()
                .title(title)
                .control(col)
                .primary_action(ok)
                .secondary_action(cancel)
                .into(),
        )
    }

    pub fn view(&self) -> Element<'_, Message> {
        let m = Message::CommandManager;
        let tool = |icon: &'static str, tip: String, on: Option<CmMsg>| {
            widget::tooltip(
                widget::button::icon(widget::icon::from_name(icon)).on_press_maybe(on.map(m)),
                widget::text::caption(tip),
                widget::tooltip::Position::Bottom,
            )
        };
        let sel = self.selected.is_some();
        let sel_cmd = self.selected.as_ref().is_some_and(|a| a.command.is_some());
        let toolbar = widget::row::with_children(vec![
            tool(
                "document-new-symbolic",
                fl!("cm-add-title"),
                Some(CmMsg::AddCommand),
            )
            .into(),
            tool(
                "folder-new-symbolic",
                fl!("cm-folder-title"),
                Some(CmMsg::AddFolder),
            )
            .into(),
            tool(
                "document-edit-symbolic",
                fl!("cm-edit"),
                sel.then_some(CmMsg::Edit),
            )
            .into(),
            tool(
                "edit-delete-symbolic",
                fl!("cm-delete"),
                sel.then_some(CmMsg::Delete),
            )
            .into(),
            tool(
                "go-up-symbolic",
                fl!("cm-up"),
                sel.then_some(CmMsg::Shift(-1)),
            )
            .into(),
            tool(
                "go-down-symbolic",
                fl!("cm-down"),
                sel.then_some(CmMsg::Shift(1)),
            )
            .into(),
            tool(
                "media-playback-start-symbolic",
                fl!("cm-send"),
                self.selected.clone().filter(|_| sel_cmd).map(CmMsg::Send),
            )
            .into(),
        ])
        .spacing(2);
        let filter = widget::search_input(fl!("sm-filter"), self.filter.clone())
            .id(self.filter_id.clone())
            .on_input(|f| Message::CommandManager(CmMsg::Filter(f)))
            .on_clear(m(CmMsg::ClearFilter));
        let rows = self.tree.rows(&self.filter, &self.open);
        let list: Element<'_, Message> = match rows.is_empty() {
            true => widget::container(widget::text::caption(match self.filter.is_empty() {
                true => fl!("cm-empty"),
                false => fl!("sm-no-match"),
            }))
            .center(Length::Fill)
            .into(),
            false => widget::scrollable(widget::column::with_children(
                rows.iter().map(|r| self.row(r)).collect::<Vec<_>>(),
            ))
            .height(Length::Fill)
            .into(),
        };
        let mut body = widget::column::with_capacity(5)
            .spacing(6)
            .padding(8)
            .push(widget::text::heading(fl!("menu-command-manager")))
            .push(toolbar)
            .push(filter)
            .push(list);
        if let Some(b) = &self.broken {
            body = body.push(widget::text::caption(b.clone()).class(cosmic::theme::Text::Accent));
        }
        widget::container(body)
            .width(Length::Fixed(260.0))
            .height(Length::Fill)
            .class(cosmic::theme::Container::Card)
            .into()
    }

    fn row(&self, r: &Row) -> Element<'_, Message> {
        let m = Message::CommandManager;
        let chevron: Element<'_, Message> = match r.is_folder {
            true => widget::button::icon(
                widget::icon::from_name(if r.open {
                    "go-down-symbolic"
                } else {
                    "go-next-symbolic"
                })
                .size(12),
            )
            .padding(2)
            .on_press(m(CmMsg::Toggle(r.key.clone())))
            .into(),
            false => widget::Space::new().width(20.0).into(),
        };
        let icon = match (r.is_folder, r.open) {
            (true, true) => "folder-open-symbolic",
            (true, false) => "folder-symbolic",
            (false, _) => "utilities-terminal-symbolic",
        };
        let line = widget::row::with_children(vec![
            widget::Space::new().width(r.depth as f32 * 14.0).into(),
            chevron,
            widget::icon::from_name(icon).size(16).into(),
            widget::text::body(r.title.clone()).into(),
        ])
        .spacing(4)
        .align_y(Alignment::Center);
        let selected = self.selected.as_ref() == Some(&r.addr);
        let cell = widget::container(line)
            .width(Length::Fill)
            .padding([2, 4])
            .class(cosmic::theme::Container::custom(move |theme| {
                let c = theme.cosmic();
                let mut s = widget::container::Style::default();
                if selected {
                    let mut bg: cosmic::iced::Color = c.accent_color().into();
                    bg.a = 0.25;
                    s.background = Some(bg.into());
                    s.border.radius = c.corner_radii.radius_xs.into();
                }
                s
            }));
        let mut area = widget::mouse_area(cell).on_press(m(CmMsg::Select(r.addr.clone())));
        area = match r.is_folder {
            true => area.on_double_click(m(CmMsg::Toggle(r.key.clone()))),
            false => area.on_double_click(m(CmMsg::Send(r.addr.clone()))),
        };
        area.into()
    }
}

fn field<'a>(label: String, w: Element<'a, Message>) -> Element<'a, Message> {
    widget::column::with_children(vec![widget::text::caption(label).into(), w])
        .spacing(2)
        .into()
}
