//! A tab's session log: the PTY's output copied to `core::logfile::Logger` on its way to the
//! terminal. The terminal's reader thread writes, the UI starts and stops it, so it is shared.

use alacritty_terminal::event::{OnResize, WindowSize};
use alacritty_terminal::tty::{self, ChildEvent, EventedPty, EventedReadWrite};
use polling::{Event, PollMode, Poller};
use shagshell_core::logfile::Logger;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct Tap(Arc<Mutex<State>>);

#[derive(Default)]
struct State {
    logger: Option<Logger>,
    /// Why the log stopped by itself (disk full…), until it is asked for.
    failed: Option<String>,
}

impl Tap {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn start(&self, path: &Path, append: bool, raw: bool) -> io::Result<()> {
        let logger = Logger::open(path, append, raw)?;
        let mut s = self.lock();
        s.failed = None;
        if let Some(old) = s.logger.replace(logger) {
            let _ = old.close();
        }
        Ok(())
    }

    pub fn stop(&self) {
        if let Some(l) = self.lock().logger.take() {
            let _ = l.close();
        }
    }

    pub fn path(&self) -> Option<PathBuf> {
        self.lock().logger.as_ref().map(|l| l.path.clone())
    }

    pub fn active(&self) -> bool {
        self.lock().logger.is_some()
    }

    /// Why the log stopped by itself, once.
    pub fn take_failure(&self) -> Option<String> {
        self.lock().failed.take()
    }

    fn write(&self, bytes: &[u8]) {
        let mut s = self.lock();
        if let Some(l) = &mut s.logger
            && let Err(e) = l.write(bytes)
        {
            let path = l.path.display().to_string();
            s.logger = None;
            s.failed = Some(format!("{path}: {e}"));
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
