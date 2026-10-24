//! Options → Global Options, in the context drawer. Each change is written at once.

use crate::app::Message;
use crate::config::{AppTheme, Config, ExistsRule, LANGUAGES};
use crate::fl;
use cosmic::app::context_drawer::{self, ContextDrawer};
use cosmic::widget::{self, settings};
use shagshell_core::highlight::{Colour, Rule, Rules};

/// `separate`: passwords in a keyring of their own (`None`: not known yet / being changed).
pub fn view<'a>(
    config: &'a Config,
    separate: Option<bool>,
    hl: &'a Rules,
) -> ContextDrawer<'a, Message> {
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
    let text = |value: &str, f: fn(&mut Config, String)| -> cosmic::Element<'static, Message> {
        let c = config.clone();
        widget::text_input("", value.to_string())
            .on_input(move |v| {
                let mut c = c.clone();
                f(&mut c, v);
                Message::SetConfig(c)
            })
            .width(cosmic::iced::Length::Fixed(260.0))
            .into()
    };
    // Colour scheme of the terminal and the keyword rules.
    let mut names = vec![fl!("options-system")];
    names.extend(
        crate::term::terminal_theme::schemes()
            .iter()
            .map(|(_, n)| (*n).to_string()),
    );
    let keys: Vec<&'static str> = crate::term::terminal_theme::schemes()
        .iter()
        .map(|(k, _)| *k)
        .collect();
    let scheme = keys
        .iter()
        .position(|k| *k == config.color_scheme)
        .map_or(0, |i| i + 1);
    let mut colours = settings::section()
        .title(fl!("options-colours"))
        .add(settings::item(
            fl!("options-scheme"),
            widget::dropdown(names, Some(scheme), {
                let c = config.clone();
                move |i| {
                    let mut c = c.clone();
                    c.color_scheme = i
                        .checked_sub(1)
                        .map_or(String::new(), |i| keys[i].to_string());
                    Message::SetConfig(c)
                }
            }),
        ))
        .add(settings::item(
            fl!("options-highlight"),
            widget::toggler(config.highlight_enabled)
                .on_toggle(set(|c, on| c.highlight_enabled = on)),
        ));
    let colour_names: Vec<String> = vec![
        fl!("hl-red"),
        fl!("hl-green"),
        fl!("hl-yellow"),
        fl!("hl-blue"),
        fl!("hl-magenta"),
        fl!("hl-cyan"),
        fl!("hl-orange"),
        fl!("hl-white"),
    ];
    for (i, r) in hl.rules.iter().enumerate() {
        let edit = |f: fn(&mut Rule, String), r0: &Rules| {
            let r0 = r0.clone();
            move |v: String| {
                let mut x = r0.clone();
                f(&mut x.rules[i], v);
                Message::SetRules(x)
            }
        };
        let at = Colour::ALL.iter().position(|c| *c == r.colour);
        colours = colours.add(
            widget::row::with_children(vec![
                widget::text_input(fl!("options-hl-word"), r.text.clone())
                    .on_input(edit(|r, v| r.text = v, hl))
                    .width(cosmic::iced::Length::Fill)
                    .into(),
                widget::dropdown(colour_names.clone(), at, {
                    let r0 = hl.clone();
                    move |c| {
                        let mut x = r0.clone();
                        x.rules[i].colour = Colour::ALL[c];
                        Message::SetRules(x)
                    }
                })
                .into(),
                widget::checkbox(r.background)
                    .label(fl!("options-hl-fill"))
                    .on_toggle({
                        let r0 = hl.clone();
                        move |on| {
                            let mut x = r0.clone();
                            x.rules[i].background = on;
                            Message::SetRules(x)
                        }
                    })
                    .into(),
                widget::button::icon(widget::icon::from_name("edit-delete-symbolic"))
                    .on_press({
                        let mut x = hl.clone();
                        x.rules.remove(i);
                        Message::SetRules(x)
                    })
                    .into(),
            ])
            .spacing(6)
            .align_y(cosmic::iced::Alignment::Center),
        );
    }
    colours = colours.add(widget::button::standard(fl!("options-hl-add")).on_press({
        let mut x = hl.clone();
        x.rules.push(Rule::default());
        Message::SetRules(x)
    }));
    let logging = settings::section()
        .title(fl!("options-logging"))
        .add(settings::item(
            fl!("options-log-dir"),
            text(&config.log_dir, |c, v| c.log_dir = v),
        ))
        .add(settings::item(
            fl!("options-log-template"),
            text(&config.log_template, |c, v| c.log_template = v),
        ))
        .add(widget::text::caption(fl!("options-log-hint")));
    let zmodem = settings::section()
        .title(fl!("options-zmodem"))
        .add(settings::item(
            fl!("options-zmodem-auto"),
            widget::toggler(config.zmodem_auto).on_toggle(set(|c, on| c.zmodem_auto = on)),
        ))
        .add(settings::item(
            fl!("options-download-dir"),
            text(&config.download_dir, |c, v| c.download_dir = v),
        ));
    let terminal = settings::section()
        .title(fl!("options-terminal"))
        .add(settings::item(
            fl!("options-font"),
            text(&config.font_family, |c, v| c.font_family = v),
        ))
        .add(settings::item(
            fl!("options-font-size"),
            text(&config.font_size.to_string(), |c, v| {
                c.font_size = v.trim().parse().unwrap_or(0)
            }),
        ))
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
        colours.into(),
        logging.into(),
        zmodem.into(),
        sftp.into(),
        passwords.into(),
    ])
    .spacing(16);
    context_drawer::context_drawer(content, Message::CloseDrawer).title(fl!("menu-global-options"))
}
