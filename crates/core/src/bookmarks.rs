//! SFTP bookmarks: folders to jump to, per place (`local`, or a server's host), in
//! `~/.config/shagshell/bookmarks.toml`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

/// The key of the local panel's bookmarks.
pub const LOCAL: &str = "local";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Bookmarks(BTreeMap<String, Vec<String>>);

pub fn default_path() -> PathBuf {
    crate::session::store::default_root().parent().map_or_else(
        || PathBuf::from("bookmarks.toml"),
        |p| p.join("bookmarks.toml"),
    )
}

impl Bookmarks {
    pub fn of(&self, place: &str) -> &[String] {
        self.0.get(place).map_or(&[], Vec::as_slice)
    }

    /// Added at the end; one already there stays where it is. `true`: it was new.
    pub fn add(&mut self, place: &str, folder: &str) -> bool {
        let list = self.0.entry(place.to_string()).or_default();
        if list.iter().any(|f| f == folder) {
            return false;
        }
        list.push(folder.to_string());
        true
    }

    pub fn remove(&mut self, place: &str, folder: &str) {
        if let Some(list) = self.0.get_mut(place) {
            list.retain(|f| f != folder);
            if list.is_empty() {
                self.0.remove(place);
            }
        }
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
    fn add_remove_and_round_trip() {
        let mut b = Bookmarks::default();
        assert!(b.add("srv", "/var/www"));
        assert!(!b.add("srv", "/var/www"));
        assert!(b.add("srv", "/etc"));
        assert!(b.add(LOCAL, "/home/u"));
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sub/bookmarks.toml");
        b.save(&p).unwrap();
        let got = Bookmarks::load(&p).unwrap();
        assert_eq!(got, b);
        assert_eq!(got.of("srv"), ["/var/www", "/etc"]);
        let mut got = got;
        got.remove("srv", "/var/www");
        got.remove(LOCAL, "/home/u");
        assert_eq!(got.of("srv"), ["/etc"]);
        assert!(got.of(LOCAL).is_empty());
        assert_eq!(
            Bookmarks::load(&dir.path().join("none")).unwrap(),
            Bookmarks::default()
        );
    }
}
