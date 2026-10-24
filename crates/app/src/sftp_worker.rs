//! An SFTP tab's server I/O, off the UI thread: `ssh … -s -- host sftp` and a
//! `core::sftp::proto::Client` on its pipes, owned by one thread. Commands in, events out (tagged
//! with the tab's id, through the subscription in `sftp_view`). A tab has two: one for browsing,
//! one for transfers, so listing a folder doesn't wait for a big copy.

use crate::term::TabId;
use shagshell_core::panel::Entry;
use shagshell_core::sftp::ops;
use shagshell_core::sftp::proto::Client;
use shagshell_core::sftp::queue::{Choice, Dir, Job};
use std::ffi::OsString;
use std::io::{self, Read};
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
    /// The folder can't be listed: why.
    ListFailed(PathBuf, String),
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

/// Gone (the tab closed, Reconnect): a copy it runs stops now, not at the end of the file.
impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::SeqCst);
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

/// `Err`: the connection is gone.
fn handle(c: &mut Sftp, cmd: Cmd, send: &impl Fn(Event), cancel: &AtomicBool) -> io::Result<()> {
    let soft = |r: io::Result<()>| match r {
        Err(e) if ops::is_fatal(&e) => Err(e),
        r => Ok(r.err().map(|e| e.to_string())),
    };
    match cmd {
        Cmd::List { path, focus } => match ops::list(c, &path) {
            Ok(entries) => send(Event::Listed {
                path,
                entries,
                focus,
            }),
            Err(e) if ops::is_fatal(&e) => return Err(e),
            // Said, not listed again (the folder may be gone: no loop).
            Err(e) => send(Event::ListFailed(path, e.to_string())),
        },
        Cmd::Mkdir(p) => send(Event::Changed(soft(c.mkdir(&p))?)),
        Cmd::Rename(a, b) => send(Event::Changed(soft(c.rename(&a, &b))?)),
        Cmd::Delete(paths) => {
            let r = paths.iter().try_for_each(|p| ops::delete_remote(c, p));
            send(Event::Changed(soft(r)?));
        }
        Cmd::Plan {
            batch,
            dir,
            srcs,
            dst_dir,
            is_move,
        } => {
            let (plan, error) = match ops::plan(c, dir, &srcs, &dst_dir) {
                Ok(p) => (p, None),
                Err(e) if ops::is_fatal(&e) => return Err(e),
                Err(e) => (ops::Plan::default(), Some(e.to_string())),
            };
            send(Event::Planned {
                batch,
                dir,
                files: plan.files,
                dirs: plan.dirs,
                is_move,
                error,
            });
        }
        Cmd::Run { job, choice } => {
            cancel.store(false, Ordering::SeqCst);
            let mut last = Instant::now();
            let progress = |n: u64| {
                // A few updates a second are enough for the queue.
                if last.elapsed() > Duration::from_millis(150) {
                    last = Instant::now();
                    send(Event::Progress(job.id, n));
                }
                !cancel.load(Ordering::SeqCst)
            };
            match ops::transfer(c, &job, choice, progress) {
                Ok(ops::Copied::Exists) => send(Event::Conflict(job.id)),
                Ok(done) => send(Event::Finished(job.id, Ok(done == ops::Copied::Skipped))),
                Err(e) => {
                    send(Event::Finished(job.id, Err(e.to_string())));
                    if ops::is_fatal(&e) {
                        return Err(e);
                    }
                }
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
