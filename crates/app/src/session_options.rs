//! Session Options, as SecureCRT's: a window of its own, categories on the left, fields on the
//! right, [Cancel] [OK]. The app saves (it owns the store); this module only edits the fields.

use crate::app::Message;
use crate::fl;
use cosmic::iced::{Length, window};
use cosmic::widget::{self, segmented_button};
use cosmic::{Element, theme};
use shagshell_core::session::form::{Form, FormError};
use shagshell_core::session::store::name_of;
use shagshell_core::session::{Protocol, Session};

/// What OK writes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// «New session…» in this folder ("" = root).
    New {
        folder: String,
    },
    Edit {
        id: String,
    },
    /// `sessions/Default.toml`, the template new sessions copy.
    Default,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Category {
    Connection,
    Ssh2,
    Log,
}

#[derive(Clone, Copy, Debug)]
pub enum Field {
    Name,
    Host,
    Port,
    User,
    Identity,
    Keepalive,
    Extra,
    LogFile,
}

#[derive(Clone, Debug)]
pub enum SoMsg {
    Field(Field, String),
    Category(segmented_button::Entity),
    /// The protocol dropdown: the n-th of `Protocol::ALL`.
    Protocol(usize),
    Browse,
    /// The file chooser's pick (`None`: cancelled), for the window that asked.
    Browsed(window::Id, Option<String>),
    Drag,
    /// The keyring looked up for this window's session: a password or passphrase is saved.
    Saved(window::Id, bool),
    /// «Учётные данные»: 0 = none, else the n-th set.
    Credential(usize),
    /// «Button Bar»: 0 = none, else the n-th bar.
    Bar(usize),
    LogAppend(bool),
    LogRaw(bool),
    /// «Забыть»: the session's saved secrets removed.
    Forget,
    Ok,
    Cancel,
}

pub struct SessionOptions {
    pub window: window::Id,
    pub target: Target,
    form: Form,
    categories: segmented_button::SingleSelectModel,
    pub error: Option<String>,
    /// The session has a secret in the keyring (Edit only).
    saved: bool,
    /// The credential sets (name, user), for the dropdown; `labels[0]` is «(none)».
    sets: Vec<(String, String)>,
    labels: Vec<String>,
    /// The Button Bars (names), for the dropdown; `bar_labels[0]` is «(none)».
    bars: Vec<String>,
    bar_labels: Vec<String>,
    protocols: Vec<String>,
    first_id: widget::Id,
}

impl SessionOptions {
    pub fn new(
        window: window::Id,
        target: Target,
        s: &Session,
        sets: Vec<(String, String)>,
        bars: Vec<String>,
    ) -> Self {
        let bar_labels = std::iter::once(fl!("so-bar-none"))
            .chain(bars.iter().cloned())
            .collect();
        let labels = std::iter::once(fl!("so-credential-none"))
            .chain(sets.iter().map(|(n, _)| n.clone()))
            .collect();
        let categories = segmented_button::Model::builder()
            .insert(|b| {
                b.text(fl!("so-connection"))
                    .data(Category::Connection)
                    .activate()
            })
            .insert(|b| b.text(fl!("so-ssh2")).data(Category::Ssh2))
            .insert(|b| b.text(fl!("so-log")).data(Category::Log))
            .build();
        let name = match &target {
            Target::Edit { id } => name_of(id).to_string(),
            _ => String::new(),
        };
        Self {
            window,
            form: Form::from_session(&name, s),
            target,
            categories,
            error: None,
            first_id: widget::Id::unique(),
            sets,
            labels,
            bars,
            bar_labels,
            protocols: vec![fl!("so-protocol-ssh2"), fl!("so-protocol-local")],
            saved: false,
        }
    }

    pub fn title(&self) -> String {
        match &self.target {
            Target::New { .. } => fl!("so-title-new"),
            Target::Edit { id } => fl!("so-title", name = name_of(id)),
            Target::Default => fl!("so-title-default"),
        }
    }

    /// Name for a new session, else Hostname.
    pub fn first_field(&self) -> widget::Id {
        self.first_id.clone()
    }

    pub fn name(&self) -> String {
        self.form.name.trim().to_string()
    }

    /// `base` with the fields put in, or why not (shown in the window).
    pub fn checked(&mut self, base: &Session) -> Option<Session> {
        let r = self.form.apply(base, self.target != Target::Default);
        self.error = r.as_ref().err().map(|e| match e {
            FormError::Host => fl!("qc-host-required"),
            FormError::Port => fl!("qc-bad-port"),
            FormError::Keepalive => fl!("so-bad-keepalive"),
            FormError::Extra => fl!("bad-extra"),
        });
        r.ok()
    }

    pub fn update(&mut self, msg: SoMsg) {
        let f = &mut self.form;
        match msg {
            SoMsg::Field(field, v) => match field {
                Field::Name => f.name = v,
                Field::Host => f.host = v,
                Field::Port => f.port = v,
                Field::User => f.user = v,
                Field::Identity => f.identity = v,
                Field::Keepalive => f.keepalive = v,
                Field::Extra => f.extra = v,
                Field::LogFile => f.log_file = v,
            },
            SoMsg::Category(e) => self.categories.activate(e),
            SoMsg::Protocol(i) => f.protocol = Protocol::ALL.get(i).copied().unwrap_or_default(),
            SoMsg::Saved(w, saved) if w == self.window => self.saved = saved,
            SoMsg::LogAppend(on) => f.log_append = on,
            SoMsg::LogRaw(on) => f.log_raw = on,
            SoMsg::Bar(i) => {
                f.button_bar = match i {
                    0 => String::new(),
                    i => self.bars.get(i - 1).cloned().unwrap_or_default(),
                }
            }
            SoMsg::Credential(i) => {
                f.credential = match i {
                    0 => String::new(),
                    i => self
                        .sets
                        .get(i - 1)
                        .map(|(n, _)| n.clone())
                        .unwrap_or_default(),
                }
            }
            SoMsg::Browsed(w, Some(path)) if w == self.window => f.identity = path,
            _ => {}
        }
    }

    pub fn view(&self, focused: bool) -> Element<'_, Message> {
        let m = Message::SessionOptions;
        let category = self.categories.active_data::<Category>().copied();
        let field = |label: String, f: Field, value: &str, first: bool| -> Element<'_, Message> {
            let mut input = widget::text_input("", value.to_string())
                .on_input(move |v| m(SoMsg::Field(f, v)))
                .on_submit(move |_| m(SoMsg::Ok));
            if first {
                input = input.id(self.first_id.clone());
            }
            widget::column::with_children(vec![widget::text::caption(label).into(), input.into()])
                .spacing(2)
                .into()
        };
        let f = &self.form;
        let local = f.protocol == Protocol::Local;
        let fields: Vec<Element<'_, Message>> = match category {
            Some(Category::Log) => vec![
                field(fl!("so-log-file"), Field::LogFile, &f.log_file, false),
                widget::text::caption(fl!("so-log-hint")).into(),
                widget::checkbox(f.log_append)
                    .label(fl!("so-log-append"))
                    .on_toggle(move |v| m(SoMsg::LogAppend(v)))
                    .into(),
                widget::checkbox(f.log_raw)
                    .label(fl!("so-log-raw"))
                    .on_toggle(move |v| m(SoMsg::LogRaw(v)))
                    .into(),
            ],
            Some(Category::Ssh2) if local => vec![widget::text::body(fl!("so-ssh-only")).into()],
            Some(Category::Ssh2) => vec![
                widget::column::with_children(vec![
                    widget::text::caption(fl!("so-identity")).into(),
                    widget::row::with_children(vec![
                        widget::text_input(fl!("so-identity-agent"), f.identity.clone())
                            .on_input(move |v| m(SoMsg::Field(Field::Identity, v)))
                            .on_submit(move |_| m(SoMsg::Ok))
                            .into(),
                        widget::button::standard(fl!("so-browse"))
                            .on_press(m(SoMsg::Browse))
                            .into(),
                    ])
                    .spacing(8)
                    .into(),
                ])
                .spacing(2)
                .into(),
                field(fl!("so-keepalive"), Field::Keepalive, &f.keepalive, false),
                field(fl!("so-extra"), Field::Extra, &f.extra, false),
            ],
            _ => {
                let mut v = Vec::new();
                let new = matches!(self.target, Target::New { .. });
                if self.target != Target::Default {
                    v.push(field(fl!("so-name"), Field::Name, &f.name, new));
                }
                let at = Protocol::ALL.iter().position(|p| *p == f.protocol);
                v.push(
                    widget::column::with_children(vec![
                        widget::text::caption(fl!("so-protocol")).into(),
                        widget::dropdown(&self.protocols, at, move |i| m(SoMsg::Protocol(i)))
                            .into(),
                    ])
                    .spacing(2)
                    .into(),
                );
                if local {
                    return self.frame(v, focused);
                }
                v.extend([
                    field(fl!("qc-host"), Field::Host, &f.host, !new),
                    field(fl!("qc-port"), Field::Port, &f.port, false),
                ]);
                // With a set: its user, not editable here (Credentials Manager).
                let set = self.sets.iter().position(|(n, _)| *n == f.credential);
                match set {
                    Some(i) => v.push(
                        widget::column::with_children(vec![
                            widget::text::caption(fl!("qc-user")).into(),
                            widget::text_input("", self.sets[i].1.clone()).into(),
                        ])
                        .spacing(2)
                        .into(),
                    ),
                    None => v.push(field(fl!("qc-user"), Field::User, &f.user, false)),
                }
                let selected = set.map_or(0, |i| i + 1);
                v.push(
                    widget::column::with_children(vec![
                        widget::text::caption(fl!("so-credential")).into(),
                        widget::dropdown(&self.labels, Some(selected), move |i| {
                            m(SoMsg::Credential(i))
                        })
                        .into(),
                    ])
                    .spacing(2)
                    .into(),
                );
                let at = self.bars.iter().position(|b| *b == f.button_bar);
                v.push(
                    widget::column::with_children(vec![
                        widget::text::caption(fl!("so-bar")).into(),
                        widget::dropdown(
                            &self.bar_labels,
                            Some(at.map_or(0, |i| i + 1)),
                            move |i| m(SoMsg::Bar(i)),
                        )
                        .into(),
                    ])
                    .spacing(2)
                    .into(),
                );
                if self.saved {
                    v.push(
                        widget::row::with_children(vec![
                            widget::text::body(fl!("so-saved")).into(),
                            widget::button::standard(fl!("so-forget"))
                                .on_press(m(SoMsg::Forget))
                                .into(),
                        ])
                        .spacing(12)
                        .align_y(cosmic::iced::Alignment::Center)
                        .into(),
                    );
                }
                v
            }
        };
        self.frame(fields, focused)
    }

    /// The window around a page's fields: header, categories, [Cancel] [OK].
    fn frame<'a>(
        &'a self,
        fields: Vec<Element<'a, Message>>,
        focused: bool,
    ) -> Element<'a, Message> {
        let m = Message::SessionOptions;
        let nav = segmented_button::vertical(&self.categories)
            .on_activate(move |e| m(SoMsg::Category(e)))
            .width(Length::Fixed(180.0));
        let mut bottom = widget::row::with_capacity(4).spacing(8).padding(16);
        if let Some(e) = &self.error {
            bottom = bottom.push(widget::text::body(e.clone()).class(theme::Text::Accent));
        }
        let bottom = bottom
            .push(widget::Space::new().width(Length::Fill))
            .push(widget::button::standard(fl!("cancel")).on_press(m(SoMsg::Cancel)))
            .push(widget::button::suggested(fl!("ok")).on_press(m(SoMsg::Ok)));
        let content = widget::column::with_children(vec![
            widget::header_bar()
                .title(self.title())
                .on_close(m(SoMsg::Cancel))
                .on_drag(m(SoMsg::Drag))
                .focused(focused)
                .into(),
            widget::row::with_children(vec![
                nav.into(),
                widget::column::with_children(fields)
                    .spacing(12)
                    .width(Length::Fill)
                    .into(),
            ])
            .spacing(16)
            .padding([8, 16])
            .height(Length::Fill)
            .into(),
            bottom.into(),
        ]);
        // A window of our own gets no background from libcosmic: without one it is see-through.
        widget::container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .class(theme::Container::WindowBackground)
            .into()
    }
}

/// The window to open: SecureCRT's Session Options size.
pub fn settings() -> window::Settings {
    window::Settings {
        size: cosmic::iced::Size::new(760.0, 540.0),
        min_size: Some(cosmic::iced::Size::new(520.0, 360.0)),
        decorations: false,
        exit_on_close_request: false,
        ..Default::default()
    }
}

/// «Обзор…»: a key file from the portal's chooser, `~/…` when in the home folder.
pub async fn browse(title: String) -> Option<String> {
    use cosmic::dialog::file_chooser::open::Dialog;
    let picked = Dialog::new().title(title).open_file().await.ok()?;
    let path = picked.url().to_file_path().ok()?;
    let home = std::env::home_dir().unwrap_or_default();
    Some(match path.strip_prefix(&home) {
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    })
}
