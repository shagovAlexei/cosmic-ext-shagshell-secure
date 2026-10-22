//! The session tree on disk. An id is the path under the root without `.toml` (`lab/sw-core`);
//! `Default.toml` at the root is the template for new sessions, not shown in the tree.

use super::Session;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

const EXT: &str = "toml";
/// The template's id.
pub const DEFAULT: &str = "Default";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameError {
    Empty,
    Slash,
    Dot,
    Reserved,
}

/// A name for a session or folder (one path segment).
pub fn valid_name(name: &str, at_root: bool) -> Result<(), NameError> {
    if name.trim().is_empty() {
        Err(NameError::Empty)
    } else if name.contains('/') {
        Err(NameError::Slash)
    } else if name.starts_with('.') {
        Err(NameError::Dot)
    } else if at_root && name == DEFAULT {
        Err(NameError::Reserved)
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Folder { name: String, children: Vec<Node> },
    Session { name: String, session: Session },
}

impl Node {
    pub fn name(&self) -> &str {
        match self {
            Node::Folder { name, .. } | Node::Session { name, .. } => name,
        }
    }
}

pub struct Store {
    pub root: PathBuf,
}

/// `$XDG_CONFIG_HOME/shagshell/sessions` (`~/.config/…` without it).
pub fn default_root() -> PathBuf {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::home_dir().map(|h| h.join(".config")))
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    config.join("shagshell").join("sessions")
}

impl Store {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// The whole tree: folders first, then sessions, each by name ignoring case. A file that
    /// doesn't parse is left out (and logged), the rest still loads.
    pub fn load(&self) -> Vec<Node> {
        load_dir(&self.root, true)
    }

    pub fn get(&self, id: &str) -> io::Result<Session> {
        let text = std::fs::read_to_string(self.file(id))?;
        toml::from_str(&text).map_err(|e| io::Error::new(ErrorKind::InvalidData, e))
    }

    /// A new session `name` in `folder` ("" = root), from the Default template; its id.
    pub fn create(&self, folder: &str, name: &str, host: &str) -> io::Result<String> {
        let mut s = self.template();
        s.host = host.to_string();
        self.add(folder, name, &s)
    }

    /// What a new session starts as: Default.toml (missing or unreadable: empty).
    pub fn template(&self) -> Session {
        self.get(DEFAULT).unwrap_or_default()
    }

    /// A new session `name` in `folder` written as `s`, in one write; its id.
    pub fn add(&self, folder: &str, name: &str, s: &Session) -> io::Result<String> {
        check_name(name, folder.is_empty())?;
        let id = join(folder, name);
        if self.exists(&id) {
            return Err(ErrorKind::AlreadyExists.into());
        }
        self.save(&id, s)?;
        Ok(id)
    }

    /// Write a session: a temp file next to it, then rename (never a half-written file).
    pub fn save(&self, id: &str, s: &Session) -> io::Result<()> {
        let path = self.file(id);
        let dir = path.parent().ok_or(ErrorKind::InvalidInput)?;
        std::fs::create_dir_all(dir)?;
        let text = toml::to_string(s).map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;
        let tmp = dir.join(format!(".{}.{}.tmp", file_name(id), std::process::id()));
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, &path).inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp);
        })
    }

    pub fn new_folder(&self, parent: &str, name: &str) -> io::Result<String> {
        check_name(name, parent.is_empty())?;
        let id = join(parent, name);
        if self.exists(&id) {
            return Err(ErrorKind::AlreadyExists.into());
        }
        std::fs::create_dir_all(self.root.join(&id))?;
        Ok(id)
    }

    /// Rename a session or folder in place; the new id.
    pub fn rename(&self, id: &str, new_name: &str) -> io::Result<String> {
        let parent = parent(id);
        check_name(new_name, parent.is_empty())?;
        let new = join(parent, new_name);
        self.relocate(id, &new)
    }

    /// Move a session or folder into `folder` ("" = root); the new id. Not into itself or its
    /// own subfolder; not over a taken name.
    pub fn move_to(&self, id: &str, folder: &str) -> io::Result<String> {
        if folder == id || folder.starts_with(&format!("{id}/")) {
            return Err(ErrorKind::InvalidInput.into());
        }
        let name = file_name(id);
        check_name(name, folder.is_empty())?;
        self.relocate(id, &join(folder, name))
    }

    /// Delete a session, or a folder with everything in it.
    pub fn delete(&self, id: &str) -> io::Result<()> {
        match self.is_folder(id) {
            true => std::fs::remove_dir_all(self.root.join(id)),
            false => std::fs::remove_file(self.file(id)),
        }
    }

    /// Sessions in a folder, recursively (1 for a session).
    pub fn count_sessions(&self, id: &str) -> usize {
        fn count(nodes: &[Node]) -> usize {
            nodes
                .iter()
                .map(|n| match n {
                    Node::Folder { children, .. } => count(children),
                    Node::Session { .. } => 1,
                })
                .sum()
        }
        match self.is_folder(id) {
            true => count(&load_dir(&self.root.join(id), false)),
            false => usize::from(self.file(id).is_file()),
        }
    }

    fn file(&self, id: &str) -> PathBuf {
        self.root.join(format!("{id}.{EXT}"))
    }

    /// A folder on disk (shown or not).
    pub fn is_folder(&self, id: &str) -> bool {
        !id.is_empty() && self.root.join(id).is_dir()
    }

    /// A session or folder by that id (a folder and a session never share one).
    fn exists(&self, id: &str) -> bool {
        self.file(id).exists() || self.is_folder(id)
    }

    fn relocate(&self, from: &str, to: &str) -> io::Result<String> {
        if from == to {
            return Ok(to.to_string());
        }
        if self.exists(to) {
            return Err(ErrorKind::AlreadyExists.into());
        }
        let (src, dst) = match self.is_folder(from) {
            true => (self.root.join(from), self.root.join(to)),
            false => (self.file(from), self.file(to)),
        };
        if let Some(dir) = dst.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::rename(src, dst)?;
        Ok(to.to_string())
    }
}

fn check_name(name: &str, at_root: bool) -> io::Result<()> {
    // InvalidFilename, not InvalidInput: that one says «into itself» (`move_to`).
    valid_name(name, at_root)
        .map_err(|e| io::Error::new(ErrorKind::InvalidFilename, format!("{e:?}")))
}

fn join(folder: &str, name: &str) -> String {
    match folder {
        "" => name.to_string(),
        f => format!("{f}/{name}"),
    }
}

/// `id` after `old` was renamed / moved to `new`: itself or something under it moves along;
/// anything else (`lab/web2` when `lab/web` moved) is `None`.
pub fn retarget(id: &str, old: &str, new: &str) -> Option<String> {
    let rest = id.strip_prefix(old)?;
    (rest.is_empty() || rest.starts_with('/')).then(|| format!("{new}{rest}"))
}

/// A session's or folder's own name: the id's last segment.
pub fn name_of(id: &str) -> &str {
    id.rsplit_once('/').map_or(id, |(_, n)| n)
}

fn parent(id: &str) -> &str {
    id.rsplit_once('/').map_or("", |(p, _)| p)
}

fn file_name(id: &str) -> &str {
    id.rsplit_once('/').map_or(id, |(_, n)| n)
}

fn load_dir(dir: &Path, root: bool) -> Vec<Node> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut folders = Vec::new();
    let mut sessions = Vec::new();
    for e in entries.flatten() {
        let path = e.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue; // not UTF-8: not one of ours
        };
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            let children = load_dir(&path, false);
            folders.push(Node::Folder {
                name: name.to_string(),
                children,
            });
            continue;
        }
        let Some(stem) = name.strip_suffix(&format!(".{EXT}")) else {
            continue;
        };
        if root && stem == DEFAULT {
            continue;
        }
        let parsed = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|t| toml::from_str::<Session>(&t).map_err(|e| e.to_string()));
        match parsed {
            Ok(session) => sessions.push(Node::Session {
                name: stem.to_string(),
                session,
            }),
            Err(e) => log::warn!("session {}: {e}", path.display()),
        }
    }
    let key = |n: &Node| n.name().to_lowercase();
    folders.sort_by_key(key);
    sessions.sort_by_key(key);
    folders.extend(sessions);
    folders
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, Store) {
        let d = tempfile::tempdir().unwrap();
        let s = Store::new(d.path().join("sessions"));
        (d, s)
    }

    fn host(s: &str) -> Session {
        Session {
            host: s.into(),
            ..Default::default()
        }
    }

    #[test]
    fn names_valid() {
        assert_eq!(valid_name("web-01", true), Ok(()));
        assert_eq!(valid_name("Default", false), Ok(()));
        assert_eq!(valid_name("сервер 1", true), Ok(()));
    }

    #[test]
    fn names_invalid() {
        assert_eq!(valid_name("", true), Err(NameError::Empty));
        assert_eq!(valid_name("  ", true), Err(NameError::Empty));
        assert_eq!(valid_name("a/b", true), Err(NameError::Slash));
        assert_eq!(valid_name(".hidden", true), Err(NameError::Dot));
        assert_eq!(valid_name("Default", true), Err(NameError::Reserved));
    }

    #[test]
    fn retarget_follows_the_moved_one_only() {
        assert_eq!(
            retarget("lab/web", "lab/web", "prod/web").as_deref(),
            Some("prod/web")
        );
        assert_eq!(
            retarget("lab/a/b", "lab", "old").as_deref(),
            Some("old/a/b")
        );
        assert_eq!(retarget("lab/web2", "lab/web", "x"), None);
        assert_eq!(retarget("other", "lab", "x"), None);
    }

    #[test]
    fn add_writes_the_session_as_given() {
        let (_d, s) = store();
        let mut x = host("h");
        x.port = Some(2222);
        assert_eq!(s.add("lab", "a", &x).unwrap(), "lab/a");
        assert_eq!(s.get("lab/a").unwrap(), x);
        assert_eq!(
            s.add("lab", "a", &x).unwrap_err().kind(),
            ErrorKind::AlreadyExists
        );
        assert_eq!(
            s.add("", "Default", &x).unwrap_err().kind(),
            ErrorKind::InvalidFilename
        );
    }

    #[test]
    fn template_missing_or_broken_is_empty() {
        let (_d, s) = store();
        assert_eq!(s.template(), Session::default());
        std::fs::create_dir_all(&s.root).unwrap();
        std::fs::write(s.root.join("Default.toml"), "port = \"x").unwrap();
        assert_eq!(s.template(), Session::default());
    }

    #[test]
    fn unknown_fields_survive_a_save() {
        let (_d, s) = store();
        std::fs::create_dir_all(&s.root).unwrap();
        let text = "host = \"h\"\njump = \"bastion\"\n\n[terminal]\nfont = \"Mono\"\n";
        std::fs::write(s.root.join("x.toml"), text).unwrap();
        let mut x = s.get("x").unwrap();
        x.port = Some(1);
        s.save("x", &x).unwrap();
        let back = std::fs::read_to_string(s.root.join("x.toml")).unwrap();
        assert!(back.contains("jump = \"bastion\""), "{back}");
        assert!(
            back.contains("[terminal]") && back.contains("font = \"Mono\""),
            "{back}"
        );
        assert_eq!(s.get("x").unwrap(), x);
    }

    #[test]
    fn create_copies_default() {
        let (_d, s) = store();
        let mut def = host("");
        def.keepalive = Some(30);
        s.save(DEFAULT, &def).unwrap();
        let id = s.create("", "web", "10.0.0.1").unwrap();
        assert_eq!(id, "web");
        let got = s.get(&id).unwrap();
        assert_eq!((got.host.as_str(), got.keepalive), ("10.0.0.1", Some(30)));
    }

    #[test]
    fn create_duplicate() {
        let (_d, s) = store();
        s.create("lab", "a", "h").unwrap();
        let e = s.create("lab", "a", "h").unwrap_err();
        assert_eq!(e.kind(), ErrorKind::AlreadyExists);
    }

    #[test]
    fn load_sorted_and_skips_bad() {
        let (_d, s) = store();
        s.save(DEFAULT, &host("")).unwrap();
        s.save("C", &host("c")).unwrap();
        s.save("b", &host("b")).unwrap();
        s.new_folder("", "zeta").unwrap();
        std::fs::write(s.root.join("broken.toml"), "host = [").unwrap();
        let names: Vec<_> = s.load().iter().map(|n| n.name().to_string()).collect();
        assert_eq!(names, ["zeta", "b", "C"]);
    }

    #[test]
    fn unknown_fields_load() {
        let (_d, s) = store();
        std::fs::create_dir_all(&s.root).unwrap();
        std::fs::write(s.root.join("x.toml"), "host = \"h\"\nfuture = 1\n").unwrap();
        let x = s.get("x").unwrap();
        assert_eq!(x.host, "h");
        assert_eq!(x.rest.get("future"), Some(&toml::Value::Integer(1)));
    }

    #[test]
    fn rename_session_and_folder() {
        let (_d, s) = store();
        s.create("lab", "a", "h").unwrap();
        assert_eq!(s.rename("lab/a", "b").unwrap(), "lab/b");
        assert_eq!(s.rename("lab", "lab2").unwrap(), "lab2");
        assert_eq!(s.get("lab2/b").unwrap().host, "h");
    }

    #[test]
    fn regression_move_into_own_subfolder() {
        let (_d, s) = store();
        s.new_folder("", "a").unwrap();
        s.new_folder("a", "b").unwrap();
        for target in ["a", "a/b"] {
            let e = s.move_to("a", target).unwrap_err();
            assert_eq!(e.kind(), ErrorKind::InvalidInput, "{target}");
        }
        assert!(s.root.join("a/b").is_dir());
    }

    #[test]
    fn move_name_taken_untouched() {
        let (_d, s) = store();
        s.create("", "x", "root").unwrap();
        s.create("lab", "x", "lab").unwrap();
        let e = s.move_to("x", "lab").unwrap_err();
        assert_eq!(e.kind(), ErrorKind::AlreadyExists);
        assert_eq!(s.get("x").unwrap().host, "root");
        assert_eq!(s.get("lab/x").unwrap().host, "lab");
        assert_eq!(
            s.move_to("lab/x", "").unwrap_err().kind(),
            ErrorKind::AlreadyExists
        );
        s.new_folder("", "other").unwrap();
        assert_eq!(s.move_to("lab/x", "other").unwrap(), "other/x");
    }

    #[test]
    fn delete_folder_recursive() {
        let (_d, s) = store();
        s.create("lab", "a", "h").unwrap();
        s.create("lab/sub", "b", "h").unwrap();
        assert_eq!(s.count_sessions("lab"), 2);
        assert_eq!(s.count_sessions("lab/a"), 1);
        s.delete("lab").unwrap();
        assert!(s.load().is_empty());
    }

    #[test]
    fn save_is_atomic() {
        let (_d, s) = store();
        let mut x = host("h");
        x.extra = "-o A=1".into();
        s.save("lab/x", &x).unwrap();
        assert_eq!(s.get("lab/x").unwrap(), x);
        let left: Vec<_> = std::fs::read_dir(s.root.join("lab")).unwrap().collect();
        assert_eq!(left.len(), 1);
    }
}
