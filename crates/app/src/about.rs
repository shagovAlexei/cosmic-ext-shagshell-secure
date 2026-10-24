//! Help → About, in the context drawer.

use crate::app::{APP_ID, Message};
use crate::fl;
use cosmic::app::context_drawer::{self, ContextDrawer};
use cosmic::widget::{self, about::About};

const REPO: &str = "https://github.com/shagovAlexei/cosmic-ext-shagshell-secure";

pub fn about() -> About {
    About::default()
        .name(fl!("app-title"))
        .icon(widget::icon::from_name(APP_ID))
        .version(env!("CARGO_PKG_VERSION"))
        .author("Shagov Alexei")
        .comments(fl!("about-comments"))
        .license("GPL-3.0-only")
        .license_url("https://www.gnu.org/licenses/gpl-3.0.html")
        .links([
            (fl!("about-source"), REPO.to_string()),
            (fl!("about-issues"), format!("{REPO}/issues")),
        ])
}

pub fn view(about: &About) -> ContextDrawer<'_, Message> {
    let content = widget::about(about, |url| Message::OpenUrl(url.to_string()));
    context_drawer::context_drawer(content, Message::CloseDrawer).title(fl!("menu-about"))
}
