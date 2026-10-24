//! Tools → Keymap Editor…: the user's own keys (`core::keymap`), each running what a Button Bar
//! button runs. A dialog in the main window: the list, and the edit of one binding. The app
//! saves and keeps the chords where `key_action` can see them (`set_bindings`).

use crate::app::Message;
use crate::button_bar::FUNCTIONS;
use crate::fl;
use cosmic::Element;
use cosmic::iced::Length;
use cosmic::widget;
use shagshell_core::buttonbar::{Button, Kind, Target};
use shagshell_core::keymap::{Binding, Chord, Keymap, Reason, Refused};
use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};

/// The chords in force, read by `app::key_action` on every key.
static BINDINGS: RwLock<Vec<Binding>> = RwLock::new(Vec::new());
/// The next key is a chord to record, not an action.
static CAPTURING: AtomicBool = AtomicBool::new(false);

/// Puts the keys in force. A hand-edited file may hold what the editor would refuse (Ctrl+C) or
/// a key twice: those are left out here (the file is not changed).
pub fn set_bindings(k: &Keymap) {
    let mut ok: Vec<Binding> = Vec::new();
    for b in &k.keys {
        if b.keys.check().is_ok() && !ok.iter().any(|o| o.keys == b.keys) {
            ok.push(b.clone());
        }
    }
    *BINDINGS.write().unwrap_or_else(|e| e.into_inner()) = ok;
}

/// A modifier key on its own (the first event of Alt+Q): not a chord yet.
pub fn is_modifier(key: &cosmic::iced::keyboard::key::Physical) -> bool {
    use cosmic::iced::keyboard::key::Physical;
    match key {
        Physical::Code(c) => {
            let n = format!("{c:?}");
            ["Shift", "Control", "Alt", "Super", "Meta", "Hyper", "Fn"]
                .iter()
                .any(|m| n.starts_with(m))
        }
        _ => false,
    }
}

/// Which binding a pressed key is (an index into the list `set_bindings` was given).
pub fn lookup(mods: cosmic::iced::keyboard::Modifiers, code: &str) -> Option<usize> {
    let g = BINDINGS.read().unwrap_or_else(|e| e.into_inner());
    // Every key press asks: nothing to look for is the usual answer. Super is the compositor's.
    if g.is_empty() || mods.logo() {
        return None;
    }
    let want = Chord::new(mods.control(), mods.alt(), mods.shift(), code);
    g.iter().position(|b| b.keys == want)
}

pub fn binding(i: usize) -> Option<Button> {
    let g = BINDINGS.read().unwrap_or_else(|e| e.into_inner());
    g.get(i).map(|b| b.button.clone())
}

pub fn capturing() -> bool {
    CAPTURING.load(Ordering::SeqCst)
}

#[derive(Clone, Debug)]
pub enum KmMsg {
    Select(usize),
    Add,
    Edit,
    Delete,
    Capture,
    /// While `capturing`: the key pressed (its code name) and the modifiers.
    Captured(
        cosmic::iced::keyboard::key::Physical,
        cosmic::iced::keyboard::Modifiers,
    ),
    Label(String),
    KindSel(usize),
    Arg(String),
    Args(String),
    FunctionSel(usize),
    TargetSel(usize),
    Ok,
    Cancel,
    Close,
}

struct Edit {
    at: Option<usize>,
    chord: Option<Chord>,
    button: Button,
    error: Option<String>,
}

pub struct Editor {
    pub keymap: Keymap,
    selected: Option<usize>,
    edit: Option<Edit>,
    kinds: Vec<String>,
    targets: Vec<String>,
    functions: Vec<String>,
}

/// What the app does after an update.
#[derive(Default)]
pub struct Outcome {
    /// The keymap changed: save it and `set_bindings`.
    pub save: bool,
    pub close: bool,
    /// Take the keys from the terminal and widgets (a chord is being recorded).
    pub unfocus: bool,
}

impl Editor {
    pub fn new(keymap: Keymap) -> Self {
        Self {
            keymap,
            selected: None,
            edit: None,
            kinds: vec![
                fl!("bb-send"),
                fl!("bb-menu"),
                fl!("bb-launch"),
                fl!("bb-script"),
            ],
            targets: vec![fl!("cw-current"), fl!("cw-all")],
            functions: FUNCTIONS.iter().map(|(_, l)| l()).collect(),
        }
    }

    /// An edit is open (Esc closes that first).
    pub fn editing(&self) -> bool {
        self.edit.is_some()
    }

    pub fn update(&mut self, msg: KmMsg, builtin: bool) -> Outcome {
        let mut out = Outcome::default();
        // Recording ends with a key (or any other click): it never stays on by accident.
        if !matches!(msg, KmMsg::Capture | KmMsg::Captured(..)) {
            CAPTURING.store(false, Ordering::SeqCst);
        }
        if let Some(e) = &mut self.edit {
            e.error = None;
        }
        match msg {
            KmMsg::Select(i) => self.selected = Some(i),
            KmMsg::Add => {
                self.edit = Some(Edit {
                    at: None,
                    chord: None,
                    button: Button::default(),
                    error: None,
                });
            }
            KmMsg::Edit => {
                if let Some(i) = self.selected.filter(|&i| i < self.keymap.keys.len()) {
                    let b = &self.keymap.keys[i];
                    self.edit = Some(Edit {
                        at: Some(i),
                        chord: Some(b.keys.clone()),
                        button: b.button.clone(),
                        error: None,
                    });
                }
            }
            KmMsg::Delete => {
                if let Some(i) = self.selected.take() {
                    self.keymap.remove(i);
                    out.save = true;
                }
            }
            KmMsg::Capture => {
                CAPTURING.store(true, Ordering::SeqCst);
                out.unfocus = true;
            }
            KmMsg::Captured(key, mods) => {
                CAPTURING.store(false, Ordering::SeqCst);
                let code = match key {
                    cosmic::iced::keyboard::key::Physical::Code(c) => format!("{c:?}"),
                    _ => String::new(),
                };
                let chord = Chord::new(mods.control(), mods.alt(), mods.shift(), &code);
                if let Some(e) = &mut self.edit {
                    // Esc on its own gives the recording up.
                    if code == "Escape" && !(chord.ctrl || chord.alt || chord.shift) {
                        return out;
                    }
                    e.error = match chord.check() {
                        Err(Reason::Modifier) => Some(fl!("km-modifier")),
                        Err(Reason::Reserved) => Some(fl!("km-reserved")),
                        Err(Reason::Terminal) => Some(fl!("km-terminal")),
                        Err(Reason::Ui) => Some(fl!("km-ui")),
                        Ok(()) if builtin => Some(fl!("km-builtin")),
                        Ok(()) => None,
                    };
                    if e.error.is_none() {
                        e.chord = Some(chord);
                    }
                }
            }
            KmMsg::Label(t) => self.map(|b| b.label = t),
            KmMsg::Arg(t) => self.map(|b| b.arg = t),
            KmMsg::Args(t) => self.map(|b| b.args = t),
            KmMsg::KindSel(i) => self.map(|b| {
                b.kind = Kind::ALL.get(i).copied().unwrap_or_default();
                b.target = Target::Current;
                b.arg = match b.kind {
                    Kind::Menu => FUNCTIONS[0].0.to_string(),
                    _ => String::new(),
                };
                b.args.clear();
            }),
            KmMsg::FunctionSel(i) => self.map(|b| {
                if let Some((name, _)) = FUNCTIONS.get(i) {
                    b.arg = (*name).to_string();
                }
            }),
            KmMsg::TargetSel(i) => self.map(|b| {
                b.target = if i == 1 { Target::All } else { Target::Current };
            }),
            KmMsg::Ok => {
                if let Some(e) = &mut self.edit {
                    let Some(chord) = e.chord.clone() else {
                        e.error = Some(fl!("km-no-key"));
                        return out;
                    };
                    let b = Binding {
                        keys: chord,
                        button: e.button.clone(),
                    };
                    match self.keymap.put(e.at, b) {
                        Ok(i) => {
                            self.selected = Some(i);
                            self.edit = None;
                            out.save = true;
                        }
                        Err(Refused::Taken) => e.error = Some(fl!("km-taken-own")),
                        Err(Refused::Empty) => e.error = Some(fl!("bb-arg-empty")),
                        Err(Refused::Chord(_)) => e.error = Some(fl!("km-terminal")),
                    }
                }
            }
            KmMsg::Cancel => {
                CAPTURING.store(false, Ordering::SeqCst);
                self.edit = None;
            }
            KmMsg::Close => {
                CAPTURING.store(false, Ordering::SeqCst);
                out.close = true;
            }
        }
        out
    }

    fn map(&mut self, f: impl FnOnce(&mut Button)) {
        if let Some(e) = &mut self.edit {
            f(&mut e.button);
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let m = Message::KeyMap;
        match &self.edit {
            Some(e) => self.edit_view(e),
            None => {
                let mut list = widget::column::with_capacity(self.keymap.keys.len().max(1))
                    .spacing(4)
                    .width(Length::Fill);
                if self.keymap.keys.is_empty() {
                    list = list.push(widget::text::body(fl!("km-empty")));
                }
                for (i, b) in self.keymap.keys.iter().enumerate() {
                    let what = match (b.button.label.trim(), b.button.kind) {
                        ("", Kind::Menu) => {
                            crate::button_bar::ButtonBar::function_label_of(&b.button.arg)
                        }
                        ("", _) => b.button.arg.chars().take(40).collect(),
                        (l, _) => l.to_string(),
                    };
                    let text = format!("{}   →   {what}", b.keys);
                    let mut btn = widget::button::custom(widget::text::body(text))
                        .width(Length::Fill)
                        .on_press(m(KmMsg::Select(i)));
                    btn = btn.class(match self.selected == Some(i) {
                        true => cosmic::theme::Button::Suggested,
                        false => cosmic::theme::Button::Text,
                    });
                    list = list.push(btn);
                }
                let sel = self.selected.is_some();
                let row = widget::row::with_children(vec![
                    widget::button::standard(fl!("km-add"))
                        .on_press(m(KmMsg::Add))
                        .into(),
                    widget::button::standard(fl!("km-edit"))
                        .on_press_maybe(sel.then(|| m(KmMsg::Edit)))
                        .into(),
                    widget::button::destructive(fl!("km-delete"))
                        .on_press_maybe(sel.then(|| m(KmMsg::Delete)))
                        .into(),
                ])
                .spacing(8);
                let body = widget::column::with_children(vec![
                    widget::text::caption(fl!("km-hint")).into(),
                    widget::scrollable(list).height(Length::Fixed(240.0)).into(),
                    row.into(),
                ])
                .spacing(10);
                widget::dialog()
                    .icon(crate::dialog_icons::Kind::Keyboard.icon(48))
                    .title(fl!("km-title"))
                    .control(body)
                    .primary_action(
                        widget::button::standard(fl!("sftp-close")).on_press(m(KmMsg::Close)),
                    )
                    .into()
            }
        }
    }

    fn edit_view<'a>(&'a self, e: &'a Edit) -> Element<'a, Message> {
        let m = Message::KeyMap;
        let field = |label: String, w: Element<'a, Message>| -> Element<'a, Message> {
            widget::column::with_children(vec![widget::text::caption(label).into(), w])
                .spacing(2)
                .into()
        };
        let text = |v: &str, f: fn(String) -> KmMsg| -> Element<'a, Message> {
            widget::text_input("", v.to_string())
                .on_input(move |t| Message::KeyMap(f(t)))
                .into()
        };
        let shown = match (capturing(), &e.chord) {
            (true, _) => fl!("km-press"),
            (false, Some(c)) => c.to_string(),
            (false, None) => fl!("km-record"),
        };
        let b = &e.button;
        let kind = Kind::ALL.iter().position(|k| *k == b.kind);
        let mut col = widget::column::with_capacity(8).spacing(10);
        col = col
            .push(field(
                fl!("km-key"),
                widget::button::standard(shown)
                    .on_press(m(KmMsg::Capture))
                    .into(),
            ))
            .push(field(fl!("bb-label"), text(&b.label, KmMsg::Label)))
            .push(field(
                fl!("bb-action"),
                widget::dropdown(&self.kinds, kind, move |i| m(KmMsg::KindSel(i))).into(),
            ));
        col = match b.kind {
            Kind::Menu => {
                let at = FUNCTIONS.iter().position(|(n, _)| *n == b.arg);
                col.push(field(
                    fl!("bb-function"),
                    widget::dropdown(&self.functions, at, move |i| m(KmMsg::FunctionSel(i))).into(),
                ))
            }
            Kind::Send => col
                .push(field(fl!("bb-string"), text(&b.arg, KmMsg::Arg)))
                .push(widget::text::caption(fl!("bb-string-hint")))
                .push(field(
                    fl!("bb-target"),
                    widget::dropdown(
                        &self.targets,
                        Some(usize::from(b.target == Target::All)),
                        move |i| m(KmMsg::TargetSel(i)),
                    )
                    .into(),
                )),
            Kind::Launch => col
                .push(field(fl!("bb-command"), text(&b.arg, KmMsg::Arg)))
                .push(widget::text::caption(fl!("bb-command-hint"))),
            Kind::Script => col
                .push(field(fl!("bb-script-file"), text(&b.arg, KmMsg::Arg)))
                .push(field(fl!("bb-script-args"), text(&b.args, KmMsg::Args)))
                .push(widget::text::caption(fl!("bb-script-hint"))),
        };
        if let Some(err) = &e.error {
            col = col.push(widget::text::body(err.clone()).class(cosmic::theme::Text::Accent));
        }
        widget::dialog()
            .icon(crate::dialog_icons::Kind::Keyboard.icon(48))
            .title(fl!("km-edit-title"))
            .control(col)
            .primary_action(widget::button::suggested(fl!("ok")).on_press(m(KmMsg::Ok)))
            .secondary_action(widget::button::standard(fl!("cancel")).on_press(m(KmMsg::Cancel)))
            .into()
    }
}
