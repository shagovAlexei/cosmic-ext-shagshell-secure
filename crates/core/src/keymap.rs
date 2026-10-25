//! Key Map: the user's own keys (`keymap.toml`), each running what a Button Bar button runs.
//! A key is a chord of modifiers and a physical key by its W3C code name (`KeyA`, `F5`,
//! `ArrowUp`…); written as `Ctrl+Alt+F5`. The program's own keys (`BUILTIN`) have their default
//! chord and may be given another one (`[builtin]` in the same file).

use crate::buttonbar::Button;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
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
    /// A plain F2–F8: the file manager's and the session tree's own keys.
    Ui,
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
        if !any && matches!(c, "F2" | "F3" | "F4" | "F5" | "F6" | "F7" | "F8") {
            return Err(Reason::Ui);
        }
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

/// The program's own keys: (function, default chord). The function names are those of the
/// Button Bar's menu functions (`menu::NAMED`). Alt+1…9 (tab N) are not in the list: fixed.
pub const BUILTIN: [(&str, &str); 16] = [
    ("ToggleSessionManager", "Alt+M"),
    ("QuickConnect", "Alt+Q"),
    ("Connect", "Alt+C"),
    ("ConnectSftp", "Alt+P"),
    ("NewLocalShell", "Ctrl+Shift+T"),
    ("CloseTab", "Ctrl+Shift+W"),
    ("Copy", "Ctrl+Shift+C"),
    ("Paste", "Ctrl+Shift+V"),
    ("NextTab", "Ctrl+Tab"),
    ("PrevTab", "Ctrl+Shift+Tab"),
    ("ZoomIn", "Ctrl+Equal"),
    ("ZoomOut", "Ctrl+Minus"),
    ("ZoomReset", "Ctrl+0"),
    ("FullScreen", "F11"),
    ("Find", "Ctrl+Shift+F"),
    ("ConnectBar", "Ctrl+Shift+R"),
];

/// The default chord of a built-in function.
pub fn builtin_default(id: &str) -> Option<Chord> {
    BUILTIN
        .iter()
        .find(|(n, _)| *n == id)
        .and_then(|(_, c)| c.parse().ok())
}

/// The numpad's twins of the zoom keys act as the main block's.
pub fn normalize(code: &str) -> &str {
    match code {
        "NumpadAdd" => "Equal",
        "NumpadSubtract" => "Minus",
        "Numpad0" => "Digit0",
        c => c,
    }
}

/// Alt+1…9 select tab N: no remapping, no binding.
fn is_tab_key(c: &Chord) -> bool {
    c.alt
        && !c.ctrl
        && !c.shift
        && c.code
            .strip_prefix("Digit")
            .is_some_and(|d| d.len() == 1 && d != "0")
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Keymap {
    /// Built-in functions given another chord (function name → chord).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub builtin: BTreeMap<String, Chord>,
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
    /// A built-in key has it (or Alt+1…9).
    Builtin,
}

impl Keymap {
    pub fn default_path() -> PathBuf {
        crate::buttonbar::default_path().with_file_name("keymap.toml")
    }

    /// The chord a built-in function answers to now.
    pub fn chord_of(&self, id: &str) -> Option<Chord> {
        self.builtin
            .get(id)
            .cloned()
            .or_else(|| builtin_default(id))
    }

    /// Every built-in function with its chord now.
    pub fn builtin_chords(&self) -> Vec<(&'static str, Chord)> {
        BUILTIN
            .iter()
            .filter_map(|(id, _)| self.chord_of(id).map(|c| (*id, c)))
            .collect()
    }

    /// Does a built-in key (but `except`) or Alt+1…9 have this chord?
    pub fn builtin_has(&self, c: &Chord, except: Option<&str>) -> bool {
        is_tab_key(c)
            || self
                .builtin_chords()
                .iter()
                .any(|(id, b)| Some(*id) != except && b == c)
    }

    /// Gives a built-in function another chord. Like a user's key it may not take the terminal's
    /// (but Ctrl+Shift+letter is the terminal emulators' own convention, and the function's
    /// default is always fine), nor a key anything else has.
    pub fn set_builtin(&mut self, id: &str, c: Chord) -> Result<(), Refused> {
        let default = builtin_default(id).ok_or(Refused::Empty)?;
        match c.check() {
            Err(Reason::Terminal) if (c.ctrl && c.shift) || c == default => {}
            Err(why) => return Err(Refused::Chord(why)),
            Ok(()) => {}
        }
        if self.builtin_has(&c, Some(id)) || self.find(&c).is_some() {
            return Err(Refused::Taken);
        }
        match c == default {
            true => self.builtin.remove(id),
            false => self.builtin.insert(id.to_string(), c),
        };
        Ok(())
    }

    pub fn reset_builtin(&mut self, id: &str) {
        self.builtin.remove(id);
    }

    pub fn find(&self, c: &Chord) -> Option<usize> {
        self.keys.iter().position(|b| &b.keys == c)
    }

    /// `b` in place of binding `at`, or added (`None`).
    pub fn put(&mut self, at: Option<usize>, b: Binding) -> Result<usize, Refused> {
        b.keys.check().map_err(Refused::Chord)?;
        if self.builtin_has(&b.keys, None) {
            return Err(Refused::Builtin);
        }
        // A lone space is a fine string to send (paging through «--More--»), nothing else.
        let lone_space = b.button.kind == crate::buttonbar::Kind::Send && b.button.arg == " ";
        if b.button.arg.trim().is_empty() && !lone_space {
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
            "F9",
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
            ("F5", Reason::Ui),
            ("F2", Reason::Ui),
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
        assert_eq!(m.put(None, bind("Alt+F1", "a")), Ok(0));
        assert_eq!(m.put(None, bind("Alt+F1", "b")), Err(Refused::Taken));
        assert_eq!(m.put(None, bind("Alt+F2", "")), Err(Refused::Empty));
        assert_eq!(
            m.put(None, bind("Ctrl+C", "x")),
            Err(Refused::Chord(Reason::Terminal))
        );
        assert_eq!(m.put(Some(0), bind("Alt+F1", "c")), Ok(0)); // itself is not a clash
        assert_eq!(m.put(None, bind("Alt+F3", " ")), Ok(1)); // a lone space is a string
        let mut script = bind("Alt+F4", " ");
        script.button.kind = Kind::Script;
        assert_eq!(m.put(None, script), Err(Refused::Empty));
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

    #[test]
    fn builtin_defaults_are_sane() {
        let mut seen = std::collections::HashSet::new();
        for (id, c) in BUILTIN {
            let chord = chord(c);
            assert!(seen.insert(chord.clone()), "{id} repeats {c}");
            assert!(!chord.code.is_empty() && !is_tab_key(&chord), "{id}");
            assert_eq!(builtin_default(id), Some(chord));
        }
        assert_eq!(
            Keymap::default().chord_of("Find"),
            Some(chord("Ctrl+Shift+F"))
        );
        assert_eq!(normalize("NumpadAdd"), "Equal");
    }

    #[test]
    fn a_builtin_key_can_be_moved_and_reset() {
        let mut m = Keymap::default();
        assert_eq!(m.set_builtin("ConnectBar", chord("Alt+R")), Ok(()));
        assert_eq!(m.chord_of("ConnectBar"), Some(chord("Alt+R")));
        // the old chord is free now, the new one taken
        assert!(!m.builtin_has(&chord("Ctrl+Shift+R"), None));
        assert!(m.builtin_has(&chord("Alt+R"), None));
        assert_eq!(m.put(None, bind("Alt+R", "x")), Err(Refused::Builtin));
        assert_eq!(
            m.put(None, bind("Ctrl+Shift+R", "x")),
            Err(Refused::Chord(Reason::Terminal))
        );
        // its default back = no override stored
        assert_eq!(m.set_builtin("ConnectBar", chord("Ctrl+Shift+R")), Ok(()));
        assert!(m.builtin.is_empty());
        m.set_builtin("Find", chord("F9")).unwrap();
        m.reset_builtin("Find");
        assert_eq!(m.chord_of("Find"), Some(chord("Ctrl+Shift+F")));
    }

    #[test]
    fn a_builtin_key_refuses_what_a_user_key_refuses() {
        let mut m = Keymap::default();
        for (c, why) in [
            ("Ctrl+C", Refused::Chord(Reason::Terminal)),
            ("Q", Refused::Chord(Reason::Terminal)),
            ("F5", Refused::Chord(Reason::Ui)),
            ("Ctrl+Insert", Refused::Chord(Reason::Reserved)),
            ("Alt+Q", Refused::Taken),
            ("Alt+3", Refused::Taken),
        ] {
            assert_eq!(m.set_builtin("Find", chord(c)), Err(why), "{c}");
        }
        m.put(None, bind("F9", "x")).unwrap();
        assert_eq!(m.set_builtin("Find", chord("F9")), Err(Refused::Taken));
        assert_eq!(m.set_builtin("Nope", chord("F10")), Err(Refused::Empty));
        // Ctrl+Shift+letter is allowed for a built-in
        assert_eq!(m.set_builtin("Find", chord("Ctrl+Shift+G")), Ok(()));
    }

    #[test]
    fn regression_a_user_key_cannot_take_a_builtin_chord() {
        let mut m = Keymap::default();
        assert_eq!(m.put(None, bind("Alt+Q", "x")), Err(Refused::Builtin));
        assert_eq!(m.put(None, bind("Alt+1", "x")), Err(Refused::Builtin));
        assert_eq!(m.put(None, bind("F11", "x")), Err(Refused::Builtin));
    }

    #[test]
    fn builtin_overrides_are_stored_and_old_files_load() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("keymap.toml");
        let mut m = Keymap::default();
        m.set_builtin("ConnectBar", chord("Alt+R")).unwrap();
        m.save(&p).unwrap();
        let text = std::fs::read_to_string(&p).unwrap();
        assert!(
            text.contains("[builtin]") && text.contains("ConnectBar = \"Alt+R\""),
            "{text}"
        );
        assert_eq!(Keymap::load(&p).unwrap(), m);
        std::fs::write(&p, "[[key]]\nkeys = \"F9\"\nkind = \"send\"\narg = \"x\"\n").unwrap();
        assert!(Keymap::load(&p).unwrap().builtin.is_empty());
    }
}
