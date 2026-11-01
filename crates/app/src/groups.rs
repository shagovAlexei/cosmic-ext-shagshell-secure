//! Window → Session Groups…: the groups of sessions, with the sessions in each. A group opens
//! as one tab per session (each tab marked `[group]`), its tabs close together, and deleting it
//! only takes the name off the sessions. The app does the work (it owns the store and the tabs).

use crate::app::Message;
use crate::fl;
use cosmic::iced::{Color, Length};
use cosmic::{Element, widget};
use shagshell_core::groups::{self, Groups};
use shagshell_core::session::store::Store;

#[derive(Clone, Debug)]
pub enum GrMsg {
    /// `None`: the built-in «Default».
    Open(Option<String>),
    CloseTabs(String),
    /// Asks first; the second press (`DeleteConfirmed`) does it.
    Delete(String),
    DeleteConfirmed,
    Cancel,
    Close,
}

pub struct GroupsDialog {
    groups: Groups,
    /// (group, its sessions); `None`: «Default».
    rows: Vec<(Option<String>, usize)>,
    confirm: Option<String>,
}

/// The group file as it is; an unreadable one is told in the log and starts over empty.
pub fn load() -> Groups {
    Groups::load(&groups::default_path()).unwrap_or_else(|e| {
        log::warn!("groups.toml: {e}");
        Groups::default()
    })
}

/// Groups a saved session names are known (and coloured) from then on.
pub fn ensure(names: &[String]) {
    let mut g = load();
    if g.ensure(names)
        && let Err(e) = g.save(&groups::default_path())
    {
        log::warn!("groups.toml: {e}");
    }
}

impl GroupsDialog {
    pub fn new(store: &Store) -> Self {
        let groups = load();
        let mut rows = vec![(None, store.in_group(None).len())];
        rows.extend(
            (groups.list.iter())
                .map(|g| (Some(g.name.clone()), store.in_group(Some(&g.name)).len())),
        );
        Self {
            groups,
            rows,
            confirm: None,
        }
    }

    pub fn ask_delete(&mut self, name: String) {
        self.confirm = Some(name);
    }

    pub fn take_confirm(&mut self) -> Option<String> {
        self.confirm.take()
    }

    pub fn cancel(&mut self) {
        self.confirm = None;
    }

    pub fn view(&self) -> Element<'_, Message> {
        let m = Message::Groups;
        let mut list = widget::column::with_capacity(self.rows.len()).spacing(8);
        for (name, count) in &self.rows {
            let [r, g, b] = name
                .as_deref()
                .map_or(groups::PALETTE[7], |n| self.groups.color(n));
            let swatch = widget::container(widget::Space::new().width(14).height(14)).class(
                cosmic::theme::Container::custom(move |_| {
                    let mut s = widget::container::Style {
                        background: Some(Color::from_rgb8(r, g, b).into()),
                        ..Default::default()
                    };
                    s.border.radius = 7.0.into();
                    s
                }),
            );
            let shown = name.clone().unwrap_or_else(|| fl!("groups-default"));
            let mut row = widget::row::with_children(vec![
                swatch.into(),
                widget::text::body(format!("{shown} ({count})"))
                    .width(Length::Fill)
                    .into(),
                widget::button::standard(fl!("groups-open"))
                    .on_press_maybe((*count > 0).then(|| m(GrMsg::Open(name.clone()))))
                    .into(),
            ])
            .spacing(8)
            .align_y(cosmic::iced::Alignment::Center);
            if let Some(n) = name {
                row = row
                    .push(
                        widget::button::standard(fl!("groups-close-tabs"))
                            .on_press(m(GrMsg::CloseTabs(n.clone()))),
                    )
                    .push(
                        widget::button::destructive(fl!("sm-delete"))
                            .on_press(m(GrMsg::Delete(n.clone()))),
                    );
            }
            list = list.push(row);
        }
        let mut body = widget::column::with_children(vec![
            widget::text::caption(fl!("groups-hint")).into(),
            list.into(),
        ])
        .spacing(12);
        if let Some(n) = &self.confirm {
            body = body.push(
                widget::row::with_children(vec![
                    widget::text::body(fl!("groups-delete-confirm", name = n.clone()))
                        .width(Length::Fill)
                        .into(),
                    widget::button::standard(fl!("cancel"))
                        .on_press(m(GrMsg::Cancel))
                        .into(),
                    widget::button::destructive(fl!("sm-delete"))
                        .on_press(m(GrMsg::DeleteConfirmed))
                        .into(),
                ])
                .spacing(8),
            );
        }
        widget::dialog()
            .title(fl!("groups-title"))
            .control(body)
            .primary_action(widget::button::suggested(fl!("close")).on_press(m(GrMsg::Close)))
            .into()
    }
}
