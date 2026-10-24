//! Quick Connect (Alt+Q), as SecureCRT's: one-off host / port / user, optionally saved.

use crate::app::Message;
use crate::fl;
use cosmic::Element;
use cosmic::widget;
use shagshell_core::session::Session;

#[derive(Clone, Debug)]
pub struct QuickConnect {
    pub host: String,
    pub port: String,
    pub user: String,
    pub save: bool,
    pub name: String,
    /// Where a saved session goes ("" = root).
    pub folder: String,
    pub error: Option<String>,
    host_id: widget::Id,
}

#[derive(Clone, Debug)]
pub enum QcMsg {
    Host(String),
    Port(String),
    User(String),
    Save(bool),
    Name(String),
    Connect,
    Cancel,
}

/// What Connect asks for: the session, and the name to save it under.
pub struct Request {
    pub session: Session,
    pub save_as: Option<(String, String)>,
}

impl QuickConnect {
    /// `folder`: «New session…» opens it with «Save» on, into that folder.
    pub fn new(folder: Option<String>) -> Self {
        Self {
            host: String::new(),
            port: String::new(),
            user: String::new(),
            save: folder.is_some(),
            name: String::new(),
            folder: folder.unwrap_or_default(),
            error: None,
            host_id: widget::Id::unique(),
        }
    }

    pub fn focus(&self) -> cosmic::app::Task<Message> {
        widget::text_input::focus(self.host_id.clone())
    }

    /// `None`: stays open (cancel closes it in the app).
    pub fn update(&mut self, msg: QcMsg) -> Option<Request> {
        self.error = None;
        match msg {
            QcMsg::Host(h) => {
                if self.name.is_empty() || self.name == self.host {
                    self.name = h.clone();
                }
                self.host = h;
            }
            QcMsg::Port(p) => self.port = p,
            QcMsg::User(u) => self.user = u,
            QcMsg::Save(s) => self.save = s,
            QcMsg::Name(n) => self.name = n,
            QcMsg::Cancel => {}
            QcMsg::Connect => match self.request() {
                Ok(r) => return Some(r),
                Err(e) => self.error = Some(e),
            },
        }
        None
    }

    fn request(&self) -> Result<Request, String> {
        let host = self.host.trim();
        if host.is_empty() || host.contains(char::is_whitespace) || host.starts_with('-') {
            return Err(fl!("qc-host-required"));
        }
        let port = match self.port.trim() {
            "" => None,
            p => Some(
                p.parse::<u16>()
                    .ok()
                    .filter(|&p| p > 0)
                    .ok_or_else(|| fl!("qc-bad-port"))?,
            ),
        };
        let user = Some(self.user.trim().to_string()).filter(|u| !u.is_empty());
        let session = Session {
            host: host.to_string(),
            port,
            user,
            ..Default::default()
        };
        let save_as = self
            .save
            .then(|| (self.folder.clone(), self.name.trim().to_string()));
        Ok(Request { session, save_as })
    }

    pub fn view(&self) -> Element<'_, Message> {
        let m = |x| Message::QuickConnect(x);
        let field = |label: String, w: Element<'static, Message>| -> Element<'static, Message> {
            widget::column::with_children(vec![widget::text::caption(label).into(), w])
                .spacing(2)
                .into()
        };
        let host = widget::text_input("", self.host.clone())
            .id(self.host_id.clone())
            .on_input(|v| Message::QuickConnect(QcMsg::Host(v)))
            .on_submit(|_| Message::QuickConnect(QcMsg::Connect));
        let port = widget::text_input("22", self.port.clone())
            .on_input(|v| Message::QuickConnect(QcMsg::Port(v)))
            .on_submit(|_| Message::QuickConnect(QcMsg::Connect))
            .width(90);
        let user = widget::text_input("", self.user.clone())
            .on_input(|v| Message::QuickConnect(QcMsg::User(v)))
            .on_submit(|_| Message::QuickConnect(QcMsg::Connect));
        let mut body = widget::column::with_children(vec![
            widget::text::body(fl!("qc-protocol")).into(),
            widget::row::with_children(vec![
                field(fl!("qc-host"), host.into()),
                field(fl!("qc-port"), port.into()),
            ])
            .spacing(8)
            .into(),
            field(fl!("qc-user"), user.into()),
            widget::checkbox(self.save)
                .label(fl!("qc-save"))
                .on_toggle(|v| Message::QuickConnect(QcMsg::Save(v)))
                .into(),
        ])
        .spacing(10);
        if self.save {
            let folder = match self.folder.as_str() {
                "" => fl!("qc-root"),
                f => f.to_string(),
            };
            let name = widget::text_input("", self.name.clone())
                .on_input(|v| Message::QuickConnect(QcMsg::Name(v)))
                .on_submit(|_| Message::QuickConnect(QcMsg::Connect));
            body = body.push(field(fl!("qc-name", folder = folder), name.into()));
        }
        if let Some(e) = &self.error {
            body = body.push(widget::text::caption(e.clone()).class(cosmic::theme::Text::Accent));
        }
        widget::dialog()
            .title(fl!("qc-title"))
            .control(body)
            .primary_action(
                widget::button::suggested(fl!("qc-connect")).on_press(m(QcMsg::Connect)),
            )
            .secondary_action(widget::button::standard(fl!("cancel")).on_press(m(QcMsg::Cancel)))
            .into()
    }
}
