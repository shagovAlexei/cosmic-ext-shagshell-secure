//! The Button Bar (SecureCRT's): named bars of buttons, in `~/.config/shagshell/buttonbars.toml`.
//! A button sends a string (`core::macros` escapes), runs a menu function of the app, or starts a
//! program. A session may name its own bar.

use serde::{Deserialize, Serialize};
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// `arg` is the string.
    #[default]
    Send,
    /// `arg` is the name of an app function (`Connect`, `Reconnect`…).
    Menu,
    /// `arg` is a command line; `%H %P %U %S` are the active session's host, port, user, name.
    Launch,
    /// `arg` is a Python script (`core::script`), `args` its arguments.
    Script,
}

impl Kind {
    pub const ALL: [Kind; 4] = [Kind::Send, Kind::Menu, Kind::Launch, Kind::Script];
}

/// Where a string goes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Target {
    #[default]
    Current,
    All,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Button {
    pub label: String,
    pub kind: Kind,
    pub arg: String,
    /// A script's arguments (a command line); empty for the others.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub args: String,
    #[serde(skip_serializing_if = "is_current")]
    pub target: Target,
}

fn is_current(t: &Target) -> bool {
    *t == Target::Current
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Bar {
    pub name: String,
    #[serde(rename = "button")]
    pub buttons: Vec<Button>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Bars {
    #[serde(rename = "bar")]
    pub bars: Vec<Bar>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarError {
    Empty,
    Taken,
}

pub fn default_path() -> PathBuf {
    crate::session::store::default_root().parent().map_or_else(
        || PathBuf::from("buttonbars.toml"),
        |p| p.join("buttonbars.toml"),
    )
}

impl Bars {
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.bars.iter().position(|b| b.name == name)
    }

    /// A new empty bar; its index.
    pub fn add(&mut self, name: &str) -> Result<usize, BarError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(BarError::Empty);
        }
        if self.index_of(name).is_some() {
            return Err(BarError::Taken);
        }
        self.bars.push(Bar {
            name: name.to_string(),
            buttons: Vec::new(),
        });
        Ok(self.bars.len() - 1)
    }

    pub fn rename(&mut self, at: usize, name: &str) -> Result<(), BarError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(BarError::Empty);
        }
        if self.index_of(name).is_some_and(|i| i != at) {
            return Err(BarError::Taken);
        }
        if let Some(b) = self.bars.get_mut(at) {
            b.name = name.to_string();
        }
        Ok(())
    }

    pub fn remove(&mut self, at: usize) {
        if at < self.bars.len() {
            self.bars.remove(at);
        }
    }

    /// Button `i` of bar `at` one place left (`-1`) or right (`1`).
    pub fn shift(&mut self, at: usize, i: usize, by: isize) {
        let Some(bar) = self.bars.get_mut(at) else {
            return;
        };
        let j = i.saturating_add_signed(by);
        if i < bar.buttons.len() && j < bar.buttons.len() {
            bar.buttons.swap(i, j);
        }
    }

    /// Missing: none yet. A broken file is an error, never an empty set to be written back.
    pub fn load(path: &Path) -> io::Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                toml::from_str(&text).map_err(|e| io::Error::new(ErrorKind::InvalidData, e))
            }
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e),
        }
    }

    /// A temporary file, then rename: never half written.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = toml::to_string(self).map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)
    }
}

/// `%H %P %U %S` replaced (`%%` is a percent sign); unknown `%x` stay.
pub fn substitute(cmd: &str, host: &str, port: &str, user: &str, session: &str) -> String {
    let mut out = String::new();
    let mut it = cmd.chars();
    while let Some(c) = it.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('H') => out.push_str(host),
            Some('P') => out.push_str(port),
            Some('U') => out.push_str(user),
            Some('S') => out.push_str(session),
            Some('%') => out.push('%'),
            Some(o) => {
                out.push('%');
                out.push(o);
            }
            None => out.push('%'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn button(label: &str, kind: Kind, arg: &str) -> Button {
        Button {
            label: label.into(),
            kind,
            arg: arg.into(),
            args: String::new(),
            target: Target::Current,
        }
    }

    #[test]
    fn round_trip_through_toml() {
        let mut b = Bars::default();
        let i = b.add("cisco").unwrap();
        b.bars[i].buttons.push(Button {
            target: Target::All,
            ..button("save", Kind::Send, "wr\\r")
        });
        b.bars[i]
            .buttons
            .push(button("again", Kind::Menu, "Reconnect"));
        b.bars[i].buttons.push(Button {
            args: "a 'b c'".into(),
            ..button("run", Kind::Script, "/home/u/x.py")
        });
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x/buttonbars.toml");
        b.save(&p).unwrap();
        let text = std::fs::read_to_string(&p).unwrap();
        assert!(
            text.contains("[[bar]]") && text.contains("[[bar.button]]"),
            "{text}"
        );
        assert_eq!(Bars::load(&p).unwrap(), b);
        assert_eq!(
            Bars::load(&dir.path().join("none")).unwrap(),
            Bars::default()
        );
        std::fs::write(&p, "[[bar]\nname=").unwrap();
        assert!(Bars::load(&p).is_err());
    }

    #[test]
    fn names_are_checked() {
        let mut b = Bars::default();
        assert_eq!(b.add("  "), Err(BarError::Empty));
        b.add("a").unwrap();
        b.add("b").unwrap();
        assert_eq!(b.add("a"), Err(BarError::Taken));
        assert_eq!(b.rename(1, "a"), Err(BarError::Taken));
        assert_eq!(b.rename(1, "b"), Ok(())); // itself
        b.rename(1, " c ").unwrap();
        assert_eq!(b.index_of("c"), Some(1));
        b.remove(0);
        assert_eq!(b.bars.len(), 1);
    }

    #[test]
    fn buttons_move_within_the_bar() {
        let mut b = Bars::default();
        b.add("x").unwrap();
        for n in ["1", "2", "3"] {
            b.bars[0].buttons.push(button(n, Kind::Send, n));
        }
        b.shift(0, 0, -1); // already first
        b.shift(0, 0, 1);
        b.shift(0, 2, 1); // already last
        let labels: Vec<_> = b.bars[0].buttons.iter().map(|x| x.label.as_str()).collect();
        assert_eq!(labels, ["2", "1", "3"]);
    }

    #[test]
    fn placeholders() {
        assert_eq!(
            substitute("ssh -p %P %U@%H # %S 100%% %x%", "h", "22", "u", "s"),
            "ssh -p 22 u@h # s 100% %x%"
        );
    }
}
