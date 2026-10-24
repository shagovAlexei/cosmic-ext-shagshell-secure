//! Script → Scheduler…, a window of its own: the jobs on the left, the selected one's fields on
//! the right. The app owns `schedule.toml` and the clock; this module only edits the fields.

use crate::app::Message;
use crate::fl;
use cosmic::iced::{Length, window};
use cosmic::widget::{self, segmented_button};
use cosmic::{Element, theme};
use shagshell_core::schedule::{Every, Job, Schedule};

#[derive(Clone, Debug)]
pub enum SchMsg {
    Select(segmented_button::Entity),
    New,
    Name(String),
    Script(String),
    Args(String),
    Daily(bool),
    Minutes(String),
    Time(String),
    Enabled(bool),
    Save,
    Delete,
    RunNow,
    Drag,
    Close,
}

pub struct SchedWindow {
    pub window: window::Id,
    list: segmented_button::SingleSelectModel,
    /// Whether each job (by index) is enabled: a disabled one is drawn dimmed.
    on: Vec<bool>,
    /// The job being edited (`None`: a new one).
    pub selected: Option<usize>,
    name: String,
    script: String,
    args: String,
    daily: bool,
    minutes: String,
    time: String,
    enabled: bool,
    pub error: Option<String>,
    name_id: widget::Id,
}

impl SchedWindow {
    pub fn new(window: window::Id, s: &Schedule) -> Self {
        let mut w = Self {
            window,
            list: segmented_button::Model::default(),
            on: Vec::new(),
            selected: None,
            name: String::new(),
            script: String::new(),
            args: String::new(),
            daily: false,
            minutes: "60".into(),
            time: "09:00".into(),
            enabled: true,
            error: None,
            name_id: widget::Id::unique(),
        };
        w.reload(s, (!s.jobs.is_empty()).then_some(0));
        w
    }

    pub fn name_field(&self) -> widget::Id {
        self.name_id.clone()
    }

    /// The list again (after a save), `select` chosen.
    pub fn reload(&mut self, s: &Schedule, select: Option<usize>) {
        self.list = segmented_button::Model::default();
        self.on.clear();
        for (i, j) in s.jobs.iter().enumerate() {
            let text = match j.enabled {
                true => j.name.clone(),
                false => format!("{} ({})", j.name, fl!("sch-off")),
            };
            let e = self.list.insert().text(text).data(i).id();
            self.on.push(j.enabled);
            if Some(i) == select {
                self.list.activate(e);
            }
        }
        self.pick(s, select);
    }

    fn pick(&mut self, s: &Schedule, i: Option<usize>) {
        let job = i
            .and_then(|i| s.jobs.get(i))
            .cloned()
            .unwrap_or_else(|| Job {
                enabled: true,
                ..Job::default()
            });
        self.selected = i.filter(|&i| i < s.jobs.len());
        self.name = job.name;
        self.script = job.script;
        self.args = job.args.join(" ");
        (self.daily, self.minutes, self.time) = match job.every.fixed() {
            Every::Minutes { n } => (false, n.to_string(), "09:00".into()),
            Every::Daily { hour, minute } => (true, "60".into(), format!("{hour:02}:{minute:02}")),
        };
        self.enabled = job.enabled;
        self.error = None;
    }

    pub fn update(&mut self, msg: SchMsg, s: &Schedule) {
        match msg {
            SchMsg::Select(e) => {
                self.list.activate(e);
                let i = self.list.data::<usize>(e).copied();
                self.pick(s, i);
            }
            SchMsg::New => {
                self.list.deactivate();
                self.pick(s, None);
            }
            SchMsg::Name(v) => self.name = v,
            SchMsg::Script(v) => self.script = v,
            SchMsg::Args(v) => self.args = v,
            SchMsg::Daily(v) => self.daily = v,
            SchMsg::Minutes(v) => self.minutes = v,
            SchMsg::Time(v) => self.time = v,
            SchMsg::Enabled(v) => self.enabled = v,
            _ => {}
        }
    }

    /// The fields as a job, checked; the error is the text to show.
    pub fn job(&self) -> Result<Job, String> {
        let name = self.name.trim();
        let script = self.script.trim();
        if name.is_empty() || script.is_empty() {
            return Err(fl!("sch-need-name-script"));
        }
        let every = match self.daily {
            false => match self.minutes.trim().parse::<u32>() {
                Ok(n) if n >= 1 => Every::Minutes { n },
                _ => return Err(fl!("sch-bad-minutes")),
            },
            true => {
                let t = self.time.trim().split_once(':').and_then(|(h, m)| {
                    Some((
                        h.parse::<u8>().ok().filter(|&h| h < 24)?,
                        m.parse::<u8>().ok().filter(|&m| m < 60)?,
                    ))
                });
                match t {
                    Some((hour, minute)) => Every::Daily { hour, minute },
                    None => return Err(fl!("sch-bad-time")),
                }
            }
        };
        Ok(Job {
            name: name.to_string(),
            script: script.to_string(),
            args: self.args.split_whitespace().map(str::to_string).collect(),
            every,
            enabled: self.enabled,
        })
    }

    pub fn view(&self, focused: bool, last: &dyn Fn(usize) -> String) -> Element<'_, Message> {
        let m = Message::Scheduler;
        let field =
            |label: String, input: Element<'static, Message>| -> Element<'static, Message> {
                widget::column::with_children(vec![widget::text::caption(label).into(), input])
                    .spacing(2)
                    .into()
            };
        let input = |v: &str, f: fn(String) -> SchMsg| {
            widget::text_input("", v.to_string())
                .on_input(move |v| m(f(v)))
                .on_submit(move |_| m(SchMsg::Save))
        };
        let kind = |label: String, on: bool, daily: bool| {
            let b = match on {
                true => widget::button::suggested(label),
                false => widget::button::standard(label),
            };
            b.on_press(m(SchMsg::Daily(daily)))
        };
        let when: Element<'_, Message> = match self.daily {
            false => field(
                fl!("sch-minutes"),
                input(&self.minutes, SchMsg::Minutes)
                    .width(Length::Fixed(100.0))
                    .into(),
            ),
            true => field(
                fl!("sch-time"),
                input(&self.time, SchMsg::Time)
                    .width(Length::Fixed(100.0))
                    .into(),
            ),
        };
        let mut right = widget::column::with_children(vec![
            widget::text::title4(match self.selected {
                Some(_) => self.name.clone(),
                None => fl!("sch-new-title"),
            })
            .into(),
            field(
                fl!("sch-name"),
                input(&self.name, SchMsg::Name)
                    .id(self.name_id.clone())
                    .into(),
            ),
            field(
                fl!("sch-script"),
                crate::script::with_browse(
                    input(&self.script, SchMsg::Script).into(),
                    crate::script::Slot::Sched,
                ),
            ),
            field(fl!("sch-args"), input(&self.args, SchMsg::Args).into()),
            widget::row::with_children(vec![
                kind(fl!("sch-every"), !self.daily, false).into(),
                kind(fl!("sch-daily"), self.daily, true).into(),
                when,
            ])
            .spacing(8)
            .align_y(cosmic::iced::Alignment::End)
            .into(),
            widget::toggler(self.enabled)
                .label(fl!("sch-enabled"))
                .on_toggle(move |v| m(SchMsg::Enabled(v)))
                .into(),
        ])
        .spacing(12)
        .width(Length::Fill);
        if let Some(i) = self.selected {
            right = right.push(widget::text::caption(last(i)));
        }
        if let Some(e) = &self.error {
            right = right.push(widget::text::body(e.clone()).class(theme::Text::Accent));
        }
        let buttons = widget::row::with_children(vec![
            widget::button::standard(fl!("sch-run-now"))
                .on_press_maybe(self.selected.map(|_| m(SchMsg::RunNow)))
                .into(),
            widget::Space::new().width(Length::Fill).into(),
            widget::button::destructive(fl!("sm-delete"))
                .on_press_maybe(self.selected.map(|_| m(SchMsg::Delete)))
                .into(),
            widget::button::suggested(fl!("cw-save"))
                .on_press(m(SchMsg::Save))
                .into(),
        ])
        .spacing(8);
        // The fields scroll, the buttons stay in sight (Russian labels are wider, the form taller).
        let right = widget::column::with_children(vec![
            widget::scrollable(right).height(Length::Fill).into(),
            buttons.into(),
        ])
        .spacing(12)
        .width(Length::Fill);
        let left = widget::column::with_children(vec![
            widget::scrollable(
                widget::column::with_children(
                    self.list
                        .iter()
                        .map(|e| {
                            let i = self.list.data::<usize>(e).copied().unwrap_or_default();
                            let text =
                                widget::text(self.list.text(e).unwrap_or_default().to_string());
                            // Enabled: the normal colour; disabled: dimmed.
                            let text = match self.on.get(i) {
                                Some(false) => text.class(theme::Text::Custom(|t| {
                                    cosmic::iced::widget::text::Style {
                                        color: Some(t.cosmic().palette.neutral_7.into()),
                                        ..Default::default()
                                    }
                                })),
                                _ => text,
                            };
                            let class = match self.list.is_active(e) {
                                true => theme::Button::Suggested,
                                false => theme::Button::Text,
                            };
                            widget::button::custom(text)
                                .class(class)
                                .width(Length::Fill)
                                .on_press(m(SchMsg::Select(e)))
                                .into()
                        })
                        .collect::<Vec<Element<'_, Message>>>(),
                )
                .spacing(4)
                .width(Length::Fixed(150.0)),
            )
            .height(Length::Fill)
            .into(),
            widget::button::standard(fl!("cw-add"))
                .leading_icon(widget::icon::from_name("list-add-symbolic"))
                .on_press(m(SchMsg::New))
                .into(),
        ])
        .spacing(8)
        .width(Length::Fixed(150.0));
        let content = widget::column::with_children(vec![
            widget::header_bar()
                .title(fl!("sch-title"))
                .on_close(m(SchMsg::Close))
                .on_drag(m(SchMsg::Drag))
                .focused(focused)
                .into(),
            widget::row::with_children(vec![left.into(), right.into()])
                .spacing(16)
                .padding(16)
                .height(Length::Fill)
                .into(),
        ]);
        widget::container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .class(theme::Container::WindowBackground)
            .into()
    }
}

pub fn settings() -> window::Settings {
    window::Settings {
        size: cosmic::iced::Size::new(760.0, 560.0),
        min_size: Some(cosmic::iced::Size::new(560.0, 360.0)),
        decorations: false,
        exit_on_close_request: false,
        ..Default::default()
    }
}
