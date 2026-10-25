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
use shagshell_core::sftp::queue::Dir;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::io::Read;
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
    /// The window's header was dragged.
    Drag,
    /// A button of a MessageBox (its SecureCRT result code).
    Button(i64),
    /// A row of a List dialog, and its filter.
    Pick(usize),
    Filter(String),
    /// A file chosen for `FileOpenDialog` / `FileSaveDialog` (`None`: given up).
    Chosen(Reply, Option<String>),
    /// The clipboard's text, for a script that asked.
    Clip(Reply, String),
    Ok,
    Cancel,
    /// A script wrote to the console (once until the window has looked).
    Output(u64),
    /// The Script Console's buttons.
    ConsoleStop,
    ConsoleClear,
    ConsoleCopy,
    ConsoleDebug(bool),
}

struct Run {
    name: String,
    cancel: Arc<AtomicBool>,
}

/// A dialog a script asked for.
struct Ask {
    call: Call,
    /// Prompt: the text typed.
    value: String,
    /// List: the rows chosen, and the filter typed.
    picked: std::collections::BTreeSet<usize>,
    filter: String,
}

impl Ask {
    fn arg(&self, i: usize) -> &str {
        self.call
            .args
            .get(i)
            .and_then(Value::as_str)
            .unwrap_or_default()
    }

    fn flags(&self) -> script::Flags {
        script::Flags::parse(self.call.args.get(2).and_then(Value::as_i64).unwrap_or(0))
    }

    fn items(&self) -> Vec<String> {
        let list = self.call.args.get(2).and_then(Value::as_array);
        list.map(|l| {
            l.iter()
                .map(|v| v.as_str().unwrap_or_default().to_string())
                .collect()
        })
        .unwrap_or_default()
    }

    fn multi(&self) -> bool {
        self.call.args.get(4).and_then(Value::as_bool) == Some(true)
    }

    /// What the script gets for OK / Enter (`None`: the dialog stays, nothing to give).
    fn accept(&self) -> Value {
        match self.call.call.as_str() {
            "Dialog.Prompt" => json!(self.value),
            "Dialog.List" => {
                let rows = script::list_picked(&self.items(), &self.picked, &self.filter);
                match (self.multi(), rows) {
                    (_, None) => Value::Null,
                    (true, Some(r)) => json!(r),
                    (false, Some(r)) => json!(r[0]),
                }
            }
            _ => json!(self.flags().enter()),
        }
    }

    /// What the script gets for Esc / the window closed.
    fn dismiss(&self) -> Value {
        match self.call.call.as_str() {
            "Dialog.Prompt" | "Dialog.List" => Value::Null,
            _ => json!(self.flags().escape()),
        }
    }
}

pub struct Scripts {
    runs: BTreeMap<u64, Run>,
    next: u64,
    taps: Taps,
    asks: VecDeque<Ask>,
    pub input_id: widget::Id,
    /// The window of the dialog that is up.
    pub win: Option<cosmic::iced::window::Id>,
    /// The size the window was last given (a different kind of dialog resizes it).
    pub win_size: cosmic::iced::Size,
    dir: PathBuf,
    console: Arc<Mutex<script::Console>>,
    /// Set when `Output` is queued, cleared when the window has taken it.
    dirty: Arc<AtomicBool>,
    pub debug: Arc<AtomicBool>,
    /// The newest run the console opened for by itself (a closed console stays closed).
    shown_run: u64,
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
    pub clip: Vec<(Option<String>, Reply)>,
    /// `FileOpenDialog` / `FileSaveDialog`: the chooser to show.
    pub files: Vec<(Call, bool)>,
    /// A script wrote (run id): the console opens when wanted.
    pub output: Option<u64>,
    /// The console's text for the clipboard.
    pub copy: Option<String>,
    /// A run ended: its id, and whether it finished well (a pre-script's session opens or not).
    pub ended: Option<(u64, bool)>,
    /// `Session.Create`: the session to save (the app owns the store).
    pub create: Option<(script::NewSession, Reply)>,
    /// `Window.Activate`: raise the main window.
    pub raise: bool,
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
            win: None,
            win_size: cosmic::iced::Size::ZERO,
            dir,
            console: Arc::default(),
            dirty: Arc::default(),
            debug: Arc::default(),
            shown_run: 0,
            tx,
            rx: Arc::new(Mutex::new(Some(rx))),
        }
    }

    pub fn active(&self) -> bool {
        !self.asks.is_empty()
    }

    /// The window size the oldest ask needs (a list is taller).
    pub fn size(&self) -> cosmic::iced::Size {
        let list = self
            .asks
            .front()
            .is_some_and(|a| a.call.call == "Dialog.List");
        cosmic::iced::Size::new(460.0, if list { 420.0 } else { 220.0 })
    }

    /// The window of the oldest ask.
    pub fn settings(&self) -> cosmic::iced::window::Settings {
        cosmic::iced::window::Settings {
            size: self.size(),
            min_size: Some(cosmic::iced::Size::new(360.0, 160.0)),
            decorations: false,
            exit_on_close_request: false,
            ..Default::default()
        }
    }

    /// Starts `script` on tab `tab`.
    pub fn run(&mut self, script: &Path, args: &[String], tab: u64) -> Result<u64, String> {
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
        let log = Arc::new(Mutex::new(open_log()));
        let (prog, args, env) = script::command(script, args, &sock, tab, &script::python_dir());
        let mut child = std::process::Command::new(&prog)
            .args(&args)
            .envs(env)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
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
        let name = name_of(script);
        self.runs.insert(
            n,
            Run {
                name,
                cancel: cancel.clone(),
            },
        );
        let sink = Sink {
            run: n,
            console: self.console.clone(),
            dirty: self.dirty.clone(),
            tx: self.tx.clone(),
            last_err: Arc::default(),
        };
        let time = shagshell_core::logfile::now().strftime("%H:%M:%S");
        sink.push(
            script::Kind::Mark,
            format!("── {} · {time} ──", name_of(script)),
        );
        let streams: [(Option<Box<dyn Read + Send>>, script::Kind); 2] = [
            (
                child.stdout.take().map(|r| Box::new(r) as _),
                script::Kind::Out,
            ),
            (
                child.stderr.take().map(|r| Box::new(r) as _),
                script::Kind::Err,
            ),
        ];
        let pumps: Vec<_> = streams
            .into_iter()
            .filter_map(|(r, kind)| {
                let (r, sink, log) = (r?, sink.clone(), log.clone());
                Some(std::thread::spawn(move || pump(r, kind, &sink, &log)))
            })
            .collect();
        let host = Arc::new(RunHost {
            run: n,
            taps: self.taps.clone(),
            sink: sink.clone(),
            debug: self.debug.clone(),
            cancel: cancel.clone(),
        });
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let end = supervise(child, l, host, &cancel, pumps, &sink);
            let _ = std::fs::remove_file(&sock);
            let _ = tx.send(ScMsg::Ended(n, end));
        });
        Ok(n)
    }

    fn sink(&self, run: u64) -> Sink {
        Sink {
            run,
            console: self.console.clone(),
            dirty: self.dirty.clone(),
            tx: self.tx.clone(),
            last_err: Arc::default(),
        }
    }

    /// Script → Cancel: every run stops (the one on the active tab is the usual).
    /// A script runs (Script → Cancel has something to stop).
    pub fn running(&self) -> bool {
        !self.runs.is_empty()
    }

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
                    "Dialog.MessageBox" | "Dialog.Prompt" | "Dialog.List" => {
                        let value = match c.call.as_str() {
                            "Dialog.Prompt" => (c.args.get(2))
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_string(),
                            _ => String::new(),
                        };
                        let mut picked = std::collections::BTreeSet::new();
                        // The row to start on (0-based); one that does not exist is none.
                        if c.call == "Dialog.List"
                            && let Some(d) = c.args.get(3).and_then(Value::as_u64)
                            && c.args
                                .get(2)
                                .and_then(Value::as_array)
                                .is_some_and(|l| (d as usize) < l.len())
                        {
                            picked.insert(d as usize);
                        }
                        self.asks.push_back(Ask {
                            call: c,
                            value,
                            picked,
                            filter: String::new(),
                        });
                        out.focus = true;
                    }
                    "Dialog.FileOpen" | "Dialog.FileSave" => {
                        let save = c.call == "Dialog.FileSave";
                        out.files.push((c, save));
                    }
                    "Clipboard.Set" => {
                        let text = c.args.first().and_then(Value::as_str).unwrap_or_default();
                        out.clip.push((Some(text.to_string()), c.reply.clone()));
                    }
                    "Clipboard.Get" => out.clip.push((None, c.reply.clone())),
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
                    "Session.Create" => match script::new_session(&c.args) {
                        Ok(n) => out.create = Some((n, c.reply.clone())),
                        Err(e) => c.reply.send(Err(e)),
                    },
                    "Session.SetStatusText" => {
                        out.notice = c.args.first().and_then(Value::as_str).map(str::to_string);
                        c.reply.send(Ok(Value::Null));
                    }
                    "Window.Activate" => {
                        out.raise = true;
                        c.reply.send(Ok(Value::Null));
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
                let failed = matches!(end, End::Failed(_));
                out.ended = Some((n, matches!(end, End::Done)));
                let notice = match end {
                    End::Done => fl!("script-done", name = name),
                    End::Cancelled => fl!("script-stopped", name = name),
                    End::Failed(e) => fl!("script-failed", name = name, error = e),
                };
                let kind = match failed {
                    true => script::Kind::Err,
                    false => script::Kind::Mark,
                };
                self.sink(n).push(kind, format!("── {notice} ──"));
                out.notice = Some(notice);
            }
            ScMsg::Output(first) => {
                self.dirty.store(false, Ordering::SeqCst);
                // Several runs can share one message: the newest one counts.
                let run = (self.console.lock().unwrap_or_else(|e| e.into_inner()))
                    .last(1)
                    .first()
                    .map_or(first, |l| l.run.max(first));
                // Once per run: a console closed by hand stays closed for it.
                if run > self.shown_run {
                    self.shown_run = run;
                    out.output = Some(run);
                }
            }
            ScMsg::ConsoleStop => {
                self.cancel();
            }
            ScMsg::ConsoleClear => self
                .console
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clear(),
            ScMsg::ConsoleCopy => {
                out.copy = Some(
                    self.console
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .text(),
                );
            }
            ScMsg::ConsoleDebug(on) => self.debug.store(on, Ordering::SeqCst),
            ScMsg::Picked(_) | ScMsg::Start(..) | ScMsg::StartOn(..) | ScMsg::Drag => {}
            ScMsg::Input(v) => {
                if let Some(a) = self.asks.front_mut() {
                    a.value = v;
                }
            }
            ScMsg::Clip(reply, text) => reply.send(Ok(json!(text))),
            ScMsg::Chosen(reply, path) => reply.send(Ok(path.map_or(Value::Null, |p| json!(p)))),
            ScMsg::Pick(i) => {
                if let Some(a) = self.asks.front_mut() {
                    match a.multi() {
                        true if a.picked.remove(&i) => {}
                        true => {
                            a.picked.insert(i);
                        }
                        false => a.picked = [i].into(),
                    }
                }
            }
            ScMsg::Filter(f) => {
                if let Some(a) = self.asks.front_mut() {
                    a.filter = f;
                }
            }
            ScMsg::Ok | ScMsg::Cancel | ScMsg::Button(_) => {
                // OK on a List with nothing to give leaves the dialog up.
                if matches!(msg, ScMsg::Ok)
                    && self
                        .asks
                        .front()
                        .is_some_and(|a| a.call.call == "Dialog.List" && a.accept().is_null())
                {
                    return out;
                }
                if let Some(a) = self.asks.pop_front() {
                    let reply = match &msg {
                        ScMsg::Ok => a.accept(),
                        ScMsg::Button(id) => json!(id),
                        _ => a.dismiss(),
                    };
                    a.call.reply.send(Ok(reply));
                    out.refocus = self.asks.is_empty();
                }
            }
        }
        out
    }

    /// The window of the oldest ask.
    pub fn window_view(&self, focused: bool) -> Option<Element<'_, Message>> {
        use cosmic::iced::Length;
        let a = self.asks.front()?;
        let m = Message::Script;
        let title = match a.arg(1) {
            "" => fl!("script-title"),
            t => t.to_string(),
        };
        let btn = |label: String, id: i64, primary: bool| {
            let b = match primary {
                true => widget::button::suggested(label),
                false => widget::button::standard(label),
            };
            b.on_press(m(ScMsg::Button(id)))
        };
        let mut body = widget::column::with_capacity(3).spacing(12);
        let mut buttons = widget::row::with_capacity(4).spacing(8);
        match a.call.call.as_str() {
            "Dialog.Prompt" => {
                let mut input = widget::text_input("", a.value.clone())
                    .id(self.input_id.clone())
                    .on_input(move |v| m(ScMsg::Input(v)))
                    .on_submit(move |_| m(ScMsg::Ok));
                if a.call.args.get(3).and_then(Value::as_bool) == Some(true) {
                    input = input.password();
                }
                let text = widget::text::body(a.arg(0).to_string());
                let kind = match a.call.args.get(3).and_then(Value::as_bool) {
                    Some(true) => crate::dialog_icons::Kind::Lock,
                    _ => crate::dialog_icons::Kind::Edit,
                };
                let top: Element<'_, Message> = widget::row::with_capacity(2)
                    .spacing(16)
                    .push(kind.icon(48))
                    .push(text)
                    .into();
                body = body.push(top).push(input);
                buttons = buttons
                    .push(widget::button::standard(fl!("cancel")).on_press(m(ScMsg::Cancel)))
                    .push(widget::button::suggested(fl!("ok")).on_press(m(ScMsg::Ok)));
            }
            "Dialog.List" => {
                let items = a.items();
                let mut rows = widget::column::with_capacity(items.len()).spacing(4);
                for i in script::list_matches(&items, &a.filter) {
                    let b = match a.picked.contains(&i) {
                        true => widget::button::suggested(items[i].clone()),
                        false => widget::button::text(items[i].clone()),
                    };
                    rows = rows.push(b.width(Length::Fill).on_press(m(ScMsg::Pick(i))));
                }
                body = body
                    .push(
                        widget::row::with_capacity(2)
                            .spacing(16)
                            .push(crate::dialog_icons::Kind::Question.icon(48))
                            .push(widget::text::body(a.arg(0).to_string())),
                    )
                    .push(
                        widget::text_input(fl!("script-list-filter"), a.filter.clone())
                            .id(self.input_id.clone())
                            .on_input(move |v| m(ScMsg::Filter(v)))
                            .on_submit(move |_| m(ScMsg::Ok)),
                    )
                    .push(widget::scrollable(rows).height(Length::Fixed(220.0)));
                buttons = buttons
                    .push(widget::button::standard(fl!("cancel")).on_press(m(ScMsg::Cancel)))
                    .push(widget::button::suggested(fl!("ok")).on_press(m(ScMsg::Ok)));
            }
            _ => {
                let f = a.flags();
                use crate::dialog_icons::Kind;
                let icon = match f.icon {
                    // A message with no icon of its own still gets one: every dialog of ours has it.
                    script::Icon::None => Some(Kind::Info),
                    script::Icon::Stop => Some(Kind::Stop),
                    script::Icon::Question => Some(Kind::Question),
                    script::Icon::Warning => Some(Kind::Warning),
                    script::Icon::Info => Some(Kind::Info),
                };
                let mut line = widget::row::with_capacity(2).spacing(16);
                if let Some(kind) = icon {
                    line = line.push(kind.icon(48));
                }
                body = body.push(line.push(widget::text::body(a.arg(0).to_string())));
                let ids = f.ids();
                let default = f.enter();
                for id in ids {
                    let label = match id {
                        script::ID_OK => fl!("ok"),
                        script::ID_CANCEL => fl!("cancel"),
                        script::ID_YES => fl!("yes"),
                        script::ID_NO => fl!("no"),
                        script::ID_RETRY => fl!("script-retry"),
                        script::ID_ABORT => fl!("script-abort"),
                        _ => fl!("script-ignore"),
                    };
                    buttons = buttons.push(btn(label, id, id == default));
                }
            }
        }
        let content = widget::column::with_children(vec![
            widget::header_bar()
                .title(title)
                .on_close(m(ScMsg::Cancel))
                .on_drag(m(ScMsg::Drag))
                .focused(focused)
                .into(),
            widget::container(
                widget::column::with_children(vec![
                    body.into(),
                    widget::container(buttons)
                        .width(Length::Fill)
                        .align_x(cosmic::iced::alignment::Horizontal::Right)
                        .into(),
                ])
                .spacing(16),
            )
            .padding(16)
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),
        ]);
        // A window of our own gets no background from libcosmic: without one it is see-through.
        Some(
            widget::container(content)
                .width(Length::Fill)
                .height(Length::Fill)
                .class(cosmic::theme::Container::WindowBackground)
                .into(),
        )
    }

    /// The Script Console window.
    pub fn console_view(&self) -> Element<'_, Message> {
        use cosmic::iced::{Color, Length};
        use script::Kind;
        let m = Message::Script;
        let lines = self
            .console
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .last(500);
        let body: Element<'_, Message> = match lines.is_empty() {
            true => widget::container(widget::text::caption(fl!("console-empty")))
                .padding(16)
                .into(),
            false => {
                let rows = lines.into_iter().map(|l| {
                    let t = widget::text(l.text).font(cosmic::font::mono()).size(13);
                    match l.kind {
                        Kind::Err => t.class(cosmic::theme::Text::Color(Color::from_rgb(
                            0.95, 0.35, 0.35,
                        ))),
                        Kind::Debug => t.class(cosmic::theme::Text::Accent),
                        Kind::Mark => t.font(cosmic::font::semibold()),
                        Kind::Out => t,
                    }
                    .into()
                });
                widget::scrollable(
                    widget::column::with_children(rows.collect::<Vec<Element<'_, Message>>>())
                        .padding([8, 12]),
                )
                .anchor_bottom()
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
            }
        };
        // Icons with a hint: the console is often a narrow tab at the side.
        let tool = |icon: &'static str, tip: String, msg: Option<ScMsg>| -> Element<'_, Message> {
            crate::toolbar::tip_button(icon, tip, msg.map(m))
        };
        let tools = widget::row::with_capacity(5)
            .spacing(8)
            .align_y(cosmic::iced::Alignment::Center)
            .padding([8, 12])
            .push(tool(
                "media-playback-stop-symbolic",
                fl!("console-stop"),
                (!self.runs.is_empty()).then_some(ScMsg::ConsoleStop),
            ))
            .push(tool(
                "edit-delete-symbolic",
                fl!("console-clear"),
                Some(ScMsg::ConsoleClear),
            ))
            .push(tool(
                "edit-copy-symbolic",
                fl!("console-copy"),
                Some(ScMsg::ConsoleCopy),
            ))
            .push(widget::Space::new().width(Length::Fill))
            .push(widget::tooltip(
                widget::toggler(self.debug.load(Ordering::SeqCst))
                    .on_toggle(move |on| m(ScMsg::ConsoleDebug(on))),
                widget::text::caption(fl!("console-debug")),
                widget::tooltip::Position::Bottom,
            ));
        let content = widget::column::with_children(vec![tools.into(), body]);
        widget::container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .class(cosmic::theme::Container::WindowBackground)
            .into()
    }

    /// A script's file chooser (`FileOpenDialog` / `FileSaveDialog`): the path, or `None`.
    pub async fn choose(args: Vec<Value>, save: bool) -> Option<String> {
        use cosmic::dialog::file_chooser::{FileFilter, open, save as sv};
        let s = |i: usize| {
            args.get(i)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        let (title, label) = (s(0), s(1));
        let filters = script::file_filters(&s(3));
        let filter = |(d, exts): &(String, Vec<String>)| {
            exts.iter()
                .fold(FileFilter::new(d), |f, e| f.glob(&format!("*.{e}")))
        };
        let url = match save {
            true => {
                let mut d = sv::Dialog::new().title(title);
                if !label.is_empty() {
                    d = d.accept_label(label);
                }
                for f in &filters {
                    d = d.filter(filter(f));
                }
                d.save_file().await.ok()?.url()?.clone()
            }
            false => {
                let mut d = open::Dialog::new().title(title);
                if !label.is_empty() {
                    d = d.accept_label(label);
                }
                for f in &filters {
                    d = d.filter(filter(f));
                }
                d.open_file().await.ok()?.url().clone()
            }
        };
        url.to_file_path().ok().map(|p| p.display().to_string())
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
        "FileTransfer.Connect" => known(c.tab).and_then(|id| {
            tabs.sftp_launch(id)
                .map(|l| json!({"program": l.program, "args": l.args, "env": l.env}))
                .ok_or_else(|| "FileTransfer: this tab is not an SSH session".to_string())
        }),
        "Session.Lock" | "Session.Unlock" => known(c.tab).and_then(|id| {
            tabs.set_lock(id, c.call == "Session.Lock", c.run)
                .map(|()| Value::Null)
        }),
        "Session.Locked" => known(c.tab).map(|id| json!(tabs.locked(id))),
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
        "Screen.Info" => known(c.tab)
            .and_then(|id| screen(tabs, id))
            .map(|s| json!([s.row + 1, s.col + 1, s.rows.len(), s.cols])),
        "Screen.Selection" => known(c.tab)
            .and_then(|id| screen(tabs, id))
            .map(|s| json!(s.selection)),
        "Session.Log" => known(c.tab).and_then(|id| {
            let flag = |i: usize| c.args.get(i).and_then(Value::as_bool);
            tabs.script_log(id, flag(0).unwrap_or(true), flag(1))
                .map(|()| Value::Null)
        }),
        "Session.LogText" => known(c.tab).and_then(|id| {
            let text = c.args.first().and_then(Value::as_str).unwrap_or_default();
            tabs.script_log_text(id, text).map(|()| Value::Null)
        }),
        "Session.LogFileName" => known(c.tab).map(|id| json!(tabs.log_file(id))),
        "Screen.Clear" => known(c.tab)
            .and_then(|id| tabs.clear_screen(id).ok_or("not a terminal tab".into()))
            .map(|()| Value::Null),
        "Window.Caption" => Ok(json!("ShagShell Secure")),
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

fn screen(tabs: &Tabs, id: u64) -> Result<crate::term::Screen, String> {
    tabs.screen(id).ok_or_else(|| "not a terminal tab".into())
}

/// Looks after one run: accepts the script's connection, serves it, ends the process on cancel.
fn supervise(
    mut child: std::process::Child,
    l: std::os::unix::net::UnixListener,
    host: Arc<RunHost>,
    cancel: &Arc<AtomicBool>,
    pumps: Vec<std::thread::JoinHandle<()>>,
    sink: &Sink,
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
    // What it wrote last is in the console before the end is told.
    // (A grandchild that kept the pipes open must not hold the end back: a moment, then go.)
    for _ in 0..10 {
        if pumps.iter().all(|p| p.is_finished()) {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    match status {
        Ok(st) if st.success() => End::Done,
        _ if user => End::Cancelled,
        Ok(st) => End::Failed(sink.last_error().unwrap_or_else(|| st.to_string())),
        Err(e) => End::Failed(e.to_string()),
    }
}

fn name_of(script: &Path) -> String {
    script
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

/// Where a run's lines go: the console, and the window is told once.
#[derive(Clone)]
struct Sink {
    run: u64,
    console: Arc<Mutex<script::Console>>,
    dirty: Arc<AtomicBool>,
    tx: tokio::sync::mpsc::UnboundedSender<ScMsg>,
    /// The last non-blank line this run wrote to stderr (a Python error ends with its message).
    last_err: Arc<Mutex<Option<String>>>,
}

impl Sink {
    fn push(&self, kind: script::Kind, text: String) {
        if kind == script::Kind::Err && !text.trim().is_empty() {
            *self.last_err.lock().unwrap_or_else(|e| e.into_inner()) =
                Some(text.trim().to_string());
        }
        self.console
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(self.run, kind, text);
        // The start and the end of a quiet run are not output: no window for them.
        if kind != script::Kind::Mark && !self.dirty.swap(true, Ordering::SeqCst) {
            let _ = self.tx.send(ScMsg::Output(self.run));
        }
    }

    fn last_error(&self) -> Option<String> {
        self.last_err
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

/// Copies a script's stdout / stderr, line by line, into the console and the log file.
fn pump(
    r: Box<dyn Read + Send>,
    kind: script::Kind,
    sink: &Sink,
    log: &Mutex<Option<std::fs::File>>,
) {
    use std::io::{BufRead, Write};
    let mut r = std::io::BufReader::new(r);
    let mut buf = Vec::new();
    // A line without an end comes in pieces of 64 KiB, not in one allocation.
    while matches!((&mut r).take(64 << 10).read_until(b'\n', &mut buf), Ok(n) if n > 0) {
        let text = String::from_utf8_lossy(&buf)
            .trim_end_matches(['\r', '\n'])
            .to_string();
        buf.clear();
        if let Some(f) = log.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            let _ = writeln!(f, "{text}");
        }
        sink.push(kind, text);
    }
}

fn log_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::home_dir().map(|h| h.join(".local/state")))?;
    Some(base.join("shagshell/script.log"))
}

/// The script log opened for appending (0600).
fn open_log() -> Option<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    let path = log_path()?;
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
    f
}

/// What one run's server thread uses.
struct RunHost {
    run: u64,
    taps: Taps,
    sink: Sink,
    debug: Arc<AtomicBool>,
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
    fn debug(&self) -> bool {
        self.debug.load(Ordering::SeqCst)
    }

    fn trace(&self, line: String) {
        self.sink.push(script::Kind::Debug, line);
    }

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
        match call {
            "FileTransfer.Upload" => self.transfer(Dir::Up, tab, args),
            "FileTransfer.Download" => self.transfer(Dir::Down, tab, args),
            _ => self.ask(call, tab, args),
        }
    }
}

impl RunHost {
    /// `crt.FileTransfer`: this thread is the script's, so it may wait for the whole copy. The
    /// tab's ssh is asked of the window, then run here as an SFTP session of its own (the open
    /// connection is reused through ControlPath).
    fn transfer(&self, dir: Dir, tab: u64, args: &[Value]) -> Answer {
        use std::process::{Command, Stdio};
        let arg = |i: usize| args.get(i).and_then(Value::as_str).unwrap_or("");
        if arg(0).is_empty() {
            return Err("FileTransfer: a file name is needed".into());
        }
        let conn = self.ask("FileTransfer.Connect", tab, &[])?;
        let list = |k: &str| -> Vec<String> {
            (conn[k].as_array().into_iter().flatten())
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        };
        let env: Vec<(String, String)> = (conn["env"].as_array().into_iter().flatten())
            .filter_map(|p| Some((p[0].as_str()?.to_string(), p[1].as_str()?.to_string())))
            .collect();
        let program = conn["program"].as_str().unwrap_or("ssh");
        let mut child = Command::new(program)
            .args(list("args"))
            .envs(env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("{program}: {e}"))?;
        // A cancelled run must not wait for a stuck ssh (an askpass prompt, a dead network).
        let pid = child.id();
        let done = Arc::new(AtomicBool::new(false));
        {
            let (done, cancel) = (done.clone(), self.cancel.clone());
            std::thread::spawn(move || {
                while !done.load(Ordering::SeqCst) {
                    if cancel.load(Ordering::SeqCst) {
                        let _ = Command::new("kill").arg(pid.to_string()).status();
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            });
        }
        let (Some(w), Some(r)) = (child.stdin.take(), child.stdout.take()) else {
            return Err("FileTransfer: no pipes to ssh".into());
        };
        let result = (|| -> std::io::Result<()> {
            let mut c = shagshell_core::sftp::proto::Client::new(r, w)?;
            let (home, given) = match dir {
                Dir::Up => (c.realpath(std::path::Path::new("."))?, arg(1).to_string()),
                Dir::Down => {
                    let home = std::env::home_dir().unwrap_or_default();
                    // `~/x` is the user's home; anything else counts from Downloads.
                    match arg(1).strip_prefix("~/") {
                        Some(rest) => (home, rest.to_string()),
                        None => (home.join("Downloads"), arg(1).to_string()),
                    }
                }
            };
            let src = match dir {
                Dir::Up => arg(0).strip_prefix("~/").map_or_else(
                    || arg(0).to_string(),
                    |r| {
                        let h = std::env::home_dir().unwrap_or_default();
                        h.join(r).to_string_lossy().into_owned()
                    },
                ),
                Dir::Down => arg(0).to_string(),
            };
            shagshell_core::sftp::ops::script_copy(&mut c, dir, &src, &given, &home, || {
                !self.cancel.load(Ordering::SeqCst)
            })
        })();
        done.store(true, Ordering::SeqCst);
        let _ = child.kill();
        let _ = child.wait();
        match result {
            Ok(()) => Ok(json!(true)),
            Err(e) => {
                // ssh's own words (host key, permission denied…), now that it is gone.
                let mut said = String::new();
                if let Some(mut err) = child.stderr.take() {
                    let _ = err.read_to_string(&mut said);
                }
                let said = said.trim();
                let tail = said.get(said.len().saturating_sub(300)..).unwrap_or(said);
                Err(match tail.is_empty() {
                    true => format!("FileTransfer: {e}"),
                    false => format!("FileTransfer: {e} ({tail})"),
                })
            }
        }
    }

    fn ask(&self, call: &str, tab: u64, args: &[Value]) -> Answer {
        let (tx, rx) = mpsc::channel();
        let c = Call {
            run: self.run,
            call: call.to_string(),
            tab,
            args: args.to_vec(),
            reply: Reply(Arc::new(Mutex::new(Some(tx)))),
        };
        self.sink
            .tx
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

/// The portal's file chooser, opened in `dir` when it is a folder (libcosmic's dialog has no way
/// to pass one).
mod portal {
    use ashpd::desktop::file_chooser::{FileFilter, OpenFileRequest, SaveFileRequest};
    use std::path::{Path, PathBuf};

    fn folder(dir: &Path) -> Option<&Path> {
        dir.is_dir().then_some(dir)
    }

    fn filters() -> [FileFilter; 2] {
        [
            FileFilter::new("Python").glob("*.py"),
            FileFilter::new(&crate::fl!("script-filter-all")).glob("*"),
        ]
    }

    pub async fn open(title: &str, dir: &Path, folders: bool) -> Option<PathBuf> {
        let mut r = OpenFileRequest::default()
            .title(Some(title))
            .modal(true)
            .directory(folders);
        if !folders {
            r = r.filters(filters());
        }
        if let Some(d) = folder(dir) {
            r = r.current_folder::<&Path>(Some(d)).ok()?;
        }
        let picked = r.send().await.ok()?.response().ok()?;
        picked.uris().first()?.to_file_path().ok()
    }

    pub async fn save(title: &str, dir: &Path) -> Option<PathBuf> {
        let mut r = SaveFileRequest::default()
            .title(Some(title))
            .modal(true)
            .filter(FileFilter::new("Python").glob("*.py"));
        if let Some(d) = folder(dir) {
            r = r.current_folder::<&Path>(Some(d)).ok()?;
        }
        let picked = r.send().await.ok()?.response().ok()?;
        picked.uris().first()?.to_file_path().ok()
    }
}

/// Stop Recording: where to put the script (`.py` is added when missing).
pub async fn save_script(title: String, dir: PathBuf) -> Option<PathBuf> {
    let mut path = portal::save(&title, &dir).await?;
    if path.extension().is_none() {
        path.set_extension("py");
    }
    Some(path)
}

/// A script path typed by hand, or taken from a chooser: where the pick goes.
#[derive(Clone, Copy, Debug)]
pub enum Slot {
    /// A Button Bar button's script.
    Button,
    /// A scheduled job's script.
    Sched,
    /// Session Options: the script before connecting / on opening.
    Pre,
    Open,
}

/// A script's field with a «Browse…» button beside it.
pub fn with_browse<'a>(input: Element<'a, Message>, slot: Slot) -> Element<'a, Message> {
    widget::row::with_children(vec![
        input,
        widget::button::standard(fl!("so-browse"))
            .on_press(Message::ScriptFile(slot))
            .into(),
    ])
    .spacing(8)
    .align_y(cosmic::iced::Alignment::Center)
    .into()
}

/// Script → Run…: the file from the portal's chooser, opened in `dir`.
pub async fn pick(title: String, dir: PathBuf) -> Option<PathBuf> {
    portal::open(&title, &dir, false).await
}

/// Script → Choose scripts folder….
pub async fn pick_folder(title: String, dir: PathBuf) -> Option<PathBuf> {
    portal::open(&title, &dir, true).await
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
