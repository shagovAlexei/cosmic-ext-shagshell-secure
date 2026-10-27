//! A Script Builder run (spec 36, part 4): where the script is, and whether it may go on.
//! `crt.py` (trace mode) reports every line of the script (`Builder.Line`); a line that must
//! stop (a breakpoint, a step, Pause) sends its variables (`Builder.Paused`) and waits here until
//! the window says Continue / Step, or the run is cancelled. Every thread of the script (parallel
//! hosts) stops while one is paused.

use std::collections::{BTreeSet, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

/// How often the window hears where the script is (it draws the blocks again).
pub const REPORT: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Run,
    /// Stop at the next line (any thread).
    Step,
    /// Stopped, or stopping at the next line.
    Pause,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// The line now, and the lines run since the last report.
    Progress { current: u64, seen: Vec<u64> },
    /// Stopped at `line`, with the frame's variables (name, short repr).
    Paused {
        line: u64,
        vars: Vec<(String, String)>,
    },
}

#[derive(Default)]
struct State {
    mode: Mode,
    breaks: HashSet<u64>,
    seen: BTreeSet<u64>,
    current: u64,
    paused: bool,
    reported: Option<Instant>,
}

#[derive(Default)]
pub struct Debugger {
    st: Mutex<State>,
    cv: Condvar,
}

impl Debugger {
    pub fn new(breaks: impl IntoIterator<Item = u64>) -> Self {
        Self {
            st: Mutex::new(State {
                breaks: breaks.into_iter().collect(),
                ..State::default()
            }),
            cv: Condvar::new(),
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.st.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The script reached `line`; `true`: it must stop here (it sends its variables next).
    pub fn line(&self, line: u64, notify: &dyn Fn(Event)) -> bool {
        let mut st = self.state();
        st.seen.insert(line);
        st.current = line;
        let stop = st.mode != Mode::Run || st.breaks.contains(&line);
        if stop {
            // Every thread stops now, not only the one that met the breakpoint.
            st.mode = Mode::Pause;
            st.paused = true;
        }
        if stop || st.reported.is_none_or(|t| t.elapsed() >= REPORT) {
            st.reported = Some(Instant::now());
            let seen = std::mem::take(&mut st.seen).into_iter().collect();
            notify(Event::Progress {
                current: line,
                seen,
            });
        }
        stop
    }

    /// Stopped at `line` with `vars`: waits until Continue / Step, or the run is cancelled.
    pub fn paused(
        &self,
        line: u64,
        vars: Vec<(String, String)>,
        cancel: &AtomicBool,
        notify: &dyn Fn(Event),
    ) {
        // Continue came first: the window hears of no pause. (`notify` runs unlocked: it may
        // answer with `resume`.)
        if !self.state().paused {
            return;
        }
        notify(Event::Paused { line, vars });
        let mut st = self.state();
        while st.paused && !cancel.load(Ordering::SeqCst) {
            st = self
                .cv
                .wait_timeout(st, Duration::from_millis(50))
                .map_or_else(|e| e.into_inner().0, |r| r.0);
        }
    }

    /// The window: Continue (`Run`), `Step`, `Pause`.
    pub fn resume(&self, mode: Mode) {
        let mut st = self.state();
        st.mode = mode;
        st.paused = mode == Mode::Pause && st.paused;
        self.cv.notify_all();
    }

    /// The window's breakpoints now (put on / taken off while it runs).
    pub fn set_breaks(&self, lines: impl IntoIterator<Item = u64>) {
        self.state().breaks = lines.into_iter().collect();
    }

    /// The lines not reported yet (the run ended).
    pub fn flush(&self, notify: &dyn Fn(Event)) {
        let mut st = self.state();
        let seen: Vec<u64> = std::mem::take(&mut st.seen).into_iter().collect();
        notify(Event::Progress {
            current: st.current,
            seen,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn events() -> (Arc<Mutex<Vec<Event>>>, impl Fn(Event)) {
        let e = Arc::new(Mutex::new(vec![]));
        let e2 = e.clone();
        (e, move |x| e2.lock().unwrap().push(x))
    }

    #[test]
    fn runs_through_and_reports() {
        let d = Debugger::new([]);
        let (ev, notify) = events();
        assert!(
            !d.line(1, &notify),
            "the first line is reported, not a stop"
        );
        assert!(!d.line(2, &notify));
        assert!(!d.line(3, &notify));
        d.flush(&notify);
        let all: BTreeSet<u64> = (ev.lock().unwrap().iter())
            .flat_map(|e| match e {
                Event::Progress { seen, .. } => seen.clone(),
                _ => vec![],
            })
            .collect();
        assert_eq!(all, BTreeSet::from([1, 2, 3]));
        assert!(matches!(
            ev.lock().unwrap().last(),
            Some(Event::Progress { current: 3, .. })
        ));
    }

    #[test]
    fn breakpoint_stops_until_continue() {
        let d = Arc::new(Debugger::new([2]));
        let (ev, notify) = events();
        assert!(!d.line(1, &notify));
        assert!(d.line(2, &notify));
        let d2 = d.clone();
        let t = std::thread::spawn(move || {
            d2.paused(
                2,
                vec![("x".into(), "5".into())],
                &AtomicBool::new(false),
                &|_| {},
            );
        });
        std::thread::sleep(Duration::from_millis(100));
        assert!(!t.is_finished(), "waits while paused");
        d.resume(Mode::Run);
        t.join().unwrap();
        assert!(!d.line(3, &notify), "Continue runs on");
        let _ = ev;
    }

    #[test]
    fn paused_tells_the_window() {
        let d = Debugger::new([1]);
        let (ev, notify) = events();
        assert!(d.line(1, &notify));
        let d = Arc::new(d);
        let d2 = d.clone();
        let t = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            d2.resume(Mode::Run);
        });
        d.paused(
            1,
            vec![("n".into(), "3".into())],
            &AtomicBool::new(false),
            &notify,
        );
        t.join().unwrap();
        assert!(ev.lock().unwrap().contains(&Event::Paused {
            line: 1,
            vars: vec![("n".into(), "3".into())]
        }));
    }

    #[test]
    fn regression_continue_before_paused_tells_nothing() {
        // Continue pressed between `line` (stop) and `paused`: the window must not show a
        // pause that is already over.
        let d = Debugger::new([1]);
        let (ev, notify) = events();
        assert!(d.line(1, &notify));
        d.resume(Mode::Run);
        d.paused(1, vec![], &AtomicBool::new(false), &notify);
        assert!(!(ev.lock().unwrap().iter()).any(|e| matches!(e, Event::Paused { .. })));
    }

    #[test]
    fn step_stops_at_the_next_line() {
        let d = Debugger::new([]);
        let (_, notify) = events();
        assert!(!d.line(1, &notify));
        d.resume(Mode::Step);
        assert!(d.line(2, &notify));
        d.resume(Mode::Run);
        assert!(!d.line(3, &notify));
    }

    #[test]
    fn pause_from_the_window_stops_every_thread() {
        let d = Debugger::new([]);
        let (_, notify) = events();
        d.resume(Mode::Pause);
        assert!(d.line(7, &notify), "one host");
        assert!(d.line(9, &notify), "another host");
    }

    #[test]
    fn cancel_ends_the_wait() {
        let d = Arc::new(Debugger::new([1]));
        let cancel = Arc::new(AtomicBool::new(false));
        let (d2, c2) = (d.clone(), cancel.clone());
        let t = std::thread::spawn(move || d2.paused(1, vec![], &c2, &|_| {}));
        std::thread::sleep(Duration::from_millis(50));
        cancel.store(true, Ordering::SeqCst);
        t.join().unwrap();
    }

    #[test]
    fn breakpoints_change_while_running() {
        let d = Debugger::new([3]);
        let (_, notify) = events();
        d.set_breaks([5]);
        assert!(!d.line(3, &notify), "taken off");
        assert!(d.line(5, &notify), "put on");
    }
}
