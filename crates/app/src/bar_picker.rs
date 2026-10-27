//! «⋯» at the end of a tab's bar (Script Builder, notes): every button, ticked when shown,
//! ↑ / ↓ to move it (up = to the left on the bar), «Reset» (`core::bar`).

use crate::app::Message;
use crate::fl;
use crate::toolbar::tip_button;
use cosmic::iced::{Alignment, Length};
use cosmic::{Element, theme, widget};
use shagshell_core::bar::Bar;

/// `open`: the list shows; `flip` opens / closes it; `set` gets the bar's new list.
pub fn view<'a>(
    bar: &'static Bar,
    list: &[String],
    defaults: Vec<String>,
    label: fn(&str) -> String,
    open: bool,
    flip: Message,
    set: fn(Vec<String>) -> Message,
) -> Element<'a, Message> {
    let more = tip_button(
        "view-more-symbolic",
        fl!("note-buttons"),
        Some(flip.clone()),
    );
    let mut pop = widget::popover(more)
        .position(widget::popover::Position::Bottom)
        .on_close(flip);
    if open {
        let rows = bar
            .rows(list)
            .into_iter()
            .map(|(name, on)| -> Element<'a, Message> {
                let mut check = widget::checkbox(on).label(label(name));
                if bar.keep != Some(name) {
                    let l = list.to_vec();
                    check = check.on_toggle(move |_| set(bar.toggle(&l, name)));
                }
                let arrow = |icon: &'static str, tip: String, left: bool| {
                    let l = list.to_vec();
                    tip_button(icon, tip, on.then(|| set(bar.shift(&l, name, left))))
                };
                widget::row::with_children(vec![
                    widget::container(check).width(Length::Fill).into(),
                    arrow("go-up-symbolic", fl!("builder-bar-up"), true),
                    arrow("go-down-symbolic", fl!("builder-bar-down"), false),
                ])
                .spacing(4)
                .align_y(Alignment::Center)
                .into()
            });
        let col = widget::column::with_children(rows.collect::<Vec<_>>())
            .spacing(4)
            .push(widget::button::text(fl!("builder-bar-reset")).on_press(set(defaults)));
        pop = pop.popup(
            widget::container(col)
                .width(Length::Fixed(300.0))
                .padding(12)
                .class(theme::Container::Dropdown),
        );
    }
    pop.into()
}
