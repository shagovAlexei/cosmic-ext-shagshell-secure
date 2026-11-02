//! Terminal colour schemes: the built-in ones (`data.rs`: SecureCRT's own and ours) and the user's
//! own in `color_schemes.toml`. An own scheme with a built-in's key replaces it until removed.

mod data;

use serde::{Deserialize, Serialize};
use std::io::{self, ErrorKind};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scheme {
    pub key: String,
    pub name: String,
    pub fg: u32,
    pub bg: u32,
    /// `None`: the text colour.
    pub cursor: Option<u32>,
    /// Black … white, then the bright ones.
    pub ansi: [u32; 16],
}

/// `"#RRGGBB"` or `"RRGGBB"`.
pub fn parse_hex(s: &str) -> Option<u32> {
    let s = s.trim().trim_start_matches('#');
    (s.len() == 6 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| u32::from_str_radix(s, 16).ok())
        .flatten()
}

pub fn hex(c: u32) -> String {
    format!("#{:06X}", c & 0xFF_FFFF)
}

/// The file form: colours as strings.
#[derive(Serialize, Deserialize)]
struct Raw {
    key: String,
    name: String,
    fg: String,
    bg: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cursor: Option<String>,
    ansi: Vec<String>,
}

impl From<&Scheme> for Raw {
    fn from(s: &Scheme) -> Self {
        Raw {
            key: s.key.clone(),
            name: s.name.clone(),
            fg: hex(s.fg),
            bg: hex(s.bg),
            cursor: s.cursor.map(hex),
            ansi: s.ansi.iter().map(|c| hex(*c)).collect(),
        }
    }
}

impl Raw {
    fn scheme(&self) -> Option<Scheme> {
        let ansi: Vec<u32> = self.ansi.iter().filter_map(|c| parse_hex(c)).collect();
        Some(Scheme {
            key: self.key.clone(),
            name: self.name.clone(),
            fg: parse_hex(&self.fg)?,
            bg: parse_hex(&self.bg)?,
            cursor: self.cursor.as_deref().and_then(parse_hex),
            ansi: ansi.try_into().ok()?,
        })
    }
}

pub fn builtins() -> Vec<Scheme> {
    data::BUILTIN
        .iter()
        .map(|&(key, name, fg, bg, ansi)| Scheme {
            key: key.into(),
            name: name.into(),
            fg,
            bg,
            cursor: None,
            ansi,
        })
        .collect()
}

/// `"Black / Cyan"` → `black-cyan`.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Schemes {
    own: Vec<Scheme>,
}

#[derive(Serialize, Deserialize, Default)]
struct File {
    #[serde(default, rename = "scheme")]
    schemes: Vec<Raw>,
}

impl Schemes {
    pub fn default_path() -> std::path::PathBuf {
        crate::buttonbar::default_path().with_file_name("color_schemes.toml")
    }

    /// Missing: none of our own. A broken file is an error, never an empty list to be written back.
    /// A scheme with a bad colour is left out.
    pub fn load(path: &Path) -> io::Result<Self> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e),
        };
        let f: File =
            toml::from_str(&text).map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;
        let mut s = Self::default();
        for r in f.schemes.iter().filter_map(Raw::scheme) {
            s.put(r);
        }
        Ok(s)
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let f = File {
            schemes: self.own.iter().map(Raw::from).collect(),
        };
        let text = toml::to_string(&f).map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(tmp, path)
    }

    /// Built-ins (own ones with the same key in their place) and the other own ones, by name.
    pub fn all(&self) -> Vec<Scheme> {
        let mut v: Vec<Scheme> = builtins()
            .into_iter()
            .filter(|b| !self.own.iter().any(|o| o.key == b.key))
            .chain(self.own.iter().cloned())
            .collect();
        v.sort_by_key(|s| s.name.to_lowercase());
        v
    }

    pub fn get(&self, key: &str) -> Option<Scheme> {
        self.own
            .iter()
            .find(|s| s.key == key)
            .cloned()
            .or_else(|| builtins().into_iter().find(|s| s.key == key))
    }

    /// Add, or replace the one with this key.
    pub fn put(&mut self, s: Scheme) {
        match self.own.iter_mut().find(|o| o.key == s.key) {
            Some(o) => *o = s,
            None => self.own.push(s),
        }
    }

    /// Drop our own; a built-in under the same key comes back. `false`: there was none.
    pub fn remove(&mut self, key: &str) -> bool {
        let n = self.own.len();
        self.own.retain(|s| s.key != key);
        self.own.len() != n
    }

    /// Is `key` a built-in one (maybe replaced)?
    pub fn is_builtin(key: &str) -> bool {
        data::BUILTIN.iter().any(|b| b.0 == key)
    }

    pub fn is_own(&self, key: &str) -> bool {
        self.own.iter().any(|s| s.key == key)
    }

    /// A key for a new scheme called `name`: its slug, `-2`, `-3`… if taken.
    pub fn new_key(&self, name: &str) -> String {
        let base = match slug(name) {
            s if s.is_empty() => "scheme".to_string(),
            s => s,
        };
        let taken = |k: &str| Self::is_builtin(k) || self.is_own(k);
        let mut key = base.clone();
        let mut n = 1;
        while taken(&key) {
            n += 1;
            key = format!("{base}-{n}");
        }
        key
    }

    /// The key of the scheme called `name` (case-insensitive), for SecureCRT's session files.
    pub fn key_of_name(&self, name: &str) -> Option<String> {
        self.all()
            .into_iter()
            .find(|s| s.name.eq_ignore_ascii_case(name.trim()))
            .map(|s| s.key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mine(key: &str) -> Scheme {
        Scheme {
            key: key.into(),
            name: "Mine".into(),
            fg: 0x112233,
            bg: 0x000000,
            cursor: Some(0xFF0000),
            ansi: [0xABCDEF; 16],
        }
    }

    #[test]
    fn hex_parse() {
        assert_eq!(parse_hex("#a0b1c2"), Some(0xA0B1C2));
        assert_eq!(parse_hex(" A0B1C2 "), Some(0xA0B1C2));
        assert_eq!(parse_hex("#fff"), None);
        assert_eq!(parse_hex("#gg0000"), None);
    }

    #[test]
    fn builtins_are_unique_and_have_securecrt_ones() {
        let b = builtins();
        for s in &b {
            assert_eq!(b.iter().filter(|o| o.key == s.key).count(), 1, "{}", s.key);
            assert_eq!(slug(&s.name), s.key);
        }
        let w = Schemes::default().get("windows").unwrap();
        assert_eq!((w.fg, w.bg), (0x000000, 0xFFFFFF));
    }

    #[test]
    fn file_round_trip_and_missing_and_broken() {
        let dir = std::env::temp_dir().join(format!("cs-{}", std::process::id()));
        let path = dir.join("color_schemes.toml");
        assert_eq!(Schemes::load(&path).unwrap(), Schemes::default());
        let mut s = Schemes::default();
        s.put(mine("mine"));
        s.save(&path).unwrap();
        assert_eq!(Schemes::load(&path).unwrap(), s);
        std::fs::write(&path, "scheme = [").unwrap();
        assert!(Schemes::load(&path).is_err());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn bad_scheme_is_left_out() {
        let dir = std::env::temp_dir().join(format!("cs-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("c.toml");
        std::fs::write(
            &path,
            "[[scheme]]\nkey='x'\nname='X'\nfg='#zzzzzz'\nbg='#000000'\nansi=[]\n",
        )
        .unwrap();
        assert!(Schemes::load(&path).unwrap().own.is_empty());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn override_hides_builtin_and_remove_brings_it_back() {
        let mut s = Schemes::default();
        let before = s.get("nord").unwrap();
        s.put(mine("nord"));
        assert_eq!(s.get("nord").unwrap().fg, 0x112233);
        assert_eq!(s.all().iter().filter(|x| x.key == "nord").count(), 1);
        assert!(s.remove("nord"));
        assert_eq!(s.get("nord").unwrap(), before);
        assert!(!s.remove("nord"));
    }

    #[test]
    fn new_key_is_unique() {
        let mut s = Schemes::default();
        assert_eq!(s.new_key("My Scheme"), "my-scheme");
        s.put(mine("my-scheme"));
        assert_eq!(s.new_key("My Scheme"), "my-scheme-2");
        assert_eq!(s.new_key("Nord"), "nord-2");
        assert_eq!(s.new_key("???"), "scheme");
    }

    #[test]
    fn name_lookup() {
        assert_eq!(
            Schemes::default().key_of_name("White / Black").as_deref(),
            Some("white-black")
        );
        assert_eq!(Schemes::default().key_of_name("nope"), None);
    }
}
