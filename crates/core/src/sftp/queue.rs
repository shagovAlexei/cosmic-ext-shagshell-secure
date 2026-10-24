//! The transfer queue of an SFTP tab: a pure state machine. The app feeds it what was planned
//! (folders already laid out into files by the worker), takes the next job, and tells it what
//! the worker reported. One job runs at a time.

use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    /// Local → server.
    Up,
    /// Server → local.
    Down,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    Waiting,
    Running,
    Done,
    Failed(String),
    Cancelled,
    /// The target was there and the user said skip.
    Skipped,
}

/// The target exists: what to do (Cancel = `cancel_batch`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Overwrite,
    Skip,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Job {
    pub id: u64,
    /// One F5 / F6: «for all» answers and the folders a move removes belong to it.
    pub batch: u64,
    pub dir: Dir,
    pub src: PathBuf,
    pub dst: PathBuf,
    pub size: u64,
    pub done: u64,
    pub state: State,
    /// F6: the source goes once it is copied whole.
    pub is_move: bool,
}

#[derive(Default)]
pub struct Queue {
    jobs: Vec<Job>,
    next_id: u64,
    next_batch: u64,
    /// «For all» answers, per batch.
    answers: HashMap<u64, Choice>,
}

impl Queue {
    pub fn jobs(&self) -> &[Job] {
        &self.jobs
    }

    /// A new batch number for `add`.
    pub fn batch(&mut self) -> u64 {
        self.next_batch += 1;
        self.next_batch
    }

    /// Files `(src, dst, size)` of one batch, waiting.
    pub fn add(
        &mut self,
        batch: u64,
        dir: Dir,
        files: Vec<(PathBuf, PathBuf, u64)>,
        is_move: bool,
    ) {
        for (src, dst, size) in files {
            self.next_id += 1;
            self.jobs.push(Job {
                id: self.next_id,
                batch,
                dir,
                src,
                dst,
                size,
                done: 0,
                state: State::Waiting,
                is_move,
            });
        }
    }

    pub fn running(&self) -> Option<&Job> {
        self.jobs.iter().find(|j| j.state == State::Running)
    }

    /// The next waiting job, now running; none while one runs.
    pub fn start_next(&mut self) -> Option<Job> {
        if self.running().is_some() {
            return None;
        }
        let j = self.jobs.iter_mut().find(|j| j.state == State::Waiting)?;
        j.state = State::Running;
        Some(j.clone())
    }

    fn get(&mut self, id: u64) -> Option<&mut Job> {
        self.jobs.iter_mut().find(|j| j.id == id)
    }

    pub fn progress(&mut self, id: u64, done: u64) {
        if let Some(j) = self.get(id).filter(|j| j.state == State::Running) {
            j.done = done;
        }
    }

    /// The worker's word on a running job (a cancelled one stays cancelled).
    pub fn finish(&mut self, id: u64, r: Result<(), String>) {
        if let Some(j) = self.get(id).filter(|j| j.state == State::Running) {
            match r {
                Ok(()) => {
                    j.done = j.size;
                    j.state = State::Done;
                }
                Err(e) => j.state = State::Failed(e),
            }
        }
    }

    pub fn skip(&mut self, id: u64) {
        if let Some(j) = self.get(id).filter(|j| j.state == State::Running) {
            j.state = State::Skipped;
        }
    }

    /// Waiting or running → cancelled. True if it was running (the worker must stop it).
    pub fn cancel(&mut self, id: u64) -> bool {
        match self.get(id) {
            Some(j) if matches!(j.state, State::Waiting | State::Running) => {
                let was = j.state == State::Running;
                j.state = State::Cancelled;
                was
            }
            _ => false,
        }
    }

    /// The conflict dialog's Cancel: the rest of the batch. True if one was running.
    pub fn cancel_batch(&mut self, batch: u64) -> bool {
        let ids: Vec<u64> = (self.jobs.iter())
            .filter(|j| j.batch == batch)
            .map(|j| j.id)
            .collect();
        let mut running = false;
        for id in ids {
            running |= self.cancel(id);
        }
        running
    }

    /// A failed or cancelled job again, from the start.
    pub fn retry(&mut self, id: u64) {
        if let Some(j) = self
            .get(id)
            .filter(|j| matches!(j.state, State::Failed(_) | State::Cancelled))
        {
            j.done = 0;
            j.state = State::Waiting;
        }
    }

    /// «Очистить готовые»: done and skipped go; failed stay for a retry.
    pub fn clear_done(&mut self) {
        self.jobs
            .retain(|j| !matches!(j.state, State::Done | State::Skipped));
    }

    pub fn answer(&mut self, batch: u64, choice: Choice) {
        self.answers.insert(batch, choice);
    }

    /// The «for all» answer of the batch, if given.
    pub fn answered(&self, batch: u64) -> Option<Choice> {
        self.answers.get(&batch).copied()
    }

    /// Nothing of the batch left to run, and every job of it copied (a move may remove its
    /// source folders then).
    pub fn batch_done(&self, batch: u64) -> bool {
        let mut it = self.jobs.iter().filter(|j| j.batch == batch).peekable();
        it.peek().is_some() && it.all(|j| j.state == State::Done)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(n: u64) -> Vec<(PathBuf, PathBuf, u64)> {
        (0..n)
            .map(|i| (format!("/s/{i}").into(), format!("/d/{i}").into(), 10))
            .collect()
    }

    #[test]
    fn one_at_a_time_in_order() {
        let mut q = Queue::default();
        let b = q.batch();
        q.add(b, Dir::Up, files(2), false);
        let a = q.start_next().unwrap();
        assert_eq!(a.src, PathBuf::from("/s/0"));
        assert_eq!(q.start_next(), None); // one runs
        q.progress(a.id, 4);
        assert_eq!(q.running().unwrap().done, 4);
        q.finish(a.id, Ok(()));
        assert_eq!(q.jobs()[0].done, 10);
        assert_eq!(q.start_next().unwrap().src, PathBuf::from("/s/1"));
    }

    #[test]
    fn fail_then_retry_from_start() {
        let mut q = Queue::default();
        let b = q.batch();
        q.add(b, Dir::Down, files(1), false);
        let j = q.start_next().unwrap();
        q.progress(j.id, 7);
        q.finish(j.id, Err("Permission denied".into()));
        assert_eq!(q.jobs()[0].state, State::Failed("Permission denied".into()));
        q.retry(j.id);
        assert_eq!((q.jobs()[0].done, &q.jobs()[0].state), (0, &State::Waiting));
        q.retry(j.id); // not failed now: nothing
        assert_eq!(q.start_next().unwrap().id, j.id);
    }

    #[test]
    fn cancel_waiting_and_running() {
        let mut q = Queue::default();
        let b = q.batch();
        q.add(b, Dir::Up, files(3), false);
        let run = q.start_next().unwrap();
        assert!(!q.cancel(q.jobs()[2].id));
        assert!(q.cancel(run.id));
        q.finish(run.id, Ok(())); // the worker's late word: still cancelled
        assert_eq!(q.jobs()[0].state, State::Cancelled);
        assert_eq!(q.start_next().unwrap().src, PathBuf::from("/s/1"));
    }

    #[test]
    fn cancel_batch_leaves_others() {
        let mut q = Queue::default();
        let (a, b) = (q.batch(), q.batch());
        q.add(a, Dir::Up, files(2), false);
        q.add(b, Dir::Up, files(1), false);
        q.start_next().unwrap();
        assert!(q.cancel_batch(a));
        let left: Vec<_> = q.jobs().iter().map(|j| j.state.clone()).collect();
        assert_eq!(left, [State::Cancelled, State::Cancelled, State::Waiting]);
    }

    #[test]
    fn clear_keeps_active_and_failed() {
        let mut q = Queue::default();
        let b = q.batch();
        q.add(b, Dir::Up, files(4), false);
        let j = q.start_next().unwrap();
        q.finish(j.id, Ok(()));
        let j = q.start_next().unwrap();
        q.skip(j.id);
        let j = q.start_next().unwrap();
        q.finish(j.id, Err("x".into()));
        q.start_next().unwrap();
        q.clear_done();
        let left: Vec<_> = q.jobs().iter().map(|j| j.state.clone()).collect();
        assert_eq!(left, [State::Failed("x".into()), State::Running]);
    }

    #[test]
    fn answers_per_batch() {
        let mut q = Queue::default();
        let (a, b) = (q.batch(), q.batch());
        q.answer(a, Choice::Skip);
        assert_eq!(q.answered(a), Some(Choice::Skip));
        assert_eq!(q.answered(b), None);
    }

    #[test]
    fn batch_done_only_when_all_copied() {
        let mut q = Queue::default();
        let b = q.batch();
        q.add(b, Dir::Up, files(2), true);
        let j = q.start_next().unwrap();
        q.finish(j.id, Ok(()));
        assert!(!q.batch_done(b));
        let j = q.start_next().unwrap();
        q.skip(j.id);
        assert!(!q.batch_done(b)); // a skipped one stays at the source: its folder must too
        assert!(!q.batch_done(99));
    }
}
