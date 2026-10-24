//! Command Window, as SecureCRT's: text under the terminals that is sent to the current tab,
//! the visible ones or all of them. Escapes of `core::macros` (`\r \t \e \p` …) work in it; Enter
//! sends every line, each with a carriage return; Ctrl+Enter (or Shift+Enter) starts a new line. The history lives only as long as the program runs: what
//! is typed here may be a password, and nothing secret is written to a file.

use crate::app::Message;
use crate::fl;
use cosmic::Element;
use cosmic::iced::{Alignment, Length};
use cosmic::widget;
use cosmic::widget::text_editor::{self, Binding};
use shagshell_core::macros::{self, Piece};

const HISTORY: usize = 100;
/// Lines shown before the editor scrolls.
const MAX_ROWS: usize = 6;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SendTo {
    #[default]
    Current,
    /// The front tab of every group.
    Visible,
    All,
}

impl SendTo {
    pub const ALL: [SendTo; 3] = [SendTo::Current, SendTo::Visible, SendTo::All];
}

#[derive(Clone, Debug)]
pub enum CwMsg {
    Edit(text_editor::Action),
    Submit,
    /// Up / down at the first / last line: older / newer history entry.
    Older,
    Newer,
    Target(usize),
    /// A history entry picked: into the line.
    History(usize),
}

pub struct CommandWindow {
    text: text_editor::Content,
    /// Position in `history` (from the end) while browsing it with the arrow keys.
    browsing: Option<usize>,
    /// Newest last, no repeats in a row.
    history: Vec<String>,
    pub to: SendTo,
    pub input_id: widget::Id,
    targets: Vec<String>,
}

impl CommandWindow {
    pub fn new() -> Self {
        Self {
            text: text_editor::Content::new(),
            browsing: None,
            history: Vec::new(),
            to: SendTo::Current,
            input_id: widget::Id::unique(),
            targets: Self::labels(),
        }
    }

    /// The language may change: the labels are made again.
    pub fn labels() -> Vec<String> {
        vec![fl!("cw-current"), fl!("cw-visible"), fl!("cw-all")]
    }

    pub fn relabel(&mut self) {
        self.targets = Self::labels();
    }

    fn set_text(&mut self, t: &str) {
        self.text = text_editor::Content::with_text(t);
        self.text
            .perform(text_editor::Action::Move(text_editor::Motion::DocumentEnd));
    }

    /// What Enter sends, and where: an empty text is a bare Enter, as SecureCRT's.
    pub fn update(&mut self, msg: CwMsg) -> Option<(SendTo, Vec<Piece>)> {
        match msg {
            CwMsg::Edit(a) => {
                if a.is_edit() {
                    self.browsing = None;
                }
                self.text.perform(a);
            }
            CwMsg::Older | CwMsg::Newer => {
                let n = self.history.len();
                let at = match (msg, self.browsing) {
                    (CwMsg::Older, None) => n.checked_sub(1),
                    (CwMsg::Older, Some(i)) => Some(i.saturating_sub(1)),
                    (_, Some(i)) if i + 1 < n => Some(i + 1),
                    _ => None,
                };
                self.browsing = at;
                let t = at.map(|i| self.history[i].clone()).unwrap_or_default();
                self.set_text(&t);
            }
            CwMsg::Target(i) => self.to = SendTo::ALL.get(i).copied().unwrap_or_default(),
            CwMsg::History(i) => {
                if let Some(h) = self.history.get(i).cloned() {
                    self.set_text(&h);
                }
            }
            CwMsg::Submit => {
                let line = self.text.text().trim_end().to_string();
                self.text = text_editor::Content::new();
                self.browsing = None;
                if !line.is_empty() && self.history.last() != Some(&line) {
                    self.history.push(line.clone());
                    if self.history.len() > HISTORY {
                        self.history.remove(0);
                    }
                }
                return Some((self.to, macros::lines(&line)));
            }
        }
        None
    }

    pub fn view(&self) -> Element<'_, Message> {
        let m = Message::CommandWindow;
        let at = SendTo::ALL.iter().position(|t| *t == self.to);
        let history: Vec<String> = self
            .history
            .iter()
            .rev()
            .map(|h| match h.split_once('\n') {
                Some((first, _)) => format!("{first} …"),
                None => h.clone(),
            })
            .collect();
        let n = history.len();
        let (line, last) = (
            self.text.cursor().position.line,
            self.text.line_count().saturating_sub(1),
        );
        let rows = self.text.line_count().clamp(1, MAX_ROWS) as f32;
        let editor = text_editor::TextEditor::new(&self.text)
            .placeholder(fl!("cw-placeholder"))
            .id(self.input_id.clone())
            .on_action(move |a| m(CwMsg::Edit(a)))
            .padding(6)
            .height(rows * 22.0 + 14.0)
            .key_binding(move |kp| {
                use cosmic::iced::keyboard::{Key, key::Named};
                let Key::Named(k) = &kp.key else {
                    return Binding::from_key_press(kp);
                };
                match k {
                    // Enter sends; with Ctrl or Shift it breaks the line, as SecureCRT's.
                    Named::Enter if kp.modifiers.control() || kp.modifiers.shift() => {
                        Some(Binding::Enter)
                    }
                    Named::Enter => Some(Binding::Custom(m(CwMsg::Submit))),
                    Named::ArrowUp if line == 0 && kp.modifiers.is_empty() => {
                        Some(Binding::Custom(m(CwMsg::Older)))
                    }
                    Named::ArrowDown if line == last && kp.modifiers.is_empty() => {
                        Some(Binding::Custom(m(CwMsg::Newer)))
                    }
                    _ => Binding::from_key_press(kp),
                }
            });
        let mut row = widget::row::with_capacity(4)
            .spacing(8)
            .align_y(Alignment::End)
            .push(widget::container(editor).width(Length::Fill));
        if n > 0 {
            // Newest first; a pick goes into the line, not sent.
            row = row.push(widget::dropdown(history, None::<usize>, move |i| {
                m(CwMsg::History(n - 1 - i))
            }));
        }
        row = row
            .push(widget::dropdown(&self.targets, at, move |i| {
                m(CwMsg::Target(i))
            }))
            .push(widget::button::suggested(fl!("cw-send")).on_press(m(CwMsg::Submit)));
        widget::container(row).padding([6, 12]).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn type_text(c: &mut CommandWindow, t: &str) {
        c.set_text(t);
    }

    #[test]
    fn submit_sends_lines_and_enter_and_remembers() {
        let mut c = CommandWindow::new();
        type_text(&mut c, "show run");
        c.update(CwMsg::Target(2));
        let (to, pieces) = c.update(CwMsg::Submit).unwrap();
        assert_eq!(to, SendTo::All);
        assert_eq!(pieces, [Piece::Bytes(b"show run\r".to_vec())]);
        assert_eq!(c.history, ["show run"]);
        // the same again: not a second entry; an empty text: a bare Enter
        type_text(&mut c, "show run");
        c.update(CwMsg::Submit);
        assert_eq!(c.history.len(), 1);
        let (_, p) = c.update(CwMsg::Submit).unwrap();
        assert_eq!(p, [Piece::Bytes(b"\r".to_vec())]);
        c.update(CwMsg::History(0));
        assert_eq!(c.text.text().trim_end(), "show run");
    }

    #[test]
    fn several_lines_go_one_by_one_and_arrows_browse() {
        let mut c = CommandWindow::new();
        type_text(&mut c, "a\nb");
        let (_, p) = c.update(CwMsg::Submit).unwrap();
        assert_eq!(
            p,
            [Piece::Bytes(b"a\r".to_vec()), Piece::Bytes(b"b\r".to_vec())]
        );
        type_text(&mut c, "c");
        c.update(CwMsg::Submit);
        c.update(CwMsg::Older);
        assert_eq!(c.text.text().trim_end(), "c");
        c.update(CwMsg::Older);
        assert_eq!(c.text.text().trim_end(), "a\nb");
        c.update(CwMsg::Newer);
        c.update(CwMsg::Newer);
        assert_eq!(c.text.text().trim_end(), "");
    }
}
