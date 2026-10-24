//! The Script menu: runs of `python3` scripts (`core::script`), the process and socket of each,
//! and what a script asks that needs the window (dialogs, tabs, sessions).

use crate::app::Message;
use crate::fl;
use crate::tabs::Tabs;
use cosmic::Element;
use cosmic::iced::{Subscription, futures::SinkExt};
use cosmic::widget;
use serde_json::{Value, json};
use shagshell_core::script::{self, Host};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

/// Every terminal tab's tap by tab id (`Tabs` keeps it), for the threads serving scripts.
pub type Taps = Arc<Mutex<HashMap<u64, crate::term::log::Tap>>>;

type Answer = Result<Value, String>;

/// Where a script's call waits for its answer. Dropped unanswered: the call fails.
#[derive(Clone)]
pub struct Reply(Arc<Mutex<Option<mpsc::Sender<Answer>>>>);

impl std::fmt::Debug for Reply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Reply")
    }
}

impl Reply {
    pub fn send(&self, a: Answer) {
        if let Some(tx) = self.0.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = tx.send(a);
        }
    }
}

/// A call of a running script that the window answers.
#[derive(Clone, Debug)]
pub struct Call {
    pub run: u64,
    pub call: String,
    pub tab: u64,
    pub args: Vec<Value>,
    pub reply: Reply,
}

/// How a run ended.
#[derive(Clone, Debug)]
pub enum End {
    Done,
    Cancelled,
    Failed(String),
}

#[derive(Clone, Debug)]
pub enum ScMsg {
    Call(Call),
    Ended(u64, End),
    /// Script → Run…: the file picked.
    Picked(Option<PathBuf>),
    /// `--script file args…`, or a Button Bar button: run it on the active terminal.
    Start(PathBuf, Vec<String>),
    /// A session's script: on the tab it opened.
    StartOn(u64, PathBuf, Vec<String>),
    Input(String),
    /// A button of a MessageBox (its SecureCRT result code).
    Button(i64),
    /// The clipboard's text, for a script that asked.
    Clip(Reply, String),
    Ok,
    Cancel,
}

struct Run {
    name: String,
    cancel: Arc<AtomicBool>,
}

/// A dialog a script asked for.
struct Ask {
    call: Call,
    value: String,
}

pub struct Scripts {
    runs: BTreeMap<u64, Run>,
    next: u64,
    taps: Taps,
    asks: VecDeque<Ask>,
    pub input_id: widget::Id,
    dir: PathBuf,
    tx: tokio::sync::mpsc::UnboundedSender<ScMsg>,
    rx: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<ScMsg>>>>,
}

/// What the app does after an update.
#[derive(Default)]
pub struct Outcome {
    pub notice: Option<String>,
    /// A dialog came up: focus its field.
    pub focus: bool,
    /// The last dialog went: the keys back to the terminal.
    pub refocus: bool,
    /// `Session.Connect` of a script: the saved session to open.
    pub connect: Option<(String, Reply)>,
    /// `Clipboard.Set` / `Clipboard.Get` of a script.
    pub clip: Option<(Option<String>, Reply)>,
}

impl Scripts {
    pub fn new(taps: Taps, dir: PathBuf) -> Self {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        Self {
            runs: BTreeMap::new(),
            next: 1,
            taps,
            asks: VecDeque::new(),
            input_id: widget::Id::unique(),
            dir,
            tx,
            rx: Arc::new(Mutex::new(Some(rx))),
        }
    }

    pub fn active(&self) -> bool {
        !self.asks.is_empty()
    }

    /// Starts `script` on tab `tab`.
    pub fn run(&mut self, script: &Path, args: &[String], tab: u64) -> Result<(), String> {
        if !script.is_file() {
            return Err(fl!("script-missing", name = script.display().to_string()));
        }
        let n = self.next;
        let sock = self
            .dir
            .join(format!("script-{}-{n}.sock", std::process::id()));
        let _ = std::fs::remove_file(&sock);
        let l = std::os::unix::net::UnixListener::bind(&sock)
            .and_then(|l| l.set_nonblocking(true).map(|()| l))
            .map_err(|e| format!("{}: {e}", sock.display()))?;
        let (log, from) = open_log();
        let (prog, args, env) = script::command(script, args, &sock, tab, &script::python_dir());
        let out = |f: &Option<std::fs::File>| {
            f.as_ref()
                .and_then(|f| f.try_clone().ok())
                .map_or_else(std::process::Stdio::null, std::process::Stdio::from)
        };
        let child = std::process::Command::new(&prog)
            .args(&args)
            .envs(env)
            .stdin(std::process::Stdio::null())
            .stdout(out(&log))
            .stderr(out(&log))
            .spawn()
            .map_err(|e| {
                let _ = std::fs::remove_file(&sock);
                fl!("script-no-python", error = e.to_string())
            })?;
        if let Some(tap) = self
            .taps
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&tab)
        {
            tap.script_buffer().arm_once();
        }
        self.next += 1;
        let cancel = Arc::new(AtomicBool::new(false));
        let name = script
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        self.runs.insert(
            n,
            Run {
                name,
                cancel: cancel.clone(),
            },
        );
        let host = Arc::new(RunHost {
            run: n,
            taps: self.taps.clone(),
            tx: self.tx.clone(),
            cancel: cancel.clone(),
        });
        let (tx, log_path) = (self.tx.clone(), log_path());
        std::thread::spawn(move || {
            let end = supervise(child, l, host, &cancel, log_path.as_deref(), from);
            let _ = std::fs::remove_file(&sock);
            let _ = tx.send(ScMsg::Ended(n, end));
        });
        Ok(())
    }

    /// Script → Cancel: every run stops (the one on the active tab is the usual).
    pub fn cancel(&mut self) {
        for r in self.runs.values() {
            r.cancel.store(true, Ordering::SeqCst);
        }
        for a in self.asks.drain(..) {
            a.call.reply.send(Err("cancelled".into()));
        }
    }

    pub fn update(&mut self, msg: ScMsg, tabs: &mut Tabs) -> Outcome {
        let mut out = Outcome::default();
        match msg {
            ScMsg::Call(c) => match answer(&c, tabs) {
                Some(a) => c.reply.send(a),
                None => match c.call.as_str() {
                    "Dialog.MessageBox" | "Dialog.Prompt" => {
                        let value = (c.args.get(2))
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string();
                        self.asks.push_back(Ask { call: c, value });
                        out.focus = true;
                    }
                    "Clipboard.Set" => {
                        let text = c.args.first().and_then(Value::as_str).unwrap_or_default();
                        out.clip = Some((Some(text.to_string()), c.reply.clone()));
                    }
                    "Clipboard.Get" => out.clip = Some((None, c.reply.clone())),
                    "Session.Connect" => {
                        match connect_arg(c.args.first().and_then(Value::as_str).unwrap_or("")) {
                            // Answered once the tab exists (the app opens it).
                            Some(name) => out.connect = Some((name, c.reply.clone())),
                            None => c.reply.send(Err(
                                "Session.Connect: only \"/S name\" (a saved session) is supported"
                                    .into(),
                            )),
                        }
                    }
                    other => c.reply.send(Err(format!("unknown call {other}"))),
                },
            },
            ScMsg::Ended(n, end) => {
                let name = self.runs.remove(&n).map(|r| r.name).unwrap_or_default();
                // Dialogs of that run go with it; their calls fail.
                let (gone, kept): (VecDeque<_>, VecDeque<_>) =
                    self.asks.drain(..).partition(|a| a.call.run == n);
                self.asks = kept;
                let gone_any = !gone.is_empty();
                for a in gone {
                    a.call.reply.send(Err("cancelled".into()));
                }
                out.refocus = gone_any && self.asks.is_empty();
                if self.runs.is_empty() {
                    for tap in self.taps.lock().unwrap_or_else(|e| e.into_inner()).values() {
                        tap.script_buffer().disarm();
                    }
                }
                out.notice = Some(match end {
                    End::Done => fl!("script-done", name = name),
                    End::Cancelled => fl!("script-stopped", name = name),
                    End::Failed(e) => fl!("script-failed", name = name, error = e),
                });
            }
            ScMsg::Picked(_) | ScMsg::Start(..) | ScMsg::StartOn(..) => {}
            ScMsg::Input(v) => {
                if let Some(a) = self.asks.front_mut() {
                    a.value = v;
                }
            }
            ScMsg::Clip(reply, text) => reply.send(Ok(json!(text))),
            ScMsg::Ok | ScMsg::Cancel | ScMsg::Button(_) => {
                if let Some(a) = self.asks.pop_front() {
                    let ok = !matches!(msg, ScMsg::Cancel);
                    let reply = match (a.call.call.as_str(), &msg) {
                        ("Dialog.Prompt", _) if ok => json!(a.value),
                        ("Dialog.Prompt", _) => Value::Null,
                        (_, ScMsg::Button(id)) => json!(id),
                        // Enter: the first button; Esc: Cancel / No, else the only one.
                        _ => json!(match (buttons(&a.call), ok) {
                            (_, true) => ID_FIRST[buttons(&a.call).min(5)],
                            (1 | 3, false) => ID_CANCEL,
                            (4, false) => ID_NO,
                            _ => ID_OK,
                        }),
                    };
                    a.call.reply.send(Ok(reply));
                    out.refocus = self.asks.is_empty();
                }
            }
        }
        out
    }

    /// The dialog of the oldest ask.
    pub fn dialog(&self) -> Option<Element<'_, Message>> {
        let a = self.asks.front()?;
        let s = |i: usize| {
            a.call
                .args
                .get(i)
                .and_then(Value::as_str)
                .unwrap_or_default()
        };
        let title = match s(1) {
            "" => fl!("script-title"),
            t => t.to_string(),
        };
        let m = Message::Script;
        let mut body = widget::column::with_capacity(2)
            .spacing(10)
            .push(widget::text::body(s(0).to_string()));
        let prompt = a.call.call == "Dialog.Prompt";
        if prompt {
            let mut input = widget::text_input("", a.value.clone())
                .id(self.input_id.clone())
                .on_input(|v| Message::Script(ScMsg::Input(v)))
                .on_submit(|_| Message::Script(ScMsg::Ok));
            if a.call.args.get(3).and_then(Value::as_bool) == Some(true) {
                input = input.password();
            }
            body = body.push(input);
        }
        let btn =
            |label: String, id: i64| widget::button::standard(label).on_press(m(ScMsg::Button(id)));
        let mut d = widget::dialog().title(title).control(body);
        d = if prompt {
            d.primary_action(widget::button::suggested(fl!("ok")).on_press(m(ScMsg::Ok)))
                .secondary_action(
                    widget::button::standard(fl!("cancel")).on_press(m(ScMsg::Cancel)),
                )
        } else {
            let ok = widget::button::suggested(fl!("ok")).on_press(m(ScMsg::Button(ID_OK)));
            let yes = widget::button::suggested(fl!("yes")).on_press(m(ScMsg::Button(ID_YES)));
            match buttons(&a.call) {
                1 => d
                    .primary_action(ok)
                    .secondary_action(btn(fl!("cancel"), ID_CANCEL)),
                3 => d
                    .primary_action(yes)
                    .secondary_action(btn(fl!("no"), ID_NO))
                    .tertiary_action(btn(fl!("cancel"), ID_CANCEL)),
                4 => d
                    .primary_action(yes)
                    .secondary_action(btn(fl!("no"), ID_NO)),
                _ => d.primary_action(ok),
            }
        };
        Some(d.into())
    }

    pub fn subscription(&self) -> Subscription<Message> {
        struct Feed(Arc<Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<ScMsg>>>>);
        impl Hash for Feed {
            fn hash<H: Hasher>(&self, _: &mut H) {}
        }
        Subscription::run_with(Feed(self.rx.clone()), |f| {
            let rx = f.0.clone();
            cosmic::iced::stream::channel(16, async move |mut out| {
                let taken = rx.lock().unwrap_or_else(|e| e.into_inner()).take();
                if let Some(mut rx) = taken {
                    while let Some(m) = rx.recv().await {
                        if out.send(Message::Script(m)).await.is_err() {
                            break;
                        }
                    }
                }
                std::future::pending::<()>().await;
            })
        })
    }
}

/// `/S "name"` or `/S name`: the saved session's name.
fn connect_arg(cmd: &str) -> Option<String> {
    let rest = cmd.trim().strip_prefix("/S")?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let name = rest.trim().trim_matches('"').trim();
    (!name.is_empty()).then(|| name.to_string())
}

/// Calls about tabs, answered at once. `None`: the window must (dialogs, sessions).
fn answer(c: &Call, tabs: &mut Tabs) -> Option<Answer> {
    let order = tabs.order();
    let known = |id: u64| {
        order
            .contains(&id)
            .then_some(id)
            .ok_or("no such tab".to_string())
    };
    Some(match c.call.as_str() {
        "GetTabCount" => Ok(json!(order.len())),
        "GetTab" => {
            let n = c.args.first().and_then(Value::as_u64).unwrap_or(0) as usize;
            n.checked_sub(1)
                .and_then(|i| order.get(i).copied())
                .map(|id| json!(id))
                .ok_or_else(|| format!("GetTab: no tab {n}"))
        }
        "GetActiveTab" => tabs
            .active_terminal()
            .map(|id| json!(id))
            .ok_or_else(|| "no terminal tab is active".into()),
        "Session.Connected" => known(c.tab).map(|id| json!(tabs.alive(id))),
        "Session.Path" => known(c.tab).map(|id| json!(tabs.session_of(id).unwrap_or_default())),
        "Session.Disconnect" => known(c.tab).map(|id| {
            tabs.disconnect_tab(id);
            Value::Null
        }),
        "Screen.Get" => known(c.tab).and_then(|id| screen(tabs, id)).map(|s| {
            let n = |i: usize| c.args.get(i).and_then(Value::as_i64).unwrap_or(0);
            json!(script::slice(&s.rows, n(0), n(1), n(2), n(3)))
        }),
        "Screen.Info" => known(c.tab).and_then(|id| screen(tabs, id)).map(|s| {
            json!([
                s.row + 1,
                s.col + 1,
                s.rows.len(),
                s.rows.first().map_or(0, |r| r.chars().count())
            ])
        }),
        "Screen.Selection" => known(c.tab)
            .and_then(|id| screen(tabs, id))
            .map(|s| json!(s.selection)),
        "Session.Log" => known(c.tab).and_then(|id| {
            let flag = |i: usize| c.args.get(i).and_then(Value::as_bool);
            tabs.script_log(id, flag(0).unwrap_or(true), flag(1))
                .map(|()| Value::Null)
        }),
        "Session.LogFileName" => known(c.tab).map(|id| json!(tabs.log_file(id))),
        "Tab.Index" => {
            known(c.tab).map(|id| json!(order.iter().position(|&t| t == id).map_or(0, |i| i + 1)))
        }
        "Tab.Caption" => known(c.tab).map(|id| json!(tabs.caption(id).unwrap_or_default())),
        "Tab.Activate" => known(c.tab).map(|id| {
            tabs.show(id);
            Value::Null
        }),
        _ => return None,
    })
}

/// SecureCRT's results of a MessageBox.
const ID_OK: i64 = 1;
const ID_CANCEL: i64 = 2;
const ID_YES: i64 = 6;
const ID_NO: i64 = 7;
/// The result of Enter for each `BUTTON_*` code (0 OK, 1 OK/Cancel, 2 –, 3 Yes/No/Cancel, 4 Yes/No, 5 –).
const ID_FIRST: [i64; 6] = [ID_OK, ID_OK, ID_OK, ID_YES, ID_YES, ID_OK];

/// The `BUTTON_*` code of a MessageBox call.
fn buttons(c: &Call) -> usize {
    c.args
        .get(2)
        .and_then(Value::as_u64)
        .map_or(0, |n| n as usize)
}

fn screen(tabs: &Tabs, id: u64) -> Result<crate::term::Screen, String> {
    tabs.screen(id).ok_or_else(|| "not a terminal tab".into())
}

/// Looks after one run: accepts the script's connection, serves it, ends the process on cancel.
fn supervise(
    mut child: std::process::Child,
    l: std::os::unix::net::UnixListener,
    host: Arc<RunHost>,
    cancel: &Arc<AtomicBool>,
    log: Option<&Path>,
    log_from: u64,
) -> End {
    let mut served = false;
    let status = loop {
        if cancel.load(Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            return End::Cancelled;
        }
        if !served && let Ok((s, _)) = l.accept() {
            served = true;
            let _ = s.set_nonblocking(false);
            let (host, cancel) = (host.clone(), cancel.clone());
            std::thread::spawn(move || {
                if let Ok(w) = s.try_clone() {
                    script::serve(w, s, &*host, &cancel);
                }
            });
        }
        match child.try_wait() {
            Ok(Some(st)) => break Ok(st),
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => break Err(e),
        }
    };
    // Waits of a script that is gone stop.
    let user = cancel.swap(true, Ordering::SeqCst);
    match status {
        Ok(st) if st.success() => End::Done,
        _ if user => End::Cancelled,
        Ok(st) => End::Failed(last_line(log, log_from).unwrap_or_else(|| st.to_string())),
        Err(e) => End::Failed(e.to_string()),
    }
}

/// The last non-empty line of the script log written since `from`.
fn last_line(log: Option<&Path>, from: u64) -> Option<String> {
    let mut f = std::fs::File::open(log?).ok()?;
    f.seek(SeekFrom::Start(from)).ok()?;
    let mut s = String::new();
    f.read_to_string(&mut s).ok()?;
    s.lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .map(|l| l.trim().to_string())
}

fn log_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::home_dir().map(|h| h.join(".local/state")))?;
    Some(base.join("shagshell/script.log"))
}

/// The script log opened for appending (0600), and how long it already is.
fn open_log() -> (Option<std::fs::File>, u64) {
    use std::os::unix::fs::OpenOptionsExt;
    let Some(path) = log_path() else {
        return (None, 0);
    };
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    // Not endless: over 1 MiB it starts again.
    let big = std::fs::metadata(&path).is_ok_and(|m| m.len() > 1 << 20);
    let f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .truncate(false)
        .mode(0o600)
        .open(&path)
        .ok();
    if big && let Some(f) = &f {
        let _ = f.set_len(0);
    }
    let len = f
        .as_ref()
        .and_then(|f| f.metadata().ok())
        .map_or(0, |m| m.len());
    (f, len)
}

/// What one run's server thread uses.
struct RunHost {
    run: u64,
    taps: Taps,
    tx: tokio::sync::mpsc::UnboundedSender<ScMsg>,
    cancel: Arc<AtomicBool>,
}

impl RunHost {
    fn tap(&self, tab: u64) -> Option<crate::term::log::Tap> {
        self.taps
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&tab)
            .cloned()
    }
}

impl Host for RunHost {
    fn buffer(&self, tab: u64) -> Option<Arc<script::Buffer>> {
        let b = self.tap(tab)?.script_buffer();
        b.arm_once();
        Some(b)
    }

    fn input(&self, tab: u64, bytes: Vec<u8>) -> bool {
        match self.tap(tab) {
            Some(t) => {
                t.input(bytes);
                true
            }
            None => false,
        }
    }

    fn ui(&self, call: &str, tab: u64, args: &[Value]) -> Answer {
        let (tx, rx) = mpsc::channel();
        let c = Call {
            run: self.run,
            call: call.to_string(),
            tab,
            args: args.to_vec(),
            reply: Reply(Arc::new(Mutex::new(Some(tx)))),
        };
        self.tx
            .send(ScMsg::Call(c))
            .map_err(|_| "cancelled".to_string())?;
        loop {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(a) => return a,
                Err(mpsc::RecvTimeoutError::Timeout) if !self.cancel.load(Ordering::SeqCst) => {}
                Err(_) => return Err("cancelled".into()),
            }
        }
    }
}

/// Script → Run…: the file from the portal's chooser.
pub async fn pick(title: String) -> Option<PathBuf> {
    use cosmic::dialog::file_chooser::open::Dialog;
    let picked = Dialog::new().title(title).open_file().await.ok()?;
    picked.url().to_file_path().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_takes_a_saved_session_name() {
        assert_eq!(connect_arg("/S \"web 01\""), Some("web 01".into()));
        assert_eq!(connect_arg(" /S core/sw1 "), Some("core/sw1".into()));
        assert_eq!(connect_arg("/SSH2 host"), None);
        assert_eq!(connect_arg("/S"), None);
    }
}
