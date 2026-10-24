//! What the SFTP tab does with files, over a `proto::Client`: list, delete a tree, lay a copy out
//! into files (making the target folders), copy one file. A transfer writes a temporary
//! `.<name>.shagshell-part` beside the target and renames it over the target only when complete:
//! a failed or cancelled overwrite leaves the old file as it was.

use super::proto::Client;
use super::queue::{Choice, Dir, Job};
use crate::panel::Entry;
use std::ffi::{OsStr, OsString};
use std::io::{self, ErrorKind, Read, Write};
use std::path::{Path, PathBuf};

/// The connection itself failed (the worker ends), not just this operation.
pub fn is_fatal(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        ErrorKind::UnexpectedEof
            | ErrorKind::BrokenPipe
            | ErrorKind::InvalidData
            | ErrorKind::ConnectionReset
    )
}

/// The server's folder as panel rows; a link shown as what it points to.
pub fn list<R: Read, W: Write>(c: &mut Client<R, W>, path: &Path) -> io::Result<Vec<Entry>> {
    let mut out = Vec::new();
    for e in c.read_dir(path)? {
        let mut a = e.attrs;
        if a.is_link()
            && let Ok(t) = c.stat(&path.join(&e.name))
        {
            a = t;
        }
        out.push(Entry {
            name: e.name,
            dir: a.is_dir(),
            size: a.size.unwrap_or(0),
            mtime: a.mtime.map(i64::from),
        });
    }
    Ok(out)
}

/// A file, or a folder with everything in it (a link is removed, not followed).
pub fn delete_remote<R: Read, W: Write>(c: &mut Client<R, W>, p: &Path) -> io::Result<()> {
    if !c.lstat(p)?.is_dir() {
        return c.remove(p);
    }
    for e in c.read_dir(p)? {
        delete_remote(c, &p.join(&e.name))?;
    }
    c.rmdir(p)
}

/// `mkdir -p` on the server.
pub fn ensure_remote_dir<R: Read, W: Write>(c: &mut Client<R, W>, p: &Path) -> io::Result<()> {
    match c.stat(p) {
        Ok(a) if a.is_dir() => return Ok(()),
        Ok(_) => {
            return Err(io::Error::new(
                ErrorKind::AlreadyExists,
                p.display().to_string(),
            ));
        }
        Err(e) if e.kind() != ErrorKind::NotFound => return Err(e),
        Err(_) => {}
    }
    if let Some(parent) = p.parent().filter(|p| !p.as_os_str().is_empty()) {
        ensure_remote_dir(c, parent)?;
    }
    c.mkdir(p)
}

/// A planned copy: its files `(src, dst, size)` and its source folders, deepest first (a move
/// removes them once all is copied).
#[derive(Debug, Default, PartialEq)]
pub struct Plan {
    pub files: Vec<(PathBuf, PathBuf, u64)>,
    pub dirs: Vec<PathBuf>,
}

/// `srcs` (files or whole folders) laid out as files under `dst_dir`; the target folders are made
/// now, so an empty folder is copied too. Links to folders are not followed (no loops).
pub fn plan<R: Read, W: Write>(
    c: &mut Client<R, W>,
    dir: Dir,
    srcs: &[PathBuf],
    dst_dir: &Path,
) -> io::Result<Plan> {
    let mut p = Plan::default();
    for s in srcs {
        plan_one(c, dir, s, dst_dir, &mut p)?;
    }
    p.dirs.reverse();
    Ok(p)
}

fn plan_one<R: Read, W: Write>(
    c: &mut Client<R, W>,
    dir: Dir,
    src: &Path,
    dst_dir: &Path,
    p: &mut Plan,
) -> io::Result<()> {
    let Some(name) = src.file_name() else {
        return Ok(());
    };
    let dst = dst_dir.join(name);
    let (is_dir, size, link) = match dir {
        Dir::Down => {
            let a = c.stat(src)?;
            (a.is_dir(), a.size.unwrap_or(0), c.lstat(src)?.is_link())
        }
        Dir::Up => {
            let m = std::fs::metadata(src)?;
            let link = std::fs::symlink_metadata(src)?.file_type().is_symlink();
            (m.is_dir(), m.len(), link)
        }
    };
    if !is_dir {
        p.files.push((src.to_path_buf(), dst, size));
        return Ok(());
    }
    if link {
        return Ok(());
    }
    let names: Vec<OsString> = match dir {
        Dir::Down => {
            std::fs::create_dir_all(&dst)?;
            c.read_dir(src)?.into_iter().map(|e| e.name).collect()
        }
        Dir::Up => {
            ensure_remote_dir(c, &dst)?;
            (std::fs::read_dir(src)?)
                .filter_map(|e| e.ok().map(|e| e.file_name()))
                .collect()
        }
    };
    p.dirs.push(src.to_path_buf());
    for n in names {
        plan_one(c, dir, &src.join(n), &dst, p)?;
    }
    Ok(())
}

/// What one file's copy came to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Copied {
    Done,
    Skipped,
    /// The target is there and nobody said what to do: ask, then again with the answer.
    Exists,
}

/// The temporary name beside `dst` a copy is written to.
fn part(dst: &Path) -> PathBuf {
    let mut n = OsString::from(".");
    n.push(dst.file_name().unwrap_or(OsStr::new("file")));
    n.push(".shagshell-part");
    dst.with_file_name(n)
}

/// One file of the queue. `progress(bytes)` false cancels (`Interrupted`); the target is replaced
/// only by a complete copy; a move removes the source only then.
pub fn transfer<R: Read, W: Write>(
    c: &mut Client<R, W>,
    job: &Job,
    choice: Option<Choice>,
    progress: impl FnMut(u64) -> bool,
) -> io::Result<Copied> {
    let exists = match job.dir {
        Dir::Down => job.dst.symlink_metadata().is_ok(),
        Dir::Up => c.lstat(&job.dst).is_ok(),
    };
    match (exists, choice) {
        (true, None) => return Ok(Copied::Exists),
        (true, Some(Choice::Skip)) => return Ok(Copied::Skipped),
        _ => {}
    }
    let tmp = part(&job.dst);
    match job.dir {
        Dir::Down => {
            if let Some(parent) = job.dst.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let r = (|| {
                let mut out = io::BufWriter::new(std::fs::File::create(&tmp)?);
                c.download(&job.src, &mut out, progress)?;
                out.flush()?;
                out.into_inner().map_err(|e| e.into_error())?.sync_all()
            })();
            if let Err(e) = r {
                let _ = std::fs::remove_file(&tmp);
                return Err(e);
            }
            std::fs::rename(&tmp, &job.dst)?;
        }
        Dir::Up => {
            let mut input = std::fs::File::open(&job.src)?;
            if let Err(e) = c.upload(&mut input, &tmp, progress) {
                if !is_fatal(&e) {
                    let _ = c.remove(&tmp);
                }
                return Err(e);
            }
            // SFTP v3 RENAME doesn't replace (OpenSSH links): the old one goes first, now that
            // the new one is whole.
            let replaced = match exists {
                true => c.remove(&job.dst).and_then(|()| c.rename(&tmp, &job.dst)),
                false => c.rename(&tmp, &job.dst),
            };
            if let Err(e) = replaced {
                if !is_fatal(&e) {
                    let _ = c.remove(&tmp);
                }
                return Err(e);
            }
        }
    }
    if job.is_move {
        match job.dir {
            Dir::Down => c.remove(&job.src)?,
            Dir::Up => std::fs::remove_file(&job.src)?,
        }
    }
    Ok(Copied::Done)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sftp::queue::State;
    use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

    const SERVER: &str = "/usr/lib/openssh/sftp-server";

    /// The "server" here is this machine's sftp-server: remote paths are local paths.
    fn server() -> Option<(Child, Client<ChildStdout, ChildStdin>)> {
        if !Path::new(SERVER).exists() {
            return None;
        }
        let mut child = (Command::new(SERVER)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped()))
        .spawn()
        .unwrap();
        let (w, r) = (child.stdin.take().unwrap(), child.stdout.take().unwrap());
        Some((child, Client::new(r, w).unwrap()))
    }

    fn job(dir: Dir, src: &Path, dst: &Path, is_move: bool) -> Job {
        Job {
            id: 1,
            batch: 1,
            dir,
            src: src.into(),
            dst: dst.into(),
            size: 0,
            done: 0,
            state: State::Running,
            is_move,
        }
    }

    #[test]
    fn plan_down_a_tree_makes_its_folders() {
        let Some((_s, mut c)) = server() else { return };
        let (r, l) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let src = r.path().join("top");
        std::fs::create_dir_all(src.join("sub/empty")).unwrap();
        std::fs::write(src.join("a"), "12").unwrap();
        std::fs::write(src.join("sub/b"), "345").unwrap();
        let mut p = plan(&mut c, Dir::Down, std::slice::from_ref(&src), l.path()).unwrap();
        p.files.sort();
        let dst = l.path().join("top");
        assert_eq!(
            p.files,
            [
                (src.join("a"), dst.join("a"), 2),
                (src.join("sub/b"), dst.join("sub/b"), 3)
            ]
        );
        assert!(dst.join("sub/empty").is_dir()); // an empty folder too
        assert_eq!(
            p.dirs.first().map(|d| d.starts_with(src.join("sub"))),
            Some(true)
        );
        assert_eq!(p.dirs.last(), Some(&src)); // deepest first, the top last
    }

    #[test]
    fn plan_up_skips_links_to_folders() {
        let Some((_s, mut c)) = server() else { return };
        let (l, r) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let src = l.path().join("top");
        std::fs::create_dir(&src).unwrap();
        std::os::unix::fs::symlink(&src, src.join("loop")).unwrap();
        std::fs::write(src.join("f"), "x").unwrap();
        let p = plan(&mut c, Dir::Up, std::slice::from_ref(&src), r.path()).unwrap();
        assert_eq!(p.files.len(), 1);
        assert!(r.path().join("top").is_dir());
    }

    #[test]
    fn conflict_then_skip_then_overwrite() {
        let Some((_s, mut c)) = server() else { return };
        let (r, l) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let (src, dst) = (r.path().join("f"), l.path().join("f"));
        std::fs::write(&src, "new").unwrap();
        std::fs::write(&dst, "old").unwrap();
        let j = job(Dir::Down, &src, &dst, false);
        assert_eq!(
            transfer(&mut c, &j, None, |_| true).unwrap(),
            Copied::Exists
        );
        let skip = Some(Choice::Skip);
        assert_eq!(
            transfer(&mut c, &j, skip, |_| true).unwrap(),
            Copied::Skipped
        );
        assert_eq!(std::fs::read_to_string(&dst).unwrap(), "old");
        let over = Some(Choice::Overwrite);
        assert_eq!(transfer(&mut c, &j, over, |_| true).unwrap(), Copied::Done);
        assert_eq!(std::fs::read_to_string(&dst).unwrap(), "new");
    }

    #[test]
    fn regression_failed_overwrite_keeps_the_original() {
        let Some((_s, mut c)) = server() else { return };
        let (r, l) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        // Down: the server's file can't be read.
        let dst = l.path().join("report.txt");
        std::fs::write(&dst, "mine").unwrap();
        let j = job(Dir::Down, &r.path().join("missing"), &dst, false);
        assert!(transfer(&mut c, &j, Some(Choice::Overwrite), |_| true).is_err());
        assert_eq!(std::fs::read_to_string(&dst).unwrap(), "mine");
        assert_eq!(std::fs::read_dir(l.path()).unwrap().count(), 1); // no .part left
        // Up, cancelled half-way: the server's old file stays.
        let (src, rdst) = (l.path().join("big"), r.path().join("big"));
        std::fs::write(&src, vec![1u8; 3 << 20]).unwrap();
        std::fs::write(&rdst, "theirs").unwrap();
        let j = job(Dir::Up, &src, &rdst, false);
        let e = transfer(&mut c, &j, Some(Choice::Overwrite), |n| n < 1 << 20).unwrap_err();
        assert_eq!(e.kind(), ErrorKind::Interrupted);
        assert_eq!(std::fs::read_to_string(&rdst).unwrap(), "theirs");
        assert_eq!(std::fs::read_dir(r.path()).unwrap().count(), 1);
        // And a complete one replaces it.
        transfer(&mut c, &j, Some(Choice::Overwrite), |_| true).unwrap();
        assert_eq!(std::fs::metadata(&rdst).unwrap().len(), 3 << 20);
    }

    #[test]
    fn move_removes_the_source_after_the_copy() {
        let Some((_s, mut c)) = server() else { return };
        let (r, l) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let (src, dst) = (r.path().join("f"), l.path().join("f"));
        std::fs::write(&src, "x").unwrap();
        transfer(&mut c, &job(Dir::Down, &src, &dst, true), None, |_| true).unwrap();
        assert!(!src.exists() && dst.exists());
        // Up, cancelled: the source stays, nothing (not even the .part) on the server.
        let up = r.path().join("g");
        let e = transfer(&mut c, &job(Dir::Up, &dst, &up, true), None, |_| false);
        assert_eq!(e.unwrap_err().kind(), ErrorKind::Interrupted);
        assert!(dst.exists() && !up.exists());
        assert_eq!(std::fs::read_dir(r.path()).unwrap().count(), 0);
    }

    #[test]
    fn delete_a_tree_and_make_parents() {
        let Some((_s, mut c)) = server() else { return };
        let d = tempfile::tempdir().unwrap();
        let deep = d.path().join("a/b/c");
        ensure_remote_dir(&mut c, &deep).unwrap();
        assert!(deep.is_dir());
        ensure_remote_dir(&mut c, &deep).unwrap(); // there already: fine
        std::fs::write(deep.join("f"), "").unwrap();
        delete_remote(&mut c, &d.path().join("a")).unwrap();
        assert!(!d.path().join("a").exists());
    }

    #[test]
    fn list_follows_links() {
        let Some((_s, mut c)) = server() else { return };
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir(d.path().join("real")).unwrap();
        std::os::unix::fs::symlink(d.path().join("real"), d.path().join("link")).unwrap();
        let mut l = list(&mut c, d.path()).unwrap();
        l.sort_by(|a, b| a.name.cmp(&b.name));
        assert!(l.iter().all(|e| e.dir), "{l:?}");
    }
}
