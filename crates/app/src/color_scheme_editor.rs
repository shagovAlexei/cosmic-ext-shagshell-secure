//! Session Options → Appearance → New… / Edit…: one colour scheme. Fields hold the text as typed
//! (`#RRGGBB`); a wrong value is shown red and OK waits. The window shows this instead of the
//! options while it is open (like Mapped Keys).

use crate::app::Message;
use crate::fl;
use cosmic::iced::{Color, Length};
use cosmic::widget;
use cosmic::{Element, theme};
use shagshell_core::color_scheme::{Scheme, hex, parse_hex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Fg,
    Bg,
    Cursor,
    Ansi(usize),
}

#[derive(Clone, Debug)]
pub enum SeMsg {
    Name(String),
    Color(Slot, String),
    Ok,
    Cancel,
}

pub struct Editor {
    /// The key of the scheme being edited; `None`: a new one (the app picks its key).
    pub key: Option<String>,
    name: String,
    fg: String,
    bg: String,
    cursor: String,
    ansi: [String; 16],
}

pub struct Outcome {
    pub save: Option<(Option<String>, Scheme)>,
    pub close: bool,
}

const NAMES: [&str; 8] = [
    "Black", "Red", "Green", "Yellow", "Blue", "Magenta", "Cyan", "White",
];

impl Editor {
    /// Edit `s`, or start a new scheme from it (`edit` false).
    pub fn new(s: &Scheme, edit: bool) -> Self {
        Editor {
            key: edit.then(|| s.key.clone()),
            name: match edit {
                true => s.name.clone(),
                false => String::new(),
            },
            fg: hex(s.fg),
            bg: hex(s.bg),
            cursor: s.cursor.map(hex).unwrap_or_default(),
            ansi: s.ansi.map(hex),
        }
    }

    fn slot(&mut self, slot: Slot) -> &mut String {
        match slot {
            Slot::Fg => &mut self.fg,
            Slot::Bg => &mut self.bg,
            Slot::Cursor => &mut self.cursor,
            Slot::Ansi(i) => &mut self.ansi[i],
        }
    }

    /// The scheme as typed; `None` while a colour (or the name) is wrong. Cursor "" = the text colour.
    fn scheme(&self) -> Option<Scheme> {
        let mut ansi = [0; 16];
        for (c, t) in ansi.iter_mut().zip(&self.ansi) {
            *c = parse_hex(t)?;
        }
        (!self.name.trim().is_empty()).then_some(())?;
        Some(Scheme {
            key: self.key.clone().unwrap_or_default(),
            name: self.name.trim().to_string(),
            fg: parse_hex(&self.fg)?,
            bg: parse_hex(&self.bg)?,
            cursor: match self.cursor.trim().is_empty() {
                true => None,
                false => Some(parse_hex(&self.cursor)?),
            },
            ansi,
        })
    }

    pub fn update(&mut self, msg: SeMsg) -> Outcome {
        let mut out = Outcome {
            save: None,
            close: false,
        };
        match msg {
            SeMsg::Name(t) => self.name = t,
            SeMsg::Color(slot, t) => *self.slot(slot) = t,
            SeMsg::Cancel => out.close = true,
            SeMsg::Ok => {
                if let Some(s) = self.scheme() {
                    out.save = Some((self.key.clone(), s));
                    out.close = true;
                }
            }
        }
        out
    }

    pub fn view(&self) -> Element<'_, Message> {
        let m = |slot| move |t| Message::SchemeEd(SeMsg::Color(slot, t));
        let color = |label: String, slot: Slot, text: &str, optional: bool| {
            let parsed = parse_hex(text);
            let ok = parsed.is_some() || (optional && text.trim().is_empty());
            let swatch = parsed.unwrap_or(0x808080);
            let (r, g, b) = ((swatch >> 16) as u8, (swatch >> 8) as u8, swatch as u8);
            widget::row::with_children(vec![
                widget::container(widget::Space::new().width(18).height(18))
                    .class(theme::Container::custom(move |_| {
                        let mut s = widget::container::Style {
                            background: Some(Color::from_rgb8(r, g, b).into()),
                            ..Default::default()
                        };
                        s.border.radius = 3.0.into();
                        s.border.width = 1.0;
                        s.border.color = Color::from_rgb(0.5, 0.5, 0.5);
                        s
                    }))
                    .into(),
                widget::text::body(label).width(Length::Fixed(80.0)).into(),
                {
                    let input = widget::text_input("#RRGGBB", text.to_string())
                        .on_input(m(slot))
                        .width(Length::Fixed(110.0));
                    match ok {
                        true => input,
                        false => input.error(fl!("scheme-bad-color")),
                    }
                }
                .into(),
            ])
            .spacing(8)
            .align_y(cosmic::iced::Alignment::Center)
            .into()
        };
        let ansi_col = |from: usize, bright: bool| {
            let rows = (0..8).map(|i| {
                let label = match bright {
                    true => format!("{} +", NAMES[i]),
                    false => NAMES[i].to_string(),
                };
                color(label, Slot::Ansi(from + i), &self.ansi[from + i], false)
            });
            widget::column::with_children(rows.collect::<Vec<Element<'_, Message>>>()).spacing(4)
        };
        let main = widget::column::with_children(vec![
            widget::text_input(fl!("scheme-name"), self.name.clone())
                .on_input(|t| Message::SchemeEd(SeMsg::Name(t)))
                .into(),
            color(fl!("scheme-text"), Slot::Fg, &self.fg, false),
            color(fl!("scheme-background"), Slot::Bg, &self.bg, false),
            color(fl!("scheme-cursor"), Slot::Cursor, &self.cursor, true),
        ])
        .spacing(4);
        let palette =
            widget::row::with_children(vec![ansi_col(0, false).into(), ansi_col(8, true).into()])
                .spacing(24);
        let ok = self.scheme().is_some();
        let buttons = widget::row::with_children(vec![
            widget::button::standard(fl!("cancel"))
                .on_press(Message::SchemeEd(SeMsg::Cancel))
                .into(),
            widget::button::suggested(fl!("ok"))
                .on_press_maybe(ok.then(|| Message::SchemeEd(SeMsg::Ok)))
                .into(),
        ])
        .spacing(8);
        widget::column::with_children(vec![
            widget::text::title4(fl!("scheme-title")).into(),
            main.into(),
            palette.into(),
            self.preview(),
            buttons.into(),
        ])
        .spacing(12)
        .padding(16)
        .into()
    }

    /// Text in every colour on the background, and a cursor block.
    fn preview(&self) -> Element<'_, Message> {
        let rgb = |t: &str, d: u32| {
            let c = parse_hex(t).unwrap_or(d);
            Color::from_rgb8((c >> 16) as u8, (c >> 8) as u8, c as u8)
        };
        let (fg, bg) = (rgb(&self.fg, 0xCCCCCC), rgb(&self.bg, 0));
        let cursor = match self.cursor.trim().is_empty() {
            true => fg,
            false => rgb(&self.cursor, 0xCCCCCC),
        };
        let text = |s: String, c: Color| {
            widget::text(s)
                .font(cosmic::font::mono())
                .size(14)
                .class(theme::Text::Color(c))
        };
        let line = |from: usize| {
            let mut r = widget::row::with_capacity(9).spacing(0);
            for (i, name) in NAMES.iter().enumerate() {
                r = r.push(text(
                    format!("{} ", name.chars().next().unwrap_or(' ')),
                    rgb(&self.ansi[from + i], 0),
                ));
            }
            r
        };
        let cursor_block = widget::container(text("█".into(), cursor));
        let body = widget::column::with_children(vec![
            widget::row::with_children(vec![
                text("user@host:~$ ls -l ".into(), fg).into(),
                cursor_block.into(),
            ])
            .into(),
            line(0).into(),
            line(8).into(),
        ]);
        widget::container(body)
            .padding(8)
            .width(Length::Fill)
            .class(theme::Container::custom(move |_| {
                widget::container::Style {
                    background: Some(bg.into()),
                    ..Default::default()
                }
            }))
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Scheme {
        Scheme {
            key: "x".into(),
            name: "X".into(),
            fg: 0xAAAAAA,
            bg: 0,
            cursor: None,
            ansi: [0x123456; 16],
        }
    }

    #[test]
    fn typed_colours_are_checked() {
        let mut e = Editor::new(&base(), true);
        assert_eq!(e.scheme().unwrap(), base());
        e.update(SeMsg::Color(Slot::Ansi(3), "#12".into()));
        assert!(e.update(SeMsg::Ok).save.is_none());
        e.update(SeMsg::Color(Slot::Ansi(3), "#00ff00".into()));
        e.update(SeMsg::Color(Slot::Cursor, "ff0000".into()));
        let out = e.update(SeMsg::Ok);
        let (key, s) = out.save.unwrap();
        assert_eq!(key.as_deref(), Some("x"));
        assert_eq!((s.ansi[3], s.cursor), (0x00FF00, Some(0xFF0000)));
    }

    #[test]
    fn new_scheme_needs_a_name_and_has_no_key() {
        let mut e = Editor::new(&base(), false);
        assert!(e.key.is_none() && e.update(SeMsg::Ok).save.is_none());
        e.update(SeMsg::Name(" Mine ".into()));
        let (key, s) = e.update(SeMsg::Ok).save.unwrap();
        assert!(key.is_none());
        assert_eq!(s.name, "Mine");
    }
}
