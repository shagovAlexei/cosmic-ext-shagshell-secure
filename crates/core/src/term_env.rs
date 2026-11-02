//! Session Options → Terminal → Emulation: the `TERM` / `COLORTERM` a tab's program gets, and the scrollback.

use serde::{Deserialize, Serialize};

pub const DEFAULT_SCROLLBACK: usize = 10_000;
pub const MIN_SCROLLBACK: u32 = 100;
pub const MAX_SCROLLBACK: u32 = 1_000_000;
pub const DEFAULT_COLS: u16 = 80;
pub const DEFAULT_ROWS: u16 = 24;
pub const MAX_CELLS: u16 = 500;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    #[default]
    Xterm,
    Vt100,
    Vt220,
    Linux,
    Ansi,
}

impl Kind {
    pub const ALL: [Kind; 5] = [
        Kind::Xterm,
        Kind::Vt100,
        Kind::Vt220,
        Kind::Linux,
        Kind::Ansi,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Kind::Xterm => "Xterm",
            Kind::Vt100 => "VT100",
            Kind::Vt220 => "VT220",
            Kind::Linux => "Linux",
            Kind::Ansi => "ANSI",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Colors {
    Mono,
    Ansi16,
    #[default]
    Ansi256,
    True,
}

impl Colors {
    pub const ALL: [Colors; 4] = [Colors::Mono, Colors::Ansi16, Colors::Ansi256, Colors::True];
}

/// What the Backspace key sends (SecureCRT: Modes → «Backspace sends»).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Backspace {
    /// `^?` (DEL), what xterm and Linux expect.
    #[default]
    Del,
    /// `^H`, for older gear.
    Ctrl,
}

impl Backspace {
    pub const ALL: [Backspace; 2] = [Backspace::Del, Backspace::Ctrl];

    /// The byte for Backspace; Ctrl+Backspace sends the other one.
    pub fn byte(self, ctrl: bool) -> u8 {
        match self == Backspace::Ctrl {
            true if !ctrl => 0x08,
            false if ctrl => 0x08,
            _ => 0x7f,
        }
    }
}

/// What the Delete key sends (SecureCRT: Modes → «Delete sends»).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Delete {
    /// `ESC [ 3 ~`, what xterm and Linux expect.
    #[default]
    Escape,
    /// `^?` (DEL), for gear that wants a plain byte.
    Del,
}

impl Delete {
    pub const ALL: [Delete; 2] = [Delete::Escape, Delete::Del];

    /// The bytes for Delete without modifiers; with modifiers it is always the escape form.
    pub fn plain(self) -> Option<&'static [u8]> {
        (self == Delete::Del).then_some(b"\x7f")
    }
}

/// What a window change does to the terminal (SecureCRT: «On resize»).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Fit {
    /// More or fewer rows and columns.
    #[default]
    Window,
    /// The logical size stays; the font grows or shrinks to fill the window.
    Font,
    /// The logical size and the font stay.
    Keep,
}

impl Fit {
    pub const ALL: [Fit; 3] = [Fit::Window, Fit::Font, Fit::Keep];
}

/// The logical size in columns, rows: a parsed form value is `1..=MAX_CELLS`.
pub fn parse_cells(s: &str) -> Option<u16> {
    s.trim()
        .parse()
        .ok()
        .filter(|n| (1..=MAX_CELLS).contains(n))
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Term {
    #[serde(skip_serializing_if = "is_default")]
    pub fit: Fit,
    /// Logical columns / rows for `Fit::Font` and `Fit::Keep`; none: 80 × 24.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cols: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rows: Option<u16>,
    /// Alt+key sends the key alone instead of ESC + key (SecureCRT: Emacs → «Use Alt as Meta» off).
    #[serde(skip_serializing_if = "is_default")]
    pub alt_plain: bool,
    #[serde(skip_serializing_if = "is_default")]
    pub delete: Delete,
    #[serde(skip_serializing_if = "is_default")]
    pub backspace: Backspace,
    #[serde(skip_serializing_if = "is_default")]
    pub kind: Kind,
    #[serde(skip_serializing_if = "is_default")]
    pub colors: Colors,
    /// Lines kept above the screen; none: `DEFAULT_SCROLLBACK`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scrollback: Option<u32>,
}

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

impl Term {
    pub fn is_default(&self) -> bool {
        *self == Term::default()
    }

    /// `TERM` and, for true colour, `COLORTERM`.
    pub fn env(&self) -> Vec<(String, String)> {
        let term = match (self.kind, self.colors) {
            (Kind::Xterm, Colors::Mono) => "xterm-mono",
            (Kind::Xterm, Colors::Ansi16) => "xterm-16color",
            (Kind::Xterm, _) => "xterm-256color",
            (Kind::Vt100, _) => "vt100",
            (Kind::Vt220, _) => "vt220",
            (Kind::Linux, _) => "linux",
            (Kind::Ansi, _) => "ansi",
        };
        let mut env = vec![("TERM".to_string(), term.to_string())];
        if self.colors == Colors::True {
            env.push(("COLORTERM".to_string(), "truecolor".to_string()));
        }
        env
    }

    /// The logical (columns, rows).
    pub fn cells(&self) -> (u16, u16) {
        (
            self.cols.unwrap_or(DEFAULT_COLS),
            self.rows.unwrap_or(DEFAULT_ROWS),
        )
    }

    pub fn history(&self) -> usize {
        self.scrollback.map_or(DEFAULT_SCROLLBACK, |n| {
            n.clamp(MIN_SCROLLBACK, MAX_SCROLLBACK) as usize
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn term_of(t: &Term) -> String {
        t.env()[0].1.clone()
    }

    #[test]
    fn default_is_todays_behaviour() {
        let t = Term::default();
        assert_eq!(
            t.env(),
            vec![("TERM".to_string(), "xterm-256color".to_string())]
        );
        assert_eq!(t.history(), 10_000);
    }

    #[test]
    fn kinds_and_colours() {
        let t = |kind, colors| Term {
            kind,
            colors,
            ..Term::default()
        };
        assert_eq!(term_of(&t(Kind::Vt100, Colors::True)), "vt100");
        assert_eq!(term_of(&t(Kind::Xterm, Colors::Ansi16)), "xterm-16color");
        assert_eq!(term_of(&t(Kind::Xterm, Colors::Mono)), "xterm-mono");
        let env = t(Kind::Xterm, Colors::True).env();
        assert_eq!(env[1], ("COLORTERM".to_string(), "truecolor".to_string()));
        assert_eq!(t(Kind::Linux, Colors::Ansi256).env().len(), 1);
    }

    #[test]
    fn delete_bytes() {
        assert_eq!(Delete::Escape.plain(), None);
        assert_eq!(Delete::Del.plain(), Some(&b"\x7f"[..]));
    }

    #[test]
    fn logical_size() {
        assert_eq!(parse_cells(" 132 "), Some(132));
        for bad in ["", "0", "501", "x", "-1"] {
            assert_eq!(parse_cells(bad), None, "{bad}");
        }
        assert_eq!(Term::default().cells(), (80, 24));
        let t = Term {
            fit: Fit::Keep,
            cols: Some(132),
            rows: Some(50),
            ..Term::default()
        };
        assert_eq!(t.cells(), (132, 50));
        let back: Term = toml::from_str(&toml::to_string(&t).unwrap()).unwrap();
        assert_eq!(back, t);
    }

    #[test]
    fn backspace_bytes() {
        assert_eq!(Backspace::Del.byte(false), 0x7f);
        assert_eq!(Backspace::Del.byte(true), 0x08);
        assert_eq!(Backspace::Ctrl.byte(false), 0x08);
        assert_eq!(Backspace::Ctrl.byte(true), 0x7f);
    }

    #[test]
    fn scrollback_is_clamped() {
        let s = |n| {
            Term {
                scrollback: Some(n),
                ..Term::default()
            }
            .history()
        };
        assert_eq!(s(5), 100);
        assert_eq!(s(50_000), 50_000);
        assert_eq!(s(u32::MAX), 1_000_000);
    }

    #[test]
    fn toml_round_trip_and_default_is_empty() {
        assert_eq!(toml::to_string(&Term::default()).unwrap(), "");
        let t = Term {
            kind: Kind::Ansi,
            colors: Colors::True,
            scrollback: Some(500),
            backspace: Backspace::Ctrl,
            delete: Delete::Del,
            alt_plain: true,
            fit: Fit::Font,
            cols: Some(100),
            rows: Some(30),
        };
        let back: Term = toml::from_str(&toml::to_string(&t).unwrap()).unwrap();
        assert_eq!(back, t);
    }
}
