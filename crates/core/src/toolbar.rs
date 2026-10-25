//! The toolbar as a list of buttons (after shagoff-commander's TC-style button bar): each runs an
//! app function (`cmd` = its name) or a program (`cmd` + `params`, `%H %S %U %P` of the active
//! session); `-` is a gap. The editor's state is here, the app draws it.

use serde::{Deserialize, Serialize};

/// A gap between groups of buttons.
pub const SEP: &str = "-";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolButton {
    /// An app function's name (the app's list), a program, or `SEP`.
    pub cmd: String,
    /// A program's arguments.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub params: String,
    /// An icon name of the theme; empty: the function's own.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub icon: String,
    /// The hint on hover; empty: the function's name.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub tip: String,
}

impl ToolButton {
    pub fn function(name: &str) -> Self {
        Self {
            cmd: name.into(),
            ..Default::default()
        }
    }

    pub fn sep() -> Self {
        Self::function(SEP)
    }

    pub fn is_sep(&self) -> bool {
        self.cmd == SEP
    }
}

#[derive(Clone, Debug)]
pub enum ToolMsg {
    Select(usize),
    /// An app function (by name): its icon and hint, no parameters.
    Function(String),
    Cmd(String),
    Params(String),
    Icon(String),
    Tip(String),
    Add,
    AddSep,
    Delete,
    Up,
    Down,
    /// Back to the default bar.
    Reset,
    /// The icon picker: open, its icons (loaded in the background), filter, pick one, back.
    PickOpen,
    PickLoaded(Vec<String>),
    PickFilter(String),
    Pick(String),
    PickClose,
}

/// The «Toolbar» window: the list being edited, the selected line, the icon picker.
#[derive(Clone, Debug, Default)]
pub struct ToolEdit {
    pub list: Vec<ToolButton>,
    pub sel: Option<usize>,
    /// (all icon names, filter) while the picker is shown.
    pub picker: Option<(Vec<String>, String)>,
    default: Vec<ToolButton>,
}

impl ToolEdit {
    pub fn new(list: Vec<ToolButton>, default: Vec<ToolButton>) -> Self {
        let sel = (!list.is_empty()).then_some(0);
        Self {
            list,
            sel,
            picker: None,
            default,
        }
    }

    fn insert(&mut self, b: ToolButton) {
        let at = self
            .sel
            .map_or(self.list.len(), |i| (i + 1).min(self.list.len()));
        self.list.insert(at, b);
        self.sel = Some(at);
    }

    pub fn update(&mut self, m: ToolMsg) {
        let sel = self.sel.filter(|&i| i < self.list.len());
        let button = sel.filter(|&i| !self.list[i].is_sep());
        match (m, button) {
            (ToolMsg::Select(i), _) => self.sel = (i < self.list.len()).then_some(i),
            (ToolMsg::Function(name), Some(i)) => self.list[i] = ToolButton::function(&name),
            (ToolMsg::Cmd(s), Some(i)) if s != SEP => self.list[i].cmd = s,
            (ToolMsg::Params(s), Some(i)) => self.list[i].params = s,
            (ToolMsg::Icon(s), Some(i)) => self.list[i].icon = s,
            (ToolMsg::Tip(s), Some(i)) => self.list[i].tip = s,
            (ToolMsg::Add, _) => self.insert(ToolButton {
                icon: "application-x-executable-symbolic".into(),
                ..Default::default()
            }),
            (ToolMsg::AddSep, _) => self.insert(ToolButton::sep()),
            (ToolMsg::Delete, _) => {
                if let Some(i) = sel {
                    self.list.remove(i);
                    self.sel = (!self.list.is_empty()).then(|| i.min(self.list.len() - 1));
                }
            }
            (ToolMsg::Up, _) => {
                if let Some(i) = sel.filter(|&i| i > 0) {
                    self.list.swap(i, i - 1);
                    self.sel = Some(i - 1);
                }
            }
            (ToolMsg::Down, _) => {
                if let Some(i) = sel.filter(|&i| i + 1 < self.list.len()) {
                    self.list.swap(i, i + 1);
                    self.sel = Some(i + 1);
                }
            }
            (ToolMsg::Reset, _) => {
                let default = std::mem::take(&mut self.default);
                *self = Self::new(default.clone(), default);
            }
            (ToolMsg::PickOpen, Some(_)) => self.picker = Some((Vec::new(), String::new())),
            (ToolMsg::PickLoaded(names), _) => {
                if let Some((all, _)) = &mut self.picker {
                    *all = names;
                }
            }
            (ToolMsg::PickFilter(s), _) => {
                if let Some((_, f)) = &mut self.picker {
                    *f = s;
                }
            }
            (ToolMsg::Pick(name), Some(i)) => {
                self.list[i].icon = name;
                self.picker = None;
            }
            (ToolMsg::PickClose, _) => self.picker = None,
            _ => {}
        }
    }
}

/// The icon names matching `filter` (each word, any case), at most `max`, and how many matched.
pub fn matching<'a>(all: &'a [String], filter: &str, max: usize) -> (Vec<&'a str>, usize) {
    let words: Vec<String> = filter.split_whitespace().map(str::to_lowercase).collect();
    let hits: Vec<&str> = (all.iter())
        .filter(|n| words.iter().all(|w| n.contains(w.as_str())))
        .map(String::as_str)
        .collect();
    let n = hits.len();
    (hits.into_iter().take(max).collect(), n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmds(t: &ToolEdit) -> Vec<&str> {
        t.list.iter().map(|b| b.cmd.as_str()).collect()
    }

    #[test]
    fn edit_add_move_delete_reset() {
        let default = vec![ToolButton::function("Connect"), ToolButton::sep()];
        let mut t = ToolEdit::new(vec![], default.clone());
        t.update(ToolMsg::Add);
        t.update(ToolMsg::Cmd("xterm".into()));
        t.update(ToolMsg::Params("-e ssh %H".into()));
        t.update(ToolMsg::AddSep);
        t.update(ToolMsg::Cmd("x".into())); // a gap has no command
        t.update(ToolMsg::Up);
        assert_eq!(cmds(&t), ["-", "xterm"]);
        assert_eq!(t.list[1].params, "-e ssh %H");
        t.update(ToolMsg::Select(1));
        t.update(ToolMsg::Function("Reconnect".into())); // a function: params and icon dropped
        assert_eq!(t.list[1], ToolButton::function("Reconnect"));
        t.update(ToolMsg::Down); // last: stays
        assert_eq!(cmds(&t), ["-", "Reconnect"]);
        t.update(ToolMsg::Delete);
        t.update(ToolMsg::Delete);
        assert_eq!((cmds(&t).len(), t.sel), (0, None));
        t.update(ToolMsg::Reset);
        assert_eq!((t.list.clone(), t.sel), (default.clone(), Some(0)));
        t.update(ToolMsg::Reset); // the default is kept for a second reset
        assert_eq!(t.list, default);
    }

    #[test]
    fn picker_filters_by_every_word_and_picks() {
        let all: Vec<String> = ["edit-copy-symbolic", "edit-cut-symbolic", "go-up-symbolic"]
            .map(String::from)
            .to_vec();
        assert_eq!(matching(&all, "  Copy EDIT ", 10).0, ["edit-copy-symbolic"]);
        assert_eq!(
            matching(&all, "", 2),
            (vec!["edit-copy-symbolic", "edit-cut-symbolic"], 3)
        );
        let mut t = ToolEdit::new(vec![ToolButton::function("Connect")], vec![]);
        t.update(ToolMsg::PickOpen);
        t.update(ToolMsg::PickLoaded(all));
        t.update(ToolMsg::Pick("go-up-symbolic".into()));
        assert_eq!(
            (t.list[0].icon.as_str(), t.picker.is_none()),
            ("go-up-symbolic", true)
        );
    }

    #[test]
    fn a_button_is_written_short() {
        let b = ToolButton::function("Connect");
        assert_eq!(toml::to_string(&b).unwrap().trim(), r#"cmd = "Connect""#);
    }
}
