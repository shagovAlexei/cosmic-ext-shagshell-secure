//! Command Window, as SecureCRT's: a line under the terminals that is sent to the current tab,
//! the visible ones or all of them. Escapes of `core::macros` (`\r \t \e \p` …) work in it; Enter
//! sends the line and a carriage return. The history lives only as long as the program runs: what
//! is typed here may be a password, and nothing secret is written to a file.

use crate::app::Message;
use crate::fl;
use cosmic::Element;
use cosmic::iced::{Alignment, Length};
use cosmic::widget;
use shagshell_core::macros::{self, Piece};

const HISTORY: usize = 100;

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
    Input(String),
    Submit,
    Target(usize),
    /// A history entry picked: into the line.
    History(usize),
    /// A click in the window: the keys are its own, not the terminal's.
    Focus,
}

pub struct CommandWindow {
    text: String,
    /// Newest last, no repeats in a row.
    history: Vec<String>,
    pub to: SendTo,
    pub input_id: widget::Id,
    targets: Vec<String>,
}

impl CommandWindow {
    pub fn new() -> Self {
        Self {
            text: String::new(),
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

    /// What Enter sends, and where: `None` for an empty line (SecureCRT sends a bare Enter, so
    /// does this: an empty line is `\r`).
    pub fn update(&mut self, msg: CwMsg) -> Option<(SendTo, Vec<Piece>)> {
        match msg {
            CwMsg::Input(t) => self.text = t,
            CwMsg::Focus => {}
            CwMsg::Target(i) => self.to = SendTo::ALL.get(i).copied().unwrap_or_default(),
            CwMsg::History(i) => {
                if let Some(h) = self.history.get(i) {
                    self.text = h.clone();
                }
            }
            CwMsg::Submit => {
                let line = std::mem::take(&mut self.text);
                if !line.is_empty() && self.history.last() != Some(&line) {
                    self.history.push(line.clone());
                    if self.history.len() > HISTORY {
                        self.history.remove(0);
                    }
                }
                let mut pieces = macros::parse(&line);
                match pieces.last_mut() {
                    Some(Piece::Bytes(b)) => b.push(b'\r'),
                    _ => pieces.push(Piece::Bytes(b"\r".to_vec())),
                }
                return Some((self.to, pieces));
            }
        }
        None
    }

    pub fn view(&self) -> Element<'_, Message> {
        let m = Message::CommandWindow;
        let at = SendTo::ALL.iter().position(|t| *t == self.to);
        let history: Vec<String> = self.history.iter().rev().cloned().collect();
        let n = history.len();
        let mut row = widget::row::with_capacity(4)
            .spacing(8)
            .align_y(Alignment::Center)
            .push(
                widget::text_input(fl!("cw-placeholder"), self.text.clone())
                    .id(self.input_id.clone())
                    .on_input(move |t| m(CwMsg::Input(t)))
                    .on_submit(move |_| m(CwMsg::Submit))
                    // The terminal keeps taking keys after a click here unless the focus is asked
                    // for as an operation (it takes the keys from every other widget).
                    .on_focus(m(CwMsg::Focus))
                    .width(Length::Fill),
            );
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

    #[test]
    fn submit_sends_line_and_enter_and_remembers() {
        let mut c = CommandWindow::new();
        c.update(CwMsg::Input("show run".into()));
        c.update(CwMsg::Target(2));
        let (to, pieces) = c.update(CwMsg::Submit).unwrap();
        assert_eq!(to, SendTo::All);
        assert_eq!(pieces, [Piece::Bytes(b"show run\r".to_vec())]);
        assert_eq!(c.history, ["show run"]);
        // the same again: not a second entry; an empty line: a bare Enter
        c.update(CwMsg::Input("show run".into()));
        c.update(CwMsg::Submit);
        assert_eq!(c.history.len(), 1);
        let (_, p) = c.update(CwMsg::Submit).unwrap();
        assert_eq!(p, [Piece::Bytes(b"\r".to_vec())]);
        c.update(CwMsg::History(0));
        assert_eq!(c.text, "show run");
    }
}
