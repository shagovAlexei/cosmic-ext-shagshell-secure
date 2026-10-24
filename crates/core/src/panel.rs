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
    /// Permission bits (the server's: `rwxr-xr-x`).
    pub mode: Option<u32>,
}

impl Entry {
    pub fn parent() -> Self {
        Self {
            name: PARENT.into(),
            dir: true,
            size: 0,
            mtime: None,
            mode: None,
        }
    }

    pub fn is_parent(&self) -> bool {
        self.name == PARENT
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortKey {
    #[default]
    Name,
    Size,
    Date,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sort {
    pub key: SortKey,
    pub asc: bool,
}

impl Default for Sort {
    fn default() -> Self {
        Self {
            key: SortKey::Name,
            asc: true,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Panel {
    cwd: PathBuf,
    /// As the folder was read.
    raw: Vec<Entry>,
    sort: Sort,
    /// Files with a leading dot shown.
    show_hidden: bool,
    /// `..` first (unless at `/`), folders, then files, in `sort` order, minus hidden ones.
    entries: Vec<Entry>,
    cursor: usize,
    marked: BTreeSet<OsString>,
}

/// Case-insensitive, digit runs as numbers (`file2` before `file10`); never `Equal` for different
/// names.
pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let (x, y) = (a.as_bytes(), b.as_bytes());
    let (mut i, mut j) = (0, 0);
    loop {
        let (c, d) = match (x.get(i), y.get(j)) {
            (None, None) => return a.cmp(b),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(&c), Some(&d)) => (c, d),
        };
        let ord = if c.is_ascii_digit() && d.is_ascii_digit() {
            let (n, m) = (digits(x, &mut i), digits(y, &mut j));
            n.len().cmp(&m.len()).then_with(|| n.cmp(m))
        } else if c.is_ascii() && d.is_ascii() {
            i += 1;
            j += 1;
            c.to_ascii_lowercase().cmp(&d.to_ascii_lowercase())
        } else {
            let (Some(c), Some(d)) = (a[i..].chars().next(), b[j..].chars().next()) else {
                return a.cmp(b);
            };
            i += c.len_utf8();
            j += d.len_utf8();
            c.to_lowercase().cmp(d.to_lowercase())
        };
        if ord != Ordering::Equal {
            return ord;
        }
    }
}

/// The digit run at `*k` without leading zeros; `*k` moves past it.
fn digits<'a>(s: &'a [u8], k: &mut usize) -> &'a [u8] {
    let start = *k;
    while s.get(*k).is_some_and(u8::is_ascii_digit) {
        *k += 1;
    }
    let run = &s[start..*k];
    &run[run.iter().take_while(|&&b| b == b'0').count()..]
}

/// Folders first (in both directions), then by `sort`, ties by name.
fn order(sort: Sort) -> impl Fn(&Entry, &Entry) -> std::cmp::Ordering {
    move |a, b| {
        let name = |a: &Entry, b: &Entry| {
            natural_cmp(&a.name.to_string_lossy(), &b.name.to_string_lossy())
                .then_with(|| a.name.cmp(&b.name))
        };
        let by = match sort.key {
            SortKey::Name => name(a, b),
            SortKey::Size => a.size.cmp(&b.size).then_with(|| name(a, b)),
            SortKey::Date => a.mtime.cmp(&b.mtime).then_with(|| name(a, b)),
        };
        b.dir.cmp(&a.dir).then(match sort.asc {
            true => by,
            false => by.reverse(),
        })
    }
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

    pub fn sort(&self) -> Sort {
        self.sort
    }

    pub fn show_hidden(&self) -> bool {
        self.show_hidden
    }

    /// A click on a column's header: that column, ascending; the same again: descending.
    pub fn sort_by(&mut self, key: SortKey) {
        self.sort = match self.sort.key == key {
            true => Sort {
                key,
                asc: !self.sort.asc,
            },
            false => Sort { key, asc: true },
        };
        self.rebuild(None);
    }

    /// Ctrl+H: files with a leading dot shown or not.
    pub fn set_show_hidden(&mut self, on: bool) {
        self.show_hidden = on;
        self.rebuild(None);
    }

    /// A new listing of `cwd`; the cursor on `focus` if there (the folder just left by `..`),
    /// else on the same place. Marks of names no longer there go.
    pub fn set_listing(&mut self, cwd: PathBuf, mut entries: Vec<Entry>, focus: Option<&OsStr>) {
        entries.retain(|e| !e.is_parent());
        let same_dir = cwd == self.cwd;
        if !same_dir {
            self.marked.clear();
        }
        self.cwd = cwd;
        self.raw = entries;
        // Same folder: the cursor stays on its name if it can, else on the same row.
        let keep = match same_dir {
            true => self.current().map(|e| e.name.clone()),
            false => None,
        };
        let row = self.cursor;
        self.rebuild(focus.map(OsStr::to_os_string).or(keep.clone()));
        if focus.is_none()
            && same_dir
            && keep.is_none_or(|k| self.current().is_none_or(|e| e.name != k))
        {
            self.cursor = row.min(self.entries.len().saturating_sub(1));
        } else if !same_dir && focus.is_none_or(|f| self.current().is_none_or(|e| e.name != f)) {
            self.cursor = 0;
        }
    }

    /// `entries` from `raw`; the cursor to `name` if given and there, else kept in range.
    fn rebuild(&mut self, name: Option<OsString>) {
        let name = name.or_else(|| self.current().map(|e| e.name.clone()));
        let mut v: Vec<Entry> = (self.raw.iter())
            .filter(|e| self.show_hidden || !e.name.as_encoded_bytes().starts_with(b"."))
            .cloned()
            .collect();
        v.sort_by(order(self.sort));
        if self.cwd.parent().is_some() {
            v.insert(0, Entry::parent());
        }
        // A mark of a hidden name waits for it to come back (it is never acted on unseen:
        // `targets` goes by the rows shown).
        self.marked
            .retain(|m| self.raw.iter().any(|e| e.name == *m));
        self.entries = v;
        let at = name.and_then(|n| self.entries.iter().position(|e| e.name == n));
        self.cursor = at.unwrap_or(self.cursor.min(self.entries.len().saturating_sub(1)));
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

/// `drwxr-xr-x` for a mode (a link `l`, else folder `d` or `-`).
pub fn mode_text(mode: u32, dir: bool) -> String {
    let kind = match (mode & 0o170000, dir) {
        (0o120000, _) => 'l',
        (_, true) => 'd',
        _ => '-',
    };
    let mut s = String::from(kind);
    for (i, ch) in "rwxrwxrwx".chars().enumerate() {
        s.push(if mode & (0o400 >> i) != 0 { ch } else { '-' });
    }
    s
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
            mode: None,
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

    fn sized(name: &str, size: u64, dir: bool) -> Entry {
        Entry {
            size,
            ..e(name, dir)
        }
    }

    #[test]
    fn natural_order_and_columns() {
        assert!(natural_cmp("file2", "file10").is_lt());
        assert!(natural_cmp("File", "file").is_ne());
        let mut p = Panel::default();
        p.set_listing(
            "/h".into(),
            vec![
                sized("b", 5, false),
                sized("a10", 9, false),
                sized("a2", 1, false),
                sized("d", 0, true),
            ],
            None,
        );
        assert_eq!(names(&p), ["..", "d", "a2", "a10", "b"]);
        p.sort_by(SortKey::Size);
        assert_eq!(names(&p), ["..", "d", "a2", "b", "a10"]);
        p.sort_by(SortKey::Size); // again: descending, folders still first
        assert_eq!(names(&p), ["..", "d", "a10", "b", "a2"]);
        p.sort_by(SortKey::Name); // another column: ascending
        assert_eq!(names(&p), ["..", "d", "a2", "a10", "b"]);
    }

    #[test]
    fn hidden_files_toggle_and_cursor_stays_on_its_name() {
        let mut p = Panel::default();
        p.set_show_hidden(true);
        p.set_listing(
            "/h".into(),
            vec![e(".a", false), e("b", false), e("c", false)],
            None,
        );
        p.set_mark(1, true);
        p.set_cursor(3); // c
        p.set_show_hidden(false);
        assert_eq!(names(&p), ["..", "b", "c"]);
        assert_eq!(p.current().unwrap().name, "c");
        assert_eq!(p.targets().len(), 1); // a hidden mark is not acted on
        p.set_show_hidden(true);
        assert!(p.is_marked(&e(".a", false))); // back with its row
        assert_eq!(names(&p), ["..", ".a", "b", "c"]);
        assert_eq!(p.current().unwrap().name, "c");
    }

    #[test]
    fn mode_text_shows_rights() {
        assert_eq!(mode_text(0o755, true), "drwxr-xr-x");
        assert_eq!(mode_text(0o640, false), "-rw-r-----");
    }
}
