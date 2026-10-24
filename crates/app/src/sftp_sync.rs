//! Synchronize Folders in an SFTP tab: the dialog's state and view (setup → compare → preview).
//! The comparison is `core::sftp::sync`; `sftp_view` reads the trees through the worker and
//! queues what is chosen.

use crate::app::Message;
use crate::fl;
use crate::sftp_view::SftpMsg;
use crate::term::TabId;
use cosmic::Element;
use cosmic::iced::Length;
use cosmic::widget;
use shagshell_core::panel::size_text;
use shagshell_core::sftp::sync::{self, Kind, Mode, Node, Op, Why};
use std::path::PathBuf;

/// Rows of the preview shown (the rest are counted): a long list is slow to draw.
const SHOWN: usize = 300;

#[derive(Clone, Debug)]
pub enum SyncMsg {
    Mode(usize),
    Recursive(bool),
    Mirror(bool),
    Compare,
    Toggle(usize),
    All(bool),
    Back,
    Run,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Setup,
    Scanning,
    Preview,
}

pub struct SyncState {
    pub local: PathBuf,
    pub remote: PathBuf,
    mode: usize,
    pub recursive: bool,
    pub mirror: bool,
    pub phase: Phase,
    pub ops: Vec<Op>,
    pub checked: Vec<bool>,
    pub error: Option<String>,
    modes: Vec<String>,
}

impl SyncState {
    pub fn new(local: PathBuf, remote: PathBuf) -> Self {
        Self {
            local,
            remote,
            mode: 0,
            recursive: true,
            mirror: false,
            phase: Phase::Setup,
            ops: Vec::new(),
            checked: Vec::new(),
            error: None,
            modes: vec![fl!("sy-up"), fl!("sy-down"), fl!("sy-both")],
        }
    }

    pub fn mode(&self) -> Mode {
        match self.mode {
            0 => Mode::Up,
            1 => Mode::Down,
            _ => Mode::Both,
        }
    }

    pub fn update(&mut self, msg: &SyncMsg) {
        self.error = None;
        match msg {
            SyncMsg::Mode(i) => {
                self.mode = *i;
                self.mirror &= self.mode() != Mode::Both;
            }
            SyncMsg::Recursive(v) => self.recursive = *v,
            SyncMsg::Mirror(v) => self.mirror = *v && (self.mode() != Mode::Both),
            SyncMsg::Toggle(i) => {
                if let Some(c) = self.checked.get_mut(*i) {
                    *c = !*c;
                }
            }
            SyncMsg::All(v) => {
                for (c, o) in self.checked.iter_mut().zip(&self.ops) {
                    *c = *v && o.kind != Kind::Conflict;
                }
            }
            SyncMsg::Back => self.phase = Phase::Setup,
            SyncMsg::Compare | SyncMsg::Run => {}
        }
    }

    /// The trees came back: the list of what differs.
    pub fn compared(&mut self, local: &[Node], remote: &[Node]) {
        self.ops = sync::diff(local, remote, self.mode(), self.mirror);
        self.checked = self.ops.iter().map(|o| o.kind != Kind::Conflict).collect();
        self.phase = Phase::Preview;
    }

    pub fn failed(&mut self, why: String) {
        self.phase = Phase::Setup;
        self.error = Some(why);
    }

    /// What is ticked (a conflict never is).
    pub fn chosen(&self) -> Vec<Op> {
        (self.ops.iter().zip(&self.checked))
            .filter(|(o, c)| **c && o.kind != Kind::Conflict)
            .map(|(o, _)| o.clone())
            .collect()
    }

    pub fn view(&self, id: TabId) -> Element<'_, Message> {
        let m = move |x| Message::Sftp(id, SftpMsg::Sync(x));
        let cancel = widget::button::standard(fl!("cancel"))
            .on_press(Message::Sftp(id, SftpMsg::DialogCancel));
        let mut body = widget::column::with_capacity(8).spacing(10);
        let path = |label: String, p: &PathBuf| {
            widget::column::with_children(vec![
                widget::text::caption(label).into(),
                widget::text::body(p.display().to_string()).into(),
            ])
            .spacing(2)
        };
        let primary = match self.phase {
            Phase::Setup => {
                body = body
                    .push(path(fl!("sy-local"), &self.local))
                    .push(path(fl!("sy-remote"), &self.remote))
                    .push(
                        widget::column::with_children(vec![
                            widget::text::caption(fl!("sy-mode")).into(),
                            widget::dropdown(&self.modes, Some(self.mode), move |i| {
                                m(SyncMsg::Mode(i))
                            })
                            .into(),
                        ])
                        .spacing(2),
                    )
                    .push(
                        widget::checkbox(self.recursive)
                            .label(fl!("sy-recursive"))
                            .on_toggle(move |v| m(SyncMsg::Recursive(v))),
                    )
                    .push(
                        widget::checkbox(self.mirror)
                            .label(fl!("sy-mirror"))
                            .on_toggle_maybe(
                                (self.mode() != Mode::Both)
                                    .then_some(move |v| m(SyncMsg::Mirror(v))),
                            ),
                    );
                if let Some(e) = &self.error {
                    body =
                        body.push(widget::text::body(e.clone()).class(cosmic::theme::Text::Accent));
                }
                widget::button::suggested(fl!("sy-compare")).on_press(m(SyncMsg::Compare))
            }
            Phase::Scanning => {
                body = body.push(widget::text::body(fl!("sy-scanning")));
                widget::button::suggested(fl!("sy-compare"))
            }
            Phase::Preview => {
                let chosen = self.chosen();
                let (files, bytes, deletes) = sync::totals(&chosen);
                if self.ops.is_empty() {
                    body = body.push(widget::text::body(fl!("sy-same")));
                } else {
                    body = body.push(widget::text::body(fl!(
                        "sy-summary",
                        files = i64::try_from(files).unwrap_or(0),
                        size = size_text(bytes),
                        deletes = i64::try_from(deletes).unwrap_or(0)
                    )));
                    let mut list = widget::column::with_capacity(self.ops.len().min(SHOWN))
                        .spacing(2)
                        .width(Length::Fill);
                    for (i, o) in self.ops.iter().enumerate().take(SHOWN) {
                        list = list.push(self.row(i, o, m));
                    }
                    if self.ops.len() > SHOWN {
                        list = list.push(widget::text::caption(fl!(
                            "sy-more",
                            n = i64::try_from(self.ops.len() - SHOWN).unwrap_or(0)
                        )));
                    }
                    body = body.push(widget::scrollable(list).height(Length::Fixed(280.0)));
                    body = body.push(
                        widget::row::with_children(vec![
                            widget::button::text(fl!("sy-all"))
                                .on_press(m(SyncMsg::All(true)))
                                .into(),
                            widget::button::text(fl!("sy-none"))
                                .on_press(m(SyncMsg::All(false)))
                                .into(),
                        ])
                        .spacing(8),
                    );
                }
                if deletes > 0 {
                    body = body.push(
                        widget::text::body(fl!(
                            "sy-delete-warn",
                            n = i64::try_from(deletes).unwrap_or(0)
                        ))
                        .class(cosmic::theme::Text::Accent),
                    );
                }
                body =
                    body.push(widget::button::standard(fl!("sy-back")).on_press(m(SyncMsg::Back)));
                let label = match deletes > 0 {
                    true => fl!("sy-run-delete"),
                    false => fl!("sy-run"),
                };
                widget::button::suggested(label).on_press_maybe(
                    (files + deletes > 0 || self.has_dirs(&chosen)).then(|| m(SyncMsg::Run)),
                )
            }
        };
        widget::dialog()
            .title(fl!("sy-title"))
            .control(body)
            .primary_action(primary)
            .secondary_action(cancel)
            .into()
    }

    fn has_dirs(&self, ops: &[Op]) -> bool {
        ops.iter()
            .any(|o| matches!(o.kind, Kind::MkdirLocal | Kind::MkdirRemote))
    }

    fn row<'a>(
        &'a self,
        i: usize,
        o: &'a Op,
        m: impl Fn(SyncMsg) -> Message + Copy + 'static,
    ) -> Element<'a, Message> {
        let sign = match o.kind {
            Kind::CopyUp => "↑",
            Kind::CopyDown => "↓",
            Kind::MkdirLocal | Kind::MkdirRemote => "+",
            Kind::DeleteLocal | Kind::DeleteRemote => "✕",
            Kind::Conflict => "⚠",
        };
        let why = match o.why {
            Some(Why::New) => fl!("sy-why-new"),
            Some(Why::Newer) => fl!("sy-why-newer"),
            Some(Why::Older) => fl!("sy-why-older"),
            Some(Why::Size) => fl!("sy-why-size"),
            None if o.kind == Kind::Conflict => fl!("sy-conflict"),
            None => String::new(),
        };
        let size = match matches!(o.kind, Kind::CopyUp | Kind::CopyDown) {
            true => format!("  {}", size_text(o.size)),
            false => String::new(),
        };
        let text = format!("{sign}  {}{size}  {why}", o.rel.display());
        let check = widget::checkbox(self.checked.get(i).copied().unwrap_or(false))
            .label(text)
            .on_toggle_maybe((o.kind != Kind::Conflict).then_some(move |_| m(SyncMsg::Toggle(i))));
        check.into()
    }
}
