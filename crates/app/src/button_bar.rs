//! Button Bar, as SecureCRT's: a row of buttons under the terminals, several named bars, a
//! session may name its own. A right click on a button: Edit / Delete / Move. The bars are in
//! `core::buttonbar`; the app runs what a button says (it owns the tabs and the menu functions).

use crate::app::Message;
use crate::fl;
use cosmic::Element;
use cosmic::iced::{Alignment, Length};
use cosmic::widget::{self, menu};
use shagshell_core::buttonbar::{self, Bar, BarError, Bars, Button, Kind, Target};

/// Menu functions a button may call: the name stored in the file, and its label.
pub use crate::menu::NAMED as FUNCTIONS;

#[derive(Clone, Debug)]
pub enum BbMsg {
    Press(usize),
    Select(usize),
    AddButton,
    Edit(usize),
    Delete(usize),
    Move(usize, isize),
    NewBar,
    RenameBar,
    DeleteBar,
    /// The «Map Button» window's fields.
    Label(String),
    Tip(String),
    KindSel(usize),
    Arg(String),
    Args(String),
    FunctionSel(usize),
    TargetSel(usize),
    ColorSel(usize),
    /// A bar's name typed.
    Name(String),
    Ok,
    Cancel,
}

enum Dialog {
    Map {
        bar: usize,
        /// `None`: a new button.
        at: Option<usize>,
        button: Button,
        error: Option<String>,
    },
    BarName {
        /// `None`: a new bar.
        rename: Option<usize>,
        text: String,
        error: Option<String>,
    },
    DeleteBar(usize),
}

pub struct ButtonBar {
    bars: Bars,
    /// The file could not be read: shown, and nothing is written over it.
    broken: Option<String>,
    /// The bar picked on the bar itself.
    pub selected: usize,
    /// The active session (its id) when a bar was picked: the pick beats that session's own bar.
    pub picked_for: Option<Option<String>>,
    /// A bar renamed (`Some(new)`) or deleted (`None`) — `(old name, …)`: the sessions follow
    /// (the app takes it).
    pub relink: Option<(String, Option<String>)>,
    dialog: Option<Dialog>,
    input_id: widget::Id,
    kinds: Vec<String>,
    /// «none», then `buttonbar::COLORS` by name, and each one's dot.
    colors: Vec<String>,
    color_dots: Vec<widget::icon::Handle>,
    targets: Vec<String>,
    functions: Vec<String>,
}

impl ButtonBar {
    pub fn new() -> Self {
        let path = buttonbar::default_path();
        let (bars, broken) = match Bars::load(&path) {
            Ok(b) => (b, None),
            Err(e) => (Bars::default(), Some(format!("{}: {e}", path.display()))),
        };
        let mut s = Self {
            bars,
            broken,
            selected: 0,
            picked_for: None,
            relink: None,
            dialog: None,
            input_id: widget::Id::unique(),
            kinds: Vec::new(),
            colors: Vec::new(),
            color_dots: (std::iter::once(
                r##"<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 12 12"><circle cx="6" cy="6" r="5" fill="none" stroke="#808080" stroke-width="1"/></svg>"##
                    .to_string(),
            ))
            .chain(
                (shagshell_core::buttonbar::COLORS.iter())
                    .filter_map(|(n, _)| shagshell_core::buttonbar::dot_svg(n)),
            )
            .map(|svg| widget::icon::from_svg_bytes(svg.into_bytes()))
            .collect(),
            targets: Vec::new(),
            functions: Vec::new(),
        };
        s.relabel();
        s
    }

    /// A function's label in the current language (the name itself if unknown).
    pub fn function_label_of(name: &str) -> String {
        FUNCTIONS
            .iter()
            .find(|(n, _)| *n == name)
            .map_or_else(|| name.to_string(), |(_, l)| l())
    }

    /// The language may change.
    pub fn relabel(&mut self) {
        self.kinds = vec![
            fl!("bb-send"),
            fl!("bb-menu"),
            fl!("bb-launch"),
            fl!("bb-script"),
        ];
        self.targets = vec![fl!("cw-current"), fl!("cw-all")];
        self.colors = std::iter::once(fl!("bb-color-none"))
            .chain(
                shagshell_core::buttonbar::COLORS
                    .iter()
                    .map(|(n, _)| match *n {
                        "black" => fl!("color-black"),
                        "blue" => fl!("hl-blue"),
                        "cyan" => fl!("hl-cyan"),
                        "green" => fl!("hl-green"),
                        "grey" => fl!("color-grey"),
                        "magenta" => fl!("hl-magenta"),
                        "orange" => fl!("hl-orange"),
                        "purple" => fl!("color-purple"),
                        "red" => fl!("hl-red"),
                        "white" => fl!("hl-white"),
                        _ => fl!("hl-yellow"),
                    }),
            )
            .collect();
        self.functions = FUNCTIONS.iter().map(|(_, label)| label()).collect();
    }

    /// Bars brought in by an import (names that are not here yet): added and saved. `Some`: why
    /// they could not be saved.
    pub fn merge(&mut self, bars: Vec<Bar>) -> Option<String> {
        let mut any = false;
        for b in bars {
            if self.bars.index_of(&b.name).is_none() {
                self.bars.bars.push(b);
                any = true;
            }
        }
        any.then(|| self.save()).flatten()
    }

    pub fn broken(&self) -> Option<&str> {
        self.broken.as_deref()
    }

    pub fn names(&self) -> Vec<String> {
        self.bars.bars.iter().map(|b| b.name.clone()).collect()
    }

    pub fn has_dialog(&self) -> bool {
        self.dialog.is_some()
    }

    /// The bar to show: the active session's own, else the one picked.
    pub fn shown(&self, session_bar: Option<&str>) -> Option<usize> {
        let own = session_bar.and_then(|n| self.bars.index_of(n));
        own.or_else(|| match self.bars.bars.len() {
            0 => None,
            n => Some(self.selected.min(n - 1)),
        })
    }

    fn save(&mut self) -> Option<String> {
        if let Some(why) = &self.broken {
            return Some(why.clone());
        }
        let path = buttonbar::default_path();
        self.bars
            .save(&path)
            .err()
            .map(|e| format!("{}: {e}", path.display()))
    }

    fn bar_error(e: BarError) -> String {
        match e {
            BarError::Empty => fl!("name-empty"),
            BarError::Taken => fl!("bb-name-taken"),
        }
    }

    /// `shown`: the bar on screen. Returns a button to run and a message to show.
    pub fn update(&mut self, msg: BbMsg, shown: Option<usize>) -> (Option<Button>, Option<String>) {
        let mut notice = None;
        match msg {
            BbMsg::Press(i) => {
                let b = shown
                    .and_then(|s| self.bars.bars.get(s))
                    .and_then(|bar| bar.buttons.get(i))
                    .cloned();
                return (b, None);
            }
            BbMsg::Select(i) => self.selected = i,
            BbMsg::AddButton => {
                // No bar yet: «OK» makes the default one (a Cancel leaves nothing behind).
                let bar = shown.unwrap_or(usize::MAX);
                self.dialog = Some(Dialog::Map {
                    bar,
                    at: None,
                    button: Button {
                        kind: Kind::Send,
                        ..Button::default()
                    },
                    error: None,
                });
            }
            BbMsg::Edit(i) => {
                if let Some(s) = shown
                    && let Some(b) = self.bars.bars.get(s).and_then(|b| b.buttons.get(i))
                {
                    self.dialog = Some(Dialog::Map {
                        bar: s,
                        at: Some(i),
                        button: b.clone(),
                        error: None,
                    });
                }
            }
            BbMsg::Delete(i) => {
                if let Some(bar) = shown.and_then(|s| self.bars.bars.get_mut(s))
                    && i < bar.buttons.len()
                {
                    bar.buttons.remove(i);
                    notice = self.save();
                }
            }
            BbMsg::Move(i, by) => {
                if let Some(s) = shown {
                    self.bars.shift(s, i, by);
                    notice = self.save();
                }
            }
            BbMsg::NewBar => {
                self.dialog = Some(Dialog::BarName {
                    rename: None,
                    text: String::new(),
                    error: None,
                });
            }
            BbMsg::RenameBar => {
                if let Some(s) = shown {
                    self.dialog = Some(Dialog::BarName {
                        rename: Some(s),
                        text: self.bars.bars[s].name.clone(),
                        error: None,
                    });
                }
            }
            BbMsg::DeleteBar => {
                if let Some(s) = shown {
                    self.dialog = Some(Dialog::DeleteBar(s));
                }
            }
            BbMsg::Label(t) => self.map(|b| b.label = t),
            BbMsg::Tip(t) => self.map(|b| b.tip = t),
            BbMsg::Arg(t) => self.map(|b| b.arg = t),
            BbMsg::Args(t) => self.map(|b| b.args = t),
            BbMsg::KindSel(i) => self.map(|b| {
                b.kind = Kind::ALL.get(i).copied().unwrap_or_default();
                b.target = Target::Current;
                // The old text means something else for another kind.
                b.arg = match b.kind {
                    Kind::Menu => FUNCTIONS[0].0.to_string(),
                    _ => String::new(),
                };
                b.args.clear();
            }),
            BbMsg::FunctionSel(i) => self.map(|b| {
                if let Some((name, _)) = FUNCTIONS.get(i) {
                    b.arg = (*name).to_string();
                }
            }),
            BbMsg::ColorSel(i) => self.map(|b| {
                b.color = (i.checked_sub(1))
                    .and_then(|i| shagshell_core::buttonbar::COLORS.get(i))
                    .map_or(String::new(), |(n, _)| n.to_string());
            }),
            BbMsg::TargetSel(i) => self.map(|b| {
                b.target = [Target::Current, Target::All]
                    .get(i)
                    .copied()
                    .unwrap_or_default();
            }),
            BbMsg::Name(t) => {
                if let Some(Dialog::BarName { text, .. }) = &mut self.dialog {
                    *text = t;
                }
            }
            BbMsg::Cancel => self.dialog = None,
            BbMsg::Ok => notice = self.ok(),
        }
        (None, notice)
    }

    fn map(&mut self, f: impl FnOnce(&mut Button)) {
        if let Some(Dialog::Map { button, error, .. }) = &mut self.dialog {
            f(button);
            *error = None;
        }
    }

    fn ok(&mut self) -> Option<String> {
        match self.dialog.take()? {
            Dialog::Map {
                bar,
                at,
                button,
                error: _,
            } => {
                let err = if button.label.trim().is_empty() {
                    Some(fl!("name-empty"))
                } else if match button.kind {
                    // A lone space is a fine string (paging through «--More--»).
                    Kind::Send => button.arg.is_empty(),
                    _ => button.arg.trim().is_empty(),
                } {
                    Some(fl!("bb-arg-empty"))
                } else {
                    None
                };
                if let Some(e) = err {
                    self.dialog = Some(Dialog::Map {
                        bar,
                        at,
                        button,
                        error: Some(e),
                    });
                    return None;
                }
                let button = Button {
                    label: button.label.trim().to_string(),
                    ..button
                };
                let bar = match self.bars.bars.get(bar) {
                    Some(_) => bar,
                    None => match self.bars.add(&fl!("bb-default-name")) {
                        Ok(i) => i,
                        Err(_) => return None,
                    },
                };
                self.selected = bar;
                if let Some(b) = self.bars.bars.get_mut(bar) {
                    match at {
                        Some(i) if i < b.buttons.len() => b.buttons[i] = button,
                        _ => b.buttons.push(button),
                    }
                }
                self.save()
            }
            Dialog::BarName { rename, text, .. } => {
                let was = rename
                    .and_then(|i| self.bars.bars.get(i))
                    .map(|b| b.name.clone());
                let r = match rename {
                    Some(i) => self.bars.rename(i, &text).map(|()| i),
                    None => self.bars.add(&text),
                };
                match r {
                    Ok(i) => {
                        if let Some(old) = was.filter(|o| *o != self.bars.bars[i].name) {
                            self.relink = Some((old, Some(self.bars.bars[i].name.clone())));
                        }
                        self.selected = i;
                        self.save()
                    }
                    Err(e) => {
                        self.dialog = Some(Dialog::BarName {
                            rename,
                            text,
                            error: Some(Self::bar_error(e)),
                        });
                        None
                    }
                }
            }
            Dialog::DeleteBar(i) => {
                if let Some(b) = self.bars.bars.get(i) {
                    self.relink = Some((b.name.clone(), None));
                }
                self.bars.remove(i);
                // The picked bar keeps being the same one; if it was this one, its neighbour.
                if self.selected >= i {
                    self.selected = self.selected.saturating_sub(1);
                }
                self.save()
            }
        }
    }

    pub fn dialog(&self) -> Option<Element<'_, Message>> {
        let m = Message::ButtonBar;
        let d = self.dialog.as_ref()?;
        fn ok<'a>(title: String, body: Element<'a, Message>, ok: String) -> Element<'a, Message> {
            widget::dialog()
                .icon(crate::dialog_icons::Kind::Edit.icon(48))
                .title(title)
                .control(body)
                .primary_action(
                    widget::button::suggested(ok).on_press(Message::ButtonBar(BbMsg::Ok)),
                )
                .secondary_action(
                    widget::button::standard(fl!("cancel"))
                        .on_press(Message::ButtonBar(BbMsg::Cancel)),
                )
                .into()
        }
        let field = |label: String, value: &str, f: fn(String) -> BbMsg, id: Option<widget::Id>| {
            let mut input = widget::text_input("", value.to_string())
                .on_input(move |t| m(f(t)))
                .on_submit(move |_| m(BbMsg::Ok));
            if let Some(id) = id {
                input = input.id(id);
            }
            widget::column::with_children(vec![widget::text::caption(label).into(), input.into()])
                .spacing(2)
        };
        let error = |e: &Option<String>| -> Element<'_, Message> {
            match e {
                Some(e) => widget::text::body(e.clone())
                    .class(cosmic::theme::Text::Accent)
                    .into(),
                None => widget::Space::new().height(Length::Fixed(0.0)).into(),
            }
        };
        Some(match d {
            Dialog::Map {
                at,
                button,
                error: e,
                ..
            } => {
                let kind = Kind::ALL.iter().position(|k| *k == button.kind);
                let mut col = widget::column::with_capacity(6).spacing(10).push(field(
                    fl!("bb-label"),
                    &button.label,
                    BbMsg::Label,
                    Some(self.input_id.clone()),
                ));
                col = col.push(
                    widget::column::with_children(vec![
                        widget::text::caption(fl!("bb-action")).into(),
                        widget::dropdown(&self.kinds, kind, move |i| m(BbMsg::KindSel(i))).into(),
                    ])
                    .spacing(2),
                );
                col = match button.kind {
                    Kind::Menu => {
                        let at = FUNCTIONS.iter().position(|(n, _)| *n == button.arg);
                        col.push(
                            widget::column::with_children(vec![
                                widget::text::caption(fl!("bb-function")).into(),
                                widget::dropdown(&self.functions, at, move |i| {
                                    m(BbMsg::FunctionSel(i))
                                })
                                .into(),
                            ])
                            .spacing(2),
                        )
                    }
                    Kind::Send => col
                        .push(field(fl!("bb-string"), &button.arg, BbMsg::Arg, None))
                        .push(widget::text::caption(fl!("bb-string-hint")))
                        .push(
                            widget::column::with_children(vec![
                                widget::text::caption(fl!("bb-target")).into(),
                                widget::dropdown(
                                    &self.targets,
                                    Some(usize::from(button.target == Target::All)),
                                    move |i| m(BbMsg::TargetSel(i)),
                                )
                                .into(),
                            ])
                            .spacing(2),
                        ),
                    Kind::Launch => col
                        .push(field(fl!("bb-command"), &button.arg, BbMsg::Arg, None))
                        .push(widget::text::caption(fl!("bb-command-hint"))),
                    Kind::Script => col
                        .push(crate::script::with_browse(
                            field(fl!("bb-script-file"), &button.arg, BbMsg::Arg, None).into(),
                            crate::script::Slot::Button,
                        ))
                        .push(field(
                            fl!("bb-script-args"),
                            &button.args,
                            BbMsg::Args,
                            None,
                        ))
                        .push(widget::text::caption(fl!("bb-script-hint"))),
                };
                let color = (shagshell_core::buttonbar::COLORS.iter())
                    .position(|(n, _)| *n == button.color)
                    .map_or(0, |i| i + 1);
                col = col.push(field(fl!("bb-tip"), &button.tip, BbMsg::Tip, None));
                col = col.push(
                    widget::column::with_children(vec![
                        widget::text::caption(fl!("bb-color")).into(),
                        widget::dropdown(&self.colors, Some(color), move |i| m(BbMsg::ColorSel(i)))
                            .icons(std::borrow::Cow::Borrowed(&self.color_dots))
                            .into(),
                    ])
                    .spacing(2),
                );
                col = col.push(error(e));
                let title = match at {
                    Some(_) => fl!("bb-edit-title"),
                    None => fl!("bb-add-title"),
                };
                ok(title, col.into(), fl!("ok"))
            }
            Dialog::BarName {
                rename,
                text,
                error: e,
            } => {
                let title = match rename {
                    Some(_) => fl!("bb-rename-title"),
                    None => fl!("bb-new-title"),
                };
                let body = widget::column::with_children(vec![
                    field(
                        fl!("bb-bar-name"),
                        text,
                        BbMsg::Name,
                        Some(self.input_id.clone()),
                    )
                    .into(),
                    error(e),
                ])
                .spacing(8);
                ok(title, body.into(), fl!("ok"))
            }
            Dialog::DeleteBar(i) => {
                let name = self
                    .bars
                    .bars
                    .get(*i)
                    .map(|b| b.name.clone())
                    .unwrap_or_default();
                widget::dialog()
                    .icon(crate::dialog_icons::Kind::Warning.icon(48))
                    .title(fl!("bb-delete-title", name = name))
                    .body(fl!("bb-delete-body"))
                    .primary_action(
                        widget::button::destructive(fl!("sm-delete")).on_press(m(BbMsg::Ok)),
                    )
                    .secondary_action(
                        widget::button::standard(fl!("cancel")).on_press(m(BbMsg::Cancel)),
                    )
                    .into()
            }
        })
    }

    /// The focus for a dialog's first field.
    pub fn first_field(&self) -> widget::Id {
        self.input_id.clone()
    }

    pub fn view(
        &self,
        shown: Option<usize>,
        window: cosmic::iced::window::Id,
        height: Option<u16>,
    ) -> Element<'_, Message> {
        let m = Message::ButtonBar;
        let mut row = widget::row::with_capacity(8)
            .spacing(6)
            .align_y(Alignment::Center);
        let names: Vec<String> = self.names();
        if names.len() > 1 {
            row = row.push(widget::dropdown(names, shown, move |i| m(BbMsg::Select(i))));
        } else if let Some(Bar { name, .. }) = shown.and_then(|s| self.bars.bars.get(s)) {
            row = row.push(widget::text::caption(name.clone()));
        }
        if let Some(bar) = shown.and_then(|s| self.bars.bars.get(s)) {
            let last = bar.buttons.len().saturating_sub(1);
            for (i, b) in bar.buttons.iter().enumerate() {
                // A coloured button: its dot before the label (SecureCRT).
                // One line: a label too long for its room is cut on the right, not wrapped.
                let label = widget::text::body(b.label.clone())
                    .wrapping(cosmic::iced::widget::text::Wrapping::None)
                    .ellipsize(cosmic::iced::advanced::text::Ellipsize::End(
                        cosmic::iced::advanced::text::EllipsizeHeightLimit::Lines(1),
                    ));
                let mut inner = widget::row::with_capacity(2)
                    .spacing(6)
                    .align_y(Alignment::Center);
                if let Some(svg) = shagshell_core::buttonbar::dot_svg(&b.color) {
                    inner = inner.push(
                        widget::icon(widget::icon::from_svg_bytes(svg.into_bytes())).size(12),
                    );
                }
                let mut button = widget::button::custom(inner.push(label))
                    .class(cosmic::theme::Button::Standard)
                    .padding([0, 16])
                    .height(Length::Fixed(32.0))
                    .on_press(m(BbMsg::Press(i)));
                if let Some(h) = height {
                    button = button.height(Length::Fixed(f32::from(h)));
                }
                let items = vec![
                    menu::Item::Button(fl!("bb-edit"), None, BarCmd::Edit(i)),
                    menu::Item::Button(fl!("sm-delete"), None, BarCmd::Delete(i)),
                    menu::Item::Divider,
                    match i {
                        0 => menu::Item::ButtonDisabled(fl!("bb-left"), None, BarCmd::Move(i, -1)),
                        _ => menu::Item::Button(fl!("bb-left"), None, BarCmd::Move(i, -1)),
                    },
                    match i == last {
                        true => {
                            menu::Item::ButtonDisabled(fl!("bb-right"), None, BarCmd::Move(i, 1))
                        }
                        false => menu::Item::Button(fl!("bb-right"), None, BarCmd::Move(i, 1)),
                    },
                ];
                // The hint, when the pointer rests on it.
                let button: Element<'_, Message> = match b.tip.trim().is_empty() {
                    true => button.into(),
                    false => widget::tooltip(
                        button,
                        widget::text::caption(b.tip.clone()),
                        widget::tooltip::Position::Top,
                    )
                    .into(),
                };
                row = row.push(
                    widget::context_menu(
                        button,
                        Some(menu::items(&std::collections::HashMap::new(), items)),
                    )
                    .window_id(window)
                    .on_surface_action(Message::Surface),
                );
            }
        }
        row = row.push(widget::tooltip(
            widget::button::icon(widget::icon::from_name("list-add-symbolic"))
                .on_press(m(BbMsg::AddButton)),
            widget::text(fl!("bb-add")),
            widget::tooltip::Position::Top,
        ));
        // Bars are managed from the menu of the empty space. Not around the whole row: a menu
        // inside a menu never opens, the outer one takes the click (a button had no «Edit»).
        let has = shown.is_some();
        let bar_item = |label: String, cmd: BarCmd, on: bool| match on {
            true => menu::Item::Button(label, None, cmd),
            false => menu::Item::ButtonDisabled(label, None, cmd),
        };
        let items = vec![
            bar_item(fl!("bb-new-bar"), BarCmd::NewBar, true),
            bar_item(fl!("bb-rename-bar"), BarCmd::RenameBar, has),
            bar_item(fl!("bb-delete-bar"), BarCmd::DeleteBar, has),
            menu::Item::Divider,
            bar_item(fl!("bb-add"), BarCmd::AddButton, true),
        ];
        row = row.push(
            widget::context_menu(
                widget::Space::new()
                    .width(Length::Fill)
                    .height(Length::Fixed(f32::from(height.unwrap_or(32)))),
                Some(menu::items(&std::collections::HashMap::new(), items)),
            )
            .window_id(window)
            .on_surface_action(Message::Surface),
        );
        widget::container(row)
            .padding([4, 12])
            .width(Length::Fill)
            .into()
    }
}

/// The context-menu entries of a button (its index is in them) and of the bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum BarCmd {
    Edit(usize),
    Delete(usize),
    Move(usize, isize),
    NewBar,
    RenameBar,
    DeleteBar,
    AddButton,
}

impl menu::Action for BarCmd {
    type Message = Message;
    fn message(&self) -> Message {
        Message::ButtonBar(match *self {
            BarCmd::Edit(i) => BbMsg::Edit(i),
            BarCmd::Delete(i) => BbMsg::Delete(i),
            BarCmd::Move(i, by) => BbMsg::Move(i, by),
            BarCmd::NewBar => BbMsg::NewBar,
            BarCmd::RenameBar => BbMsg::RenameBar,
            BarCmd::DeleteBar => BbMsg::DeleteBar,
            BarCmd::AddButton => BbMsg::AddButton,
        })
    }
}
