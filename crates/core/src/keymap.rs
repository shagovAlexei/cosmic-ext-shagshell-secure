//! Key Map: the user's own keys (`keymap.toml`), each running what a Button Bar button runs.
//! A key is a chord of modifiers and a physical key by its W3C code name (`KeyA`, `F5`,
//! `ArrowUp`…); written as `Ctrl+Alt+F5`.

use crate::buttonbar::Button;
use serde::{Deserialize, Serialize};
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Chord {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    /// `KeyA`, `Digit1`, `F5`, `ArrowUp`…
    pub code: String,
}

/// Why a chord is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// A modifier on its own.
    Modifier,
    /// Ctrl+Insert / Shift+Insert: the system's.
    Reserved,
    /// A plain, Shift, Ctrl or Ctrl+Shift letter, digit or editing key: the terminal's.
    Terminal,
}

fn is_function(code: &str) -> bool {
    code.strip_prefix('F')
        .is_some_and(|n| !n.is_empty() && n.len() <= 2 && n.bytes().all(|b| b.is_ascii_digit()))
}

fn is_navigation(code: &str) -> bool {
    matches!(
        code,
        "ArrowUp"
            | "ArrowDown"
            | "ArrowLeft"
            | "ArrowRight"
            | "Home"
            | "End"
            | "PageUp"
            | "PageDown"
    )
}

impl Chord {
    pub fn new(ctrl: bool, alt: bool, shift: bool, code: &str) -> Self {
        Self {
            ctrl,
            alt,
            shift,
            code: code.to_string(),
        }
    }

    /// May this chord be bound? Anything the terminal's programs need (Ctrl+C, plain letters)
    /// may not.
    pub fn check(&self) -> Result<(), Reason> {
        let c = self.code.as_str();
        if c.is_empty()
            || ["Shift", "Control", "Alt", "Super", "Meta", "Hyper", "Fn"]
                .iter()
                .any(|m| c.starts_with(m))
        {
            return Err(Reason::Modifier);
        }
        if c == "Insert" && (self.ctrl || self.shift) && !self.alt {
            return Err(Reason::Reserved);
        }
        let any = self.ctrl || self.alt || self.shift;
        if is_function(c) || self.alt || (is_navigation(c) && any) {
            return Ok(());
        }
        Err(Reason::Terminal)
    }
}

impl std::fmt::Display for Chord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (on, name) in [
            (self.ctrl, "Ctrl"),
            (self.alt, "Alt"),
            (self.shift, "Shift"),
        ] {
            if on {
                write!(f, "{name}+")?;
            }
        }
        let c = self.code.as_str();
        match (c.strip_prefix("Key"), c.strip_prefix("Digit")) {
            (Some(k), _) if k.len() == 1 => f.write_str(k),
            (_, Some(d)) if d.len() == 1 => f.write_str(d),
            _ => f.write_str(c),
        }
    }
}

impl std::str::FromStr for Chord {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let mut parts: Vec<&str> = s.split('+').map(str::trim).collect();
        let key = parts.pop().filter(|k| !k.is_empty()).ok_or("no key")?;
        let mut c = Chord::default();
        for m in parts {
            match m {
                "Ctrl" => c.ctrl = true,
                "Alt" => c.alt = true,
                "Shift" => c.shift = true,
                other => return Err(format!("unknown modifier {other}")),
            }
        }
        c.code = match key.len() == 1 {
            true if key.as_bytes()[0].is_ascii_uppercase() => format!("Key{key}"),
            true if key.as_bytes()[0].is_ascii_digit() => format!("Digit{key}"),
            _ => key.to_string(),
        };
        Ok(c)
    }
}

impl Serialize for Chord {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Chord {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    pub keys: Chord,
    #[serde(flatten)]
    pub button: Button,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Keymap {
    #[serde(rename = "key")]
    pub keys: Vec<Binding>,
}

/// Why a binding is refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    Chord(Reason),
    /// Another binding has it.
    Taken,
    /// Nothing to do: no text / function / program / script.
    Empty,
}

impl Keymap {
    pub fn default_path() -> PathBuf {
        crate::buttonbar::default_path().with_file_name("keymap.toml")
    }

    pub fn find(&self, c: &Chord) -> Option<usize> {
        self.keys.iter().position(|b| &b.keys == c)
    }

    /// `b` in place of binding `at`, or added (`None`).
    pub fn put(&mut self, at: Option<usize>, b: Binding) -> Result<usize, Refused> {
        b.keys.check().map_err(Refused::Chord)?;
        if b.button.arg.trim().is_empty() && !(b.button.arg == " ") {
            return Err(Refused::Empty);
        }
        if self.find(&b.keys).is_some_and(|i| Some(i) != at) {
            return Err(Refused::Taken);
        }
        match at.filter(|&i| i < self.keys.len()) {
            Some(i) => {
                self.keys[i] = b;
                Ok(i)
            }
            None => {
                self.keys.push(b);
                Ok(self.keys.len() - 1)
            }
        }
    }

    pub fn remove(&mut self, i: usize) {
        if i < self.keys.len() {
            self.keys.remove(i);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buttonbar::Kind;

    fn chord(s: &str) -> Chord {
        s.parse().unwrap()
    }

    fn bind(k: &str, arg: &str) -> Binding {
        Binding {
            keys: chord(k),
            button: Button {
                kind: Kind::Send,
                arg: arg.into(),
                ..Default::default()
            },
        }
    }

    #[test]
    fn text_form_round_trips() {
        for s in [
            "Ctrl+Alt+F5",
            "Alt+Q",
            "Alt+Shift+7",
            "F12",
            "Ctrl+ArrowUp",
            "Alt+Numpad1",
        ] {
            assert_eq!(chord(s).to_string(), s);
        }
        assert_eq!(chord("Alt+Q").code, "KeyQ");
        assert_eq!(chord("Alt+7").code, "Digit7");
        assert!("Hyper+Q".parse::<Chord>().is_err());
        assert!("Alt+".parse::<Chord>().is_err());
    }

    #[test]
    fn what_may_be_bound() {
        let ok = |s: &str| chord(s).check();
        for s in [
            "Alt+Q",
            "Ctrl+Alt+Q",
            "F5",
            "Shift+F5",
            "Ctrl+F1",
            "Ctrl+ArrowUp",
            "Shift+PageUp",
            "Alt+Enter",
            "Ctrl+Alt+Insert",
        ] {
            assert_eq!(ok(s), Ok(()), "{s}");
        }
        // the terminal's, the system's
        for (s, why) in [
            ("Ctrl+C", Reason::Terminal),
            ("Ctrl+Shift+T", Reason::Terminal),
            ("Q", Reason::Terminal),
            ("Shift+7", Reason::Terminal),
            ("Ctrl+Enter", Reason::Terminal),
            ("ArrowUp", Reason::Terminal),
            ("Ctrl+Insert", Reason::Reserved),
            ("Shift+Insert", Reason::Reserved),
        ] {
            assert_eq!(ok(s), Err(why), "{s}");
        }
        assert_eq!(
            Chord::new(false, true, false, "ShiftLeft").check(),
            Err(Reason::Modifier)
        );
        assert_eq!(Chord::default().check(), Err(Reason::Modifier));
    }

    #[test]
    fn put_checks_and_replaces() {
        let mut m = Keymap::default();
        assert_eq!(m.put(None, bind("Alt+1", "a")), Ok(0));
        assert_eq!(m.put(None, bind("Alt+1", "b")), Err(Refused::Taken));
        assert_eq!(m.put(None, bind("Alt+2", "")), Err(Refused::Empty));
        assert_eq!(
            m.put(None, bind("Ctrl+C", "x")),
            Err(Refused::Chord(Reason::Terminal))
        );
        assert_eq!(m.put(Some(0), bind("Alt+1", "c")), Ok(0)); // itself is not a clash
        assert_eq!(m.put(None, bind("Alt+3", " ")), Ok(1)); // a lone space is a string
        assert_eq!(m.keys[0].button.arg, "c");
        m.remove(0);
        assert_eq!(m.keys.len(), 1);
    }

    #[test]
    fn file_round_trip_and_broken() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x/keymap.toml");
        let mut m = Keymap::default();
        m.put(
            None,
            Binding {
                keys: chord("Ctrl+Alt+F5"),
                button: Button {
                    kind: Kind::Script,
                    arg: "/a/b.py".into(),
                    args: "x".into(),
                    label: "run".into(),
                    ..Default::default()
                },
            },
        )
        .unwrap();
        m.save(&p).unwrap();
        let text = std::fs::read_to_string(&p).unwrap();
        assert!(
            text.contains("[[key]]") && text.contains("keys = \"Ctrl+Alt+F5\""),
            "{text}"
        );
        assert_eq!(Keymap::load(&p).unwrap(), m);
        assert_eq!(
            Keymap::load(&dir.path().join("none")).unwrap(),
            Keymap::default()
        );
        std::fs::write(&p, "[[key]]\nkeys = \"Hyper+Q\"\n").unwrap();
        assert!(Keymap::load(&p).is_err());
    }
}
