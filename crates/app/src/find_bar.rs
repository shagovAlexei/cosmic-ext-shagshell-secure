//! Edit → Find (Ctrl+Shift+F): a bar over the terminals; Enter / ↓ the next match, ↑ the one
//! before, Esc closes. The active terminal searches (`Terminal::search`), the text taken literally.

use crate::app::Message;
use crate::fl;
use cosmic::Element;
use cosmic::widget;

#[derive(Clone, Debug)]
pub enum FindMsg {
    Open,
    Text(String),
    Next,
    Prev,
    Close,
}

/// The field (focused by id, so the terminal lets go of the keys).
pub fn field_id() -> widget::Id {
    static ID: std::sync::LazyLock<widget::Id> = std::sync::LazyLock::new(widget::Id::unique);
    ID.clone()
}

pub fn view(text: &str) -> Element<'static, Message> {
    let m = Message::Find;
    widget::row::with_children(vec![
        widget::search_input(fl!("find-hint"), text.to_string())
            .id(field_id())
            .on_focus(Message::FocusField(field_id()))
            .on_input(move |t| m(FindMsg::Text(t)))
            .on_submit(move |_| m(FindMsg::Next))
            .width(cosmic::iced::Length::Fixed(320.0))
            .into(),
        crate::toolbar::tip_button("go-up-symbolic", fl!("find-prev"), Some(m(FindMsg::Prev))),
        crate::toolbar::tip_button("go-down-symbolic", fl!("find-next"), Some(m(FindMsg::Next))),
        crate::toolbar::tip_button(
            "window-close-symbolic",
            fl!("find-close"),
            Some(m(FindMsg::Close)),
        ),
    ])
    .spacing(4)
    .padding([2, 8])
    .align_y(cosmic::iced::Alignment::Center)
    .into()
}
