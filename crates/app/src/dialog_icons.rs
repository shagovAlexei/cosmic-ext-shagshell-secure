//! The icons of our dialogs: full colour, drawn for this app (`res/icons/dialog/*.svg`), so a
//! dialog looks the same under any icon theme (the system's `dialog-*-symbolic` ones are grey).

use cosmic::widget::icon;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // `Success` is part of the set, for the dialogs that come
pub enum Kind {
    Info,
    Question,
    Warning,
    Stop,
    Success,
    Lock,
}

impl Kind {
    pub fn handle(self) -> icon::Handle {
        let svg: &'static [u8] = match self {
            Kind::Info => include_bytes!("../../../res/icons/dialog/info.svg"),
            Kind::Question => include_bytes!("../../../res/icons/dialog/question.svg"),
            Kind::Warning => include_bytes!("../../../res/icons/dialog/warning.svg"),
            Kind::Stop => include_bytes!("../../../res/icons/dialog/stop.svg"),
            Kind::Success => include_bytes!("../../../res/icons/dialog/success.svg"),
            Kind::Lock => include_bytes!("../../../res/icons/dialog/lock.svg"),
        };
        icon::from_svg_bytes(svg)
    }

    /// The icon at `size` px, ready for a dialog.
    pub fn icon(self, size: u16) -> icon::Icon {
        icon::icon(self.handle()).size(size)
    }
}
