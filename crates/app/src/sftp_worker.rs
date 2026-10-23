//! An SFTP tab's server I/O, off the UI thread: `ssh … -s -- host sftp` and a
//! `core::sftp::proto::Client` on its pipes, owned by one thread. Commands in, events out (tagged
//! with the tab's id, through the subscription in `sftp_view`). A tab has two: one for browsing,
//! one for transfers, so listing a folder doesn't wait for a big copy.

use crate::term::TabId;
use shagshell_core::panel::Entry;
use shagshell_core::sftp::proto::Client;
use shagshell_core::sftp::queue::{Choice, Dir, Job};
use std::ffi::OsString;
use std::io::{self, ErrorKind, Read};
use std::path::{Path, PathBuf};
use std::process::{ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};
use tokio::sync::mpsc::UnboundedSender;

/// Events with their tab and their worker's tag (a worker replaced by Reconnect may still say
/// something: its tag is no longer the tab's).
pub type Events = UnboundedSender<(TabId, u64, Event)>;

static TAGS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

#[derive(Debug)]
pub enum Cmd {
    /// The server's folder (browse worker).
    List {
        path: PathBuf,
        focus: Option<OsString>,
    },
    Mkdir(PathBuf),
    Rename(PathBuf, PathBuf),
    /// Files and folders (recursively).
    Delete(Vec<PathBuf>),
    /// F5 / F6 (transfer worker): `srcs` laid out into files under `dst_dir`, the target folders
    /// made.
    Plan {
        batch: u64,
        dir: Dir,
        srcs: Vec<PathBuf>,
        dst_dir: PathBuf,
        is_move: bool,
    },
    /// One file; `choice`: what to do if the target is there (none: ask).
    Run {
        job: Job,
        choice: Option<Choice>,
    },
    /// A move done: its source folders, deepest first (only empty ones go).
    RemoveDirs(Dir, Vec<PathBuf>),
}

#[derive(Clone, Debug)]
pub enum Event {
    /// Connected; the server's home folder.
    Connected(PathBuf),
    Listed {
        path: PathBuf,
        entries: Vec<Entry>,
        focus: Option<OsString>,
    },
    /// A change on the server is done (relist), or why it failed.
    Changed(Option<String>),
    Planned {
        batch: u64,
        dir: Dir,
        files: Vec<(PathBuf, PathBuf, u64)>,
        /// The source folders, deepest first (a move removes them at the end).
        dirs: Vec<PathBuf>,
        is_move: bool,
        error: Option<String>,
    },
    Progress(u64, u64),
    /// The target exists: ask (then `Run` again with the answer).
    Conflict(u64),
    /// `Ok(true)`: skipped.
    Finished(u64, Result<bool, String>),
    Disconnected(String),
}

/// A worker: dropping it ends its thread and its ssh.
pub struct Worker {
    pub tag: u64,
    tx: mpsc::Sender<Cmd>,
    /// Set to stop the running transfer.
    pub cancel: Arc<AtomicBool>,
}

impl Worker {
    pub fn send(&self, c: Cmd) {
        let _ = self.tx.send(c);
    }
}

pub fn start(
    id: TabId,
    program: String,
    args: Vec<String>,
    env: Vec<(String, String)>,
    events: Events,
) -> Worker {
    let (tx, rx) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    let tag = TAGS.fetch_add(1, Ordering::Relaxed);
    std::thread::spawn(move || run((id, tag), &program, &args, &env, &events, &rx, &flag));
    Worker { tag, tx, cancel }
}

type Sftp = Client<ChildStdout, ChildStdin>;

fn run(
    (id, tag): (TabId, u64),
    program: &str,
    args: &[String],
    env: &[(String, String)],
    events: &Events,
    rx: &mpsc::Receiver<Cmd>,
    cancel: &AtomicBool,
) {
    let send = |e: Event| drop(events.send((id, tag, e)));
    let mut child = match Command::new(program)
        .args(args)
        .envs(env.iter().cloned())
        .env("SHAGSHELL_TAB", id.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return send(Event::Disconnected(format!("{program}: {e}"))),
    };
    // ssh's own words (Connection refused, Permission denied…): the reason shown on failure.
    let said = Arc::new(Mutex::new(String::new()));
    if let Some(mut err) = child.stderr.take() {
        let said = said.clone();
        std::thread::spawn(move || {
            let mut buf = [0; 4096];
            while let Ok(n) = err.read(&mut buf) {
                if n == 0 {
                    break;
                }
                let mut s = said.lock().unwrap_or_else(|e| e.into_inner());
                s.push_str(&String::from_utf8_lossy(&buf[..n]));
                let keep = s.len().saturating_sub(2000);
                s.drain(..keep);
            }
        });
    }
    let why = |e: &io::Error| {
        // The stderr reader may be a moment behind the pipe closing.
        std::thread::sleep(Duration::from_millis(100));
        let s = said.lock().unwrap_or_else(|e| e.into_inner());
        let last = s
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .map(str::trim);
        last.map_or_else(|| e.to_string(), str::to_string)
    };
    let (Some(w), Some(r)) = (child.stdin.take(), child.stdout.take()) else {
        return;
    };
    let mut c = match Client::new(r, w).and_then(|mut c| c.realpath(Path::new(".")).map(|h| (c, h)))
    {
        Ok((c, home)) => {
            send(Event::Connected(home));
            c
        }
        Err(e) => {
            let _ = child.kill();
            let _ = child.wait();
            return send(Event::Disconnected(why(&e)));
        }
    };
    while let Ok(cmd) = rx.recv() {
        if let Err(e) = handle(&mut c, cmd, &send, cancel) {
            send(Event::Disconnected(why(&e)));
            break;
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// The connection itself failed (the worker ends), not just this command.
fn fatal(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        ErrorKind::UnexpectedEof
            | ErrorKind::BrokenPipe
            | ErrorKind::InvalidData
            | ErrorKind::ConnectionReset
    )
}

/// `Err`: the connection is gone.
fn handle(c: &mut Sftp, cmd: Cmd, send: &impl Fn(Event), cancel: &AtomicBool) -> io::Result<()> {
    let soft = |r: io::Result<()>| match r {
        Err(e) if fatal(&e) => Err(e),
        r => Ok(r.err().map(|e| e.to_string())),
    };
    match cmd {
        Cmd::List { path, focus } => match list(c, &path) {
            Ok(entries) => send(Event::Listed {
                path,
                entries,
                focus,
            }),
            Err(e) if fatal(&e) => return Err(e),
            Err(e) => send(Event::Changed(Some(e.to_string()))),
        },
        Cmd::Mkdir(p) => send(Event::Changed(soft(c.mkdir(&p))?)),
        Cmd::Rename(a, b) => send(Event::Changed(soft(c.rename(&a, &b))?)),
        Cmd::Delete(paths) => {
            let r = paths.iter().try_for_each(|p| delete_remote(c, p));
            send(Event::Changed(soft(r)?));
        }
        Cmd::Plan {
            batch,
            dir,
            srcs,
            dst_dir,
            is_move,
        } => {
            let mut files = Vec::new();
            let mut dirs = Vec::new();
            let r = srcs
                .iter()
                .try_for_each(|s| plan(c, dir, s, &dst_dir, &mut files, &mut dirs));
            dirs.reverse(); // deepest first
            let error = soft(r)?;
            send(Event::Planned {
                batch,
                dir,
                files,
                dirs,
                is_move,
                error,
            });
        }
        Cmd::Run { job, choice } => {
            cancel.store(false, Ordering::SeqCst);
            match transfer(c, &job, choice, send, cancel) {
                Ok(Some(skipped)) => send(Event::Finished(job.id, Ok(skipped))),
                Ok(None) => send(Event::Conflict(job.id)),
                Err(e) if fatal(&e) => {
                    send(Event::Finished(job.id, Err(e.to_string())));
                    return Err(e);
                }
                Err(e) => send(Event::Finished(job.id, Err(e.to_string()))),
            }
        }
        Cmd::RemoveDirs(dir, dirs) => {
            for d in dirs {
                let _ = match dir {
                    Dir::Down => c.rmdir(&d),
                    Dir::Up => std::fs::remove_dir(&d),
                };
            }
        }
    }
    Ok(())
}

fn list(c: &mut Sftp, path: &Path) -> io::Result<Vec<Entry>> {
    let mut out = Vec::new();
    for e in c.read_dir(path)? {
        let mut a = e.attrs;
        // A link: what it points to (a link to a folder opens like one).
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

fn delete_remote(c: &mut Sftp, p: &Path) -> io::Result<()> {
    let a = c.lstat(p)?;
    if !a.is_dir() {
        return c.remove(p);
    }
    for e in c.read_dir(p)? {
        delete_remote(c, &p.join(&e.name))?;
    }
    c.rmdir(p)
}

/// `src` (a file or a folder, its whole tree) as files under `dst_dir`; the target folders are
/// made now (an empty folder is copied too).
fn plan(
    c: &mut Sftp,
    dir: Dir,
    src: &Path,
    dst_dir: &Path,
    files: &mut Vec<(PathBuf, PathBuf, u64)>,
    dirs: &mut Vec<PathBuf>,
) -> io::Result<()> {
    let Some(name) = src.file_name() else {
        return Ok(());
    };
    let dst = dst_dir.join(name);
    let (is_dir, size) = match dir {
        Dir::Down => {
            let a = c.stat(src)?;
            (a.is_dir(), a.size.unwrap_or(0))
        }
        Dir::Up => {
            let m = std::fs::metadata(src)?;
            (m.is_dir(), m.len())
        }
    };
    if !is_dir {
        files.push((src.to_path_buf(), dst, size));
        return Ok(());
    }
    // A folder that links back up would never end: links to folders are not followed.
    let link = match dir {
        Dir::Down => c.lstat(src)?.is_link(),
        Dir::Up => std::fs::symlink_metadata(src)?.file_type().is_symlink(),
    };
    if link {
        return Ok(());
    }
    let names: Vec<OsString> = match dir {
        Dir::Down => {
            ensure_local_dir(&dst)?;
            c.read_dir(src)?.into_iter().map(|e| e.name).collect()
        }
        Dir::Up => {
            ensure_remote_dir(c, &dst)?;
            std::fs::read_dir(src)?
                .filter_map(|e| e.ok().map(|e| e.file_name()))
                .collect()
        }
    };
    dirs.push(src.to_path_buf());
    for n in names {
        plan(c, dir, &src.join(n), &dst, files, dirs)?;
    }
    Ok(())
}

fn ensure_local_dir(p: &Path) -> io::Result<()> {
    std::fs::create_dir_all(p)
}

/// `mkdir -p` on the server.
fn ensure_remote_dir(c: &mut Sftp, p: &Path) -> io::Result<()> {
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

/// One file. `Ok(None)`: the target is there and nobody said what to do; `Ok(Some(true))`:
/// skipped.
fn transfer(
    c: &mut Sftp,
    job: &Job,
    choice: Option<Choice>,
    send: &impl Fn(Event),
    cancel: &AtomicBool,
) -> io::Result<Option<bool>> {
    let exists = match job.dir {
        Dir::Down => job.dst.exists(),
        Dir::Up => c.stat(&job.dst).is_ok(),
    };
    match (exists, choice) {
        (true, None) => return Ok(None),
        (true, Some(Choice::Skip)) => return Ok(Some(true)),
        _ => {}
    }
    let mut last = Instant::now();
    let progress = |n: u64| {
        // A few updates a second are enough for the queue.
        if last.elapsed() > Duration::from_millis(150) {
            last = Instant::now();
            send(Event::Progress(job.id, n));
        }
        !cancel.load(Ordering::SeqCst)
    };
    let r = match job.dir {
        Dir::Down => {
            if let Some(parent) = job.dst.parent() {
                ensure_local_dir(parent)?;
            }
            let mut out = std::io::BufWriter::new(std::fs::File::create(&job.dst)?);
            let r = c.download(&job.src, &mut out, progress);
            let r = r.and_then(|n| std::io::Write::flush(&mut out).map(|()| n));
            if r.is_err() {
                drop(out);
                let _ = std::fs::remove_file(&job.dst); // not a half file
            }
            r
        }
        Dir::Up => {
            let mut input = std::fs::File::open(&job.src)?;
            let r = c.upload(&mut input, &job.dst, progress);
            if r.as_ref().is_err_and(|e| !fatal(e)) {
                let _ = c.remove(&job.dst);
            }
            r
        }
    };
    r?;
    if job.is_move {
        match job.dir {
            Dir::Down => c.remove(&job.src)?,
            Dir::Up => std::fs::remove_file(&job.src)?,
        }
    }
    Ok(Some(false))
}
