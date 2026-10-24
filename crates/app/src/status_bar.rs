//! Status bar at the bottom: connection state, later protocol and rows×cols.

use crate::app::Message;
use crate::fl;
use cosmic::Element;
use cosmic::widget;

pub fn view() -> Element<'static, Message> {
    widget::container(widget::text::caption(fl!("status-disconnected")))
        .padding([2, 8])
        .into()
}
