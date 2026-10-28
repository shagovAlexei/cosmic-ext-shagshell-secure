//! Script → Start Recording: what is typed in a terminal, as a `crt` script.
//!
//! The recorder sees the program's output (the prompt before each step) and the user's input
//! (the step). Passwords are never kept: a step typed after a password prompt becomes a
//! `crt.Dialog.Prompt`.

use crate::logfile::Cleaner;

/// A prompt longer than this is waited for by its last `TAIL` characters.
const LONG: usize = 80;
const TAIL: usize = 40;
/// How much output after a step is kept to look for its echo.
const AFTER: usize = 4096;

#[derive(Debug, PartialEq, Eq)]
struct Step {
    /// The text on the line before the first key of the step (the prompt).
    prompt: String,
    /// What was sent, with its Enter (`\r`) when it had one.
    text: String,
    /// Typed after a password / passphrase prompt: not kept.
    secret: bool,
    /// The output from its first key on (its echo, if any), at most `AFTER` bytes.
    after: String,
    /// The step ended with Enter (or Ctrl+C / Ctrl+D, which are in `text`).
    enter: bool,
}

#[derive(Default)]
pub struct Recorder {
    clean: Cleaner,
    steps: Vec<Step>,
    /// The step being typed, and the prompt it started at.
    typing: String,
    prompt: Option<String>,
    /// The output since the first key of the step being typed.
    after: String,
}

fn keep(to: &mut String, text: &str) {
    for c in text.chars() {
        if to.len() >= AFTER {
            break;
        }
        to.push(c);
    }
}

impl Recorder {
    pub fn new() -> Self {
        Self::default()
    }

    /// The program's output.
    pub fn output(&mut self, bytes: &[u8]) {
        let mut sink = Vec::new();
        self.clean.feed(bytes, &mut sink);
        let text = String::from_utf8_lossy(&sink);
        if self.prompt.is_some() {
            keep(&mut self.after, &text);
        } else if let Some(s) = self.steps.last_mut() {
            keep(&mut s.after, &text);
        }
    }

    /// What the user typed or pasted.
    pub fn input(&mut self, bytes: &[u8]) {
        // Paste markers are the terminal's, not part of the line.
        let text = String::from_utf8_lossy(bytes)
            .replace("\x1b[200~", "")
            .replace("\x1b[201~", "");
        for c in text.chars() {
            if self.prompt.is_none() {
                let line = self.clean.partial();
                // The line the last step's answer left (the new prompt) is part of that answer.
                if let Some(s) = self.steps.last_mut() {
                    keep(&mut s.after, &line);
                }
                self.prompt = Some(line.trim().to_string());
                self.after.clear();
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
        let secret = looks_secret(&prompt);
        self.steps.push(Step {
            enter: last.is_some(),
            prompt,
            text: if secret { String::new() } else { text },
            secret,
            after: std::mem::take(&mut self.after),
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
        // The line not ended yet holds the last echo.
        let partial = self.clean.partial();
        if let Some(s) = self.steps.last_mut() {
            keep(&mut s.after, &partial);
        }
        for s in &mut self.steps {
            if !s.secret && unechoed(s) {
                s.secret = true;
                s.text.clear();
            }
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
                // Asked into a variable first: a block of its own in the Script Builder.
                let enter = if s.enter { " + \"\\r\"" } else { "" };
                out.push_str(&format!(
                    "password = crt.Dialog.Prompt({}, \"Password\", \"\", True)\ncrt.Screen.Send(password{enter})\n",
                    literal(&s.prompt)
                ));
            } else {
                out.push_str(&format!("crt.Screen.Send({})\n", literal(&s.text)));
            }
        }
        out
    }
}

/// Typed, answered, and the answer doesn't show what was typed: not echoed, as a password
/// isn't (whatever language the prompt is in). No answer at all: can't tell, kept.
fn unechoed(s: &Step) -> bool {
    let typed: String = s.text.chars().filter(|c| !c.is_control()).collect();
    let seen = s.after.replacen(&s.prompt, "", 1);
    !typed.is_empty() && !seen.trim().is_empty() && !seen.contains(&typed)
}

/// A prompt that asks for a secret, by its words (the echo of the line is not known here).
fn looks_secret(prompt: &str) -> bool {
    let p = prompt.to_lowercase();
    [
        "password",
        "passphrase",
        "passcode",
        "pin:",
        "secret",
        "парол",
    ]
    .iter()
    .any(|w| p.contains(w))
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
    fn regression_unechoed_step_is_secret_whatever_the_prompt_says() {
        // «Passwort:» / «Contraseña:» are not in the word list: the password was kept in the .py.
        for prompt in ["Passwort: ", "Contraseña: ", "Token: "] {
            let mut r = Recorder::new();
            r.output(prompt.as_bytes());
            r.input(b"geheim7\r");
            r.output(b"\r\nrouter> ");
            let s = body(r);
            assert!(!s.contains("geheim7"), "{prompt}: {s}");
            assert!(s.contains("crt.Dialog.Prompt("), "{prompt}");
        }
        // Not the last step: the next prompt (no newline yet) is the answer.
        let mut r = Recorder::new();
        r.output(b"Passwort: ");
        r.input(b"geheim7\r");
        r.output(b"\r\nrouter> ");
        r.input(b"uptime\r");
        r.output(b"uptime\r\nup 3 days\r\nrouter> ");
        let s = body(r);
        assert!(!s.contains("geheim7"), "{s}");
        assert!(s.contains("Send(\"uptime\\r\")"), "{s}");
        // Echoed as stars (MikroTik, Windows): not the text either.
        let mut r = Recorder::new();
        r.output(b"Kennwort: ");
        r.input(b"abc12\r");
        r.output(b"*****\r\n> ");
        assert!(!body(r).contains("abc12"));
    }

    #[test]
    fn a_pasted_line_echoed_late_is_kept() {
        // The echo comes after the Enter, in one chunk with the answer.
        let mut r = Recorder::new();
        r.output(b"router> ");
        r.input(b"show version\r");
        r.output(b"show version\r\nRouterOS 7.16\r\nrouter> ");
        assert!(body(r).contains("Send(\"show version\\r\")"));
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
    fn other_secret_prompts_and_unfinished_secret_steps() {
        for p in [
            "Password for admin@db: ",
            "Enter PIN:",
            "Password (again):",
            "Пароль:",
        ] {
            let mut r = Recorder::new();
            r.output(p.as_bytes());
            r.input(b"s3cret\r");
            assert!(!body(r).contains("s3cret"), "{p}");
        }
        let mut r = Recorder::new();
        r.output(b"Password: ");
        r.input(b"abc"); // never ended by Enter
        assert!(
            !body(r).contains("\\r\")"),
            "no Enter the user did not press"
        );
    }

    #[test]
    fn paste_markers_are_dropped() {
        let mut r = Recorder::new();
        r.output(b"$ ");
        r.input(b"\x1b[200~ls -l\x1b[201~\r");
        assert!(body(r).contains("Send(\"ls -l\\r\")"));
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

#[cfg(test)]
mod builder_tests {
    use super::*;
    use crate::builder::{self, Block};

    /// Stop opens the recording in the Script Builder: every step is a block of its own (a
    /// `crt` call, a password asked for), not «Custom Python».
    #[test]
    fn recording_opens_as_blocks() {
        let mut r = Recorder::new();
        r.output(b"user@host's password: ");
        r.input(b"hunter2\r");
        r.output(b"\r\nuser@host:~$ ");
        r.input(b"uptime\r");
        let blocks = builder::parse::tests::parse(&r.script("Recorded")).unwrap();
        let calls: Vec<&str> = (blocks.iter())
            .filter_map(|b| match b {
                Block::Call { func, .. } => Some(func.as_str()),
                _ => None,
            })
            .collect();
        assert!(calls.contains(&"crt.Dialog.Prompt"), "{blocks:?}");
        assert!(calls.contains(&"crt.Screen.Send"), "{blocks:?}");
        assert!(
            !blocks.iter().any(|b| matches!(b, Block::Raw { .. })),
            "{blocks:?}"
        );
    }
}
