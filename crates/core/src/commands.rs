//! Command Manager: saved commands in folders (`commands.toml`), sent to a terminal with a
//! double click. Folders come before commands in a folder, as in SecureCRT.

use crate::buttonbar::Target;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Command {
    /// Empty: the first line of the text is shown.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub label: String,
    /// What is sent (`core::macros` escapes; a real line break is Enter).
    pub text: String,
    #[serde(skip_serializing_if = "is_current")]
    pub target: Target,
}

fn is_current(t: &Target) -> bool {
    *t == Target::Current
}

impl Command {
    /// The name in the tree.
    pub fn title(&self) -> String {
        let l = self.label.trim();
        if !l.is_empty() {
            return l.to_string();
        }
        let line = self.text.lines().next().unwrap_or("").trim();
        line.chars().take(48).collect()
    }

    /// The text as bytes to send: line breaks are Enter, then the usual escapes.
    pub fn to_send(&self) -> Vec<crate::macros::Piece> {
        crate::macros::parse(&self.text.replace("\r\n", "\r").replace('\n', "\r"))
    }
}

/// A folder; the one at the top has no name.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Folder {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(rename = "folder", skip_serializing_if = "Vec::is_empty")]
    pub folders: Vec<Folder>,
    #[serde(rename = "command", skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<Command>,
}

/// Where something is: the indices of the folders down from the top, and a command in the last.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Addr {
    pub folders: Vec<usize>,
    pub command: Option<usize>,
}

/// A line of the tree on screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub addr: Addr,
    pub depth: usize,
    pub title: String,
    pub is_folder: bool,
    /// The names down from the top: what «open» is remembered by.
    pub key: Vec<String>,
    pub open: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Empty,
    Taken,
    NoSuch,
}

impl Folder {
    fn at(&self, path: &[usize]) -> Option<&Folder> {
        path.iter().try_fold(self, |f, &i| f.folders.get(i))
    }

    fn at_mut(&mut self, path: &[usize]) -> Option<&mut Folder> {
        path.iter().try_fold(self, |f, &i| f.folders.get_mut(i))
    }

    pub fn folder(&self, path: &[usize]) -> Option<&Folder> {
        self.at(path)
    }

    pub fn command(&self, a: &Addr) -> Option<&Command> {
        self.at(&a.folders)?.commands.get(a.command?)
    }

    /// The tree as lines: folders first, then commands. With a `filter` only what matches and
    /// the folders it is in, all open.
    pub fn rows(&self, filter: &str, open: &HashSet<Vec<String>>) -> Vec<Row> {
        let filter = filter.trim().to_lowercase();
        let mut out = Vec::new();
        self.collect(&mut Vec::new(), &mut Vec::new(), 0, &filter, open, &mut out);
        out
    }

    /// Something in here (or the folder's own name) matches.
    fn matches(&self, filter: &str) -> bool {
        filter.is_empty()
            || self.name.to_lowercase().contains(filter)
            || self.commands.iter().any(|c| c.hit(filter))
            || self.folders.iter().any(|f| f.matches(filter))
    }

    fn collect(
        &self,
        path: &mut Vec<usize>,
        key: &mut Vec<String>,
        depth: usize,
        filter: &str,
        open: &HashSet<Vec<String>>,
        out: &mut Vec<Row>,
    ) {
        // A folder whose own name matches shows everything in it.
        let all = !filter.is_empty()
            && !self.name.is_empty()
            && self.name.to_lowercase().contains(filter);
        let filter = if all { "" } else { filter };
        for (i, f) in self.folders.iter().enumerate() {
            if !f.matches(filter) {
                continue;
            }
            path.push(i);
            key.push(f.name.clone());
            let is_open = !filter.is_empty() || all || open.contains(key);
            out.push(Row {
                addr: Addr {
                    folders: path.clone(),
                    command: None,
                },
                depth,
                title: f.name.clone(),
                is_folder: true,
                key: key.clone(),
                open: is_open,
            });
            if is_open {
                f.collect(path, key, depth + 1, filter, open, out);
            }
            key.pop();
            path.pop();
        }
        for (i, c) in self.commands.iter().enumerate() {
            if c.hit(filter) {
                out.push(Row {
                    addr: Addr {
                        folders: path.clone(),
                        command: Some(i),
                    },
                    depth,
                    title: c.title(),
                    is_folder: false,
                    key: key.clone(),
                    open: false,
                });
            }
        }
    }

    fn name_ok(&self, name: &str, except: Option<usize>) -> Result<(), Error> {
        if name.trim().is_empty() {
            return Err(Error::Empty);
        }
        let taken = (self.folders.iter().enumerate())
            .any(|(i, f)| Some(i) != except && f.name == name.trim());
        taken.then_some(()).map_or(Ok(()), |()| Err(Error::Taken))
    }

    /// A new folder in `parent`; its index.
    pub fn add_folder(&mut self, parent: &[usize], name: &str) -> Result<usize, Error> {
        let p = self.at_mut(parent).ok_or(Error::NoSuch)?;
        p.name_ok(name, None)?;
        p.folders.push(Folder {
            name: name.trim().to_string(),
            ..Default::default()
        });
        Ok(p.folders.len() - 1)
    }

    pub fn rename_folder(&mut self, path: &[usize], name: &str) -> Result<(), Error> {
        let (last, parent) = path.split_last().ok_or(Error::NoSuch)?;
        let p = self.at_mut(parent).ok_or(Error::NoSuch)?;
        p.name_ok(name, Some(*last))?;
        p.folders.get_mut(*last).ok_or(Error::NoSuch)?.name = name.trim().to_string();
        Ok(())
    }

    /// A command in `parent` (`at`: replacing that one); its index.
    pub fn put_command(
        &mut self,
        parent: &[usize],
        at: Option<usize>,
        c: Command,
    ) -> Result<usize, Error> {
        if c.text.trim().is_empty() {
            return Err(Error::Empty);
        }
        let p = self.at_mut(parent).ok_or(Error::NoSuch)?;
        match at {
            Some(i) => {
                *p.commands.get_mut(i).ok_or(Error::NoSuch)? = c;
                Ok(i)
            }
            None => {
                p.commands.push(c);
                Ok(p.commands.len() - 1)
            }
        }
    }

    pub fn remove(&mut self, a: &Addr) {
        match a.command {
            Some(i) => {
                if let Some(p) = self.at_mut(&a.folders)
                    && i < p.commands.len()
                {
                    p.commands.remove(i);
                }
            }
            None => {
                if let Some((last, parent)) = a.folders.split_last()
                    && let Some(p) = self.at_mut(parent)
                    && *last < p.folders.len()
                {
                    p.folders.remove(*last);
                }
            }
        }
    }

    /// One place up (`-1`) or down (`1`) among its own kind; the address it has then.
    pub fn shift(&mut self, a: &Addr, by: isize) -> Addr {
        let mut now = a.clone();
        match a.command {
            Some(i) => {
                if let Some(p) = self.at_mut(&a.folders) {
                    let j = i.saturating_add_signed(by);
                    if i < p.commands.len() && j < p.commands.len() {
                        p.commands.swap(i, j);
                        now.command = Some(j);
                    }
                }
            }
            None => {
                if let Some((last, parent)) = a.folders.split_last()
                    && let Some(p) = self.at_mut(parent)
                {
                    let j = last.saturating_add_signed(by);
                    if *last < p.folders.len() && j < p.folders.len() {
                        p.folders.swap(*last, j);
                        now.folders = [parent, &[j]].concat();
                    }
                }
            }
        }
        now
    }

    /// Takes in `other`: folders by name (inside, the same), commands not there already (same
    /// text). How many commands were added.
    pub fn merge(&mut self, other: Folder) -> usize {
        let mut added = 0;
        for f in other.folders {
            match self.folders.iter_mut().find(|m| m.name == f.name) {
                Some(m) => added += m.merge(f),
                None => {
                    added += f.count();
                    self.folders.push(f);
                }
            }
        }
        for c in other.commands {
            if !self.commands.iter().any(|m| m.text == c.text) {
                self.commands.push(c);
                added += 1;
            }
        }
        added
    }

    /// All commands in here and below.
    pub fn count(&self) -> usize {
        self.commands.len() + self.folders.iter().map(Folder::count).sum::<usize>()
    }

    pub fn default_path() -> PathBuf {
        crate::buttonbar::default_path().with_file_name("commands.toml")
    }

    /// Missing: empty. A broken file is an error, never an empty tree to be written back.
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

impl Command {
    fn hit(&self, filter: &str) -> bool {
        filter.is_empty()
            || self.label.to_lowercase().contains(filter)
            || self.text.to_lowercase().contains(filter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::macros::Piece;

    fn cmd(label: &str, text: &str) -> Command {
        Command {
            label: label.into(),
            text: text.into(),
            target: Target::Current,
        }
    }

    fn sample() -> Folder {
        let mut t = Folder::default();
        let a = t.add_folder(&[], "web").unwrap();
        t.add_folder(&[a], "nginx").unwrap();
        t.put_command(&[a], None, cmd("", "systemctl status nginx\\r"))
            .unwrap();
        t.put_command(&[a, 0], None, cmd("reload", "nginx -s reload\\r"))
            .unwrap();
        t.put_command(&[], None, cmd("up", "uptime\\r")).unwrap();
        t
    }

    fn titles(rows: &[Row]) -> Vec<(usize, &str)> {
        rows.iter().map(|r| (r.depth, r.title.as_str())).collect()
    }

    #[test]
    fn rows_folders_first_and_only_open_ones_show_inside() {
        let t = sample();
        let mut open = HashSet::new();
        assert_eq!(titles(&t.rows("", &open)), [(0, "web"), (0, "up")]);
        open.insert(vec!["web".to_string()]);
        assert_eq!(
            titles(&t.rows("", &open)),
            [
                (0, "web"),
                (1, "nginx"),
                (1, "systemctl status nginx\\r"),
                (0, "up")
            ]
        );
    }

    #[test]
    fn filter_shows_matches_and_their_folders_open() {
        let t = sample();
        let rows = t.rows("RELOAD", &HashSet::new());
        assert_eq!(titles(&rows), [(0, "web"), (1, "nginx"), (2, "reload")]);
        assert_eq!(t.command(&rows[2].addr).unwrap().text, "nginx -s reload\\r");
        assert!(t.rows("zzz", &HashSet::new()).is_empty());
    }

    #[test]
    fn a_folders_own_name_finds_it_with_everything_in_it() {
        let t = sample();
        let rows = t.rows("nginx", &HashSet::new());
        // `nginx` is a folder name here, and a text in `web`'s first command
        assert_eq!(
            titles(&rows),
            [
                (0, "web"),
                (1, "nginx"),
                (2, "reload"),
                (1, "systemctl status nginx\\r")
            ]
        );
        let only = t.rows("web", &HashSet::new());
        assert_eq!(only.len(), 4); // the folder and all of it
    }

    #[test]
    fn names_are_checked() {
        let mut t = sample();
        assert_eq!(t.add_folder(&[], " "), Err(Error::Empty));
        assert_eq!(t.add_folder(&[], "web"), Err(Error::Taken));
        assert!(t.add_folder(&[0], "web").is_ok()); // another level
        assert_eq!(t.rename_folder(&[0], "web"), Ok(())); // itself is no clash
        assert!(t.add_folder(&[], "db").is_ok());
        assert_eq!(t.rename_folder(&[1], "web"), Err(Error::Taken));
        assert_eq!(t.put_command(&[], None, cmd("x", "  ")), Err(Error::Empty));
        assert_eq!(t.add_folder(&[9], "x"), Err(Error::NoSuch));
    }

    #[test]
    fn edit_remove_shift() {
        let mut t = sample();
        t.put_command(&[], Some(0), cmd("up2", "uptime -p\\r"))
            .unwrap();
        assert_eq!(t.commands[0].label, "up2");
        let a = Addr {
            folders: vec![],
            command: Some(0),
        };
        t.put_command(&[], None, cmd("second", "w\\r")).unwrap();
        let now = t.shift(&a, 1);
        assert_eq!(
            (t.commands[1].label.as_str(), now.command),
            ("up2", Some(1))
        );
        assert_eq!(t.shift(&now, 1), now); // at the end already
        t.remove(&now);
        assert_eq!(t.commands.len(), 1);
        t.remove(&Addr {
            folders: vec![0, 0],
            command: None,
        }); // a folder
        assert!(t.folders[0].folders.is_empty());
        t.remove(&Addr {
            folders: vec![5],
            command: None,
        }); // nothing: no panic
    }

    #[test]
    fn what_is_sent() {
        assert_eq!(
            cmd("", "a\r\nb\\r\nc").to_send(),
            [Piece::Bytes(b"a\rb\r\rc".to_vec())]
        );
        assert_eq!(cmd("", "x").title(), "x");
        assert_eq!(cmd("", "first\nsecond").title(), "first");
    }

    #[test]
    fn merge_does_not_duplicate() {
        let mut t = sample();
        let n = t.merge(sample());
        assert_eq!(n, 0);
        let mut other = sample();
        other.put_command(&[0], None, cmd("", "new\\r")).unwrap();
        other.add_folder(&[], "fresh").unwrap();
        assert_eq!(t.merge(other), 1);
        assert_eq!(t.count(), 4);
    }

    #[test]
    fn file_round_trip_and_broken() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x/commands.toml");
        let t = sample();
        t.save(&p).unwrap();
        let text = std::fs::read_to_string(&p).unwrap();
        assert!(
            text.contains("[[folder]]") && text.contains("[[command]]"),
            "{text}"
        );
        assert_eq!(Folder::load(&p).unwrap(), t);
        assert_eq!(
            Folder::load(&dir.path().join("none")).unwrap(),
            Folder::default()
        );
        std::fs::write(&p, "[[folder]\nname=").unwrap();
        assert!(Folder::load(&p).is_err());
    }
}
