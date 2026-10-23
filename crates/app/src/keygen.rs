//! Tools → Create Public Key… and Upload Public Key…: the dialogs. The work is
//! `shagshell_core::keygen`; the passphrase is a `Hidden` (never in `Debug` output).

use crate::app::Message;
use crate::askpass::Hidden;
use crate::fl;
use cosmic::Element;
use cosmic::widget;
use shagshell_core::keygen::{self, Error, Generated, Kind, Spec};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub enum KgMsg {
    Kind(usize),
    Size(usize),
    File(String),
    Comment(String),
    Pass(Hidden),
    Pass2(Hidden),
    Create,
    Done(Result<Generated, Error>),
    Copy,
    /// The «Upload» dialog's key.
    Key(usize),
    Upload,
    Close,
}

pub struct KeyGen {
    kind: usize,
    size: usize,
    file: String,
    comment: String,
    pass: Hidden,
    pass2: Hidden,
    error: Option<String>,
    busy: bool,
    pub result: Option<Generated>,
    sizes: Vec<String>,
    kinds: Vec<String>,
    first: widget::Id,
}

fn field<'a>(label: String, w: Element<'a, Message>) -> Element<'a, Message> {
    widget::column::with_children(vec![widget::text::caption(label).into(), w])
        .spacing(2)
        .into()
}

fn sizes_of(k: Kind) -> Vec<String> {
    (k.sizes().iter())
        .map(|b| fl!("kg-bits", bits = i64::from(*b)))
        .collect()
}

fn ssh_dir() -> PathBuf {
    std::env::home_dir().unwrap_or_default().join(".ssh")
}

impl KeyGen {
    pub fn new() -> Self {
        Self {
            kind: 0,
            size: 0,
            file: keygen::default_file(Kind::Ed25519, &ssh_dir())
                .display()
                .to_string(),
            comment: keygen::default_comment(),
            pass: Hidden(String::new()),
            pass2: Hidden(String::new()),
            error: None,
            busy: false,
            result: None,
            sizes: sizes_of(Kind::Ed25519),
            kinds: Kind::ALL.iter().map(|k| k.name().to_uppercase()).collect(),
            first: widget::Id::unique(),
        }
    }

    pub fn focus(&self) -> cosmic::app::Task<Message> {
        widget::text_input::focus(self.first.clone())
    }

    fn kind(&self) -> Kind {
        Kind::ALL[self.kind.min(Kind::ALL.len() - 1)]
    }

    /// `Some`: make this key now (off the UI thread), the result comes as `Done`.
    pub fn update(&mut self, msg: KgMsg) -> Option<Spec> {
        self.error = None;
        match msg {
            KgMsg::Kind(i) => {
                // The default file follows the type while it is still the default one.
                let old = keygen::default_file(self.kind(), &ssh_dir())
                    .display()
                    .to_string();
                self.kind = i;
                self.size = 0;
                self.sizes = sizes_of(self.kind());
                if self.file == old {
                    self.file = keygen::default_file(self.kind(), &ssh_dir())
                        .display()
                        .to_string();
                }
            }
            KgMsg::Size(i) => self.size = i,
            KgMsg::File(v) => self.file = v,
            KgMsg::Comment(v) => self.comment = v,
            KgMsg::Pass(v) => self.pass = v,
            KgMsg::Pass2(v) => self.pass2 = v,
            KgMsg::Create if self.busy || self.result.is_some() => {}
            KgMsg::Create => {
                if self.pass.0 != self.pass2.0 {
                    self.error = Some(fl!("kg-pass-differ"));
                    return None;
                }
                self.busy = true;
                let file = match self.file.trim().strip_prefix("~/") {
                    Some(rest) => std::env::home_dir().unwrap_or_default().join(rest),
                    None => PathBuf::from(self.file.trim()),
                };
                return Some(Spec {
                    kind: self.kind(),
                    size: self.size,
                    file,
                    comment: self.comment.trim().to_string(),
                    passphrase: self.pass.0.clone(),
                });
            }
            KgMsg::Done(r) => {
                self.busy = false;
                match r {
                    Ok(g) => {
                        // The phrase is not needed any more.
                        self.pass = Hidden(String::new());
                        self.pass2 = Hidden(String::new());
                        self.result = Some(g);
                    }
                    Err(Error::Exists(p)) => {
                        self.error = Some(fl!("kg-exists", file = p.display().to_string()))
                    }
                    Err(Error::NoFile) => self.error = Some(fl!("kg-no-file")),
                    Err(Error::Failed(e)) => self.error = Some(e),
                }
            }
            KgMsg::Copy | KgMsg::Key(_) | KgMsg::Upload | KgMsg::Close => {}
        }
        None
    }

    pub fn view(&self) -> Element<'_, Message> {
        let m = Message::KeyGen;
        if let Some(g) = &self.result {
            let body = widget::column::with_children(vec![
                widget::text::body(fl!("kg-done", file = self.file.clone())).into(),
                widget::text::caption(g.fingerprint.clone()).into(),
                widget::text_input("", g.public.clone()).into(),
                widget::text::caption(fl!("kg-done-hint")).into(),
            ])
            .spacing(10);
            return widget::dialog()
                .title(fl!("kg-title"))
                .control(body)
                .primary_action(widget::button::suggested(fl!("kg-copy")).on_press(m(KgMsg::Copy)))
                .secondary_action(
                    widget::button::standard(fl!("sftp-close")).on_press(m(KgMsg::Close)),
                )
                .into();
        }
        let secret = |v: &Hidden, f: fn(Hidden) -> KgMsg| {
            widget::secure_input("", v.0.clone(), None, true)
                .on_input(move |t| Message::KeyGen(f(Hidden(t))))
                .on_submit(|_| Message::KeyGen(KgMsg::Create))
        };
        let mut types = widget::row::with_capacity(2).spacing(8).push(field(
            fl!("kg-type"),
            widget::dropdown(&self.kinds, Some(self.kind), move |i| m(KgMsg::Kind(i))).into(),
        ));
        if !self.sizes.is_empty() {
            types = types.push(field(
                fl!("kg-size"),
                widget::dropdown(&self.sizes, Some(self.size), move |i| m(KgMsg::Size(i))).into(),
            ));
        }
        let mut body = widget::column::with_capacity(8)
            .spacing(10)
            .push(types)
            .push(field(
                fl!("kg-file"),
                widget::text_input("", self.file.clone())
                    .id(self.first.clone())
                    .on_input(|v| Message::KeyGen(KgMsg::File(v)))
                    .on_submit(|_| Message::KeyGen(KgMsg::Create))
                    .into(),
            ))
            .push(field(
                fl!("kg-comment"),
                widget::text_input("", self.comment.clone())
                    .on_input(|v| Message::KeyGen(KgMsg::Comment(v)))
                    .on_submit(|_| Message::KeyGen(KgMsg::Create))
                    .into(),
            ))
            .push(field(
                fl!("kg-pass"),
                secret(&self.pass, KgMsg::Pass).into(),
            ))
            .push(field(
                fl!("kg-pass2"),
                secret(&self.pass2, KgMsg::Pass2).into(),
            ));
        if self.pass.0.is_empty() {
            body = body.push(widget::text::caption(fl!("kg-no-pass")));
        }
        if let Some(e) = &self.error {
            body = body.push(widget::text::body(e.clone()).class(cosmic::theme::Text::Accent));
        }
        widget::dialog()
            .title(fl!("kg-title"))
            .control(body)
            .primary_action(
                widget::button::suggested(fl!("kg-create"))
                    .on_press_maybe((!self.busy).then(|| m(KgMsg::Create))),
            )
            .secondary_action(widget::button::standard(fl!("cancel")).on_press(m(KgMsg::Close)))
            .into()
    }
}

/// Tools → Upload Public Key…: which key goes to the session.
pub struct Upload {
    pub session: String,
    keys: Vec<PathBuf>,
    names: Vec<String>,
    pub key: usize,
}

impl Upload {
    /// `None`: no key in `~/.ssh` yet.
    pub fn new(session: String) -> Option<Self> {
        let keys = keygen::public_keys(&ssh_dir());
        if keys.is_empty() {
            return None;
        }
        let names = keys
            .iter()
            .filter_map(|k| k.file_name().map(|n| n.to_string_lossy().into_owned()))
            .collect();
        Some(Self {
            session,
            keys,
            names,
            key: 0,
        })
    }

    pub fn chosen(&self) -> Option<&PathBuf> {
        self.keys.get(self.key)
    }

    pub fn view(&self) -> Element<'_, Message> {
        let m = Message::KeyGen;
        let body = widget::column::with_children(vec![
            widget::text::body(fl!("ku-session", name = self.session.clone())).into(),
            widget::column::with_children(vec![
                widget::text::caption(fl!("ku-key")).into(),
                widget::dropdown(&self.names, Some(self.key), move |i| m(KgMsg::Key(i))).into(),
            ])
            .spacing(2)
            .into(),
            widget::text::caption(fl!("ku-hint")).into(),
        ])
        .spacing(10);
        widget::dialog()
            .title(fl!("ku-title"))
            .control(body)
            .primary_action(widget::button::suggested(fl!("ku-run")).on_press(m(KgMsg::Upload)))
            .secondary_action(widget::button::standard(fl!("cancel")).on_press(m(KgMsg::Close)))
            .into()
    }
}
