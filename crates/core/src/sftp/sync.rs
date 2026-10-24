//! Folder synchronisation (SecureFX's Synchronize Folders): two trees in, a list of what to do
//! out. Pure: the worker reads the trees, the app shows the list and queues the copies.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Times in SFTP v3 are whole seconds, and file systems round differently: files whose times
/// differ by no more than this are the same age.
pub const SLACK: i64 = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    /// Path under the synchronised folder.
    pub rel: PathBuf,
    pub size: u64,
    /// Seconds since the epoch.
    pub mtime: i64,
    pub is_dir: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Local → server.
    Up,
    /// Server → local.
    Down,
    /// The newer of two differing files goes both ways; nothing is deleted.
    Both,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    CopyUp,
    CopyDown,
    MkdirLocal,
    MkdirRemote,
    /// A file or a whole folder.
    DeleteLocal,
    DeleteRemote,
    /// Not decided by the program: a file and a folder of one name, or two files that differ in
    /// size but not in age.
    Conflict,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// Not there yet.
    New,
    /// The source is newer.
    Newer,
    /// The target is newer but the mode makes it equal to the source.
    Older,
    /// Another size, the same age.
    Size,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Op {
    pub kind: Kind,
    pub rel: PathBuf,
    /// Bytes to copy (0 for the rest).
    pub size: u64,
    pub why: Option<Why>,
}

/// The same file on both sides?
pub fn same(a: &Node, b: &Node) -> bool {
    a.size == b.size && (a.mtime - b.mtime).abs() <= SLACK
}

fn op(kind: Kind, n: &Node, why: Option<Why>) -> Op {
    Op {
        kind,
        rel: n.rel.clone(),
        size: if n.is_dir { 0 } else { n.size },
        why,
    }
}

/// What makes `remote` like `local` (Up), `local` like `remote` (Down), or both like the newer
/// (Both). `mirror`: what the target has and the source has not is deleted (Up / Down only).
/// Folders to make first, then copies, then deletes; each in path order.
pub fn diff(local: &[Node], remote: &[Node], mode: Mode, mirror: bool) -> Vec<Op> {
    let l: BTreeMap<&Path, &Node> = local.iter().map(|n| (n.rel.as_path(), n)).collect();
    let r: BTreeMap<&Path, &Node> = remote.iter().map(|n| (n.rel.as_path(), n)).collect();
    let mut paths: Vec<&Path> = l.keys().chain(r.keys()).copied().collect();
    paths.sort();
    paths.dedup();
    let (mut dirs, mut copies, mut deletes, mut conflicts) = (vec![], vec![], vec![], vec![]);
    // Folders deleted whole: what is inside them is not listed again.
    let mut gone: Vec<&Path> = Vec::new();
    for p in paths {
        if gone.iter().any(|g| p.starts_with(g)) {
            continue;
        }
        match (l.get(p), r.get(p)) {
            (Some(a), Some(b)) if a.is_dir != b.is_dir => {
                conflicts.push(op(Kind::Conflict, a, None))
            }
            (Some(a), Some(b)) if a.is_dir => {
                let _ = b;
            }
            (Some(a), Some(b)) => {
                if same(a, b) {
                    continue;
                }
                let why =
                    |src: &Node, dst: &Node| match (src.size != dst.size, src.mtime - dst.mtime) {
                        (true, d) if d.abs() <= SLACK => Why::Size,
                        (_, d) if d > 0 => Why::Newer,
                        _ => Why::Older,
                    };
                match mode {
                    Mode::Up => copies.push(op(Kind::CopyUp, a, Some(why(a, b)))),
                    Mode::Down => copies.push(op(Kind::CopyDown, b, Some(why(b, a)))),
                    Mode::Both => match a.mtime - b.mtime {
                        d if d > SLACK => copies.push(op(Kind::CopyUp, a, Some(Why::Newer))),
                        d if d < -SLACK => copies.push(op(Kind::CopyDown, b, Some(Why::Newer))),
                        _ => conflicts.push(op(Kind::Conflict, a, Some(Why::Size))),
                    },
                }
            }
            (Some(a), None) => match mode {
                Mode::Up | Mode::Both if a.is_dir => dirs.push(op(Kind::MkdirRemote, a, None)),
                Mode::Up | Mode::Both => copies.push(op(Kind::CopyUp, a, Some(Why::New))),
                Mode::Down if mirror => {
                    deletes.push(op(Kind::DeleteLocal, a, None));
                    if a.is_dir {
                        gone.push(p);
                    }
                }
                Mode::Down => {}
            },
            (None, Some(b)) => match mode {
                Mode::Down | Mode::Both if b.is_dir => dirs.push(op(Kind::MkdirLocal, b, None)),
                Mode::Down | Mode::Both => copies.push(op(Kind::CopyDown, b, Some(Why::New))),
                Mode::Up if mirror => {
                    deletes.push(op(Kind::DeleteRemote, b, None));
                    if b.is_dir {
                        gone.push(p);
                    }
                }
                Mode::Up => {}
            },
            (None, None) => {}
        }
    }
    dirs.into_iter()
        .chain(copies)
        .chain(conflicts)
        .chain(deletes)
        .collect()
}

/// The files to copy and their bytes, and the deletes.
pub fn totals(ops: &[Op]) -> (usize, u64, usize) {
    let copies = ops
        .iter()
        .filter(|o| matches!(o.kind, Kind::CopyUp | Kind::CopyDown));
    let (n, bytes) = copies.fold((0, 0), |(n, b), o| (n + 1, b + o.size));
    let deletes = ops
        .iter()
        .filter(|o| matches!(o.kind, Kind::DeleteLocal | Kind::DeleteRemote))
        .count();
    (n, bytes, deletes)
}

/// The local tree under `root` (links are skipped, as in a copy; unreadable folders too).
pub fn scan_local(root: &Path, recursive: bool) -> std::io::Result<Vec<Node>> {
    let mut out = Vec::new();
    scan_dir(root, Path::new(""), recursive, &mut out)?;
    Ok(out)
}

fn scan_dir(root: &Path, rel: &Path, recursive: bool, out: &mut Vec<Node>) -> std::io::Result<()> {
    for e in std::fs::read_dir(root.join(rel))? {
        let Ok(e) = e else { continue };
        let Ok(m) = std::fs::symlink_metadata(e.path()) else {
            continue;
        };
        if m.file_type().is_symlink() {
            continue;
        }
        let rel = rel.join(e.file_name());
        let mtime = m
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs() as i64);
        out.push(Node {
            rel: rel.clone(),
            size: m.len(),
            mtime,
            is_dir: m.is_dir(),
        });
        if m.is_dir() && recursive {
            // A folder that can't be read is left out, not a failure of everything.
            let _ = scan_dir(root, &rel, recursive, out);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(rel: &str, size: u64, mtime: i64) -> Node {
        Node {
            rel: rel.into(),
            size,
            mtime,
            is_dir: false,
        }
    }

    fn d(rel: &str) -> Node {
        Node {
            rel: rel.into(),
            size: 0,
            mtime: 0,
            is_dir: true,
        }
    }

    fn kinds(ops: &[Op]) -> Vec<(Kind, &str, Option<Why>)> {
        ops.iter()
            .map(|o| (o.kind, o.rel.to_str().unwrap(), o.why))
            .collect()
    }

    #[test]
    fn same_within_the_slack() {
        assert!(same(&f("a", 5, 100), &f("a", 5, 102)));
        assert!(!same(&f("a", 5, 100), &f("a", 5, 103)));
        assert!(!same(&f("a", 5, 100), &f("a", 6, 100)));
    }

    #[test]
    fn up_copies_new_and_changed_only() {
        let l = [
            f("same", 1, 10),
            f("new", 2, 10),
            f("newer", 3, 50),
            f("size", 4, 10),
            f("older", 5, 10),
        ];
        let r = [
            f("same", 1, 11),
            f("newer", 3, 10),
            f("size", 9, 10),
            f("older", 5, 99),
            f("extra", 1, 1),
        ];
        let ops = diff(&l, &r, Mode::Up, false);
        assert_eq!(
            kinds(&ops),
            [
                (Kind::CopyUp, "new", Some(Why::New)),
                (Kind::CopyUp, "newer", Some(Why::Newer)),
                (Kind::CopyUp, "older", Some(Why::Older)),
                (Kind::CopyUp, "size", Some(Why::Size)),
            ]
        );
        assert_eq!(totals(&ops), (4, 2 + 3 + 5 + 4, 0));
    }

    #[test]
    fn down_is_the_mirror_image() {
        let l = [f("a", 1, 10)];
        let r = [f("a", 1, 50), f("b", 7, 1)];
        let ops = diff(&l, &r, Mode::Down, false);
        assert_eq!(
            kinds(&ops),
            [
                (Kind::CopyDown, "a", Some(Why::Newer)),
                (Kind::CopyDown, "b", Some(Why::New))
            ]
        );
    }

    #[test]
    fn mirror_deletes_the_targets_extras_last_and_folders_whole() {
        let l = [f("keep", 1, 1)];
        let r = [
            f("keep", 1, 1),
            d("old"),
            f("old/x", 1, 1),
            f("old/y/z", 1, 1),
            f("loose", 1, 1),
        ];
        let ops = diff(&l, &r, Mode::Up, true);
        assert_eq!(
            kinds(&ops),
            [
                (Kind::DeleteRemote, "loose", None),
                (Kind::DeleteRemote, "old", None)
            ]
        );
        assert_eq!(totals(&ops), (0, 0, 2));
        // not asked for: nothing goes
        assert!(diff(&l, &r, Mode::Up, false).is_empty());
        // Both never deletes
        assert!(
            kinds(&diff(&l, &r, Mode::Both, true))
                .iter()
                .all(|(k, ..)| !matches!(k, Kind::DeleteLocal | Kind::DeleteRemote))
        );
    }

    #[test]
    fn both_ways_the_newer_wins() {
        let l = [f("l", 1, 10), f("x", 1, 100), f("y", 1, 5), f("t", 1, 10)];
        let r = [f("r", 1, 10), f("x", 1, 10), f("y", 1, 500), f("t", 2, 11)];
        let ops = diff(&l, &r, Mode::Both, false);
        assert_eq!(
            kinds(&ops),
            [
                (Kind::CopyUp, "l", Some(Why::New)),
                (Kind::CopyDown, "r", Some(Why::New)),
                (Kind::CopyUp, "x", Some(Why::Newer)),
                (Kind::CopyDown, "y", Some(Why::Newer)),
                (Kind::Conflict, "t", Some(Why::Size)),
            ]
        );
    }

    #[test]
    fn folders_are_made_first_and_clashes_are_flagged() {
        let l = [d("sub"), f("sub/a", 1, 1), f("both", 1, 1)];
        let r = [d("both")];
        let ops = diff(&l, &r, Mode::Up, false);
        assert_eq!(
            kinds(&ops),
            [
                (Kind::MkdirRemote, "sub", None),
                (Kind::CopyUp, "sub/a", Some(Why::New)),
                (Kind::Conflict, "both", None),
            ]
        );
        let down = diff(&[], &[d("e")], Mode::Down, false);
        assert_eq!(kinds(&down), [(Kind::MkdirLocal, "e", None)]);
    }

    #[test]
    fn scan_local_skips_links_and_can_stay_flat() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("a/b")).unwrap();
        std::fs::write(dir.path().join("a/b/f"), "xyz").unwrap();
        std::fs::write(dir.path().join("top"), "1").unwrap();
        std::os::unix::fs::symlink(dir.path().join("top"), dir.path().join("ln")).unwrap();
        let mut deep: Vec<String> = scan_local(dir.path(), true)
            .unwrap()
            .iter()
            .map(|n| n.rel.display().to_string())
            .collect();
        deep.sort();
        assert_eq!(deep, ["a", "a/b", "a/b/f", "top"]);
        let flat = scan_local(dir.path(), false).unwrap();
        assert_eq!(flat.len(), 2);
        let f = scan_local(dir.path(), true)
            .unwrap()
            .into_iter()
            .find(|n| n.rel == Path::new("a/b/f"))
            .unwrap();
        assert_eq!((f.size, f.is_dir), (3, false));
    }
}
