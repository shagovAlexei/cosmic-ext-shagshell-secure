//! Active Sessions: the Session Manager pane's second page, the open tabs (SecureCRT's dock tab).
//! The grouping is `shagshell_core::active_tabs`; this module draws it.

use crate::app::Message;
use crate::fl;
use crate::tabs::Tabs;
use cosmic::iced::Length;
use cosmic::widget;
use cosmic::{Element, iced};
use shagshell_core::active_tabs::{Kind, group};

#[derive(Clone, Debug)]
pub enum ActiveMsg {
    Filter(String),
    /// Bring the tab to the front.
    Show(u64),
    Close(u64),
    CloseAll,
    /// `true`: Active Sessions, else the session tree.
    Mode(bool),
}

/// The pane: the two-button switch, then the session tree (`tree`) or the open tabs.
pub fn view<'a>(
    active: bool,
    filter: &'a str,
    tabs: &'a Tabs,
    tree: impl FnOnce() -> Element<'a, Message>,
) -> Element<'a, Message> {
    let page = |label: String, on: bool, mode: bool| {
        let b = match on {
            true => widget::button::suggested(label),
            false => widget::button::standard(label),
        };
        b.on_press(Message::Active(ActiveMsg::Mode(mode)))
            .width(Length::Fill)
    };
    let switch = widget::row::with_children(vec![
        page(fl!("as-page-tree"), !active, false).into(),
        page(fl!("as-page-active"), active, true).into(),
    ])
    .spacing(4)
    .padding([8, 8, 0, 8]);
    let body = match active {
        true => list(filter, tabs),
        false => tree(),
    };
    widget::column::with_children(vec![switch.into(), body])
        .height(Length::Fill)
        .into()
}

fn list<'a>(filter: &'a str, tabs: &'a Tabs) -> Element<'a, Message> {
    let m = Message::Active;
    let (rows, mut icons): (Vec<_>, std::collections::HashMap<_, _>) = {
        let all = tabs.active_rows();
        let rows: Vec<_> = all.iter().map(|(r, _)| r.clone()).collect();
        (rows, all.into_iter().map(|(r, i)| (r.id, i)).collect())
    };
    let filter_box = widget::search_input(fl!("sm-filter"), filter.to_string())
        .on_input(move |f| m(ActiveMsg::Filter(f)))
        .on_clear(m(ActiveMsg::Filter(String::new())));
    let sections = group(&rows, filter);
    let mut col = widget::column::with_capacity(sections.len() * 2).spacing(2);
    for s in sections {
        let title = match s.kind {
            Kind::Session => fl!("as-sessions"),
            Kind::Editor => fl!("as-editors"),
        };
        col = col.push(widget::text::heading(format!("{title} ({})", s.rows.len())));
        for r in s.rows {
            let icon = icons.remove(&r.id);
            let mut line = widget::row::with_capacity(3)
                .spacing(4)
                .align_y(iced::Alignment::Center);
            if let Some(i) = icon {
                line = line.push(i);
            }
            line = line.push(widget::text::body(r.title).width(Length::Fill));
            line = line.push(widget::tooltip(
                widget::button::icon(widget::icon::from_name("window-close-symbolic").size(12))
                    .padding(2)
                    .on_press(m(ActiveMsg::Close(r.id))),
                widget::text(fl!("as-close")),
                widget::tooltip::Position::Bottom,
            ));
            col = col.push(
                widget::mouse_area(widget::container(line).padding([2, 4]).width(Length::Fill))
                    .on_press(m(ActiveMsg::Show(r.id))),
            );
        }
    }
    let body: Element<'_, Message> = match rows.is_empty() {
        true => widget::container(widget::text::caption(fl!("as-empty")))
            .center(Length::Fill)
            .into(),
        false => widget::scrollable(col).height(Length::Fill).into(),
    };
    let close_all = widget::button::standard(fl!("as-close-all"))
        .on_press_maybe((!rows.is_empty()).then(|| m(ActiveMsg::CloseAll)));
    widget::column::with_children(vec![filter_box.into(), body, close_all.into()])
        .spacing(6)
        .padding(8)
        .height(Length::Fill)
        .into()
}
