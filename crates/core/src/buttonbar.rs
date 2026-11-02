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
    /// Its dot (SecureCRT's button colours): a name of `COLORS`, empty = none.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub color: String,
    /// The hint shown when the pointer rests on the button; empty: none.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub tip: String,
}

/// The button colours (SecureCRT's 11) and how they are drawn.
pub const COLORS: [(&str, &str); 11] = [
    ("black", "#1e1e1e"),
    ("blue", "#2f6fdf"),
    ("cyan", "#1fb6c9"),
    ("green", "#2ea043"),
    ("grey", "#8b949e"),
    ("magenta", "#d63dc1"),
    ("orange", "#f08c2e"),
    ("purple", "#8957e5"),
    ("red", "#da3633"),
    ("white", "#f0f0f0"),
    ("yellow", "#e3c21a"),
];

/// The dot of colour `name` (12×12 SVG, a thin outline so black / white show on any theme).
pub fn dot_svg(name: &str) -> Option<String> {
    let (_, hex) = COLORS.iter().find(|(n, _)| *n == name)?;
    Some(format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 12 12"><circle cx="6" cy="6" r="5" fill="{hex}" stroke="#808080" stroke-width="0.8"/></svg>"##
    ))
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
    /// Kept in alphabetical order (Button Bar Manager's check box).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub sort: bool,
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

impl Bar {
    /// `buttons` after button `after` (`None` or past the end: at the end); the index of the
    /// first one inserted.
    pub fn insert(&mut self, after: Option<usize>, buttons: &[Button]) -> usize {
        let at = after.map_or(self.buttons.len(), |i| (i + 1).min(self.buttons.len()));
        self.buttons.splice(at..at, buttons.iter().cloned());
        at
    }
}

impl Bars {
    /// Alphabetical (case blind) when `sort` is on.
    pub fn resort(&mut self) {
        if self.sort {
            self.bars.sort_by_key(|b| b.name.to_lowercase());
        }
    }

    /// Bars brought in by an import whose names are not here yet; whether any came, and where
    /// the bar at `selected` is now.
    pub fn merge(&mut self, new: Vec<Bar>, selected: usize) -> (bool, usize) {
        let picked = self.bars.get(selected).map(|b| b.name.clone());
        let before = self.bars.len();
        for b in new {
            if self.index_of(&b.name).is_none() {
                self.bars.push(b);
            }
        }
        self.resort();
        let at = picked.and_then(|n| self.index_of(&n)).unwrap_or(selected);
        (self.bars.len() > before, at)
    }

    /// Where `name` is now (after a `resort`).
    fn place(&mut self, name: &str) -> usize {
        self.resort();
        self.index_of(name).unwrap_or(0)
    }

    /// Bar `at` one place up (`-1`) or down (`1`); where it is now.
    pub fn move_bar(&mut self, at: usize, by: isize) -> usize {
        let j = at.saturating_add_signed(by);
        if at < self.bars.len() && j < self.bars.len() {
            self.bars.swap(at, j);
            return j;
        }
        at
    }

    /// A copy of bar `at` under `name`; its index.
    pub fn duplicate(&mut self, at: usize, name: &str) -> Result<usize, BarError> {
        let buttons = self.bars.get(at).map(|b| b.buttons.clone());
        let i = self.add(name)?;
        if let Some(b) = buttons {
            self.bars[i].buttons = b;
        }
        Ok(i)
    }

    /// «name (copy)», «name (copy 2)»… — the first one not taken (`word`: the language's «copy»).
    pub fn copy_name(&self, name: &str, word: &str) -> String {
        let mut cur = format!("{name} ({word})");
        let mut n = 2;
        while self.index_of(&cur).is_some() {
            cur = format!("{name} ({word} {n})");
            n += 1;
        }
        cur
    }

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
        Ok(self.place(name))
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
        self.resort();
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

    #[test]
    fn a_button_colour_is_kept_and_drawn() {
        let mut b = button("x", Kind::Send, "ls\\r");
        assert!(!toml::to_string(&b).unwrap().contains("color")); // none: not written
        b.color = "green".into();
        b.tip = "list files".into();
        let back: Button = toml::from_str(&toml::to_string(&b).unwrap()).unwrap();
        assert_eq!(
            (back.color.as_str(), back.tip.as_str()),
            ("green", "list files")
        );
        assert!(dot_svg("green").unwrap().contains("#2ea043"));
        assert_eq!(dot_svg("nope"), None);
    }

    fn button(label: &str, kind: Kind, arg: &str) -> Button {
        Button {
            label: label.into(),
            kind,
            arg: arg.into(),
            args: String::new(),
            target: Target::Current,
            color: String::new(),
            tip: String::new(),
        }
    }

    #[test]
    fn round_trip_through_toml() {
        let mut b = Bars::default();
        let i = b.add("cisco").unwrap();
        b.bars[i].buttons.push(Button {
            target: Target::All,
            color: String::new(),
            tip: String::new(),
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

    fn labels(bar: &Bar) -> Vec<&str> {
        bar.buttons.iter().map(|x| x.label.as_str()).collect()
    }

    #[test]
    fn buttons_are_inserted_after_one_or_at_the_end() {
        let mut bar = Bar::default();
        for n in ["1", "2"] {
            bar.buttons.push(button(n, Kind::Send, n));
        }
        let two = [button("a", Kind::Send, "a"), button("b", Kind::Send, "b")];
        assert_eq!(bar.insert(Some(0), &two), 1);
        assert_eq!(labels(&bar), ["1", "a", "b", "2"]);
        assert_eq!(bar.insert(None, &two[..1]), 4);
        assert_eq!(labels(&bar), ["1", "a", "b", "2", "a"]);
        assert_eq!(bar.insert(Some(99), &two[1..]), 5); // past the end: the end
        assert_eq!(Bar::default().insert(Some(0), &two), 0);
    }

    #[test]
    fn a_bar_is_duplicated_with_a_free_name() {
        let mut b = Bars::default();
        b.add("x").unwrap();
        b.bars[0].buttons.push(button("1", Kind::Send, "1"));
        assert_eq!(b.copy_name("x", "copy"), "x (copy)");
        let i = b.duplicate(0, "x (copy)").unwrap();
        assert_eq!((i, b.bars[1].buttons.len()), (1, 1));
        assert_eq!(b.copy_name("x", "copy"), "x (copy 2)");
        assert_eq!(b.duplicate(0, "x (copy)"), Err(BarError::Taken));
        assert_eq!(b.duplicate(0, " "), Err(BarError::Empty));
    }

    #[test]
    fn bars_move_and_sort() {
        let mut b = Bars::default();
        for n in ["b", "c", "A"] {
            b.add(n).unwrap();
        }
        let names = |b: &Bars| b.bars.iter().map(|x| x.name.clone()).collect::<Vec<_>>();
        assert_eq!(b.move_bar(0, 1), 1);
        assert_eq!(b.move_bar(2, 1), 2); // already last
        assert_eq!(b.move_bar(0, -1), 0); // already first
        assert_eq!(names(&b), ["c", "b", "A"]);
        b.sort = true;
        b.resort();
        assert_eq!(names(&b), ["A", "b", "c"]);
        // sorted: a new bar lands in its place
        let i = b.add("a2").unwrap();
        assert_eq!((i, names(&b)[1].as_str()), (1, "a2"));
    }

    #[test]
    fn regression_merge_keeps_the_picked_bar() {
        let mut b = Bars {
            sort: true,
            ..Bars::default()
        };
        b.add("m").unwrap();
        let bar = |n: &str| Bar {
            name: n.into(),
            buttons: Vec::new(),
        };
        // "m" is picked (0); imported "a" sorts before it, "m" comes again and is not doubled.
        assert_eq!(b.merge(vec![bar("a"), bar("m")], 0), (true, 1));
        assert_eq!(b.bars.len(), 2);
        assert_eq!(b.merge(vec![bar("a")], 1), (false, 1));
    }

    #[test]
    fn an_old_file_has_no_sort() {
        let b: Bars = toml::from_str("[[bar]]\nname = \"x\"\n").unwrap();
        assert!(!b.sort);
        assert!(!toml::to_string(&b).unwrap().contains("sort"));
    }

    #[test]
    fn placeholders() {
        assert_eq!(
            substitute("ssh -p %P %U@%H # %S 100%% %x%", "h", "22", "u", "s"),
            "ssh -p 22 u@h # s 100% %x%"
        );
    }
}
