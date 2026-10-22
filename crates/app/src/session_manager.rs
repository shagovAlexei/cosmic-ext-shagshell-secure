//! Session Manager pane on the left. Empty until phase 3 brings the session tree.

use crate::app::Message;
use crate::fl;
use cosmic::Element;
use cosmic::iced::Length;
use cosmic::widget;

pub const WIDTH: f32 = 240.0;

pub fn view() -> Element<'static, Message> {
    let body = widget::column::with_children(vec![
        widget::text::heading(fl!("menu-session-manager")).into(),
        widget::container(widget::text(fl!("sessions-empty")))
            .center(Length::Fill)
            .into(),
    ])
    .spacing(8)
    .padding(8);
    widget::container(body)
        .width(WIDTH)
        .height(Length::Fill)
        .class(cosmic::theme::Container::Card)
        .into()
}
