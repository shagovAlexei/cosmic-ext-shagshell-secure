//! A tab's session log: the PTY's output copied to `core::logfile::Logger` on its way to the
//! terminal. The terminal's reader thread writes, the UI starts and stops it, so it is shared.

use crate::fl;
use alacritty_terminal::event::{OnResize, WindowSize};
use alacritty_terminal::event_loop::{EventLoopSender, Msg};
use alacritty_terminal::tty::{self, ChildEvent, EventedPty, EventedReadWrite};
use polling::{Event, PollMode, Poller};
use shagshell_core::logfile::Logger;
use shagshell_core::logon::{Automaton, Step};
use shagshell_core::script::Buffer;
use shagshell_core::zmodem::{self, Detector};
use std::fs::File;
use std::io::Write;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{ChildStdin, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct Tap {
    logger: Arc<Mutex<Option<Logger>>>,
    /// Asked by the UI on every frame: no lock (the reader thread holds one while it writes, and
    /// a stalled disk must not stall the window).
    on: Arc<AtomicBool>,
    /// Why the log stopped by itself (disk full…), until it is asked for.
    failed: Arc<Mutex<std::collections::VecDeque<String>>>,
    /// Logon Actions: watches the output, and what to do with an answer that is due.
    logon: Arc<Mutex<Option<(Automaton, Sender)>>>,
    /// The PTY's input (set when the terminal is made).
    pty: Arc<Mutex<Option<EventLoopSender>>>,
    /// Scripts (`crt.Screen.WaitForString`): the output as text, while one runs.
    script: Arc<Buffer>,
    /// A file transfer (lrzsz) is running: the output goes to its stdin, not to the terminal.
    xfer: Arc<Mutex<Option<(u64, ChildStdin)>>>,
    /// The last transfer that was cancelled (its end is then said as such).
    cancelled: Arc<std::sync::atomic::AtomicU64>,
    /// Where a Zmodem send from the server is received (none: not noticed).
    auto: Arc<Mutex<Option<PathBuf>>>,
    detector: Arc<Mutex<Detector>>,
}

/// What the automaton's sends are handed to (the app expands and writes them).
pub type Sender = Arc<dyn Fn(String) + Send + Sync>;

impl Tap {
    fn lock(&self) -> std::sync::MutexGuard<'_, Option<Logger>> {
        self.logger.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn start(&self, path: &Path, append: bool, raw: bool) -> io::Result<()> {
        let logger = Logger::open(path, append, raw)?;
        self.failed
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        let old = self.lock().replace(logger);
        self.on.store(true, Ordering::SeqCst);
        drop(old); // written out here, not under the lock
        Ok(())
    }

    pub fn stop(&self) {
        self.on.store(false, Ordering::SeqCst);
        let old = self.lock().take();
        drop(old);
    }

    /// A script's own text into the log (through the logger, in order with the output).
    pub fn log_text(&self, text: &str) -> io::Result<()> {
        match self.lock().as_mut() {
            Some(l) => l.write(text.as_bytes()),
            None => Err(io::Error::other("the log is off")),
        }
    }

    /// The file of the log that is on.
    pub fn path(&self) -> Option<PathBuf> {
        self.lock().as_ref().map(|l| l.path.clone())
    }

    pub fn active(&self) -> bool {
        self.on.load(Ordering::SeqCst)
    }

    /// Why the log stopped by itself, once.
    pub fn take_failure(&self) -> Option<String> {
        self.failed.try_lock().ok()?.pop_front()
    }

    /// The terminal's PTY input, for what Logon Actions answer.
    pub fn set_pty(&self, sender: EventLoopSender) {
        *self.pty.lock().unwrap_or_else(|e| e.into_inner()) = Some(sender);
    }

    /// Bytes to the program as if typed.
    pub fn input(&self, bytes: Vec<u8>) {
        if let Some(s) = self.pty.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            let _ = s.send(Msg::Input(bytes.into()));
        }
    }

    /// Zmodem sends from the server are received into `dir` (none: ignored).
    pub fn set_auto_receive(&self, dir: Option<PathBuf>) {
        *self.auto.lock().unwrap_or_else(|e| e.into_inner()) = dir;
    }

    pub fn transferring(&self) -> bool {
        self.xfer
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some()
    }

    /// Runs lrzsz's `cmd` on this connection: what the server sends goes to its stdin, what it
    /// says goes to the server. Ends by itself; the result is said once (`take_failure`).
    pub fn begin(&self, cmd: &zmodem::Command) -> io::Result<()> {
        let mut slot = self.xfer.lock().unwrap_or_else(|e| e.into_inner());
        if slot.is_some() {
            return Err(io::Error::other(fl!("zm-busy")));
        }
        let mut c = std::process::Command::new(cmd.program);
        c.args(&cmd.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(dir) = &cmd.cwd {
            std::fs::create_dir_all(dir)?;
            c.current_dir(dir);
        }
        let mut child = c
            .spawn()
            .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", cmd.program)))?;
        let (stdin, mut stdout, mut stderr) =
            match (child.stdin.take(), child.stdout.take(), child.stderr.take()) {
                (Some(i), Some(o), Some(e)) => (i, o, e),
                _ => return Err(io::Error::other("pipes")),
            };
        static GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let gen_id = GEN.fetch_add(1, Ordering::Relaxed);
        *slot = Some((gen_id, stdin));
        drop(slot);
        let me = self.clone();
        let out = std::thread::spawn(move || {
            let mut buf = [0; 8192];
            while let Ok(n) = stdout.read(&mut buf) {
                if n == 0 {
                    break;
                }
                me.input(buf[..n].to_vec());
            }
        });
        let err = std::thread::spawn(move || {
            let mut s = String::new();
            let mut buf = [0; 4096];
            while let Ok(n) = stderr.read(&mut buf) {
                if n == 0 {
                    break;
                }
                s.push_str(&String::from_utf8_lossy(&buf[..n]));
                let mut keep = s.len().saturating_sub(4000);
                while !s.is_char_boundary(keep) {
                    keep += 1;
                }
                s.drain(..keep);
            }
            s
        });
        let me = self.clone();
        let name = cmd.program;
        std::thread::spawn(move || {
            let status = child.wait();
            let _ = out.join();
            let said = err.join().unwrap_or_default();
            // Only its own: a newer transfer may have begun since it was cancelled.
            {
                let mut slot = me.xfer.lock().unwrap_or_else(|e| e.into_inner());
                if slot.as_ref().is_some_and(|(g, _)| *g == gen_id) {
                    *slot = None;
                }
            }
            let line = zmodem::last_line(&said).unwrap_or_default();
            me.fail(match status {
                _ if me.cancelled.load(Ordering::SeqCst) == gen_id => fl!("zm-cancelled"),
                Ok(s) if s.success() => fl!("zm-done", what = line),
                Ok(s) => fl!(
                    "zm-failed",
                    why = if line.is_empty() { s.to_string() } else { line }
                ),
                Err(e) => fl!("zm-failed", why = format!("{name}: {e}")),
            });
        });
        Ok(())
    }

    /// Stop the transfer: the server is told (CAN), the program's input ends.
    pub fn cancel_transfer(&self) {
        let sink = self.xfer.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some((g, _)) = sink {
            self.cancelled.store(g, Ordering::SeqCst);
            self.input(zmodem::CANCEL.to_vec());
        }
    }

    /// The PTY gave `bytes`: a transfer takes them (`true`), or a server's Zmodem send starts one,
    /// or they are for the terminal (`false`).
    fn route(&self, bytes: &[u8]) -> bool {
        if self.feed_transfer(bytes) {
            return true;
        }
        let dir = self.auto.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let Some(dir) = dir else { return false };
        let start = self
            .detector
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .feed(bytes);
        if !start {
            return false;
        }
        match self.begin(&zmodem::receive(zmodem::Proto::Zmodem, &dir, None)) {
            Ok(()) => {
                self.feed_transfer(bytes);
                true
            }
            Err(e) => {
                self.fail(fl!("zm-failed", why = e.to_string()));
                false
            }
        }
    }

    /// `true`: a transfer is running and has the bytes.
    fn feed_transfer(&self, bytes: &[u8]) -> bool {
        let mut g = self.xfer.lock().unwrap_or_else(|e| e.into_inner());
        let Some((_, w)) = g.as_mut() else {
            return false;
        };
        if w.write_all(bytes).and_then(|()| w.flush()).is_err() {
            // The program is gone: this is already the terminal's again (a prompt after the
            // last handshake).
            *g = None;
            return false;
        }
        true
    }

    /// The output as text for scripts.
    pub fn script_buffer(&self) -> Arc<Buffer> {
        self.script.clone()
    }

    /// Something went wrong that the user should see (shown once, like a log failure).
    pub fn fail(&self, why: String) {
        self.failed
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push_back(why);
    }

    /// Logon Actions on: the steps, and what receives each due answer (the first ones, those
    /// that wait for nothing, go now).
    pub fn set_logon(&self, steps: Vec<Step>, on_send: Sender) {
        let mut a = Automaton::new(steps);
        let first = a.start();
        // All rows may have gone at once: nothing is left to watch.
        if !a.done() {
            *self.logon.lock().unwrap_or_else(|e| e.into_inner()) = Some((a, on_send.clone()));
        }
        for s in first {
            on_send(s);
        }
    }

    /// The PTY gave `bytes`: to the log, and to Logon Actions.
    fn output(&self, bytes: &[u8]) {
        self.write(bytes);
        self.script.feed(bytes);
        let due = {
            let mut g = self.logon.lock().unwrap_or_else(|e| e.into_inner());
            match g.as_mut() {
                Some((a, send)) => {
                    let due = a.feed(bytes);
                    if a.done() {
                        let send = send.clone();
                        *g = None;
                        (due, Some(send))
                    } else {
                        (due, Some(send.clone()))
                    }
                }
                None => (Vec::new(), None),
            }
        };
        if let (due, Some(send)) = due {
            for s in due {
                send(s);
            }
        }
    }

    fn write(&self, bytes: &[u8]) {
        if !self.active() {
            return;
        }
        let mut guard = self.lock();
        if let Some(l) = guard.as_mut()
            && let Err(e) = l.write(bytes)
        {
            let msg = format!("{}: {e}", l.path.display());
            *guard = None;
            self.on.store(false, Ordering::SeqCst);
            self.failed
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push_back(msg);
        }
    }
}

/// What the event loop reads: the PTY's master (a second handle on the same file), and what it
/// gives goes to the log too.
pub struct TapReader {
    file: File,
    tap: Tap,
}

impl Read for TapReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut n = self.file.read(buf)?;
        loop {
            if n == 0 {
                return Ok(0);
            }
            if !self.tap.route(&buf[..n]) {
                self.tap.output(&buf[..n]);
                return Ok(n);
            }
            // A transfer has it. Read what else is waiting, so the event loop is not left with
            // data it was not told about (`WouldBlock` ends it once the pipe is empty).
            n = self.file.read(buf)?;
        }
    }
}

/// `tty::Pty` with a tap on its output.
pub struct TeePty {
    inner: tty::Pty,
    reader: TapReader,
}

impl TeePty {
    pub fn new(inner: tty::Pty, tap: Tap) -> io::Result<Self> {
        let file = inner.file().try_clone()?;
        Ok(Self {
            inner,
            reader: TapReader { file, tap },
        })
    }
}

impl EventedReadWrite for TeePty {
    type Reader = TapReader;
    type Writer = File;

    unsafe fn register(&mut self, p: &Arc<Poller>, e: Event, m: PollMode) -> io::Result<()> {
        unsafe { self.inner.register(p, e, m) }
    }

    fn reregister(&mut self, p: &Arc<Poller>, e: Event, m: PollMode) -> io::Result<()> {
        self.inner.reregister(p, e, m)
    }

    fn deregister(&mut self, p: &Arc<Poller>) -> io::Result<()> {
        self.inner.deregister(p)
    }

    fn reader(&mut self) -> &mut TapReader {
        &mut self.reader
    }

    fn writer(&mut self) -> &mut File {
        self.inner.writer()
    }
}

impl EventedPty for TeePty {
    fn next_child_event(&mut self) -> Option<ChildEvent> {
        self.inner.next_child_event()
    }
}

impl OnResize for TeePty {
    fn on_resize(&mut self, size: WindowSize) {
        self.inner.on_resize(size);
    }
}
