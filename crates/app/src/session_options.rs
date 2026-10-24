//! Session Options, as SecureCRT's: a window of its own, categories on the left, fields on the
//! right, [Cancel] [OK]. The app saves (it owns the store); this module only edits the fields.

use crate::app::Message;
use crate::fl;
use cosmic::iced::{Length, window};
use cosmic::widget::{self, segmented_button};
use cosmic::{Element, theme};
use shagshell_core::session::Session;
use shagshell_core::session::form::{Form, FormError};
use shagshell_core::session::store::name_of;

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
}

#[derive(Clone, Debug)]
pub enum SoMsg {
    Field(Field, String),
    Category(segmented_button::Entity),
    Browse,
    /// The file chooser's pick (`None`: cancelled), for the window that asked.
    Browsed(window::Id, Option<String>),
    Drag,
    /// The keyring looked up for this window's session: a password or passphrase is saved.
    Saved(window::Id, bool),
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
    first_id: widget::Id,
}

impl SessionOptions {
    pub fn new(window: window::Id, target: Target, s: &Session) -> Self {
        let categories = segmented_button::Model::builder()
            .insert(|b| {
                b.text(fl!("so-connection"))
                    .data(Category::Connection)
                    .activate()
            })
            .insert(|b| b.text(fl!("so-ssh2")).data(Category::Ssh2))
            .build();
        Self {
            window,
            target,
            form: Form::from_session("", s),
            categories,
            error: None,
            first_id: widget::Id::unique(),
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
            },
            SoMsg::Category(e) => self.categories.activate(e),
            SoMsg::Saved(w, saved) if w == self.window => self.saved = saved,
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
        let fields: Vec<Element<'_, Message>> = match category {
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
                if new {
                    v.push(field(fl!("so-name"), Field::Name, &f.name, true));
                }
                v.extend([
                    field(fl!("qc-host"), Field::Host, &f.host, !new),
                    field(fl!("qc-port"), Field::Port, &f.port, false),
                    field(fl!("qc-user"), Field::User, &f.user, false),
                ]);
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
