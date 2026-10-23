//! Session logs (SecureCRT's Log File): the file name from a template, and the terminal's output
//! as plain text (escape sequences taken out) or raw.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Now, in the local time zone (the app has no `jiff` of its own).
pub fn now() -> jiff::Zoned {
    jiff::Zoned::now()
}

/// What a name template can use.
pub struct Ctx<'a> {
    pub host: &'a str,
    /// The session's name (its last path part); the tab's title for an unsaved one.
    pub session: &'a str,
    pub port: Option<u16>,
    pub user: &'a str,
    pub now: jiff::Zoned,
}

/// `%H` host, `%S` session, `%P` port, `%U` user, `%Y %M %D` date, `%h %m %s` time, `%%` a percent
/// sign. A value is made safe as a file name (no `/`, no control characters); `%x` unknown stays.
pub fn expand(template: &str, c: &Ctx) -> String {
    let safe = |v: &str| -> String {
        v.chars()
            .map(|ch| {
                if ch == '/' || ch.is_control() {
                    '_'
                } else {
                    ch
                }
            })
            .collect()
    };
    let mut out = String::new();
    let mut it = template.chars();
    while let Some(ch) = it.next() {
        if ch != '%' {
            out.push(ch);
            continue;
        }
        let two = |n: i8| format!("{n:02}");
        match it.next() {
            Some('H') => out.push_str(&safe(c.host)),
            Some('S') => out.push_str(&safe(c.session)),
            Some('P') => out.push_str(&c.port.map_or_else(String::new, |p| p.to_string())),
            Some('U') => out.push_str(&safe(c.user)),
            Some('Y') => out.push_str(&format!("{:04}", c.now.year())),
            Some('M') => out.push_str(&two(c.now.month())),
            Some('D') => out.push_str(&two(c.now.day())),
            Some('h') => out.push_str(&two(c.now.hour())),
            Some('m') => out.push_str(&two(c.now.minute())),
            Some('s') => out.push_str(&two(c.now.second())),
            Some('%') => out.push('%'),
            Some(o) => {
                out.push('%');
                out.push(o);
            }
            None => out.push('%'),
        }
    }
    out
}

/// The file a template names: absolute as it is (`~/` the home), else under `dir`.
pub fn resolve(dir: &Path, template: &str, c: &Ctx, home: &Path) -> PathBuf {
    let name = expand(template, c);
    match name.strip_prefix("~/") {
        Some(rest) => home.join(rest),
        None if name.starts_with('/') => PathBuf::from(name),
        None => dir.join(name),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Normal,
    Esc,
    Csi,
    Osc,
    OscEsc,
    /// `ESC ( B` and the like: one more character.
    Charset,
}

/// Terminal output to text: escape sequences gone, `\r` back to the start of the line (what
/// follows overwrites, as a progress bar does), backspace one place back, `ESC [ K` clears the
/// rest. Whole lines come out when the `\n` does.
pub struct Cleaner {
    state: State,
    line: Vec<char>,
    col: usize,
    /// The CSI parameters so far (only to tell `K`'s argument).
    params: Vec<u8>,
    /// Bytes of a UTF-8 character not yet whole.
    pending: Vec<u8>,
}

impl Default for Cleaner {
    fn default() -> Self {
        Self {
            state: State::Normal,
            line: Vec::new(),
            col: 0,
            params: Vec::new(),
            pending: Vec::new(),
        }
    }
}

impl Cleaner {
    pub fn feed(&mut self, bytes: &[u8], out: &mut Vec<u8>) {
        for &b in bytes {
            self.byte(b, out);
        }
    }

    /// The line being written, as text so far.
    pub fn partial(&self) -> String {
        self.line.iter().collect()
    }

    /// The unfinished line, when the log ends.
    pub fn finish(&mut self, out: &mut Vec<u8>) {
        if !self.line.is_empty() {
            self.emit(out, false);
        }
    }

    fn emit(&mut self, out: &mut Vec<u8>, newline: bool) {
        out.extend(self.line.iter().collect::<String>().bytes());
        if newline {
            out.push(b'\n');
        }
        self.line.clear();
        self.col = 0;
    }

    fn put(&mut self, c: char) {
        match self.line.get_mut(self.col) {
            Some(slot) => *slot = c,
            None => self.line.push(c),
        }
        self.col += 1;
    }

    fn byte(&mut self, b: u8, out: &mut Vec<u8>) {
        // A character cut short by a control byte is not glued to what follows.
        if self.state == State::Normal && !self.pending.is_empty() && (b < 0x20 || b == 0x7f) {
            self.pending.clear();
            self.put('\u{fffd}');
        }
        match self.state {
            State::Normal => match b {
                0x1b => self.state = State::Esc,
                b'\n' => self.emit(out, true),
                b'\r' => self.col = 0,
                0x08 => self.col = self.col.saturating_sub(1),
                b'\t' => self.put('\t'),
                0..=0x1f | 0x7f => {}
                _ => {
                    self.pending.push(b);
                    // Whole, or wrong (then the replacement character): never stuck.
                    match std::str::from_utf8(&self.pending) {
                        Ok(s) => {
                            let c = s.chars().next().unwrap_or('\u{fffd}');
                            self.pending.clear();
                            self.put(c);
                        }
                        Err(e) => {
                            if let Some(n) = e.error_len() {
                                // The wrong bytes are one replacement character; what follows
                                // them (this byte, if it did not belong) starts over.
                                let rest = self.pending.split_off(n);
                                self.pending.clear();
                                self.put('\u{fffd}');
                                for x in rest {
                                    self.byte(x, out);
                                }
                            }
                        }
                    }
                }
            },
            State::Esc => {
                self.state = match b {
                    b'[' => {
                        self.params.clear();
                        State::Csi
                    }
                    b']' => State::Osc,
                    b'(' | b')' | b'*' | b'+' | b'#' => State::Charset,
                    _ => State::Normal,
                }
            }
            State::Csi => {
                if (0x40..=0x7e).contains(&b) {
                    // Clear to the end of the line (0), the default.
                    if b == b'K' && matches!(self.params.as_slice(), b"" | b"0") {
                        self.line.truncate(self.col);
                    }
                    self.state = State::Normal;
                } else {
                    self.params.push(b);
                }
            }
            State::Osc => match b {
                0x07 => self.state = State::Normal,
                0x1b => self.state = State::OscEsc,
                _ => {}
            },
            State::OscEsc => self.state = State::Normal,
            State::Charset => self.state = State::Normal,
        }
    }
}

/// An open log.
pub struct Logger {
    file: File,
    clean: Option<Cleaner>,
    pub path: PathBuf,
}

impl Logger {
    /// `append`: add to the file; else it starts over. `raw`: the bytes as they came.
    pub fn open(path: &Path, append: bool, raw: bool) -> io::Result<Self> {
        use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
        // A transcript may hold what was pasted or shown (a config with passwords): ours alone.
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(dir)?;
        }
        let mut o = OpenOptions::new();
        o.create(true).write(true).mode(0o600);
        match append {
            true => o.append(true),
            false => o.truncate(true),
        };
        Ok(Self {
            file: o.open(path)?,
            clean: (!raw).then(Cleaner::default),
            path: path.to_path_buf(),
        })
    }

    pub fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        match &mut self.clean {
            Some(c) => {
                let mut out = Vec::with_capacity(bytes.len());
                c.feed(bytes, &mut out);
                self.file.write_all(&out)
            }
            None => self.file.write_all(bytes),
        }
    }

    /// The log ends: the last unfinished line too.
    pub fn close(mut self) -> io::Result<()> {
        self.finish()
    }

    fn finish(&mut self) -> io::Result<()> {
        if let Some(c) = &mut self.clean {
            let mut out = Vec::new();
            c.finish(&mut out);
            self.file.write_all(&out)?;
        }
        self.file.flush()
    }
}

/// Dropped (the tab closed, the program ended): the last line is written all the same.
impl Drop for Logger {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(now: &str) -> Ctx<'static> {
        Ctx {
            host: "10.0.0.1",
            session: "sw core",
            port: Some(22),
            user: "adm/in",
            now: now.parse::<jiff::Zoned>().unwrap(),
        }
    }

    #[test]
    fn template_fields() {
        let c = ctx("2026-03-07T04:05:09+02:00[Europe/Kyiv]");
        assert_eq!(
            expand("%S_%H-%P_%U_%Y%M%D-%h%m%s_100%%_%x%", &c),
            "sw core_10.0.0.1-22_adm_in_20260307-040509_100%_%x%"
        );
    }

    #[test]
    fn resolve_places() {
        let c = ctx("2026-03-07T04:05:09+02:00[Europe/Kyiv]");
        let (dir, home) = (Path::new("/logs"), Path::new("/home/u"));
        assert_eq!(
            resolve(dir, "%H.log", &c, home),
            Path::new("/logs/10.0.0.1.log")
        );
        assert_eq!(
            resolve(dir, "~/x/%D.log", &c, home),
            Path::new("/home/u/x/07.log")
        );
        assert_eq!(
            resolve(dir, "/var/log/a.log", &c, home),
            Path::new("/var/log/a.log")
        );
    }

    fn clean(chunks: &[&[u8]]) -> String {
        let mut c = Cleaner::default();
        let mut out = Vec::new();
        for ch in chunks {
            c.feed(ch, &mut out);
        }
        c.finish(&mut out);
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn escapes_and_crlf() {
        assert_eq!(
            clean(&[b"\x1b[1;32mok\x1b[0m done\r\nnext\x1b]0;title\x07 line\r\n"]),
            "ok done\nnext line\n"
        );
        assert_eq!(clean(&[b"a\x1b(Bb\x1b=c\x1b]0;t\x1b\\d"]), "abcd");
    }

    #[test]
    fn carriage_return_overwrites_and_backspace() {
        assert_eq!(clean(&[b"10%\r50%\r100%\n"]), "100%\n");
        assert_eq!(clean(&[b"abc\x08\x08X\n"]), "aXc\n");
        assert_eq!(clean(&[b"hello\r\x1b[Kworld\n"]), "world\n");
        assert_eq!(clean(&[b"hello\r\x1b[2Kx\n"]), "xello\n"); // other K: not handled
    }

    #[test]
    fn utf8_split_across_chunks_and_bad_bytes() {
        let s = "żółć\n".as_bytes();
        assert_eq!(clean(&[&s[..3], &s[3..]]), "żółć\n");
        assert_eq!(clean(&[b"a\xffb\n"]), "a\u{fffd}b\n");
    }

    #[test]
    fn a_cut_character_is_not_glued_to_what_follows() {
        assert_eq!(clean(&[b"a\xc5\r\n", b"\xc5b\n"]), "a\u{fffd}\n\u{fffd}b\n");
        assert_eq!(clean(&[b"\xc5\x1b[0mab\n"]), "\u{fffd}ab\n");
    }

    #[test]
    fn dropping_a_logger_writes_the_last_line() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("x.log");
        let mut l = Logger::open(&p, false, false).unwrap();
        l.write(b"no newline").unwrap();
        drop(l);
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "no newline");
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&p).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn logger_appends_or_starts_over_and_flushes_the_last_line() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("sub/x.log");
        let mut l = Logger::open(&p, false, false).unwrap();
        l.write(b"one\r\ntwo").unwrap();
        l.close().unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "one\ntwo");
        let mut l = Logger::open(&p, true, true).unwrap();
        l.write(b"\x1b[0mraw\n").unwrap();
        l.close().unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "one\ntwo\x1b[0mraw\n");
        Logger::open(&p, false, false).unwrap().close().unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "");
    }
}
