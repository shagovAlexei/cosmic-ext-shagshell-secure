//! Session Options → Terminal → Emulation: the `TERM` / `COLORTERM` a tab's program gets, and the scrollback.

use serde::{Deserialize, Serialize};

pub const DEFAULT_SCROLLBACK: usize = 10_000;
pub const MIN_SCROLLBACK: u32 = 100;
pub const MAX_SCROLLBACK: u32 = 1_000_000;

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

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Term {
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
            scrollback: None,
        };
        assert_eq!(term_of(&t(Kind::Vt100, Colors::True)), "vt100");
        assert_eq!(term_of(&t(Kind::Xterm, Colors::Ansi16)), "xterm-16color");
        assert_eq!(term_of(&t(Kind::Xterm, Colors::Mono)), "xterm-mono");
        let env = t(Kind::Xterm, Colors::True).env();
        assert_eq!(env[1], ("COLORTERM".to_string(), "truecolor".to_string()));
        assert_eq!(t(Kind::Linux, Colors::Ansi256).env().len(), 1);
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
        };
        let back: Term = toml::from_str(&toml::to_string(&t).unwrap()).unwrap();
        assert_eq!(back, t);
    }
}
