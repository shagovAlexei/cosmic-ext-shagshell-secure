//! Tools → Credentials Manager, a window of its own (SecureCRT's): the sets on the left, the
//! selected one's name / user / password on the right. The app saves (it owns the store and the
//! keyring calls); this module only edits the fields.

use crate::app::Message;
use crate::askpass::Hidden;
use crate::fl;
use cosmic::iced::{Length, window};
use cosmic::widget::{self, segmented_button};
use cosmic::{Element, theme};
use shagshell_core::credentials::Credentials;

/// A save of the selected set (or a new one), as typed.
#[derive(Clone, Debug)]
pub struct Edit {
    pub old: Option<String>,
    pub name: String,
    pub user: String,
    pub password: Hidden,
}

#[derive(Clone, Debug)]
pub enum CwMsg {
    Select(segmented_button::Entity),
    New,
    Name(String),
    User(String),
    Password(Hidden),
    Save,
    Delete,
    /// The keyring rename of a save came back (then the files are written).
    Renamed(Edit, Result<(), String>),
    /// The delete confirmed (after «used in N sessions»).
    DeleteConfirmed,
    DeleteCancelled,
    /// The keyring looked up for the selected set: a password is saved.
    HasPassword(window::Id, String, bool),
    Drag,
    Close,
}

pub struct CredWindow {
    pub window: window::Id,
    pub creds: Credentials,
    list: segmented_button::SingleSelectModel,
    /// The set being edited (`None`: a new one).
    pub selected: Option<String>,
    pub name: String,
    pub user: String,
    pub password: String,
    /// The selected set has a password saved (`None`: not known yet).
    has_password: Option<bool>,
    pub error: Option<String>,
    /// Delete asked: the sessions using the set.
    pub confirm: Option<usize>,
    name_id: widget::Id,
}

impl CredWindow {
    pub fn new(window: window::Id, creds: Credentials) -> Self {
        let mut w = Self {
            window,
            creds,
            list: segmented_button::Model::default(),
            selected: None,
            name: String::new(),
            user: String::new(),
            password: String::new(),
            has_password: None,
            error: None,
            confirm: None,
            name_id: widget::Id::unique(),
        };
        let first = w.creds.0.keys().next().cloned();
        w.reload(first.as_deref());
        w
    }

    pub fn name_field(&self) -> widget::Id {
        self.name_id.clone()
    }

    /// The list again (after a save), `select` chosen; returns it to look its password up.
    pub fn reload(&mut self, select: Option<&str>) -> Option<String> {
        self.list = segmented_button::Model::default();
        for name in self.creds.0.keys() {
            let e = self
                .list
                .insert()
                .text(name.clone())
                .data(name.clone())
                .id();
            if Some(name.as_str()) == select {
                self.list.activate(e);
            }
        }
        self.pick(select.map(str::to_string))
    }

    fn pick(&mut self, name: Option<String>) -> Option<String> {
        self.user = (name.as_ref())
            .and_then(|n| self.creds.0.get(n))
            .map(|s| s.user.clone())
            .unwrap_or_default();
        self.name = name.clone().unwrap_or_default();
        self.password.clear();
        self.has_password = None;
        self.error = None;
        self.confirm = None;
        self.selected = name.clone();
        name
    }

    /// Returns a set to look the password of up.
    pub fn update(&mut self, msg: CwMsg) -> Option<String> {
        match msg {
            CwMsg::Select(e) => {
                self.list.activate(e);
                let name = self.list.data::<String>(e).cloned();
                return self.pick(name);
            }
            CwMsg::New => {
                self.list.deactivate();
                self.pick(None);
            }
            CwMsg::Name(v) => self.name = v,
            CwMsg::User(v) => self.user = v,
            CwMsg::Password(Hidden(v)) => self.password = v,
            CwMsg::DeleteCancelled => self.confirm = None,
            CwMsg::HasPassword(w, name, has)
                if w == self.window && self.selected.as_ref() == Some(&name) =>
            {
                self.has_password = Some(has);
            }
            _ => {}
        }
        None
    }

    pub fn view(&self, focused: bool) -> Element<'_, Message> {
        let m = Message::Credentials;
        let field =
            |label: String, input: Element<'static, Message>| -> Element<'static, Message> {
                widget::column::with_children(vec![widget::text::caption(label).into(), input])
                    .spacing(2)
                    .into()
            };
        let name = widget::text_input("", self.name.clone())
            .id(self.name_id.clone())
            .on_input(move |v| m(CwMsg::Name(v)))
            .on_submit(move |_| m(CwMsg::Save));
        let user = widget::text_input("", self.user.clone())
            .on_input(move |v| m(CwMsg::User(v)))
            .on_submit(move |_| m(CwMsg::Save));
        let placeholder = match self.has_password {
            Some(true) => fl!("cw-password-saved"),
            Some(false) => fl!("cw-password-none"),
            None => String::new(),
        };
        let password = widget::text_input(placeholder, self.password.clone())
            .password()
            .on_input(move |v| m(CwMsg::Password(Hidden(v))))
            .on_submit(move |_| m(CwMsg::Save));
        let mut right = widget::column::with_children(vec![
            widget::text::title4(match &self.selected {
                Some(n) => n.clone(),
                None => fl!("cw-new-title"),
            })
            .into(),
            field(fl!("cw-name"), name.into()),
            field(fl!("qc-user"), user.into()),
            field(fl!("cw-password"), password.into()),
        ])
        .spacing(12)
        .width(Length::Fill);
        if let Some(e) = &self.error {
            right = right.push(widget::text::body(e.clone()).class(theme::Text::Accent));
        }
        let actions: Element<'_, Message> = match self.confirm {
            Some(n) => widget::column::with_children(vec![
                widget::text::body(fl!("cw-delete-confirm", count = n)).into(),
                widget::row::with_children(vec![
                    widget::Space::new().width(Length::Fill).into(),
                    widget::button::standard(fl!("cancel"))
                        .on_press(m(CwMsg::DeleteCancelled))
                        .into(),
                    widget::button::destructive(fl!("sm-delete"))
                        .on_press(m(CwMsg::DeleteConfirmed))
                        .into(),
                ])
                .spacing(8)
                .into(),
            ])
            .spacing(8)
            .into(),
            None => widget::row::with_children(vec![
                widget::Space::new().width(Length::Fill).into(),
                widget::button::destructive(fl!("sm-delete"))
                    .on_press_maybe(self.selected.as_ref().map(|_| m(CwMsg::Delete)))
                    .into(),
                widget::button::suggested(fl!("cw-save"))
                    .on_press(m(CwMsg::Save))
                    .into(),
            ])
            .spacing(8)
            .into(),
        };
        right = right.push(actions);
        let left = widget::column::with_children(vec![
            widget::scrollable(
                segmented_button::vertical(&self.list)
                    .on_activate(move |e| m(CwMsg::Select(e)))
                    .width(Length::Fixed(200.0)),
            )
            .height(Length::Fill)
            .into(),
            widget::button::standard(fl!("cw-add"))
                .leading_icon(widget::icon::from_name("list-add-symbolic"))
                .on_press(m(CwMsg::New))
                .into(),
        ])
        .spacing(8)
        .width(Length::Fixed(200.0));
        let content = widget::column::with_children(vec![
            widget::header_bar()
                .title(fl!("cw-title"))
                .on_close(m(CwMsg::Close))
                .on_drag(m(CwMsg::Drag))
                .focused(focused)
                .into(),
            widget::row::with_children(vec![left.into(), right.into()])
                .spacing(16)
                .padding(16)
                .height(Length::Fill)
                .into(),
        ]);
        // A window of our own gets no background from libcosmic: without one it is see-through.
        widget::container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .class(theme::Container::WindowBackground)
            .into()
    }
}

pub fn settings() -> window::Settings {
    window::Settings {
        size: cosmic::iced::Size::new(640.0, 420.0),
        min_size: Some(cosmic::iced::Size::new(480.0, 320.0)),
        decorations: false,
        exit_on_close_request: false,
        ..Default::default()
    }
}
