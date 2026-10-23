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
    /// The finished lines seen (the line being written is the cleaner's).
    seen: String,
    /// How much of `seen` + the line being written earlier steps have used up.
    skip: usize,
}

impl Automaton {
    pub fn new(steps: Vec<Step>) -> Self {
        Self {
            steps,
            at: 0,
            clean: Cleaner::default(),
            seen: String::new(),
            skip: 0,
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
            self.skip = self.skip.saturating_sub(at);
        }
        self.fire(true)
    }

    fn fire(&mut self, with_output: bool) -> Vec<String> {
        let mut sends = Vec::new();
        while let Some(step) = self.steps.get(self.at) {
            if step.expect.is_empty() {
                // Waits for nothing: at the start, or right after the step before it.
                if with_output && sends.is_empty() && self.at == 0 {
                    break;
                }
            } else {
                let text = format!("{}{}", self.seen, self.clean.partial());
                let from = (self.skip.min(text.len())..text.len())
                    .find(|&i| text.is_char_boundary(i))
                    .unwrap_or(text.len());
                let (tail, want) = (text[from..].to_lowercase(), step.expect.to_lowercase());
                match tail.find(&want) {
                    // What was matched is used up: the same prompt answers one step only, and
                    // what follows it in the same chunk is looked at for the next step.
                    Some(at) => self.skip = from + at + want.len().min(tail.len() - at),
                    None => break,
                }
            }
            sends.push(step.send.clone());
            self.at += 1;
        }
        sends
    }
}

/// `%user%` and `%password%` in one piece of bytes to send, after escapes were read (so a
/// password with a backslash is sent as it is). `None`: it needs a password and there is none.
pub fn fill(bytes: &[u8], user: &str, password: Option<&str>) -> Option<Vec<u8>> {
    let replace = |hay: &[u8], needle: &[u8], with: &[u8]| -> Vec<u8> {
        let mut out = Vec::with_capacity(hay.len());
        let mut i = 0;
        while i < hay.len() {
            if hay[i..].starts_with(needle) {
                out.extend_from_slice(with);
                i += needle.len();
            } else {
                out.push(hay[i]);
                i += 1;
            }
        }
        out
    };
    let out = replace(bytes, b"%user%", user.as_bytes());
    match out.windows(10).any(|w| w == b"%password%") {
        true => Some(replace(&out, b"%password%", password?.as_bytes())),
        false => Some(out),
    }
}

/// The send string needs the saved password.
pub fn needs_password(send: &str) -> bool {
    send.contains("%password%")
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
    fn regression_two_prompts_in_one_chunk_answer_two_steps() {
        let mut a = Automaton::new(vec![step("login:", "u"), step("password:", "p")]);
        assert_eq!(a.feed(b"login: \r\nPassword: "), ["u", "p"]);
        // the second one's text came before the first's: it must not count
        let mut b = Automaton::new(vec![step("login:", "u"), step("password:", "p")]);
        assert_eq!(b.feed(b"password: login: "), ["u"]);
        assert_eq!(b.feed(b"Password:"), ["p"]);
    }

    #[test]
    fn regression_a_password_with_escapes_is_sent_as_it_is() {
        assert_eq!(fill(b"%user%", "adm", None).unwrap(), b"adm");
        assert_eq!(
            fill(b"x%password%y", "u", Some(r"a\pb\r")).unwrap(),
            br"xa\pb\ry"
        );
        assert_eq!(fill(b"%password%", "u", None), None);
        assert!(needs_password("a%password%") && !needs_password("%user%"));
    }
}
