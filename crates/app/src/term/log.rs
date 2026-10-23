//! A tab's session log: the PTY's output copied to `core::logfile::Logger` on its way to the
//! terminal. The terminal's reader thread writes, the UI starts and stops it, so it is shared.

use alacritty_terminal::event::{OnResize, WindowSize};
use alacritty_terminal::event_loop::{EventLoopSender, Msg};
use alacritty_terminal::tty::{self, ChildEvent, EventedPty, EventedReadWrite};
use polling::{Event, PollMode, Poller};
use shagshell_core::logfile::Logger;
use shagshell_core::logon::{Automaton, Step};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
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
        let n = self.file.read(buf)?;
        if n > 0 {
            self.tap.output(&buf[..n]);
        }
        Ok(n)
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
