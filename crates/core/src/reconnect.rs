//! When a dropped ssh connection is connected again by itself.

use std::time::Duration;

/// A connection that stayed up this long was a real one (not a refused or failed start).
pub const HELD: Duration = Duration::from_secs(10);

/// What a tab's program ended with.
pub struct Ended {
    /// ssh's exit code (255: the connection is lost).
    pub code: Option<i32>,
    /// The user ended it (Disconnect).
    pub stopped: bool,
    /// How long that run lasted.
    pub uptime: Duration,
    /// Automatic reconnects since the connection last stayed up.
    pub attempts: u32,
}

/// `Some(n)`: connect again, this being attempt `n` of `tries`; `None`: leave it ended. A run
/// that stayed up starts the count over; a failed start is retried only inside a series that a
/// real drop began (a wrong password or an unknown host is not repeated by itself).
pub fn next_attempt(e: &Ended, tries: u32) -> Option<u32> {
    if e.code != Some(255) || e.stopped {
        return None;
    }
    let held = e.uptime >= HELD;
    let attempts = if held { 0 } else { e.attempts };
    ((held || attempts > 0) && attempts < tries).then_some(attempts + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ended(code: i32, stopped: bool, secs: u64, attempts: u32) -> Ended {
        Ended {
            code: Some(code),
            stopped,
            uptime: Duration::from_secs(secs),
            attempts,
        }
    }

    #[test]
    fn a_lost_connection_is_retried_and_a_failed_start_is_not() {
        assert_eq!(next_attempt(&ended(255, false, 60, 0), 3), Some(1));
        assert_eq!(next_attempt(&ended(255, false, 2, 0), 3), None); // refused at once
        assert_eq!(next_attempt(&ended(255, false, 2, 1), 3), Some(2)); // inside a series
        assert_eq!(next_attempt(&ended(255, false, 2, 3), 3), None); // out of attempts
        assert_eq!(next_attempt(&ended(255, false, 60, 3), 3), Some(1)); // it stayed up: afresh
    }

    #[test]
    fn exits_that_are_not_a_drop() {
        assert_eq!(next_attempt(&ended(0, false, 60, 0), 3), None); // `exit`
        assert_eq!(next_attempt(&ended(1, false, 60, 0), 3), None);
        assert_eq!(next_attempt(&ended(255, true, 60, 0), 3), None); // Disconnect
        let none = Ended {
            code: None,
            ..ended(0, false, 60, 0)
        };
        assert_eq!(next_attempt(&none, 3), None);
    }
}
