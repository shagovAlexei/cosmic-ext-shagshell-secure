//! Logon Actions (SecureCRT's): after connecting, wait for a text in the session's output and
//! send an answer, step by step (`Username:` → the user, `Password:` → the password…). The
//! matching is here; sending is the app's (it knows the PTY and the keyring).

use crate::logfile::Cleaner;
use serde::{Deserialize, Serialize};

/// One row of the table: when `expect` has been seen, `send` goes out. An empty `expect` fires
/// at once (before anything arrives).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Step {
    pub expect: String,
    /// Escapes of `core::macros`; `%user%` and `%password%` are filled in when it is sent.
    pub send: String,
}

/// How much of the latest output is looked through (a prompt is short).
const WINDOW: usize = 4096;

pub struct Automaton {
    steps: Vec<Step>,
    at: usize,
    clean: Cleaner,
    /// The text seen since the last match: finished lines and the one being written.
    seen: String,
}

impl Automaton {
    pub fn new(steps: Vec<Step>) -> Self {
        Self {
            steps,
            at: 0,
            clean: Cleaner::default(),
            seen: String::new(),
        }
    }

    pub fn done(&self) -> bool {
        self.at >= self.steps.len()
    }

    /// The sends due before any output (steps with an empty `expect`).
    pub fn start(&mut self) -> Vec<String> {
        self.fire(false)
    }

    /// Output arrived: the sends it makes due (in order; one chunk may complete several steps).
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<String> {
        if self.done() {
            return Vec::new();
        }
        let mut out = Vec::new();
        self.clean.feed(bytes, &mut out);
        self.seen.push_str(&String::from_utf8_lossy(&out));
        if self.seen.len() > WINDOW {
            let cut = self.seen.len() - WINDOW;
            let at = (cut..self.seen.len())
                .find(|&i| self.seen.is_char_boundary(i))
                .unwrap_or(self.seen.len());
            self.seen.drain(..at);
        }
        self.fire(true)
    }

    fn fire(&mut self, with_output: bool) -> Vec<String> {
        let mut sends = Vec::new();
        while let Some(step) = self.steps.get(self.at) {
            let hit = match step.expect.is_empty() {
                true => !with_output || !self.seen.is_empty() || sends.is_empty(),
                false => {
                    let text = format!("{}{}", self.seen, self.clean.partial());
                    text.to_lowercase().contains(&step.expect.to_lowercase())
                }
            };
            if !hit {
                break;
            }
            sends.push(step.send.clone());
            self.at += 1;
            // What was matched is used up: the same prompt does not answer the next step.
            self.seen.clear();
            self.clean = Cleaner::default();
            if !step.expect.is_empty() {
                break;
            }
        }
        sends
    }
}

/// `%user%` and `%password%` in a send string (`None`: no saved password; nothing to send).
pub fn fill(send: &str, user: &str, password: Option<&str>) -> Option<String> {
    let mut out = send.replace("%user%", user);
    if out.contains("%password%") {
        out = out.replace("%password%", password?);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(e: &str, s: &str) -> Step {
        Step {
            expect: e.into(),
            send: s.into(),
        }
    }

    #[test]
    fn a_login_dialogue() {
        let mut a = Automaton::new(vec![
            step("login:", "adm\\r"),
            step("Password:", "%password%\\r"),
            step("#", "terminal length 0\\r"),
        ]);
        assert!(a.start().is_empty());
        assert!(a.feed(b"Welcome\r\nUser Acc").is_empty());
        assert_eq!(a.feed(b"ess Verification\r\nLogin:"), ["adm\\r"]);
        assert!(a.feed(b" adm\r\n").is_empty());
        assert_eq!(a.feed(b"passw"), Vec::<String>::new()); // split across chunks
        assert_eq!(a.feed(b"ord: "), ["%password%\\r"]);
        assert!(!a.done());
        assert_eq!(a.feed(b"\x1b[1mrouter#\x1b[0m "), ["terminal length 0\\r"]);
        assert!(a.done());
        assert!(a.feed(b"router# ").is_empty());
    }

    #[test]
    fn one_prompt_answers_one_step_and_an_empty_expect_goes_first() {
        let mut a = Automaton::new(vec![step("", "\\r"), step("$", "a"), step("$", "b")]);
        assert_eq!(a.start(), ["\\r"]);
        assert_eq!(a.feed(b"host$ "), ["a"]); // that `$` is used up
        assert!(a.feed(b"x").is_empty());
        assert_eq!(a.feed(b"host$ "), ["b"]);
        // empty expects in a row all go at the start
        let mut b = Automaton::new(vec![step("", "1"), step("", "2")]);
        assert_eq!(b.start(), ["1", "2"]);
        assert!(b.done());
    }

    #[test]
    fn fill_in_user_and_password() {
        assert_eq!(fill("%user%\\r", "adm", None).unwrap(), "adm\\r");
        assert_eq!(fill("%password%", "u", Some("s3")).unwrap(), "s3");
        assert_eq!(fill("%password%", "u", None), None);
    }
}
