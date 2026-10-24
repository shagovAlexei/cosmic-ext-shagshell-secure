//! Script → Start Recording: what is typed in a terminal, as a `crt` script.
//!
//! The recorder sees the program's output (the prompt before each step) and the user's input
//! (the step). Passwords are never kept: a step typed after a password prompt becomes a
//! `crt.Dialog.Prompt`.

use crate::askpass::{Kind, classify};
use crate::logfile::Cleaner;

/// A prompt longer than this is waited for by its last `TAIL` characters.
const LONG: usize = 80;
const TAIL: usize = 40;

#[derive(Debug, PartialEq, Eq)]
struct Step {
    /// The text on the line before the first key of the step (the prompt).
    prompt: String,
    /// What was sent, with its Enter (`\r`) when it had one.
    text: String,
    /// Typed after a password / passphrase prompt: not kept.
    secret: bool,
}

#[derive(Default)]
pub struct Recorder {
    clean: Cleaner,
    steps: Vec<Step>,
    /// The step being typed, and the prompt it started at.
    typing: String,
    prompt: Option<String>,
}

impl Recorder {
    pub fn new() -> Self {
        Self::default()
    }

    /// The program's output.
    pub fn output(&mut self, bytes: &[u8]) {
        let mut sink = Vec::new();
        self.clean.feed(bytes, &mut sink);
    }

    /// What the user typed or pasted.
    pub fn input(&mut self, bytes: &[u8]) {
        for c in String::from_utf8_lossy(bytes).chars() {
            if self.prompt.is_none() {
                self.prompt = Some(self.clean.partial().trim().to_string());
            }
            match c {
                '\r' | '\n' => self.end(Some('\r')),
                // Ctrl+C / Ctrl+D end the step on their own.
                '\x03' | '\x04' => self.end(Some(c)),
                '\x7f' | '\x08' => {
                    self.typing.pop();
                }
                c => self.typing.push(c),
            }
        }
    }

    fn end(&mut self, last: Option<char>) {
        let mut text = std::mem::take(&mut self.typing);
        text.extend(last);
        let prompt = self.prompt.take().unwrap_or_default();
        let secret = matches!(classify(&prompt), Kind::Password | Kind::Passphrase);
        self.steps.push(Step {
            prompt,
            text: if secret { String::new() } else { text },
            secret,
        });
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty() && self.typing.is_empty()
    }

    /// The Python script. What is typed but not ended by Enter is kept as the last step.
    pub fn script(mut self, header: &str) -> String {
        if !self.typing.is_empty() {
            self.end(None);
        }
        let mut out = format!(
            "# $language = \"Python\"\n# $interface = \"1.0\"\n# {header}\n\ncrt.Screen.Synchronous = True\n"
        );
        for s in &self.steps {
            out.push('\n');
            let wait = wait_text(&s.prompt);
            if !wait.is_empty() {
                out.push_str(&format!("crt.Screen.WaitForString({})\n", literal(&wait)));
            }
            if s.secret {
                out.push_str(&format!(
                    "crt.Screen.Send(crt.Dialog.Prompt({}, \"Password\", \"\", True) + \"\\r\")\n",
                    literal(&s.prompt)
                ));
            } else {
                out.push_str(&format!("crt.Screen.Send({})\n", literal(&s.text)));
            }
        }
        out
    }
}

/// What to wait for: the prompt, or its end when it is long.
fn wait_text(prompt: &str) -> String {
    let n = prompt.chars().count();
    match n > LONG {
        true => prompt.chars().skip(n - TAIL).collect(),
        false => prompt.to_string(),
    }
}

/// A Python string literal (double quotes): controls as escapes, other characters as they are.
fn literal(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\r' => out.push_str("\\r"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\x{:02x}", c as u32))
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(r: Recorder) -> String {
        let s = r.script("h");
        s.split("Synchronous = True\n").nth(1).unwrap().to_string()
    }

    #[test]
    fn each_enter_is_a_step_after_its_prompt() {
        let mut r = Recorder::new();
        r.output(b"user@host:~$ ");
        r.input(b"ls");
        r.input(b"\r");
        r.output(b"ls\r\nfile\r\nuser@host:~$ ");
        r.input(b"pwd\r");
        assert_eq!(
            body(r),
            "\ncrt.Screen.WaitForString(\"user@host:~$\")\ncrt.Screen.Send(\"ls\\r\")\n\ncrt.Screen.WaitForString(\"user@host:~$\")\ncrt.Screen.Send(\"pwd\\r\")\n"
        );
    }

    #[test]
    fn backspace_edits_the_line_and_is_not_recorded() {
        let mut r = Recorder::new();
        r.output(b"$ ");
        r.input(b"lss\x7f\r");
        assert!(body(r).contains("Send(\"ls\\r\")"));
    }

    #[test]
    fn a_password_is_never_kept() {
        let mut r = Recorder::new();
        r.output(b"user@host's password: ");
        r.input(b"hunter2\r");
        let s = body(r);
        assert!(!s.contains("hunter2"));
        assert!(
            s.contains("crt.Dialog.Prompt(\"user@host's password:\", \"Password\", \"\", True)")
        );
    }

    #[test]
    fn ctrl_c_is_a_step_and_escapes_are_literal() {
        let mut r = Recorder::new();
        r.output(b"$ ");
        r.input(b"sleep 9\r");
        r.input(b"\x03");
        r.input(b"say \"hi\" \\ \xd0\xbf\r");
        let s = body(r);
        assert!(s.contains("Send(\"\\x03\")"));
        assert!(s.contains("Send(\"say \\\"hi\\\" \\\\ \u{43f}\\r\")"));
    }

    #[test]
    fn long_prompt_waits_for_its_tail_and_empty_prompt_not_at_all() {
        let mut r = Recorder::new();
        r.output(format!("{}$ ", "d".repeat(100)).as_bytes());
        r.input(b"x\r");
        let s = body(r);
        assert!(s.contains(&format!("WaitForString(\"{}$\")", "d".repeat(39))));
        let mut r = Recorder::new();
        r.input(b"x\r");
        assert!(!body(r).contains("WaitForString"));
    }

    #[test]
    fn unfinished_line_is_kept_and_nothing_is_empty() {
        let mut r = Recorder::new();
        assert!(r.is_empty());
        r.input(b"ls");
        assert!(!r.is_empty());
        assert!(body(r).contains("Send(\"ls\")"));
    }
}
