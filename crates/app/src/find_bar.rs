//! Edit → Find (Ctrl+Shift+F): a bar over the tabs; Enter / ↓ the next match, ↑ the one
//! before, Esc closes. The active terminal searches (`Terminal::search`), the text taken literally;
//! a note searches its text ignoring case, and the bar also replaces there.

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
    /// A note's bar: the replacement typed, Replace (this match, then the next), Replace all.
    With(String),
    Replace,
    ReplaceAll,
}

/// The field (focused by id, so the terminal lets go of the keys).
pub fn field_id() -> widget::Id {
    static ID: std::sync::LazyLock<widget::Id> = std::sync::LazyLock::new(widget::Id::unique);
    ID.clone()
}

/// `with`: the active tab is a note, and this is the replacement typed.
pub fn view(text: &str, with: Option<&str>) -> Element<'static, Message> {
    let m = Message::Find;
    let hint = match with {
        Some(_) => fl!("find-hint-note"),
        None => fl!("find-hint"),
    };
    let mut row = widget::row::with_children(vec![
        widget::search_input(hint, text.to_string())
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
    ]);
    if let Some(with) = with {
        row = row
            .push(
                widget::text_input(fl!("find-with"), with.to_string())
                    .id(with_id())
                    .on_focus(Message::FocusField(with_id()))
                    .on_input(move |t| m(FindMsg::With(t)))
                    .on_submit(move |_| m(FindMsg::Replace))
                    .width(cosmic::iced::Length::Fixed(220.0)),
            )
            .push(widget::button::standard(fl!("find-replace")).on_press(m(FindMsg::Replace)))
            .push(
                widget::button::standard(fl!("find-replace-all")).on_press(m(FindMsg::ReplaceAll)),
            );
    }
    // A narrow window: what does not fit goes on a second line.
    row.spacing(4)
        .padding([2, 8])
        .align_y(cosmic::iced::Alignment::Center)
        .wrap()
        .vertical_spacing(4)
        .into()
}

/// The replacement's field.
pub fn with_id() -> widget::Id {
    static ID: std::sync::LazyLock<widget::Id> = std::sync::LazyLock::new(widget::Id::unique);
    ID.clone()
}
