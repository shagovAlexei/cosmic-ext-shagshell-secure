//! Options → Global Options, in the context drawer. Each change is written at once.

use crate::app::Message;
use crate::config::Config;
use crate::fl;
use cosmic::app::context_drawer::{self, ContextDrawer};
use cosmic::widget::{self, settings};

pub fn view(config: &Config) -> ContextDrawer<'_, Message> {
    let set = |f: fn(&mut Config, bool)| {
        let c = config.clone();
        move |on| {
            let mut c = c.clone();
            f(&mut c, on);
            Message::SetConfig(c)
        }
    };
    let terminal = settings::section()
        .title(fl!("options-terminal"))
        .add(settings::item(
            fl!("options-copy-on-select"),
            widget::toggler(config.copy_on_select).on_toggle(set(|c, on| c.copy_on_select = on)),
        ))
        .add(settings::item(
            fl!("options-paste-on-middle"),
            widget::toggler(config.paste_on_middle_click)
                .on_toggle(set(|c, on| c.paste_on_middle_click = on)),
        ));
    context_drawer::context_drawer(terminal, Message::CloseDrawer).title(fl!("menu-global-options"))
}
