//! Options → Global Options, in the context drawer. Each change is written at once.

use crate::app::Message;
use crate::config::{AppTheme, Config, ExistsRule, LANGUAGES};
use crate::fl;
use cosmic::app::context_drawer::{self, ContextDrawer};
use cosmic::widget::{self, settings};

/// `separate`: passwords in a keyring of their own (`None`: not known yet / being changed).
pub fn view(config: &Config, separate: Option<bool>) -> ContextDrawer<'_, Message> {
    let set = |f: fn(&mut Config, bool)| {
        let c = config.clone();
        move |on| {
            let mut c = c.clone();
            f(&mut c, on);
            Message::SetConfig(c)
        }
    };
    let language = LANGUAGES
        .iter()
        .position(|l| *l == config.language)
        .map_or(0, |i| i + 1);
    let languages = vec![fl!("options-system"), "English".into(), "Русский".into()];
    let themes = vec![
        fl!("options-system"),
        fl!("options-light"),
        fl!("options-dark"),
    ];
    let theme = AppTheme::ALL.iter().position(|t| *t == config.app_theme);
    let interface = settings::section()
        .title(fl!("options-interface"))
        .add(settings::item(
            fl!("options-language"),
            widget::dropdown(languages, Some(language), {
                let c = config.clone();
                move |i| {
                    let mut c = c.clone();
                    c.language = i
                        .checked_sub(1)
                        .map_or(String::new(), |i| LANGUAGES[i].into());
                    Message::SetConfig(c)
                }
            }),
        ))
        .add(settings::item(
            fl!("options-theme"),
            widget::dropdown(themes, theme, {
                let c = config.clone();
                move |i| {
                    let mut c = c.clone();
                    c.app_theme = AppTheme::ALL[i];
                    Message::SetConfig(c)
                }
            }),
        ));
    let rules = vec![
        fl!("options-ow-ask"),
        fl!("sftp-overwrite"),
        fl!("sftp-newer"),
        fl!("sftp-skip"),
    ];
    let rule = ExistsRule::ALL
        .iter()
        .position(|r| *r == config.sftp_overwrite);
    let sftp = settings::section()
        .title(fl!("options-sftp"))
        .add(settings::item(
            fl!("options-sftp-exists"),
            widget::dropdown(rules, rule, {
                let c = config.clone();
                move |i| {
                    let mut c = c.clone();
                    c.sftp_overwrite = ExistsRule::ALL[i];
                    Message::SetConfig(c)
                }
            }),
        ))
        .add(settings::item(
            fl!("options-sftp-hidden"),
            widget::toggler(config.sftp_show_hidden)
                .on_toggle(set(|c, on| c.sftp_show_hidden = on)),
        ));
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
    let mut master = widget::toggler(separate.unwrap_or(false));
    if separate.is_some() {
        master = master.on_toggle(Message::SetSeparate);
    }
    let passwords = settings::section()
        .title(fl!("options-passwords"))
        .add(settings::item_row(vec![
            widget::column::with_children(vec![
                widget::text::body(fl!("options-master")).into(),
                widget::text::caption(fl!("options-master-hint")).into(),
            ])
            .width(cosmic::iced::Length::Fill)
            .into(),
            master.into(),
        ]))
        .add(settings::item(
            fl!("options-credentials"),
            widget::button::standard(fl!("options-open"))
                .on_press(Message::Action(crate::menu::Action::Credentials)),
        ));
    let content = widget::column::with_children(vec![
        interface.into(),
        terminal.into(),
        sftp.into(),
        passwords.into(),
    ])
    .spacing(16);
    context_drawer::context_drawer(content, Message::CloseDrawer).title(fl!("menu-global-options"))
}
