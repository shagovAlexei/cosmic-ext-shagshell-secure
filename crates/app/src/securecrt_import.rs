//! File → Import from SecureCRT…: a window of the app (a dialog) asking for SecureCRT's `Config`
//! folder, what to bring (sessions, Button Bar), and telling what came across. The work is
//! `core::securecrt::import`, run off the UI thread (a real config has over a thousand files).

use crate::app::Message;
use crate::fl;
use cosmic::Element;
use cosmic::iced::Length;
use cosmic::widget;
use shagshell_core::securecrt::Report;

#[derive(Clone, Debug)]
pub enum ScMsg {
    Path(String),
    Browse,
    Browsed(Option<String>),
    Sessions(bool),
    Bars(bool),
    Run,
    /// The import is over: the report, or why not.
    Done(Result<ReportView, String>),
    Close,
}

/// A report in numbers (what the app needs after the work: the new bars go to the Button Bar).
#[derive(Clone, Debug)]
pub struct ReportView {
    pub text: String,
    pub bars: Vec<shagshell_core::buttonbar::Bar>,
}

impl ReportView {
    pub fn of(r: Report) -> Self {
        let buttons = r.bars.iter().map(|b| b.buttons.len()).sum::<usize>() as i64;
        let (added, existing, skipped) = (
            r.sessions_added as i64,
            r.sessions_existing as i64,
            r.sessions_skipped as i64,
        );
        let (bars, run) = (r.bars.len() as i64, r.buttons_skipped as i64);
        let text = fl!(
            "sc-report",
            sessions = added,
            existing = existing,
            skipped = skipped,
            bars = bars,
            buttons = buttons,
            run = run
        );
        Self { text, bars: r.bars }
    }
}

pub struct ScImport {
    pub path: String,
    sessions: bool,
    bars: bool,
    busy: bool,
    /// What happened last: the report or the error.
    result: Option<(bool, String)>,
    pub input_id: widget::Id,
}

impl ScImport {
    pub fn new() -> Self {
        let path = std::env::home_dir()
            .and_then(|h| shagshell_core::securecrt::find_config(&h))
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        Self {
            path,
            sessions: true,
            bars: true,
            busy: false,
            result: None,
            input_id: widget::Id::unique(),
        }
    }

    pub fn path(&self) -> std::path::PathBuf {
        std::path::PathBuf::from(self.path.trim())
    }

    pub fn wants(&self) -> (bool, bool) {
        (self.sessions, self.bars)
    }

    /// `true`: start the import.
    pub fn update(&mut self, msg: ScMsg) -> bool {
        match msg {
            ScMsg::Path(p) => self.path = p,
            ScMsg::Browsed(Some(p)) => self.path = p,
            ScMsg::Sessions(on) => self.sessions = on,
            ScMsg::Bars(on) => self.bars = on,
            ScMsg::Run => {
                let ok = !self.busy && (self.sessions || self.bars) && self.path().is_dir();
                if !self.path().is_dir() {
                    self.result = Some((false, fl!("sc-no-folder")));
                } else if ok {
                    self.busy = true;
                    self.result = None;
                }
                return ok;
            }
            ScMsg::Done(r) => {
                self.busy = false;
                self.result = Some(match r {
                    Ok(v) => (true, v.text),
                    Err(e) => (false, e),
                });
            }
            ScMsg::Browse | ScMsg::Browsed(None) | ScMsg::Close => {}
        }
        false
    }

    pub fn view(&self) -> Element<'_, Message> {
        let m = Message::SecureCrt;
        let mut col = widget::column::with_capacity(6)
            .spacing(10)
            .push(widget::text::body(fl!("sc-hint")));
        col = col.push(
            widget::row::with_children(vec![
                widget::text_input(fl!("sc-folder"), self.path.clone())
                    .id(self.input_id.clone())
                    .on_input(move |p| m(ScMsg::Path(p)))
                    .on_submit(move |_| m(ScMsg::Run))
                    .width(Length::Fill)
                    .into(),
                widget::button::standard(fl!("so-browse"))
                    .on_press(m(ScMsg::Browse))
                    .into(),
            ])
            .spacing(8),
        );
        col = col
            .push(
                widget::checkbox(self.sessions)
                    .label(fl!("sc-sessions"))
                    .on_toggle(move |v| m(ScMsg::Sessions(v))),
            )
            .push(
                widget::checkbox(self.bars)
                    .label(fl!("sc-bars"))
                    .on_toggle(move |v| m(ScMsg::Bars(v))),
            );
        if self.busy {
            col = col.push(widget::text::body(fl!("sc-working")));
        }
        if let Some((ok, text)) = &self.result {
            let t = widget::text::body(text.clone());
            col = col.push(match ok {
                true => t,
                false => t.class(cosmic::theme::Text::Accent),
            });
        }
        let run = widget::button::suggested(fl!("sc-import"))
            .on_press_maybe((!self.busy).then(|| m(ScMsg::Run)));
        widget::dialog()
            .title(fl!("sc-title"))
            .control(col)
            .primary_action(run)
            .secondary_action(widget::button::standard(fl!("sftp-close")).on_press(m(ScMsg::Close)))
            .into()
    }
}

/// «Обзор…»: the folder from the portal's chooser.
pub async fn browse(title: String) -> Option<String> {
    use cosmic::dialog::file_chooser::open::Dialog;
    let picked = Dialog::new().title(title).open_folder().await.ok()?;
    Some(picked.url().to_file_path().ok()?.display().to_string())
}
