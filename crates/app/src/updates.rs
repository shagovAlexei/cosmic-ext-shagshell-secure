//! A newer release at launch (and Help → Check for Updates…): the window with Update / Skip and
//! the donate button. Nothing is downloaded here: Update opens the release page.

use crate::app::Message;
use crate::dialog_icons::Kind;
use crate::fl;
use cosmic::Element;
use cosmic::widget;
use shagshell_core::update::Status;

pub const REPO: &str = "https://github.com/shagovAlexei/cosmic-ext-shagshell-secure";
/// The Payment Link of the project's own Stripe account (amount of the donor's choice, EUR).
// TODO: the Payment Link of the separate Stripe account; until it exists, the project's page.
pub const DONATE_URL: &str = REPO;

#[derive(Clone, Debug)]
pub enum UpMsg {
    /// The answer of GitHub; `true`: asked from the Help menu (says «latest» and errors too).
    Checked(Status, bool),
    Update,
    Skip,
    Later,
    Donate,
}

pub fn release_url(tag: &str) -> String {
    format!("{REPO}/releases/tag/{tag}")
}

pub fn dialog(tag: &str) -> Element<'_, Message> {
    let m = |x| Message::Update(x);
    let body = widget::column::with_capacity(3)
        .spacing(12)
        .push(widget::text::body(fl!(
            "up-body",
            current = env!("CARGO_PKG_VERSION")
        )))
        .push(widget::text::body(fl!("up-support")));
    widget::dialog()
        .icon(Kind::Heart.icon(48))
        .title(fl!("up-title", tag = tag))
        .control(body)
        .primary_action(widget::button::suggested(fl!("up-update")).on_press(m(UpMsg::Update)))
        .secondary_action(widget::button::standard(fl!("up-skip")).on_press(m(UpMsg::Skip)))
        .tertiary_action(widget::button::standard(fl!("up-donate")).on_press(m(UpMsg::Donate)))
        .into()
}
