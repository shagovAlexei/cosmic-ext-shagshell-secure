//! Session Options, as SecureCRT's: a window of its own, categories on the left, fields on the
//! right, [Cancel] [OK]. The app saves (it owns the store); this module only edits the fields.

use crate::app::Message;
use crate::askpass::Hidden;
use crate::fl;
use cosmic::iced::{Alignment, Length, window};
use cosmic::widget::{self, segmented_button};
use cosmic::{Element, theme};
use shagshell_core::session::ForwardKind;
use shagshell_core::session::form::{Form, FormError, FwdRow};
use shagshell_core::session::store::name_of;
use shagshell_core::session::{Flow, Parity, Protocol, Session};

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
    Forward,
    Logon,
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
    Jump,
    ReconnectDelay,
    ReconnectTries,
    Device,
    Baud,
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
    Reconnect(bool),
    /// Port Forwarding: a row's kind (n-th of `ForwardKind::ALL`), a text (0 listen, 1 host,
    /// 2 port), a row added / removed.
    FwdKind(usize, usize),
    FwdText(usize, u8, String),
    FwdAdd,
    FwdDel(usize),
    /// Serial line: data bits (n-th of 5…8), parity, stop bits (n-th of 1, 2), flow control.
    SerialBits(usize),
    SerialParity(usize),
    SerialStops(usize),
    SerialFlow(usize),
    /// Logon Actions: a row's text (0 expect, 1 send), a row added / removed, the password typed.
    LogonText(usize, u8, String),
    LogonAdd,
    LogonDel(usize),
    Password(Hidden),
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
    /// A password typed for Logon Actions (empty: the saved one is left as it is); it goes to
    /// the keyring on OK and is never in the form or a file.
    password: Hidden,
    /// The session has a secret in the keyring (Edit only).
    saved: bool,
    /// The credential sets (name, user), for the dropdown; `labels[0]` is «(none)».
    sets: Vec<(String, String)>,
    labels: Vec<String>,
    /// The Button Bars (names), for the dropdown; `bar_labels[0]` is «(none)».
    bars: Vec<String>,
    bar_labels: Vec<String>,
    protocols: Vec<String>,
    fwd_kinds: Vec<String>,
    parities: Vec<String>,
    flows: Vec<String>,
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
            .insert(|b| b.text(fl!("so-forward")).data(Category::Forward))
            .insert(|b| b.text(fl!("so-logon")).data(Category::Logon))
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
            password: Hidden(String::new()),
            first_id: widget::Id::unique(),
            sets,
            labels,
            bars,
            bar_labels,
            protocols: vec![
                fl!("so-protocol-ssh2"),
                fl!("so-protocol-local"),
                fl!("so-protocol-telnet"),
                fl!("so-protocol-serial"),
            ],
            parities: vec![
                fl!("so-parity-none"),
                fl!("so-parity-even"),
                fl!("so-parity-odd"),
            ],
            flows: vec![
                fl!("so-flow-none"),
                fl!("so-flow-hardware"),
                fl!("so-flow-software"),
            ],
            fwd_kinds: vec![
                fl!("so-fwd-local"),
                fl!("so-fwd-remote"),
                fl!("so-fwd-dynamic"),
            ],
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

    /// The password typed, if any.
    pub fn password(&self) -> Option<String> {
        Some(self.password.0.clone()).filter(|p| !p.is_empty())
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
            FormError::Jump => fl!("so-bad-jump"),
            FormError::Device => fl!("so-bad-device"),
            FormError::Baud => fl!("so-bad-baud"),
            FormError::Forward(n) => {
                let n = (*n + 1) as i64;
                fl!("so-bad-forward", n = n)
            }
            FormError::Reconnect => fl!("so-bad-reconnect"),
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
                Field::Jump => f.jump = v,
                Field::Device => f.device = v,
                Field::Baud => f.baud = v,
                Field::ReconnectDelay => f.reconnect_delay = v,
                Field::ReconnectTries => f.reconnect_tries = v,
            },
            SoMsg::Category(e) => self.categories.activate(e),
            SoMsg::Protocol(i) => f.protocol = Protocol::ALL.get(i).copied().unwrap_or_default(),
            SoMsg::Saved(w, saved) if w == self.window => self.saved = saved,
            SoMsg::SerialBits(i) => f.data_bits = 5 + i as u8,
            SoMsg::SerialParity(i) => f.parity = Parity::ALL.get(i).copied().unwrap_or_default(),
            SoMsg::SerialStops(i) => f.stop_bits = 1 + i as u8,
            SoMsg::SerialFlow(i) => f.flow = Flow::ALL.get(i).copied().unwrap_or_default(),
            SoMsg::Password(p) => self.password = p,
            SoMsg::LogonAdd => f.logon.push(Default::default()),
            SoMsg::LogonDel(i) => {
                if i < f.logon.len() {
                    f.logon.remove(i);
                }
            }
            SoMsg::LogonText(i, col, v) => {
                if let Some(r) = f.logon.get_mut(i) {
                    match col {
                        0 => r.expect = v,
                        _ => r.send = v,
                    }
                }
            }
            SoMsg::Reconnect(on) => f.reconnect = on,
            SoMsg::FwdAdd => f.forwards.push(FwdRow::default()),
            SoMsg::FwdDel(i) => {
                if i < f.forwards.len() {
                    f.forwards.remove(i);
                }
            }
            SoMsg::FwdKind(i, k) => {
                if let Some(r) = f.forwards.get_mut(i) {
                    r.kind = ForwardKind::ALL.get(k).copied().unwrap_or_default();
                }
            }
            SoMsg::FwdText(i, col, v) => {
                if let Some(r) = f.forwards.get_mut(i) {
                    match col {
                        0 => r.listen = v,
                        1 => r.host = v,
                        _ => r.port = v,
                    }
                }
            }
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
        let ssh = f.protocol == Protocol::Ssh2;
        let fields: Vec<Element<'_, Message>> = match category {
            Some(Category::Ssh2 | Category::Forward) if !ssh => {
                vec![widget::text::body(fl!("so-ssh-only")).into()]
            }
            Some(Category::Forward) => {
                let kinds = &self.fwd_kinds;
                let mut rows: Vec<Element<'_, Message>> =
                    vec![widget::text::caption(fl!("so-forward-hint")).into()];
                for (i, r) in f.forwards.iter().enumerate() {
                    let at = ForwardKind::ALL.iter().position(|k| *k == r.kind);
                    let text = |v: &str, col: u8, w: f32, ph: String| {
                        widget::text_input(ph, v.to_string())
                            .on_input(move |t| m(SoMsg::FwdText(i, col, t)))
                            .width(Length::Fixed(w))
                    };
                    let dynamic = r.kind == ForwardKind::Dynamic;
                    let mut line = widget::row::with_capacity(6)
                        .spacing(6)
                        .align_y(Alignment::Center)
                        .push(widget::dropdown(kinds, at, move |k| {
                            m(SoMsg::FwdKind(i, k))
                        }))
                        .push(text(&r.listen, 0, 105.0, fl!("so-fwd-listen")));
                    if !dynamic {
                        line = line
                            .push(widget::text::body("→"))
                            .push(text(&r.host, 1, 105.0, fl!("so-fwd-host")))
                            .push(text(&r.port, 2, 60.0, fl!("so-fwd-port")));
                    }
                    rows.push(
                        line.push(
                            widget::button::icon(widget::icon::from_name("edit-delete-symbolic"))
                                .on_press(m(SoMsg::FwdDel(i))),
                        )
                        .into(),
                    );
                }
                rows.push(
                    widget::button::standard(fl!("so-fwd-add"))
                        .on_press(m(SoMsg::FwdAdd))
                        .into(),
                );
                rows
            }
            Some(Category::Logon) => {
                let mut rows: Vec<Element<'_, Message>> =
                    vec![widget::text::caption(fl!("so-logon-hint")).into()];
                for (i, r) in f.logon.iter().enumerate() {
                    let text = |v: &str, col: u8, ph: String| {
                        widget::text_input(ph, v.to_string())
                            .on_input(move |t| m(SoMsg::LogonText(i, col, t)))
                            .width(Length::Fill)
                    };
                    rows.push(
                        widget::row::with_children(vec![
                            text(&r.expect, 0, fl!("so-logon-expect")).into(),
                            widget::text::body("→").into(),
                            text(&r.send, 1, fl!("so-logon-send")).into(),
                            widget::button::icon(widget::icon::from_name("edit-delete-symbolic"))
                                .on_press(m(SoMsg::LogonDel(i)))
                                .into(),
                        ])
                        .spacing(6)
                        .align_y(Alignment::Center)
                        .into(),
                    );
                }
                rows.push(
                    widget::button::standard(fl!("so-logon-add"))
                        .on_press(m(SoMsg::LogonAdd))
                        .into(),
                );
                rows.push(labelled(
                    fl!("so-logon-password"),
                    widget::secure_input("", self.password.0.clone(), None, true)
                        .on_input(move |t| m(SoMsg::Password(Hidden(t))))
                        .into(),
                ));
                rows
            }
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
                field(fl!("so-jump"), Field::Jump, &f.jump, false),
                widget::text::caption(fl!("so-jump-hint")).into(),
                field(fl!("so-extra"), Field::Extra, &f.extra, false),
                widget::checkbox(f.reconnect)
                    .label(fl!("so-reconnect"))
                    .on_toggle(move |v| m(SoMsg::Reconnect(v)))
                    .into(),
                field(
                    fl!("so-reconnect-delay"),
                    Field::ReconnectDelay,
                    &f.reconnect_delay,
                    false,
                ),
                field(
                    fl!("so-reconnect-tries"),
                    Field::ReconnectTries,
                    &f.reconnect_tries,
                    false,
                ),
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
                match f.protocol {
                    Protocol::Local => return self.frame(v, focused),
                    Protocol::Serial => {
                        let bits = [5, 6, 7, 8].map(|b| b.to_string()).to_vec();
                        v.push(field(fl!("so-device"), Field::Device, &f.device, !new));
                        v.push(field(fl!("so-baud"), Field::Baud, &f.baud, false));
                        v.push(labelled(
                            fl!("so-databits"),
                            widget::dropdown(
                                bits,
                                Some(usize::from(f.data_bits.clamp(5, 8) - 5)),
                                move |i| m(SoMsg::SerialBits(i)),
                            )
                            .into(),
                        ));
                        v.push(labelled(
                            fl!("so-parity"),
                            widget::dropdown(
                                &self.parities,
                                Parity::ALL.iter().position(|p| *p == f.parity),
                                move |i| m(SoMsg::SerialParity(i)),
                            )
                            .into(),
                        ));
                        v.push(labelled(
                            fl!("so-stopbits"),
                            widget::dropdown(
                                vec!["1".to_string(), "2".to_string()],
                                Some(usize::from(f.stop_bits.clamp(1, 2) - 1)),
                                move |i| m(SoMsg::SerialStops(i)),
                            )
                            .into(),
                        ));
                        v.push(labelled(
                            fl!("so-flow"),
                            widget::dropdown(
                                &self.flows,
                                Flow::ALL.iter().position(|x| *x == f.flow),
                                move |i| m(SoMsg::SerialFlow(i)),
                            )
                            .into(),
                        ));
                        return self.frame(v, focused);
                    }
                    Protocol::Telnet => {
                        v.extend([
                            field(fl!("qc-host"), Field::Host, &f.host, !new),
                            field(fl!("qc-port"), Field::Port, &f.port, false),
                        ]);
                        return self.frame(v, focused);
                    }
                    Protocol::Ssh2 => {}
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

/// A label over a control.
fn labelled<'a>(label: String, body: Element<'a, Message>) -> Element<'a, Message> {
    widget::column::with_children(vec![widget::text::caption(label).into(), body])
        .spacing(2)
        .into()
}
