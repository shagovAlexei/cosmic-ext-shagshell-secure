//! Credential sets (SecureCRT's Credentials Manager): a name and a user in
//! `~/.config/shagshell/credentials.toml`, the password in the keyring (`kind=credential`).
//! Sessions refer to a set by name; change it once, every session follows.

use crate::session::Session;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Set {
    pub user: String,
    /// Fields this version doesn't know, written back unchanged.
    #[serde(flatten)]
    pub rest: toml::Table,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Credentials(pub BTreeMap<String, Set>);

pub fn default_path() -> PathBuf {
    crate::session::store::default_root().parent().map_or_else(
        || PathBuf::from("credentials.toml"),
        |p| p.join("credentials.toml"),
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PutError {
    Name(crate::session::store::NameError),
    Taken,
}

impl Credentials {
    /// Add set `name` (`old` = None) or change set `old` (renamed if `name` differs): checked, not
    /// written. The sessions and the keyring follow a rename in the caller.
    pub fn put(&mut self, old: Option<&str>, name: &str, user: &str) -> Result<(), PutError> {
        crate::session::store::valid_name(name, false).map_err(PutError::Name)?;
        if old != Some(name) && self.0.contains_key(name) {
            return Err(PutError::Taken);
        }
        let mut set = old.and_then(|o| self.0.remove(o)).unwrap_or_default();
        set.user = user.to_string();
        self.0.insert(name.to_string(), set);
        Ok(())
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

    /// A temp file next to it, then rename (never half written).
    pub fn save(&self, path: &Path) -> io::Result<()> {
        let dir = path.parent().ok_or(ErrorKind::InvalidInput)?;
        std::fs::create_dir_all(dir)?;
        let text = toml::to_string(self).map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;
        let tmp = dir.join(format!(".credentials.{}.tmp", std::process::id()));
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path).inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp);
        })
    }

    /// The session as it connects: the set's user (a set that is gone: the session's own).
    pub fn resolve(&self, s: &Session) -> Session {
        let mut s = s.clone();
        if let Some(set) = s.credential.as_ref().and_then(|c| self.0.get(c))
            && !set.user.is_empty()
        {
            s.user = Some(set.user.clone());
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_keeps_unknown() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("c/credentials.toml");
        assert_eq!(Credentials::load(&p).unwrap(), Credentials::default());
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, "[lab-admin]\nuser = \"admin\"\nfuture = 1\n").unwrap();
        let c = Credentials::load(&p).unwrap();
        assert_eq!(c.0["lab-admin"].user, "admin");
        c.save(&p).unwrap();
        let back = std::fs::read_to_string(&p).unwrap();
        assert!(
            back.contains("future = 1") && back.contains("[lab-admin]"),
            "{back}"
        );
    }

    #[test]
    fn put_add_rename_taken() {
        use crate::session::store::NameError;
        let mut c = Credentials::default();
        c.put(None, "adm", "admin").unwrap();
        assert_eq!(c.put(None, "adm", "x"), Err(PutError::Taken));
        assert_eq!(
            c.put(None, "a/b", "x"),
            Err(PutError::Name(NameError::Slash))
        );
        assert_eq!(c.put(None, "", "x"), Err(PutError::Name(NameError::Empty)));
        c.put(None, "ro", "reader").unwrap();
        assert_eq!(c.put(Some("adm"), "ro", "x"), Err(PutError::Taken));
        c.put(Some("adm"), "adm", "root").unwrap(); // same name: just the user
        c.put(Some("adm"), "admin", "root").unwrap();
        assert_eq!(c.0.keys().collect::<Vec<_>>(), ["admin", "ro"]);
        assert_eq!(c.0["admin"].user, "root");
    }

    #[test]
    fn resolve_user_from_set() {
        let mut c = Credentials::default();
        c.0.insert(
            "adm".into(),
            Set {
                user: "admin".into(),
                ..Default::default()
            },
        );
        let s = Session {
            user: Some("me".into()),
            credential: Some("adm".into()),
            ..Default::default()
        };
        assert_eq!(c.resolve(&s).user.as_deref(), Some("admin"));
    }

    #[test]
    fn resolve_missing_set() {
        let s = Session {
            user: Some("me".into()),
            credential: Some("gone".into()),
            ..Default::default()
        };
        assert_eq!(
            Credentials::default().resolve(&s).user.as_deref(),
            Some("me")
        );
    }
}
