//! A tab's session log: the PTY's output copied to `core::logfile::Logger` on its way to the
//! terminal. The terminal's reader thread writes, the UI starts and stops it, so it is shared.

use alacritty_terminal::event::{OnResize, WindowSize};
use alacritty_terminal::tty::{self, ChildEvent, EventedPty, EventedReadWrite};
use polling::{Event, PollMode, Poller};
use shagshell_core::logfile::Logger;
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
    failed: Arc<Mutex<Option<String>>>,
}

impl Tap {
    fn lock(&self) -> std::sync::MutexGuard<'_, Option<Logger>> {
        self.logger.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn start(&self, path: &Path, append: bool, raw: bool) -> io::Result<()> {
        let logger = Logger::open(path, append, raw)?;
        *self.failed.lock().unwrap_or_else(|e| e.into_inner()) = None;
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
        self.failed.try_lock().ok()?.take()
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
            *self.failed.lock().unwrap_or_else(|e| e.into_inner()) = Some(msg);
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
            self.tap.write(&buf[..n]);
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
