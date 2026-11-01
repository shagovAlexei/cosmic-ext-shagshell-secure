//! Session groups (like Chrome's tab groups, Ansible's inventory groups): a session names the
//! groups it belongs to (`Session::groups`), `~/.config/shagshell/groups.toml` keeps each group's
//! colour. A session with no group is in the built-in «Default» group, which is never stored.

use serde::{Deserialize, Serialize};
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

/// The built-in group of sessions that name none; the UI shows it in the user's language.
pub const DEFAULT: &str = "Default";

/// Group colours (RGB), handed out in turn to new groups.
pub const PALETTE: &[[u8; 3]] = &[
    [0x1a, 0x73, 0xe8],
    [0xd9, 0x30, 0x25],
    [0xf9, 0xab, 0x00],
    [0x1e, 0x8e, 0x3e],
    [0xd0, 0x18, 0x8b],
    [0x00, 0x97, 0xa7],
    [0x7b, 0x1f, 0xa2],
    [0x5f, 0x63, 0x68],
];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Group {
    pub name: String,
    /// Index into `PALETTE`.
    pub color: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Groups {
    #[serde(default, rename = "group")]
    pub list: Vec<Group>,
}

pub fn default_path() -> PathBuf {
    crate::session::store::default_root()
        .parent()
        .map_or_else(|| PathBuf::from("groups.toml"), |p| p.join("groups.toml"))
}

/// «a, b ,a» → ["a", "b"]: trimmed, each once, no empty names, no «Default» (that is no group).
pub fn parse(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for n in text.split(',').map(str::trim) {
        if !n.is_empty() && !n.eq_ignore_ascii_case(DEFAULT) && !out.iter().any(|o| o == n) {
            out.push(n.to_string());
        }
    }
    out
}

impl Groups {
    /// The colour of `name`; «Default» and unknown names get the grey one.
    pub fn color(&self, name: &str) -> [u8; 3] {
        let i = self
            .list
            .iter()
            .find(|g| g.name == name)
            .map_or(7, |g| g.color);
        PALETTE[i % PALETTE.len()]
    }

    /// Adds the groups not known yet, each with the next colour. `true`: something was added.
    pub fn ensure(&mut self, names: &[String]) -> bool {
        let before = self.list.len();
        for n in names {
            if !self.list.iter().any(|g| &g.name == n) {
                let color = self.list.len() % PALETTE.len();
                self.list.push(Group {
                    name: n.clone(),
                    color,
                });
            }
        }
        self.list.len() != before
    }

    pub fn remove(&mut self, name: &str) {
        self.list.retain(|g| g.name != name);
    }

    /// Missing: none yet.
    pub fn load(path: &Path) -> io::Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                toml::from_str(&text).map_err(|e| io::Error::new(ErrorKind::InvalidData, e))
            }
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e),
        }
    }

    /// Written whole, through a temporary file (a crash leaves the old one).
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

    #[test]
    fn parse_trims_dedups_and_drops_default() {
        assert_eq!(parse(" web, db ,web,, default"), ["web", "db"]);
        assert!(parse("").is_empty());
    }

    #[test]
    fn ensure_hands_out_colors_and_round_trips() {
        let mut g = Groups::default();
        assert!(g.ensure(&["a".into(), "b".into()]));
        assert!(!g.ensure(&["a".into()]));
        assert_eq!(g.color("a"), PALETTE[0]);
        assert_eq!(g.color("b"), PALETTE[1]);
        assert_eq!(g.color(DEFAULT), PALETTE[7]);
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("groups.toml");
        g.save(&p).unwrap();
        assert_eq!(Groups::load(&p).unwrap(), g);
        g.remove("a");
        assert_eq!(g.list.len(), 1);
        assert_eq!(
            Groups::load(&dir.path().join("none")).unwrap(),
            Groups::default()
        );
    }
}
