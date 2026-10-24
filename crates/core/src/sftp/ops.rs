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
            mode: a.perm.map(|p| p & 0o7777),
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

/// Turn on `set` and off `clear` (only the bits the user changed: files that differ keep their
/// other bits) on `paths`, and under folders when `recursive`. Links are left alone (chmod would
/// change their target). The first failure stops it.
pub fn chmod_remote<R: Read, W: Write>(
    c: &mut Client<R, W>,
    paths: &[PathBuf],
    set: u32,
    clear: u32,
    recursive: bool,
) -> io::Result<()> {
    // A file that refuses doesn't stop the rest: the first refusal is reported at the end.
    let mut first: Option<io::Error> = None;
    for p in paths {
        let r = (|| {
            let a = c.lstat(p)?;
            if a.is_link() {
                return Ok(());
            }
            let old = a.perm.unwrap_or(0) & 0o7777;
            let new = (old & !clear) | set;
            if new != old {
                c.chmod(p, new)?;
            }
            if recursive && a.is_dir() {
                let inside: Vec<PathBuf> =
                    (c.read_dir(p)?.iter()).map(|e| p.join(&e.name)).collect();
                chmod_remote(c, &inside, set, clear, true)?;
            }
            Ok(())
        })();
        match r {
            Err(e) if is_fatal(&e) => return Err(e),
            Err(e) => drop(first.get_or_insert(e)),
            Ok(()) => {}
        }
    }
    first.map_or(Ok(()), Err)
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

/// What the target's state and the answer come to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Plan1 {
    Skip,
    /// The whole file, replacing what is there.
    Full,
    /// From byte n on, onto the shorter target.
    Resume(u64),
}

/// `(size, mtime secs)` of the source and of the target.
type Stat = (u64, i64);

fn decide(choice: Choice, src: Stat, dst: Stat) -> Plan1 {
    match choice {
        Choice::Skip => Plan1::Skip,
        Choice::Overwrite => Plan1::Full,
        Choice::Newer if src.1 > dst.1 => Plan1::Full,
        Choice::Newer => Plan1::Skip,
        Choice::Resume if dst.0 < src.0 && dst.0 > 0 => Plan1::Resume(dst.0),
        Choice::Resume => Plan1::Full,
    }
}

fn local_stat(p: &Path) -> io::Result<Stat> {
    let m = std::fs::metadata(p)?;
    let secs = m
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs() as i64);
    Ok((m.len(), secs))
}

fn remote_stat<R: Read, W: Write>(c: &mut Client<R, W>, p: &Path) -> io::Result<Stat> {
    let a = c.stat(p)?;
    Ok((a.size.unwrap_or(0), i64::from(a.mtime.unwrap_or(0))))
}

/// The size of the file a job would replace, if it is there.
pub fn target_size<R: Read, W: Write>(c: &mut Client<R, W>, job: &Job) -> Option<u64> {
    match job.dir {
        Dir::Down => std::fs::metadata(&job.dst).ok().map(|m| m.len()),
        Dir::Up => c.stat(&job.dst).ok().and_then(|a| a.size),
    }
}

/// The last bytes of the shorter target (up to 64 KiB) are the same bytes of the source.
fn tail_matches<R: Read, W: Write>(c: &mut Client<R, W>, job: &Job, len: u64) -> io::Result<bool> {
    use std::io::{Seek, SeekFrom};
    let n = len.min(64 * 1024);
    let at = len - n;
    let (src, dst) = match job.dir {
        Dir::Down => {
            let src = c.read_range(&job.src, at, n as u32)?;
            let mut f = std::fs::File::open(&job.dst)?;
            f.seek(SeekFrom::Start(at))?;
            let mut dst = vec![0; n as usize];
            f.read_exact(&mut dst)?;
            (src, dst)
        }
        Dir::Up => {
            let mut f = std::fs::File::open(&job.src)?;
            f.seek(SeekFrom::Start(at))?;
            let mut src = vec![0; n as usize];
            f.read_exact(&mut src)?;
            (src, c.read_range(&job.dst, at, n as u32)?)
        }
    };
    Ok(src == dst)
}

/// One file of the queue. `progress(bytes)` false cancels (`Interrupted`); the target is replaced
/// only by a complete copy; a move removes the source only then. A resumed copy is written onto
/// the shorter target itself (it was incomplete anyway), so a cancel leaves it resumable again.
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
    let mut how = Plan1::Full;
    if exists {
        let Some(choice) = choice else {
            return Ok(Copied::Exists);
        };
        // Only these two look at the files (a dangling link or an unreadable one must still be
        // skippable and replaceable).
        if matches!(choice, Choice::Newer | Choice::Resume) {
            let (src, dst) = match job.dir {
                Dir::Down => (remote_stat(c, &job.src)?, local_stat(&job.dst)?),
                Dir::Up => (local_stat(&job.src)?, remote_stat(c, &job.dst)?),
            };
            how = decide(choice, src, dst);
            // The target must be the start of the source: the end of it is checked, so an
            // older, different file of the same name is replaced, never extended into garbage.
            if let Plan1::Resume(from) = how
                && !tail_matches(c, job, from)?
            {
                how = Plan1::Full;
            }
        } else {
            how = if choice == Choice::Skip {
                Plan1::Skip
            } else {
                Plan1::Full
            };
        }
        if how == Plan1::Skip {
            return Ok(Copied::Skipped);
        }
    }
    // The source's time as it was before the copy: a file changed meanwhile then differs, and a
    // later sync copies it again.
    let src_secs = source_secs(c, job).ok();
    let tmp = part(&job.dst);
    match (job.dir, how) {
        (Dir::Down, Plan1::Resume(from)) => {
            let mut out = std::fs::OpenOptions::new().append(true).open(&job.dst)?;
            c.download_from(&job.src, from, &mut out, progress)?;
            out.sync_all()?;
        }
        (Dir::Up, Plan1::Resume(from)) => {
            use std::io::{Seek, SeekFrom};
            let mut input = std::fs::File::open(&job.src)?;
            input.seek(SeekFrom::Start(from))?;
            c.upload_from(&mut input, &job.dst, from, progress)?;
        }
        (Dir::Down, _) => {
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
        (Dir::Up, _) => {
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
    // The copy keeps the source's time (SecureFX's «preserve times»): a later sync then sees the
    // two as the same. Only a nicety: a server that refuses it is no failure of the copy.
    match preserve_times(c, job, src_secs) {
        Err(e) if is_fatal(&e) => return Err(e),
        _ => {}
    }
    if job.is_move {
        match job.dir {
            Dir::Down => c.remove(&job.src)?,
            Dir::Up => std::fs::remove_file(&job.src)?,
        }
    }
    Ok(Copied::Done)
}

/// The source's modification time (seconds).
fn source_secs<R: Read, W: Write>(c: &mut Client<R, W>, job: &Job) -> io::Result<i64> {
    match job.dir {
        Dir::Down => remote_stat(c, &job.src).map(|s| s.1),
        Dir::Up => local_stat(&job.src).map(|s| s.1),
    }
}

/// The target of `job` gets the source's modification time (`secs`, if it was known).
fn preserve_times<R: Read, W: Write>(
    c: &mut Client<R, W>,
    job: &Job,
    secs: Option<i64>,
) -> io::Result<()> {
    let Some(secs) = secs else { return Ok(()) };
    match job.dir {
        Dir::Down => {
            let t = std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs.max(0) as u64);
            std::fs::OpenOptions::new()
                .write(true)
                .open(&job.dst)?
                .set_modified(t)
        }
        Dir::Up => c.set_times(&job.dst, u32::try_from(secs.max(0)).unwrap_or(u32::MAX)),
    }
}

/// Where `crt.FileTransfer` puts a file called `name`: `given` empty → `home/name`; ending in `/`
/// or naming a folder → inside it; else that is the new name. A relative `given` is counted from
/// `home`.
pub fn script_target(home: &Path, given: &str, name: &OsStr, given_is_dir: bool) -> PathBuf {
    if given.is_empty() {
        return home.join(name);
    }
    let full = home.join(given);
    if given.ends_with('/') || given_is_dir {
        full.join(name)
    } else {
        full
    }
}

/// `crt.FileTransfer`: the file or folder `src` (on this side for `Up`, the server for `Down`) to
/// `given` on the other, `home` being where an empty or relative `given` starts. Files already
/// there are replaced. `go()` false cancels (`Interrupted`).
pub fn script_copy<R: Read, W: Write>(
    c: &mut Client<R, W>,
    dir: Dir,
    src: &str,
    given: &str,
    home: &Path,
    mut go: impl FnMut() -> bool,
) -> io::Result<()> {
    let src_path = Path::new(src);
    let name = src_path
        .file_name()
        .ok_or_else(|| io::Error::new(ErrorKind::InvalidInput, format!("no file name: {src}")))?;
    let (src_is_dir, given_is_dir) = match dir {
        Dir::Up => {
            if src_path.symlink_metadata().is_err() {
                return Err(io::Error::new(ErrorKind::NotFound, src.to_string()));
            }
            (
                src_path.is_dir(),
                c.stat(&home.join(given)).is_ok_and(|a| a.is_dir()),
            )
        }
        Dir::Down => (c.stat(src_path)?.is_dir(), home.join(given).is_dir()),
    };
    let mut files = Vec::new();
    if src_is_dir {
        let dst_dir = home.join(given);
        files = plan(c, dir, &[src_path.to_path_buf()], &dst_dir)?.files;
    } else {
        let dst = script_target(home, given, name, given_is_dir);
        if let Some(parent) = dst.parent().filter(|p| !p.as_os_str().is_empty()) {
            match dir {
                Dir::Up => ensure_remote_dir(c, parent)?,
                Dir::Down => std::fs::create_dir_all(parent)?,
            }
        }
        files.push((src_path.to_path_buf(), dst, 0));
    }
    for (n, (from, to, size)) in files.into_iter().enumerate() {
        let job = Job {
            id: n as u64 + 1,
            batch: 1,
            dir,
            src: from,
            dst: to,
            size,
            done: 0,
            state: crate::sftp::queue::State::Running,
            is_move: false,
        };
        transfer(c, &job, Some(Choice::Overwrite), |_| go())?;
    }
    Ok(())
}

/// The server's tree under `root` as `sync::Node`s, with the folders that could not be read.
/// A name that is not one plain file name (a `/` in it) is not trusted and left out: it would
/// lead out of the synchronised folder.
pub fn scan_remote<R: Read, W: Write>(
    c: &mut Client<R, W>,
    root: &Path,
    recursive: bool,
) -> io::Result<(Vec<crate::sftp::sync::Node>, Vec<PathBuf>)> {
    let (mut out, mut unread) = (Vec::new(), Vec::new());
    scan_remote_dir(c, root, Path::new(""), recursive, &mut out, &mut unread)?;
    Ok((out, unread))
}

fn scan_remote_dir<R: Read, W: Write>(
    c: &mut Client<R, W>,
    root: &Path,
    rel: &Path,
    recursive: bool,
    out: &mut Vec<crate::sftp::sync::Node>,
    unread: &mut Vec<PathBuf>,
) -> io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let entries = match c.read_dir(&root.join(rel)) {
        Ok(e) => e,
        // The folder asked for must be there; one below it may be closed to us.
        Err(e) if !rel.as_os_str().is_empty() && !is_fatal(&e) => {
            unread.push(rel.to_path_buf());
            return Ok(());
        }
        Err(e) => return Err(e),
    };
    for e in entries {
        let name = e.name.as_bytes();
        if name.is_empty() || name.contains(&b'/') || name == b"." || name == b".." {
            continue;
        }
        let rel = rel.join(&e.name);
        let link = e.attrs.is_link();
        out.push(crate::sftp::sync::Node {
            rel: rel.clone(),
            size: e.attrs.size.unwrap_or(0),
            mtime: i64::from(e.attrs.mtime.unwrap_or(0)),
            is_dir: e.attrs.is_dir() && !link,
            link,
        });
        if e.attrs.is_dir() && !link && recursive {
            scan_remote_dir(c, root, &rel, recursive, out, unread)?;
        }
    }
    Ok(())
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

    fn set_mtime(p: &Path, secs: u64) {
        let t = std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs);
        std::fs::OpenOptions::new()
            .write(true)
            .open(p)
            .unwrap()
            .set_modified(t)
            .unwrap();
    }

    #[test]
    fn copies_keep_the_sources_time_both_ways() {
        let Some((_s, mut c)) = server() else { return };
        let (r, l) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let (rf, lf) = (r.path().join("f"), l.path().join("f"));
        std::fs::write(&rf, "remote").unwrap();
        set_mtime(&rf, 1_000_000_000);
        let j = job(Dir::Down, &rf, &lf, false);
        assert_eq!(transfer(&mut c, &j, None, |_| true).unwrap(), Copied::Done);
        assert_eq!(local_stat(&lf).unwrap().1, 1_000_000_000);
        // and up: a new file on the server with the local one's time
        let up = r.path().join("up");
        std::fs::write(&lf, "local").unwrap();
        set_mtime(&lf, 1_100_000_000);
        let j = job(Dir::Up, &lf, &up, false);
        assert_eq!(transfer(&mut c, &j, None, |_| true).unwrap(), Copied::Done);
        assert_eq!(remote_stat(&mut c, &up).unwrap().1, 1_100_000_000);
    }

    #[test]
    fn scan_remote_reads_the_tree_and_marks_links() {
        let Some((_s, mut c)) = server() else { return };
        let r = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(r.path().join("a/b")).unwrap();
        std::fs::write(r.path().join("a/b/f"), "xyz").unwrap();
        std::os::unix::fs::symlink(r.path().join("a"), r.path().join("ln")).unwrap();
        let mut rel: Vec<String> = scan_remote(&mut c, r.path(), true)
            .unwrap()
            .0
            .iter()
            .map(|n| n.rel.display().to_string())
            .collect();
        rel.sort();
        assert_eq!(rel, ["a", "a/b", "a/b/f", "ln"]); // the link is there, marked
        assert_eq!(scan_remote(&mut c, r.path(), false).unwrap().0.len(), 2);
        let nodes = scan_remote(&mut c, r.path(), true).unwrap().0;
        assert!(
            nodes
                .iter()
                .any(|n| n.link && n.rel == Path::new("ln") && !n.is_dir)
        );
        assert!(scan_remote(&mut c, &r.path().join("none"), true).is_err());
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

    #[test]
    fn decide_rules() {
        let (new, old) = ((10, 200), (10, 100));
        assert_eq!(decide(Choice::Newer, new, old), Plan1::Full);
        assert_eq!(decide(Choice::Newer, old, new), Plan1::Skip);
        assert_eq!(decide(Choice::Newer, old, old), Plan1::Skip);
        assert_eq!(decide(Choice::Resume, (10, 1), (4, 1)), Plan1::Resume(4));
        assert_eq!(decide(Choice::Resume, (10, 1), (10, 1)), Plan1::Full);
        assert_eq!(decide(Choice::Resume, (10, 1), (12, 1)), Plan1::Full);
        assert_eq!(decide(Choice::Resume, (10, 1), (0, 1)), Plan1::Full);
    }

    #[test]
    fn newer_skips_an_older_source() {
        let Some((_s, mut c)) = server() else { return };
        let (r, l) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let (src, dst) = (r.path().join("f"), l.path().join("f"));
        std::fs::write(&src, "source").unwrap();
        std::fs::write(&dst, "target").unwrap();
        let t = |secs| std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs);
        let set = |p: &Path, secs| {
            std::fs::File::options()
                .write(true)
                .open(p)
                .unwrap()
                .set_modified(t(secs))
                .unwrap()
        };
        let j = job(Dir::Down, &src, &dst, false);
        set(&src, 1000);
        set(&dst, 2000);
        let newer = Some(Choice::Newer);
        assert_eq!(
            transfer(&mut c, &j, newer, |_| true).unwrap(),
            Copied::Skipped
        );
        assert_eq!(std::fs::read_to_string(&dst).unwrap(), "target");
        set(&src, 3000);
        assert_eq!(transfer(&mut c, &j, newer, |_| true).unwrap(), Copied::Done);
        assert_eq!(std::fs::read_to_string(&dst).unwrap(), "source");
    }

    #[test]
    fn resume_carries_on_in_both_directions() {
        let Some((_s, mut c)) = server() else { return };
        let (r, l) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let whole: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        let resume = Some(Choice::Resume);
        for dir in [Dir::Down, Dir::Up] {
            let (src, dst) = match dir {
                Dir::Down => (r.path().join("d"), l.path().join("d")),
                Dir::Up => (l.path().join("u"), r.path().join("u")),
            };
            std::fs::write(&src, &whole).unwrap();
            std::fs::write(&dst, &whole[..100_000]).unwrap();
            let j = job(dir, &src, &dst, false);
            assert_eq!(
                transfer(&mut c, &j, resume, |_| true).unwrap(),
                Copied::Done
            );
            assert_eq!(std::fs::read(&dst).unwrap(), whole, "{dir:?}");
        }
    }

    #[test]
    fn chmod_changes_only_the_asked_bits_recursively() {
        use std::os::unix::fs::PermissionsExt;
        let Some((_s, mut c)) = server() else { return };
        let d = tempfile::tempdir().unwrap();
        let top = d.path().join("top");
        std::fs::create_dir_all(top.join("sub")).unwrap();
        let (a, b) = (top.join("a"), top.join("sub/b"));
        std::fs::write(&a, "").unwrap();
        std::fs::write(&b, "").unwrap();
        let mode =
            |p: &Path, m| std::fs::set_permissions(p, std::fs::Permissions::from_mode(m)).unwrap();
        mode(&a, 0o644);
        mode(&b, 0o600);
        // group write on, other read off
        chmod_remote(&mut c, std::slice::from_ref(&top), 0o020, 0o004, true).unwrap();
        let got = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o7777;
        assert_eq!(got(&a), 0o660);
        assert_eq!(got(&b), 0o620);
        // not recursive: the folder only
        chmod_remote(&mut c, std::slice::from_ref(&top), 0o001, 0, false).unwrap();
        assert_eq!(got(&a), 0o660);
        assert_eq!(got(&top) & 0o001, 0o001);
    }

    #[test]
    fn regression_resume_refuses_a_different_older_file() {
        let Some((_s, mut c)) = server() else { return };
        let (r, l) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let (src, dst) = (r.path().join("f"), l.path().join("f"));
        std::fs::write(&src, vec![1u8; 200_000]).unwrap();
        std::fs::write(&dst, vec![2u8; 50_000]).unwrap(); // shorter, but not its start
        let j = job(Dir::Down, &src, &dst, false);
        let resume = Some(Choice::Resume);
        assert_eq!(
            transfer(&mut c, &j, resume, |_| true).unwrap(),
            Copied::Done
        );
        assert_eq!(std::fs::read(&dst).unwrap(), vec![1u8; 200_000]);
    }

    #[test]
    fn regression_skip_and_overwrite_need_no_stat() {
        let Some((_s, mut c)) = server() else { return };
        let (r, l) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let (src, dst) = (r.path().join("f"), l.path().join("f"));
        std::fs::write(&src, "new").unwrap();
        std::os::unix::fs::symlink(l.path().join("gone"), &dst).unwrap(); // dangling
        let j = job(Dir::Down, &src, &dst, false);
        assert_eq!(
            transfer(&mut c, &j, Some(Choice::Skip), |_| true).unwrap(),
            Copied::Skipped
        );
    }

    #[test]
    fn script_target_reads_the_given_path() {
        let home = Path::new("/home/u");
        let t = |g, d| script_target(home, g, OsStr::new("a.txt"), d);
        assert_eq!(t("", false), Path::new("/home/u/a.txt"));
        assert_eq!(t("up/", false), Path::new("/home/u/up/a.txt"));
        assert_eq!(t("up", true), Path::new("/home/u/up/a.txt"));
        assert_eq!(t("up/b.txt", false), Path::new("/home/u/up/b.txt"));
        assert_eq!(t("/srv/b.txt", false), Path::new("/srv/b.txt"));
    }

    #[test]
    fn script_copy_goes_both_ways_and_replaces() {
        let Some((mut child, mut c)) = server() else {
            return;
        };
        let tmp = tempfile::tempdir().unwrap();
        let (local, remote) = (tmp.path().join("l"), tmp.path().join("r"));
        std::fs::create_dir_all(local.join("tree/sub")).unwrap();
        std::fs::create_dir_all(&remote).unwrap();
        std::fs::write(local.join("a.txt"), "one").unwrap();
        std::fs::write(local.join("tree/sub/x"), "deep").unwrap();
        let a = local.join("a.txt");
        let go = || true;
        // A new name, then replaced by a second copy.
        script_copy(&mut c, Dir::Up, a.to_str().unwrap(), "b.txt", &remote, go).unwrap();
        std::fs::write(&a, "two").unwrap();
        script_copy(&mut c, Dir::Up, a.to_str().unwrap(), "b.txt", &remote, go).unwrap();
        assert_eq!(
            std::fs::read_to_string(remote.join("b.txt")).unwrap(),
            "two"
        );
        // A folder, whole, into a folder that is not there yet.
        let tree = local.join("tree");
        script_copy(&mut c, Dir::Up, tree.to_str().unwrap(), "in", &remote, go).unwrap();
        assert_eq!(
            std::fs::read_to_string(remote.join("in/tree/sub/x")).unwrap(),
            "deep"
        );
        // Down, into a folder, and a missing source is an error.
        let dl = tmp.path().join("dl");
        script_copy(
            &mut c,
            Dir::Down,
            remote.join("b.txt").to_str().unwrap(),
            "",
            &dl,
            go,
        )
        .unwrap();
        assert_eq!(std::fs::read_to_string(dl.join("b.txt")).unwrap(), "two");
        assert!(script_copy(&mut c, Dir::Up, "/no/such/file", "", &remote, go).is_err());
        let _ = child.kill();
    }
}
