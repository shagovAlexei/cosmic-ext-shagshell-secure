//! Python scripts (SecureCRT's `crt` model): a script is a `python3` process that talks to the
//! app over a unix socket, one JSON object per line. Request `{"id":n,"call":"Screen.Send",
//! "tab":5,"args":[…]}`, answer `{"id":n,"ok":value}` or `{"id":n,"err":"text"}`. The script
//! blocks on each answer. Reading and sending on a tab are done here (`serve`); what needs the
//! window (dialogs, tabs, sessions) goes to the app through `Host::ui`.

use crate::logfile::Cleaner;
use crate::macros::{self, Piece};
use serde_json::{Value, json};
use std::io::{self, BufRead};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// Text kept for waiting: the last bytes of it.
const WINDOW: usize = 256 << 10;

/// How often a blocked wait looks at the cancel flag.
const TICK: Duration = Duration::from_millis(100);

/// A tab's output as plain text for `WaitForString` / `ReadString`. It only collects while a
/// script runs (`arm`), and what a wait finds is used up.
#[derive(Default)]
pub struct Buffer {
    inner: Mutex<Inner>,
    cv: Condvar,
}

#[derive(Default)]
struct Inner {
    armed: bool,
    clean: Cleaner,
    /// Finished lines (the one being written is the cleaner's).
    seen: String,
    /// Bytes of `seen` + the line being written that earlier waits used up.
    skip: usize,
}

/// How a wait ended.
#[derive(Debug, PartialEq, Eq)]
pub enum Wait {
    /// Which needle (0-based) and the text before it since the last wait.
    Found(usize, String),
    Timeout,
    Cancelled,
}

impl Buffer {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A script starts: what came before is not for it.
    pub fn arm(&self) {
        *self.lock() = Inner {
            armed: true,
            ..Default::default()
        };
    }

    /// Collect from now on, unless already doing so (what came since is kept).
    pub fn arm_once(&self) {
        let mut g = self.lock();
        g.armed = true;
    }

    pub fn disarm(&self) {
        *self.lock() = Inner::default();
    }

    /// The output of the program (called for every chunk; cheap while not armed).
    pub fn feed(&self, bytes: &[u8]) {
        let mut g = self.lock();
        if !g.armed {
            return;
        }
        let mut out = Vec::new();
        g.clean.feed(bytes, &mut out);
        g.seen.push_str(&String::from_utf8_lossy(&out));
        if g.seen.len() > WINDOW {
            let cut = g.seen.len() - WINDOW;
            let at = (cut..g.seen.len())
                .find(|&i| g.seen.is_char_boundary(i))
                .unwrap_or(g.seen.len());
            g.seen.drain(..at);
            g.skip = g.skip.saturating_sub(at);
        }
        drop(g);
        self.cv.notify_all();
    }

    /// Waits until one of `needles` shows up in the output (the first by position; the first of
    /// the list on a tie), for `timeout` (`None`: no limit) or until `cancel`.
    pub fn wait(
        &self,
        needles: &[String],
        timeout: Option<Duration>,
        ignore_case: bool,
        cancel: &AtomicBool,
    ) -> Wait {
        let end = timeout.map(|t| Instant::now() + t);
        let fold = |s: &str| {
            if ignore_case {
                s.to_ascii_lowercase()
            } else {
                s.to_string()
            }
        };
        let needles: Vec<String> = needles.iter().map(|n| fold(n)).collect();
        let mut g = self.lock();
        loop {
            // What no wait has used up: the rest of `seen` and the line being written.
            let partial = g.clean.partial();
            let seen_len = g.seen.len();
            let (from, tail) = if g.skip < seen_len {
                let f = boundary(&g.seen, g.skip);
                (f, format!("{}{partial}", &g.seen[f..]))
            } else {
                let f = boundary(&partial, g.skip - seen_len);
                (seen_len + f, partial[f..].to_string())
            };
            let tail = tail.as_str();
            let hay = fold(tail);
            // ASCII folding keeps every length, so positions in `hay` are positions in `tail`.
            let hit = needles
                .iter()
                .enumerate()
                .filter(|(_, n)| !n.is_empty())
                .filter_map(|(i, n)| hay.find(n.as_str()).map(|at| (at, i, n.len())))
                .min_by_key(|&(at, i, _)| (at, i));
            if let Some((at, i, len)) = hit {
                let before = tail[..at].to_string();
                g.skip = from + (at + len).min(tail.len());
                return Wait::Found(i, before);
            }
            if cancel.load(Ordering::SeqCst) {
                return Wait::Cancelled;
            }
            let step = match end {
                Some(end) => {
                    let now = Instant::now();
                    if now >= end {
                        return Wait::Timeout;
                    }
                    TICK.min(end - now)
                }
                None => TICK,
            };
            g = self
                .cv
                .wait_timeout(g, step)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }
}

/// `Screen.Get(r1, c1, r2, c2)`: the block of `rows` (1-based, ends included, clamped to the
/// screen; a column below 1 counts as 1; an end before the start is an empty block) as lines
/// joined by `\n`.
pub fn slice(rows: &[String], r1: i64, c1: i64, r2: i64, c2: i64) -> String {
    let at = |v: i64| usize::try_from(v.max(1)).unwrap_or(1);
    let width = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0);
    let (r1, r2, c1) = (at(r1), at(r2), at(c1));
    let c2 = usize::try_from(c2).unwrap_or(0).min(width);
    let want = (c2 + 1).saturating_sub(c1);
    let lines: Vec<String> = rows
        .iter()
        .take(r2)
        .skip(r1 - 1)
        .map(|row| row.chars().skip(c1 - 1).take(want).collect())
        .collect();
    lines.join("\n")
}

/// SecureCRT's `MessageBox` results.
pub const ID_OK: i64 = 1;
pub const ID_CANCEL: i64 = 2;
pub const ID_ABORT: i64 = 3;
pub const ID_RETRY: i64 = 4;
pub const ID_IGNORE: i64 = 5;
pub const ID_YES: i64 = 6;
pub const ID_NO: i64 = 7;

/// The `flags` of `MessageBox`: `BUTTON_*` (0–5) | `ICON_*` (16 stop, 32 question, 48 warning,
/// 64 information) | `DEFBUTTON1/2/3` (0, 256, 512).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Flags {
    pub buttons: i64,
    pub icon: Icon,
    /// Index into `Flags::ids()`.
    pub default: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    None,
    Stop,
    Question,
    Warning,
    Info,
}

impl Flags {
    pub fn parse(n: i64) -> Self {
        Self {
            buttons: n & 0xF,
            icon: match n & 0xF0 {
                16 => Icon::Stop,
                32 => Icon::Question,
                48 => Icon::Warning,
                64 => Icon::Info,
                _ => Icon::None,
            },
            default: usize::try_from((n >> 8) & 3).unwrap_or(0),
        }
    }

    /// The buttons, left to right, as result ids.
    pub fn ids(&self) -> Vec<i64> {
        match self.buttons {
            1 => vec![ID_OK, ID_CANCEL],
            2 => vec![ID_ABORT, ID_RETRY, ID_IGNORE],
            3 => vec![ID_YES, ID_NO, ID_CANCEL],
            4 => vec![ID_YES, ID_NO],
            5 => vec![ID_RETRY, ID_CANCEL],
            _ => vec![ID_OK],
        }
    }

    /// Enter: the default button (the first when `default` is beyond the buttons).
    pub fn enter(&self) -> i64 {
        let ids = self.ids();
        ids.get(self.default).copied().unwrap_or(ids[0])
    }

    /// Esc, or the window closed: Cancel when there is one, else No / Ignore / OK.
    pub fn escape(&self) -> i64 {
        let ids = self.ids();
        [ID_CANCEL, ID_NO, ID_IGNORE]
            .into_iter()
            .find(|i| ids.contains(i))
            .unwrap_or(ids[0])
    }
}

/// A SecureCRT file filter, `"Text (*.txt)|*.txt|All (*.*)|*.*"`, as (description, extensions);
/// a pattern for any file gives no extensions.
pub fn file_filters(spec: &str) -> Vec<(String, Vec<String>)> {
    let parts: Vec<&str> = spec.split('|').collect();
    parts
        .chunks(2)
        .filter(|c| c.len() == 2 && !c[0].trim().is_empty())
        .map(|c| {
            let exts = c[1]
                .split(';')
                .filter_map(|p| p.trim().strip_prefix("*."))
                .filter(|e| !e.is_empty() && *e != "*")
                .map(str::to_string)
                .collect();
            (c[0].trim().to_string(), exts)
        })
        .collect()
}

/// The items of a List dialog that contain `filter` (any case), as indexes into `items`.
pub fn list_matches(items: &[String], filter: &str) -> Vec<usize> {
    let f = filter.to_lowercase();
    (0..items.len())
        .filter(|&i| items[i].to_lowercase().contains(&f))
        .collect()
}

/// `~/x` as the home folder's `x`; any other path as it is.
pub fn expand_home(path: &std::path::Path) -> std::path::PathBuf {
    match (path.strip_prefix("~"), std::env::home_dir()) {
        (Ok(rest), Some(home)) => home.join(rest),
        _ => path.to_path_buf(),
    }
}

/// The first char boundary of `s` at or after `i` (`s.len()` past the end).
fn boundary(s: &str, i: usize) -> usize {
    (i.min(s.len())..=s.len())
        .find(|&j| s.is_char_boundary(j))
        .unwrap_or(s.len())
}

/// What the app gives a running script.
pub trait Host: Send + Sync {
    /// A tab's output (`None`: no such terminal tab).
    fn buffer(&self, tab: u64) -> Option<Arc<Buffer>>;
    /// Bytes to the tab's program as if typed; false: no such tab.
    fn input(&self, tab: u64, bytes: Vec<u8>) -> bool;
    /// Everything else (dialogs, tabs, sessions): the app's window answers.
    fn ui(&self, call: &str, tab: u64, args: &[Value]) -> Result<Value, String>;
}

/// Answers the requests of one script until it closes the connection or `cancel` is set.
pub fn serve<S: io::Read + io::Write>(
    stream: S,
    reader: impl io::Read,
    host: &dyn Host,
    cancel: &AtomicBool,
) {
    let mut out = stream;
    for line in io::BufReader::new(reader).lines() {
        let Ok(line) = line else { return };
        if cancel.load(Ordering::SeqCst) {
            return;
        }
        let Ok(req) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let id = req["id"].clone();
        let reply = match handle(&req, host, cancel) {
            Ok(v) => json!({"id": id, "ok": v}),
            Err(e) => json!({"id": id, "err": e}),
        };
        if cancel.load(Ordering::SeqCst) {
            return;
        }
        if writeln!(out, "{reply}").and_then(|()| out.flush()).is_err() {
            return;
        }
    }
}

fn handle(req: &Value, host: &dyn Host, cancel: &AtomicBool) -> Result<Value, String> {
    let call = req["call"].as_str().ok_or("no call")?;
    let tab = req["tab"].as_u64().unwrap_or(0);
    let args: &[Value] = req["args"].as_array().map_or(&[], |a| a.as_slice());
    let text = |i: usize| -> Result<String, String> {
        args.get(i)
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| format!("{call}: argument {} must be a string", i + 1))
    };
    // Seconds; 0 or missing: no limit (SecureCRT: 0 waits forever).
    let timeout = |i: usize| {
        let s = args.get(i).and_then(Value::as_f64).unwrap_or(0.0);
        (s > 0.0).then(|| Duration::from_secs_f64(s))
    };
    let fold = |i: usize| args.get(i).and_then(Value::as_bool).unwrap_or(false);
    let buffer = || host.buffer(tab).ok_or_else(|| "no such tab".to_string());
    match call {
        "Screen.Send" => {
            for p in macros::parse(&text(0)?) {
                match p {
                    Piece::Bytes(b) => {
                        if !host.input(tab, b) {
                            return Err("no such tab".into());
                        }
                    }
                    Piece::Pause(d) => nap(d, cancel),
                }
            }
            Ok(Value::Null)
        }
        "Screen.WaitForString" | "Screen.ReadString" => {
            let needles = vec![text(0)?];
            let w = buffer()?.wait(&needles, timeout(1), fold(2), cancel);
            Ok(match (call, w) {
                ("Screen.WaitForString", Wait::Found(..)) => json!(true),
                ("Screen.WaitForString", _) => json!(false),
                (_, Wait::Found(_, before)) => json!(before),
                _ => Value::Null,
            })
        }
        "Screen.WaitForStrings" | "Screen.ReadStrings" => {
            let needles: Vec<String> = args
                .first()
                .and_then(Value::as_array)
                .ok_or("argument 1 must be a list")?
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect();
            let w = buffer()?.wait(&needles, timeout(1), fold(2), cancel);
            Ok(match w {
                Wait::Found(i, _) => json!(i + 1),
                _ => json!(0),
            })
        }
        "Sleep" => {
            let ms = args.first().and_then(Value::as_f64).unwrap_or(0.0).max(0.0);
            nap(Duration::from_secs_f64(ms / 1000.0), cancel);
            Ok(Value::Null)
        }
        _ => host.ui(call, tab, args),
    }
}

/// Sleeps `d`, in ticks, so a cancel is not waited out.
fn nap(d: Duration, cancel: &AtomicBool) {
    let end = Instant::now() + d;
    while !cancel.load(Ordering::SeqCst) {
        let now = Instant::now();
        if now >= end {
            return;
        }
        std::thread::sleep(TICK.min(end - now));
    }
}

/// Where `crt.py` lives: installed, or (running from the repository) `res/python`.
pub fn python_dir() -> std::path::PathBuf {
    use std::path::PathBuf;
    if let Some(d) = std::env::var_os("SHAGSHELL_PYTHON") {
        return d.into();
    }
    let installed = PathBuf::from("/usr/share/shagshell/python");
    if installed.join("crt.py").is_file() {
        return installed;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../res/python")
}

/// The command that runs `script` with `crt` defined, talking to `sock`, about tab `tab`:
/// program, arguments, environment.
pub fn command(
    script: &std::path::Path,
    args: &[String],
    sock: &std::path::Path,
    tab: u64,
    python: &std::path::Path,
) -> (String, Vec<String>, Vec<(String, String)>) {
    let mut argv = vec![
        "-c".to_string(),
        "import crt; crt.main()".to_string(),
        script.display().to_string(),
    ];
    argv.extend(args.iter().cloned());
    let env = vec![
        ("PYTHONPATH".into(), python.display().to_string()),
        ("PYTHONUNBUFFERED".into(), "1".into()),
        ("PYTHONDONTWRITEBYTECODE".into(), "1".into()),
        ("SHAGSHELL_SCRIPT_SOCK".into(), sock.display().to_string()),
        ("SHAGSHELL_SCRIPT_TAB".into(), tab.to_string()),
        ("SHAGSHELL_SCRIPT".into(), script.display().to_string()),
    ];
    ("python3".into(), argv, env)
}

#[cfg(test)]
mod tests {
    #[test]
    fn message_box_flags() {
        let f = Flags::parse(4 | 32 | 256); // YESNO | QUESTION | DEFBUTTON2
        assert_eq!(
            (f.icon, f.ids(), f.enter(), f.escape()),
            (Icon::Question, vec![ID_YES, ID_NO], ID_NO, ID_NO)
        );
        let f = Flags::parse(3 | 48); // YESNOCANCEL | WARN
        assert_eq!(
            (f.icon, f.enter(), f.escape()),
            (Icon::Warning, ID_YES, ID_CANCEL)
        );
        let f = Flags::parse(5); // RETRYCANCEL
        assert_eq!(
            (f.ids(), f.enter(), f.escape()),
            (vec![ID_RETRY, ID_CANCEL], ID_RETRY, ID_CANCEL)
        );
        let f = Flags::parse(2 | 512); // ABORTRETRYIGNORE, DEFBUTTON3
        assert_eq!((f.enter(), f.escape()), (ID_IGNORE, ID_IGNORE));
        let f = Flags::parse(0 | 512); // OK only: the default is beyond the buttons
        assert_eq!(
            (f.ids(), f.enter(), f.escape()),
            (vec![ID_OK], ID_OK, ID_OK)
        );
    }

    #[test]
    fn file_filter_spec() {
        let f = file_filters("Text (*.txt;*.log)|*.txt;*.log|All Files (*.*)|*.*");
        assert_eq!(
            f,
            vec![
                (
                    "Text (*.txt;*.log)".to_string(),
                    vec!["txt".to_string(), "log".to_string()]
                ),
                ("All Files (*.*)".to_string(), vec![]),
            ]
        );
        assert!(file_filters("").is_empty());
        assert!(file_filters("broken").is_empty());
    }

    #[test]
    fn list_filter_is_case_blind() {
        let items: Vec<String> = ["Web-01", "db-01", "WEB-02"].map(String::from).to_vec();
        assert_eq!(list_matches(&items, "web"), vec![0, 2]);
        assert_eq!(list_matches(&items, ""), vec![0, 1, 2]);
    }

    #[test]
    fn slice_takes_a_block_clamped_to_the_screen() {
        let rows: Vec<String> = ["abcde", "fghij", "klmno"].map(String::from).to_vec();
        assert_eq!(slice(&rows, 1, 2, 2, 4), "bcd\nghi");
        assert_eq!(slice(&rows, 2, 0, 2, 2), "fg"); // column 0 is column 1
        assert_eq!(slice(&rows, 3, 4, 9, 9), "no"); // past the end: clamped
        assert_eq!(slice(&rows, 1, 1, 1, 0), ""); // a prompt of no characters (cursor in column 1)
        assert_eq!(slice(&rows, 1, 1, 1, 1_000_000_000), "abcde");
        assert_eq!(slice(&rows, 2, 3, 1, 3), ""); // backwards: nothing
    }

    use super::*;
    use std::io::Write;

    fn no() -> AtomicBool {
        AtomicBool::new(false)
    }

    fn armed() -> Buffer {
        let b = Buffer::default();
        b.arm();
        b
    }

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn finds_text_and_returns_what_came_before() {
        let b = armed();
        b.feed(b"login: ");
        let w = b.wait(
            &s(&["login:"]),
            Some(Duration::from_millis(50)),
            false,
            &no(),
        );
        assert_eq!(w, Wait::Found(0, String::new()));
        b.feed(b"\x1b[1mhello\x1b[0m\r\nrouter# ");
        let w = b.wait(&s(&["#"]), Some(Duration::from_millis(50)), false, &no());
        assert_eq!(w, Wait::Found(0, " hello\nrouter".into()));
    }

    #[test]
    fn used_text_is_not_found_twice() {
        let b = armed();
        b.feed(b"a$ ");
        assert!(matches!(
            b.wait(&s(&["$"]), Some(Duration::from_millis(20)), false, &no()),
            Wait::Found(..)
        ));
        assert_eq!(
            b.wait(&s(&["$"]), Some(Duration::from_millis(20)), false, &no()),
            Wait::Timeout
        );
    }

    #[test]
    fn earliest_needle_wins_and_case_can_be_ignored() {
        let b = armed();
        b.feed(b"Password: then Login:");
        let w = b.wait(
            &s(&["login:", "password:"]),
            Some(Duration::from_millis(20)),
            true,
            &no(),
        );
        assert_eq!(w, Wait::Found(1, String::new()));
        let w = b.wait(
            &s(&["login:"]),
            Some(Duration::from_millis(20)),
            false,
            &no(),
        );
        assert_eq!(w, Wait::Timeout); // "Login:" is not "login:"
    }

    #[test]
    fn waits_for_output_that_comes_later() {
        let b = Arc::new(armed());
        let b2 = b.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(60));
            b2.feed(b"done\r\n");
        });
        let w = b.wait(&s(&["done"]), Some(Duration::from_secs(5)), false, &no());
        assert!(matches!(w, Wait::Found(0, _)));
    }

    #[test]
    fn cancel_ends_a_wait_with_no_limit() {
        let b = armed();
        let c = Arc::new(AtomicBool::new(false));
        let c2 = c.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            c2.store(true, Ordering::SeqCst);
        });
        assert_eq!(b.wait(&s(&["x"]), None, false, &c), Wait::Cancelled);
    }

    #[test]
    fn not_armed_collects_nothing() {
        let b = Buffer::default();
        b.feed(b"hello");
        assert_eq!(
            b.wait(
                &s(&["hello"]),
                Some(Duration::from_millis(20)),
                false,
                &no()
            ),
            Wait::Timeout
        );
    }

    #[test]
    fn multibyte_text_and_case_folding_keep_char_boundaries() {
        let b = armed();
        b.feed("Привет, SERVER> ".as_bytes());
        let w = b.wait(
            &s(&["server>"]),
            Some(Duration::from_millis(20)),
            true,
            &no(),
        );
        assert!(matches!(w, Wait::Found(0, _)));
    }

    #[test]
    fn home_is_expanded_only_as_a_first_component() {
        use std::path::Path;
        let home = std::env::home_dir().unwrap();
        assert_eq!(expand_home(Path::new("~/a.py")), home.join("a.py"));
        assert_eq!(expand_home(Path::new("/x/~/a.py")), Path::new("/x/~/a.py"));
        assert_eq!(expand_home(Path::new("~a/b.py")), Path::new("~a/b.py"));
    }

    #[test]
    fn regression_text_after_a_consumed_partial_line_is_found() {
        // A prompt was used up while still the unfinished line; the output that follows it
        // (same line, then new lines) is searched from there.
        let b = armed();
        b.feed(b"router# ");
        assert!(matches!(
            b.wait(&s(&["#"]), Some(Duration::from_millis(20)), false, &no()),
            Wait::Found(..)
        ));
        b.feed(b"show ver\r\nVersion 15\r\nrouter# ");
        let w = b.wait(
            &s(&["Version"]),
            Some(Duration::from_millis(20)),
            false,
            &no(),
        );
        assert_eq!(w, Wait::Found(0, " show ver\n".into()));
        let w = b.wait(&s(&["#"]), Some(Duration::from_millis(20)), false, &no());
        assert_eq!(w, Wait::Found(0, " 15\nrouter".into()));
    }

    struct Fake {
        buf: Arc<Buffer>,
        sent: Mutex<Vec<u8>>,
    }

    impl Host for Fake {
        fn buffer(&self, tab: u64) -> Option<Arc<Buffer>> {
            (tab == 1).then(|| self.buf.clone())
        }
        fn input(&self, tab: u64, bytes: Vec<u8>) -> bool {
            self.sent.lock().unwrap().extend(bytes);
            tab == 1
        }
        fn ui(&self, call: &str, _: u64, args: &[Value]) -> Result<Value, String> {
            match call {
                "Dialog.Prompt" => Ok(args[0].clone()),
                "GetTab" => Err("no such tab".into()),
                _ => Err(format!("unknown call {call}")),
            }
        }
    }

    fn ask(host: &Fake, lines: &str) -> Vec<Value> {
        use std::os::unix::net::UnixStream;
        let (a, b) = UnixStream::pair().unwrap();
        (&a).write_all(lines.as_bytes()).unwrap();
        a.shutdown(std::net::Shutdown::Write).unwrap();
        serve(b.try_clone().unwrap(), b, host, &no());
        io::BufReader::new(a)
            .lines()
            .map(|l| serde_json::from_str(&l.unwrap()).unwrap())
            .collect()
    }

    #[test]
    fn serves_send_wait_and_ui_calls() {
        let buf = Arc::new(armed());
        buf.feed(b"user@host:~$ ");
        let host = Fake {
            buf,
            sent: Mutex::default(),
        };
        let r = ask(
            &host,
            concat!(
                r#"{"id":1,"call":"Screen.WaitForString","tab":1,"args":["$",1]}"#,
                "\n",
                r#"{"id":2,"call":"Screen.Send","tab":1,"args":["ls\r"]}"#,
                "\n",
                r#"{"id":3,"call":"Screen.WaitForString","tab":1,"args":["nope",0.05]}"#,
                "\n",
                r#"{"id":4,"call":"Dialog.Prompt","tab":1,"args":["x"]}"#,
                "\n",
                r#"{"id":5,"call":"Screen.Send","tab":9,"args":["a"]}"#,
                "\n",
                r#"{"id":6,"call":"Bogus","tab":1,"args":[]}"#,
                "\n",
            ),
        );
        assert_eq!(r[0], json!({"id":1,"ok":true}));
        assert_eq!(r[1], json!({"id":2,"ok":null}));
        assert_eq!(host.sent.lock().unwrap().as_slice(), b"ls\ra");
        assert_eq!(r[2], json!({"id":3,"ok":false}));
        assert_eq!(r[3], json!({"id":4,"ok":"x"}));
        assert_eq!(r[4]["err"], "no such tab");
        assert!(r[5]["err"].as_str().unwrap().contains("Bogus"));
    }

    /// A real `python3` and the real `crt.py` against `serve`.
    #[test]
    fn python_script_runs_against_serve() {
        use std::os::unix::net::UnixListener;
        use std::process::{Command, Stdio};
        if Command::new("python3").arg("-V").output().is_err() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("s.sock");
        let script = dir.path().join("t.py");
        std::fs::write(
            &script,
            "ok = crt.Screen.WaitForString('$', 2)\n\
             crt.Screen.Send('ls\\r')\n\
             n = crt.Dialog.Prompt('who?', 'T', 'me')\n\
             crt.Screen.Send(n + ' ' + str(crt.Arguments.Count) + ' ' + str(ok) + '\\r')\n\
             try:\n    crt.GetTab(7)\nexcept Exception as e:\n    crt.Screen.Send(type(e).__name__ + '\\r')\n",
        )
        .unwrap();
        let l = UnixListener::bind(&sock).unwrap();
        let (prog, args, env) = command(&script, &["a".into()], &sock, 1, &python_dir());
        let mut child = Command::new(prog)
            .args(args)
            .envs(env)
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let buf = Arc::new(armed());
        buf.feed(b"host$ ");
        let host = Fake {
            buf,
            sent: Mutex::default(),
        };
        let (s, _) = l.accept().unwrap();
        serve(s.try_clone().unwrap(), s, &host, &no());
        assert!(child.wait().unwrap().success());
        let sent = String::from_utf8(host.sent.lock().unwrap().clone()).unwrap();
        assert_eq!(sent, "ls\rwho? 1 True\rScriptError\r");
    }
}
