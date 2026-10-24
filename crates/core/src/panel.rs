//! One file panel of the SFTP tab (local or server): the listing, the cursor, the marks.
//! After shagoff-commander's `crates/core/src/panel.rs` / `format.rs`, cut down to what a
//! transfer needs. Names are `OsString`: a server's may not be UTF-8.

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// The synthetic «go up» row.
pub const PARENT: &str = "..";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: OsString,
    pub dir: bool,
    pub size: u64,
    /// Seconds since the epoch.
    pub mtime: Option<i64>,
}

impl Entry {
    pub fn parent() -> Self {
        Self {
            name: PARENT.into(),
            dir: true,
            size: 0,
            mtime: None,
        }
    }

    pub fn is_parent(&self) -> bool {
        self.name == PARENT
    }
}

#[derive(Clone, Debug, Default)]
pub struct Panel {
    cwd: PathBuf,
    /// `..` first (unless at `/`), folders, then files, by name.
    entries: Vec<Entry>,
    cursor: usize,
    marked: BTreeSet<OsString>,
}

/// Folders first, then by name ignoring case (as a file manager does).
fn order(a: &Entry, b: &Entry) -> std::cmp::Ordering {
    let key = |e: &Entry| e.name.to_string_lossy().to_lowercase();
    b.dir
        .cmp(&a.dir)
        .then_with(|| key(a).cmp(&key(b)))
        .then_with(|| a.name.cmp(&b.name))
}

impl Panel {
    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// A new listing of `cwd`; the cursor on `focus` if there (the folder just left by `..`),
    /// else on the same place. Marks of names no longer there go.
    pub fn set_listing(&mut self, cwd: PathBuf, mut entries: Vec<Entry>, focus: Option<&OsStr>) {
        entries.retain(|e| !e.is_parent());
        entries.sort_by(order);
        if cwd.parent().is_some() {
            entries.insert(0, Entry::parent());
        }
        let same_dir = cwd == self.cwd;
        let at = focus.and_then(|f| entries.iter().position(|e| e.name == f));
        self.cursor = match (at, same_dir) {
            (Some(i), _) => i,
            (None, true) => self.cursor.min(entries.len().saturating_sub(1)),
            (None, false) => 0,
        };
        if !same_dir {
            self.marked.clear();
        }
        self.marked.retain(|m| entries.iter().any(|e| e.name == *m));
        self.cwd = cwd;
        self.entries = entries;
    }

    pub fn current(&self) -> Option<&Entry> {
        self.entries.get(self.cursor)
    }

    pub fn set_cursor(&mut self, i: usize) {
        self.cursor = i.min(self.entries.len().saturating_sub(1));
    }

    pub fn move_cursor(&mut self, delta: isize) {
        let i = self.cursor.saturating_add_signed(delta);
        self.set_cursor(i);
    }

    pub fn is_marked(&self, e: &Entry) -> bool {
        self.marked.contains(&e.name)
    }

    /// Insert / Space: the current row (not `..`) flips, the cursor goes down.
    pub fn toggle_mark(&mut self) {
        if let Some(e) = self.current().filter(|e| !e.is_parent()).cloned()
            && !self.marked.remove(&e.name)
        {
            self.marked.insert(e.name);
        }
        self.move_cursor(1);
    }

    pub fn set_mark(&mut self, i: usize, on: bool) {
        if let Some(e) = self.entries.get(i).filter(|e| !e.is_parent()) {
            match on {
                true => self.marked.insert(e.name.clone()),
                false => self.marked.remove(&e.name),
            };
        }
    }

    /// Ctrl+A: all, or none if all were.
    pub fn mark_all(&mut self) {
        let all: BTreeSet<OsString> = (self.entries.iter())
            .filter(|e| !e.is_parent())
            .map(|e| e.name.clone())
            .collect();
        self.marked = match self.marked == all {
            true => BTreeSet::new(),
            false => all,
        };
    }

    pub fn clear_marks(&mut self) {
        self.marked.clear();
    }

    /// What F5 / F6 / F8 act on: the marked rows, else the current one; never `..`.
    pub fn targets(&self) -> Vec<&Entry> {
        let marked: Vec<&Entry> = self.entries.iter().filter(|e| self.is_marked(e)).collect();
        match marked.is_empty() {
            false => marked,
            true => self
                .current()
                .filter(|e| !e.is_parent())
                .into_iter()
                .collect(),
        }
    }
}

/// `1 234 567` bytes; with a unit from a megabyte on (`1.2 MB`).
pub fn size_text(n: u64) -> String {
    const UNITS: [&str; 5] = ["KB", "MB", "GB", "TB", "PB"];
    if n < 1_000_000 {
        let digits = n.to_string();
        let mut out = String::new();
        for (i, c) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i).is_multiple_of(3) {
                out.push(' ');
            }
            out.push(c);
        }
        return out;
    }
    let mut v = n as f64 / 1024.0;
    let mut unit = 0;
    while v >= 1024.0 && unit < UNITS.len() - 1 {
        v /= 1024.0;
        unit += 1;
    }
    format!("{v:.1} {}", UNITS[unit])
}

/// `2026-10-23 11:09` in the local time zone.
pub fn time_text(secs: i64) -> String {
    jiff::Timestamp::from_second(secs)
        .map(|t| {
            t.to_zoned(jiff::tz::TimeZone::system())
                .strftime("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(name: &str, dir: bool) -> Entry {
        Entry {
            name: name.into(),
            dir,
            size: 1,
            mtime: None,
        }
    }

    fn names(p: &Panel) -> Vec<String> {
        p.entries()
            .iter()
            .map(|e| e.name.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn folders_first_by_name_and_parent() {
        let mut p = Panel::default();
        let list = vec![
            e("b.txt", false),
            e("Zdir", true),
            e("A.txt", false),
            e("adir", true),
        ];
        p.set_listing("/home".into(), list.clone(), None);
        assert_eq!(names(&p), ["..", "adir", "Zdir", "A.txt", "b.txt"]);
        p.set_listing("/".into(), list, None);
        assert_eq!(names(&p)[0], "adir"); // no `..` at the root
    }

    #[test]
    fn focus_after_going_up() {
        let mut p = Panel::default();
        p.set_listing("/home/u".into(), vec![e("x", false)], None);
        let list = vec![e("u", true), e("a", true)];
        p.set_listing("/home".into(), list, Some(OsStr::new("u")));
        assert_eq!(p.current().unwrap().name, "u");
    }

    #[test]
    fn targets_marked_else_current_never_parent() {
        let mut p = Panel::default();
        p.set_listing("/h".into(), vec![e("a", false), e("b", false)], None);
        assert!(p.targets().is_empty()); // on `..`
        p.toggle_mark(); // `..` can't be marked; the cursor moves on
        assert_eq!(p.current().unwrap().name, "a");
        assert_eq!(p.targets()[0].name, "a");
        p.toggle_mark();
        p.toggle_mark();
        let t: Vec<_> = p.targets().iter().map(|e| e.name.clone()).collect();
        assert_eq!(t, ["a", "b"]);
        p.mark_all(); // all were marked: none now
        assert_eq!(p.targets()[0].name, "b"); // the cursor's
    }

    #[test]
    fn relisting_keeps_marks_of_names_still_there() {
        let mut p = Panel::default();
        p.set_listing("/h".into(), vec![e("a", false), e("b", false)], None);
        p.set_mark(1, true);
        p.set_mark(2, true);
        p.set_listing("/h".into(), vec![e("a", false)], None);
        assert_eq!(p.targets().len(), 1);
        p.set_listing("/other".into(), vec![e("a", false)], None);
        assert_eq!(p.targets().len(), 0); // another folder: no marks, cursor on `..`
    }

    #[test]
    fn sizes() {
        assert_eq!(size_text(0), "0");
        assert_eq!(size_text(1234), "1 234");
        assert_eq!(size_text(999_999), "999 999");
        assert_eq!(size_text(5 << 20), "5.0 MB");
        assert_eq!(size_text(3 << 30), "3.0 GB");
    }
}
