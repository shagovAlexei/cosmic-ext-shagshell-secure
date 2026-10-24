//! The SFTP tab (SecureCRT's «Connect SFTP Tab», laid out as a commander): the local panel, the
//! server's panel, the transfer queue under them. Keys as in shagoff commander (Tab, Enter,
//! Backspace, Insert, F5–F8). Local folders are read here (fast); the server through the
//! workers in `sftp_worker`.

use crate::app::Message;
use crate::fl;
use crate::sftp_worker::{self, Cmd, Event, Events, Worker};
use crate::term::TabId;
use cosmic::app::Task;
use cosmic::iced::keyboard::{Modifiers, key::Code, key::Physical};
use cosmic::iced::widget::scrollable::{self as scroll, AbsoluteOffset};
use cosmic::iced::{Alignment, Length, Subscription, futures::SinkExt};
use cosmic::widget;
use cosmic::{Element, theme};
use shagshell_core::panel::{Entry, Panel, size_text, time_text};
use shagshell_core::sftp::queue::{Choice, Dir, Queue, State};
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::Instant;

const LOCAL: usize = 0;
const REMOTE: usize = 1;
const ROW: f32 = 26.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
    Enter,
    Back,
    Switch,
    Mark,
    MarkAll,
    Copy,
    Move,
    Rename,
    Mkdir,
    Delete,
    Refresh,
}

/// The commander's keys, by physical key (any layout).
pub fn key(k: &Physical, m: Modifiers) -> Option<Key> {
    let Physical::Code(code) = k else {
        return None;
    };
    let plain = !m.control() && !m.alt() && !m.logo();
    Some(match code {
        Code::ArrowUp if plain => Key::Up,
        Code::ArrowDown if plain => Key::Down,
        Code::PageUp if plain => Key::PageUp,
        Code::PageDown if plain => Key::PageDown,
        Code::Home if plain => Key::Home,
        Code::End if plain => Key::End,
        Code::Enter | Code::NumpadEnter if plain => Key::Enter,
        Code::Backspace if plain => Key::Back,
        Code::Tab if plain && !m.shift() => Key::Switch,
        Code::Insert | Code::Space if plain && !m.shift() => Key::Mark,
        Code::KeyA if m.control() && !m.shift() && !m.alt() => Key::MarkAll,
        Code::KeyR if m.control() && !m.shift() && !m.alt() => Key::Refresh,
        Code::F5 if plain => Key::Copy,
        Code::F6 if plain && m.shift() => Key::Rename,
        Code::F6 if plain => Key::Move,
        Code::F7 if plain => Key::Mkdir,
        Code::F8 | Code::Delete if plain => Key::Delete,
        _ => return None,
    })
}

#[derive(Clone, Debug)]
pub enum SftpMsg {
    Event(u64, Event),
    Key(Key),
    Click(usize, usize, Modifiers),
    Open(usize, usize),
    /// The left button came up over a panel (`Some`) or anywhere in the tab (`None`).
    Release(Option<usize>),
    Scrolled(usize, f32, f32),
    Input(String),
    DialogOk,
    DialogCancel,
    ConflictAll(bool),
    Conflict(Option<Choice>),
    Cancel(u64),
    Retry(u64),
    ClearDone,
    Reconnect,
    /// The path over a panel clicked.
    Goto(usize),
}

enum Dialog {
    Copy {
        is_move: bool,
        dir: Dir,
        srcs: Vec<PathBuf>,
        dest: String,
    },
    Mkdir(String),
    Rename(OsString, String),
    /// A click on the path: go to a typed one.
    Goto(String),
    Delete(Vec<PathBuf>),
    Conflict {
        job: u64,
        batch: u64,
        name: String,
        all: bool,
    },
}

/// What ssh needs to open a connection (again for the transfer worker, and on Reconnect).
#[derive(Clone)]
pub struct Connect {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

pub struct SftpView {
    id: TabId,
    connect: Connect,
    events: Events,
    browse: Option<Worker>,
    transfer: Option<Worker>,
    pub connected: bool,
    panels: [Panel; 2],
    active: usize,
    queue: Queue,
    /// A move's source folders, removed once its batch is all copied.
    move_dirs: HashMap<u64, (Dir, Vec<PathBuf>)>,
    /// The running job and when it started (speed, time left).
    started: Option<(u64, Instant)>,
    /// The transfer connection failed: wait for a retry or a new copy (not a reconnect loop).
    paused: bool,
    status: Option<String>,
    dialog: Option<Dialog>,
    input_id: widget::Id,
    scroll_ids: [widget::Id; 2],
    /// Each panel's scroll offset and height.
    views: [(f32, f32); 2],
    /// A row is held: dropping it on the other panel copies.
    drag: Option<usize>,
}

impl SftpView {
    pub fn new(id: TabId, connect: Connect, events: Events) -> Self {
        let home = std::env::home_dir().unwrap_or_else(|| "/".into());
        let mut v = Self {
            id,
            connect,
            events,
            browse: None,
            transfer: None,
            connected: false,
            panels: Default::default(),
            active: REMOTE,
            queue: Queue::default(),
            move_dirs: HashMap::new(),
            started: None,
            paused: false,
            status: Some(fl!("sftp-connecting")),
            dialog: None,
            input_id: widget::Id::unique(),
            scroll_ids: [widget::Id::unique(), widget::Id::unique()],
            views: [(0.0, 0.0); 2],
            drag: None,
        };
        v.list_local(home, None);
        v.browse = Some(v.worker());
        v
    }

    fn worker(&self) -> Worker {
        let c = self.connect.clone();
        sftp_worker::start(self.id, c.program, c.args, c.env, self.events.clone())
    }

    pub fn has_dialog(&self) -> bool {
        self.dialog.is_some()
    }

    /// The session's settings as they are now (Reconnect uses them).
    pub fn set_connect(&mut self, c: Connect) {
        self.connect = c;
    }

    /// Reconnect (and Enter on a dead tab): a new browse connection, the transfer one on demand.
    /// A copy that was running stops (its worker goes): cancelled, for a Retry; the queue goes
    /// on once connected.
    pub fn reconnect(&mut self) {
        if let Some(id) = self.queue.running().map(|j| j.id) {
            self.queue.cancel(id);
        }
        if matches!(self.dialog, Some(Dialog::Conflict { .. })) {
            self.dialog = None;
        }
        self.started = None;
        self.transfer = None;
        self.connected = false;
        self.paused = false;
        self.status = Some(fl!("sftp-connecting"));
        self.browse = Some(self.worker());
    }

    fn list_local(&mut self, path: PathBuf, focus: Option<OsString>) {
        match read_local(&path) {
            Ok(entries) => {
                if path != self.panels[LOCAL].cwd() {
                    self.views[LOCAL].0 = 0.0;
                }
                self.panels[LOCAL].set_listing(path, entries, focus.as_deref());
            }
            Err(e) => self.status = Some(format!("{}: {e}", path.display())),
        }
    }

    /// Another folder: its list from the top.
    fn scroll_top(&mut self, side: usize) -> Task<Message> {
        self.views[side].0 = 0.0;
        self.reveal_in(side)
    }

    fn list(&mut self, side: usize, path: PathBuf, focus: Option<OsString>) {
        match side {
            LOCAL => self.list_local(path, focus),
            _ => {
                if let Some(w) = &self.browse {
                    w.send(Cmd::List { path, focus });
                }
            }
        }
    }

    fn relist(&mut self, side: usize) {
        let cwd = self.panels[side].cwd().to_path_buf();
        self.list(side, cwd, None);
    }

    /// The next job to the transfer worker (started on first use).
    fn pump(&mut self) {
        if self.paused || self.queue.running().is_some() {
            return;
        }
        let Some(job) = self.queue.start_next() else {
            return;
        };
        if self.transfer.is_none() {
            self.transfer = Some(self.worker());
        }
        let choice = self.queue.answered(job.batch);
        self.started = Some((job.id, Instant::now()));
        if let Some(w) = &self.transfer {
            w.send(Cmd::Run { job, choice });
        }
    }

    pub fn update(&mut self, msg: SftpMsg) -> Task<Message> {
        match msg {
            SftpMsg::Event(tag, e) => return self.event(tag, e),
            SftpMsg::Key(k) => return self.key(k),
            SftpMsg::Click(panel, row, mods) => {
                self.active = panel;
                self.drag = Some(panel);
                let p = &mut self.panels[panel];
                if mods.control() {
                    let on = !p.entries().get(row).is_some_and(|e| p.is_marked(e));
                    p.set_mark(row, on);
                } else if mods.shift() {
                    let (a, b) = (p.cursor().min(row), p.cursor().max(row));
                    for i in a..=b {
                        p.set_mark(i, true);
                    }
                }
                p.set_cursor(row);
            }
            SftpMsg::Open(panel, row) => {
                self.active = panel;
                self.panels[panel].set_cursor(row);
                return self.key(Key::Enter);
            }
            SftpMsg::Release(Some(panel)) => {
                if let Some(from) = self.drag.take()
                    && from != panel
                {
                    self.active = from;
                    return self.ask_copy(false);
                }
            }
            SftpMsg::Release(None) => self.drag = None,
            SftpMsg::Scrolled(panel, y, h) => self.views[panel] = (y, h),
            SftpMsg::Input(v) => match &mut self.dialog {
                Some(Dialog::Copy { dest, .. }) => *dest = v,
                Some(Dialog::Mkdir(s)) | Some(Dialog::Rename(_, s)) | Some(Dialog::Goto(s)) => {
                    *s = v
                }
                _ => {}
            },
            // The conflict dialog's primary is Overwrite, its Cancel stops the copy: never just
            // closed (its job would wait forever).
            SftpMsg::DialogOk if matches!(self.dialog, Some(Dialog::Conflict { .. })) => {
                self.conflict(Some(Choice::Overwrite));
            }
            SftpMsg::DialogCancel if matches!(self.dialog, Some(Dialog::Conflict { .. })) => {
                self.conflict(None);
            }
            SftpMsg::DialogOk => self.dialog_ok(),
            SftpMsg::DialogCancel => self.dialog = None,
            SftpMsg::ConflictAll(on) => {
                if let Some(Dialog::Conflict { all, .. }) = &mut self.dialog {
                    *all = on;
                }
            }
            SftpMsg::Conflict(choice) => self.conflict(choice),
            SftpMsg::Cancel(id) => {
                let asked = matches!(self.dialog, Some(Dialog::Conflict { job, .. }) if job == id);
                if self.queue.cancel(id) {
                    match asked {
                        // Waiting for the «already exists» answer, the worker runs nothing:
                        // nobody will finish it — the next one now.
                        true => {
                            self.dialog = None;
                            self.started = None;
                            self.pump();
                        }
                        false => {
                            if let Some(w) = &self.transfer {
                                w.cancel.store(true, Ordering::SeqCst);
                            }
                        }
                    }
                }
            }
            SftpMsg::Retry(id) => {
                self.queue.retry(id);
                self.paused = false;
                self.pump();
            }
            SftpMsg::ClearDone => self.queue.clear_done(),
            SftpMsg::Reconnect => self.reconnect(),
            SftpMsg::Goto(panel) => {
                self.active = panel;
                let at = self.panels[panel].cwd().to_string_lossy().into_owned();
                self.dialog = Some(Dialog::Goto(at));
                return widget::text_input::focus(self.input_id.clone());
            }
        }
        Task::none()
    }

    fn event(&mut self, tag: u64, e: Event) -> Task<Message> {
        let browse = self.browse.as_ref().is_some_and(|w| w.tag == tag);
        let transfer = self.transfer.as_ref().is_some_and(|w| w.tag == tag);
        if !browse && !transfer {
            return Task::none(); // a replaced worker's
        }
        match e {
            Event::Connected(home) if browse => {
                self.connected = true;
                self.status = None;
                let cwd = self.panels[REMOTE].cwd().to_path_buf();
                let at = if cwd.as_os_str().is_empty() {
                    home
                } else {
                    cwd
                };
                self.list(REMOTE, at, None);
                self.pump(); // what waited for the connection
            }
            Event::Connected(_) => {}
            Event::Listed {
                path,
                entries,
                focus,
            } => {
                let moved = path != self.panels[REMOTE].cwd();
                self.panels[REMOTE].set_listing(path, entries, focus.as_deref());
                if moved {
                    return self.scroll_top(REMOTE);
                }
            }
            Event::ListFailed(path, e) => {
                self.status = Some(format!("{}: {e}", path.display()));
            }
            Event::Changed(error) => {
                self.status = error;
                self.relist(REMOTE);
            }
            Event::Planned {
                batch,
                dir,
                files,
                dirs,
                is_move,
                error,
            } => {
                if error.is_some() {
                    self.status = error;
                }
                if is_move && !dirs.is_empty() {
                    self.move_dirs.insert(batch, (dir, dirs));
                }
                self.queue.add(batch, dir, files, is_move);
                // An empty folder has no file: it was made by the plan already.
                self.relist(target(dir));
                self.paused = false;
                self.pump();
            }
            Event::Progress(id, n) => self.queue.progress(id, n),
            Event::Conflict(id) => {
                let Some(job) = self.queue.jobs().iter().find(|j| j.id == id).cloned() else {
                    return Task::none();
                };
                let name = job.dst.to_string_lossy().into_owned();
                self.dialog = Some(Dialog::Conflict {
                    job: id,
                    batch: job.batch,
                    name,
                    all: false,
                });
            }
            Event::Finished(id, r) => {
                let job = self.queue.jobs().iter().find(|j| j.id == id).cloned();
                match r {
                    Ok(true) => self.queue.skip(id),
                    Ok(false) => self.queue.finish(id, Ok(())),
                    Err(e) => self.queue.finish(id, Err(e)),
                }
                self.started = None;
                if let Some(job) = job {
                    self.relist(target(job.dir));
                    if job.is_move {
                        self.relist(source(job.dir));
                    }
                    if self.queue.batch_done(job.batch)
                        && let Some((dir, dirs)) = self.move_dirs.remove(&job.batch)
                        && let Some(w) = &self.transfer
                    {
                        w.send(Cmd::RemoveDirs(dir, dirs));
                    }
                }
                self.pump();
            }
            Event::Disconnected(why) if browse => {
                self.connected = false;
                self.browse = None;
                self.status = Some(fl!("sftp-disconnected", why = why));
            }
            Event::Disconnected(why) => {
                self.transfer = None;
                self.paused = true;
                if let Some(id) = self.queue.running().map(|j| j.id) {
                    self.queue.finish(id, Err(why.clone()));
                }
                self.status = Some(fl!("sftp-disconnected", why = why));
            }
        }
        Task::none()
    }

    fn key(&mut self, k: Key) -> Task<Message> {
        if self.dialog.is_some() {
            return Task::none();
        }
        // Not connected: Enter connects again (as on an ended terminal tab).
        if k == Key::Enter && self.browse.is_none() {
            self.reconnect();
            return Task::none();
        }
        let page = match self.views[self.active].1 {
            h if h > 0.0 => ((h / ROW) as isize - 1).max(1),
            _ => 20, // not known yet
        };
        let p = &mut self.panels[self.active];
        match k {
            Key::Up => p.move_cursor(-1),
            Key::Down => p.move_cursor(1),
            Key::PageUp => p.move_cursor(-page),
            Key::PageDown => p.move_cursor(page),
            Key::Home => p.set_cursor(0),
            Key::End => p.set_cursor(usize::MAX),
            Key::Mark => p.toggle_mark(),
            Key::MarkAll => p.mark_all(),
            Key::Switch => self.active = 1 - self.active,
            Key::Refresh => self.relist(self.active),
            Key::Enter => {
                let Some(e) = p.current().cloned() else {
                    return Task::none();
                };
                match (e.is_parent(), e.dir) {
                    (true, _) => return self.key(Key::Back),
                    (false, true) => {
                        let path = p.cwd().join(&e.name);
                        self.list(self.active, path, None);
                    }
                    (false, false) => {}
                }
            }
            Key::Back => {
                let cwd = p.cwd().to_path_buf();
                if let Some(up) = cwd.parent() {
                    let focus = cwd.file_name().map(OsString::from);
                    self.list(self.active, up.to_path_buf(), focus);
                }
            }
            Key::Copy => return self.ask_copy(false),
            Key::Move => return self.ask_copy(true),
            Key::Mkdir => {
                self.dialog = Some(Dialog::Mkdir(String::new()));
                return widget::text_input::focus(self.input_id.clone());
            }
            Key::Rename => {
                if let Some(e) = p.current().filter(|e| !e.is_parent()).cloned() {
                    let name = e.name.to_string_lossy().into_owned();
                    self.dialog = Some(Dialog::Rename(e.name, name));
                    return widget::text_input::focus(self.input_id.clone());
                }
            }
            Key::Delete => {
                let paths: Vec<PathBuf> =
                    p.targets().iter().map(|e| p.cwd().join(&e.name)).collect();
                if !paths.is_empty() {
                    self.dialog = Some(Dialog::Delete(paths));
                }
            }
        }
        self.reveal()
    }

    /// The cursor's row in sight.
    fn reveal(&self) -> Task<Message> {
        self.reveal_in(self.active)
    }

    fn reveal_in(&self, side: usize) -> Task<Message> {
        let p = &self.panels[side];
        let (y, h) = self.views[side];
        // The height not known yet (not scrolled by hand): the same share of the list as the
        // cursor of its rows keeps the cursor in sight at any height.
        if h <= 0.0 {
            let n = p.entries().len().saturating_sub(1).max(1);
            return scroll::snap_to(
                self.scroll_ids[side].clone(),
                scroll::RelativeOffset {
                    x: None,
                    y: Some(p.cursor() as f32 / n as f32),
                },
            );
        }
        let top = p.cursor() as f32 * ROW;
        let to = match () {
            _ if top < y => top,
            _ if h > 0.0 && top + ROW > y + h => top + ROW - h,
            _ => return Task::none(),
        };
        scroll::scroll_to(
            self.scroll_ids[side].clone(),
            AbsoluteOffset {
                x: None,
                y: Some(to),
            },
        )
    }

    fn ask_copy(&mut self, is_move: bool) -> Task<Message> {
        let p = &self.panels[self.active];
        let srcs: Vec<PathBuf> = p.targets().iter().map(|e| p.cwd().join(&e.name)).collect();
        if srcs.is_empty() || !self.connected {
            return Task::none();
        }
        let dir = match self.active {
            LOCAL => Dir::Up,
            _ => Dir::Down,
        };
        let dest = self.panels[1 - self.active]
            .cwd()
            .to_string_lossy()
            .into_owned();
        self.dialog = Some(Dialog::Copy {
            is_move,
            dir,
            srcs,
            dest,
        });
        widget::text_input::focus(self.input_id.clone())
    }

    fn dialog_ok(&mut self) {
        let Some(d) = self.dialog.take() else {
            return;
        };
        let side = self.active;
        let cwd = self.panels[side].cwd().to_path_buf();
        match d {
            Dialog::Copy {
                is_move,
                dir,
                srcs,
                dest,
            } => {
                let batch = self.queue.batch();
                self.panels[source(dir)].clear_marks();
                if self.transfer.is_none() {
                    self.transfer = Some(self.worker());
                }
                if let Some(w) = &self.transfer {
                    w.send(Cmd::Plan {
                        batch,
                        dir,
                        srcs,
                        dst_dir: PathBuf::from(dest.trim()),
                        is_move,
                    });
                }
            }
            Dialog::Mkdir(name) if !name.trim().is_empty() => {
                let path = cwd.join(name.trim());
                match side {
                    LOCAL => {
                        self.status = std::fs::create_dir(&path).err().map(|e| e.to_string());
                        let focus = path.file_name().map(OsString::from);
                        self.list_local(cwd, focus);
                    }
                    _ => self.send_browse(Cmd::Mkdir(path)),
                }
            }
            Dialog::Rename(from, to) if !to.trim().is_empty() => {
                let (a, b) = (cwd.join(&from), cwd.join(to.trim()));
                match side {
                    LOCAL => {
                        self.status = std::fs::rename(&a, &b).err().map(|e| e.to_string());
                        self.list_local(cwd, b.file_name().map(OsString::from));
                    }
                    _ => self.send_browse(Cmd::Rename(a, b)),
                }
            }
            Dialog::Delete(paths) => {
                self.panels[side].clear_marks();
                match side {
                    LOCAL => {
                        for p in &paths {
                            let r = match std::fs::symlink_metadata(p) {
                                Ok(m) if m.is_dir() => std::fs::remove_dir_all(p),
                                _ => std::fs::remove_file(p),
                            };
                            if let Err(e) = r {
                                self.status = Some(format!("{}: {e}", p.display()));
                            }
                        }
                        self.list_local(cwd, None);
                    }
                    _ => self.send_browse(Cmd::Delete(paths)),
                }
            }
            Dialog::Goto(path) if !path.trim().is_empty() => {
                self.list(side, PathBuf::from(path.trim()), None);
            }
            Dialog::Conflict { .. } | Dialog::Mkdir(_) | Dialog::Rename(..) | Dialog::Goto(_) => {}
        }
    }

    fn send_browse(&mut self, c: Cmd) {
        match &self.browse {
            Some(w) => w.send(c),
            None => self.status = Some(fl!("sftp-not-connected")),
        }
    }

    /// The conflict dialog's answer (`None`: Cancel — the rest of that copy too).
    fn conflict(&mut self, choice: Option<Choice>) {
        let Some(Dialog::Conflict {
            job, batch, all, ..
        }) = self.dialog.take()
        else {
            return;
        };
        let Some(choice) = choice else {
            self.queue.cancel_batch(batch);
            self.started = None;
            self.pump();
            return;
        };
        if all {
            self.queue.answer(batch, choice);
        }
        let Some(j) = self.queue.jobs().iter().find(|j| j.id == job).cloned() else {
            return;
        };
        if let Some(w) = &self.transfer {
            w.send(Cmd::Run {
                job: j,
                choice: Some(choice),
            });
        }
    }

    pub fn dialog(&self) -> Option<Element<'_, Message>> {
        let d = self.dialog.as_ref()?;
        let m = move |x| Message::Sftp(self.id, x);
        let input = |value: &str| -> Element<'_, Message> {
            widget::text_input("", value.to_string())
                .id(self.input_id.clone())
                .on_input(move |v| m(SftpMsg::Input(v)))
                .on_submit(move |_| m(SftpMsg::DialogOk))
                .into()
        };
        fn ok_cancel<'a>(
            m: impl Fn(SftpMsg) -> Message,
            title: String,
            body: Element<'a, Message>,
            ok: String,
        ) -> Element<'a, Message> {
            widget::dialog()
                .title(title)
                .control(body)
                .primary_action(widget::button::suggested(ok).on_press(m(SftpMsg::DialogOk)))
                .secondary_action(
                    widget::button::standard(fl!("cancel")).on_press(m(SftpMsg::DialogCancel)),
                )
                .into()
        }
        Some(match d {
            Dialog::Copy {
                is_move,
                srcs,
                dest,
                ..
            } => {
                let what = match srcs.as_slice() {
                    [one] => one
                        .file_name()
                        .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
                    many => fl!("sftp-n-items", count = many.len()),
                };
                let (title, ok) = match is_move {
                    true => (fl!("sftp-move-title", what = what), fl!("sftp-move")),
                    false => (fl!("sftp-copy-title", what = what), fl!("sftp-copy")),
                };
                let body = widget::column::with_children(vec![
                    widget::text::caption(fl!("sftp-to")).into(),
                    input(dest),
                ])
                .spacing(4);
                ok_cancel(m, title, body.into(), ok)
            }
            Dialog::Mkdir(name) => {
                ok_cancel(m, fl!("sftp-mkdir-title"), input(name), fl!("sftp-create"))
            }
            Dialog::Rename(_, name) => {
                ok_cancel(m, fl!("sftp-rename-title"), input(name), fl!("sm-rename"))
            }
            Dialog::Goto(path) => ok_cancel(m, fl!("sftp-goto-title"), input(path), fl!("sftp-go")),
            Dialog::Delete(paths) => {
                let what = match paths.as_slice() {
                    [one] => one.to_string_lossy().into_owned(),
                    many => fl!("sftp-n-items", count = many.len()),
                };
                widget::dialog()
                    .title(fl!("sftp-delete-title", what = what))
                    .body(fl!("sftp-delete-body"))
                    .primary_action(
                        widget::button::destructive(fl!("sm-delete"))
                            .on_press(m(SftpMsg::DialogOk)),
                    )
                    .secondary_action(
                        widget::button::standard(fl!("cancel")).on_press(m(SftpMsg::DialogCancel)),
                    )
                    .into()
            }
            Dialog::Conflict { name, all, .. } => widget::dialog()
                .title(fl!("sftp-exists-title"))
                .control(
                    widget::column::with_children(vec![
                        widget::text::body(name.clone()).into(),
                        widget::checkbox(*all)
                            .label(fl!("sftp-for-all"))
                            .on_toggle(move |v| m(SftpMsg::ConflictAll(v)))
                            .into(),
                    ])
                    .spacing(10),
                )
                .primary_action(
                    widget::button::suggested(fl!("sftp-overwrite"))
                        .on_press(m(SftpMsg::Conflict(Some(Choice::Overwrite)))),
                )
                .secondary_action(
                    widget::button::standard(fl!("sftp-skip"))
                        .on_press(m(SftpMsg::Conflict(Some(Choice::Skip)))),
                )
                .tertiary_action(
                    widget::button::text(fl!("cancel")).on_press(m(SftpMsg::Conflict(None))),
                )
                .into(),
        })
    }

    pub fn view(&self) -> Element<'_, Message> {
        let m = move |x| Message::Sftp(self.id, x);
        let panels = widget::row::with_children(vec![self.panel(LOCAL), self.panel(REMOTE)])
            .spacing(4)
            .height(Length::Fill);
        let mut col = widget::column::with_capacity(3).push(panels);
        if !self.queue.jobs().is_empty() {
            col = col.push(self.queue_view());
        }
        let status: Element<'_, Message> = match (&self.status, self.connected) {
            (Some(s), false) if self.browse.is_none() => widget::row::with_children(vec![
                widget::text::body(s.clone())
                    .class(theme::Text::Accent)
                    .into(),
                widget::Space::new().width(Length::Fill).into(),
                widget::button::standard(fl!("menu-reconnect"))
                    .on_press(m(SftpMsg::Reconnect))
                    .into(),
            ])
            .align_y(Alignment::Center)
            .into(),
            (Some(s), _) => widget::text::caption(s.clone()).into(),
            (None, _) => widget::text::caption(fl!("sftp-keys")).into(),
        };
        col = col.push(widget::container(status).padding([2, 4]));
        widget::mouse_area(col.spacing(4).padding(4))
            .on_release(m(SftpMsg::Release(None)))
            .into()
    }

    fn panel(&self, side: usize) -> Element<'_, Message> {
        let m = move |x| Message::Sftp(self.id, x);
        let p = &self.panels[side];
        let active = self.active == side;
        let title = match side {
            LOCAL => p.cwd().to_string_lossy().into_owned(),
            _ => format!("{}:{}", self.connect_host(), p.cwd().to_string_lossy()),
        };
        let title = widget::text::body(title);
        let title = match active {
            true => title.class(theme::Text::Accent),
            false => title,
        };
        let header =
            widget::mouse_area(widget::container(title).padding([4, 8]).width(Length::Fill))
                .on_release(m(SftpMsg::Goto(side)));
        let rows: Vec<Element<'_, Message>> = (p.entries().iter().enumerate())
            .map(|(i, e)| {
                row(
                    e,
                    i == p.cursor() && active,
                    i == p.cursor(),
                    p.is_marked(e),
                    side,
                    i,
                    self.id,
                )
            })
            .collect();
        let list = widget::scrollable(widget::column::with_children(rows))
            .id(self.scroll_ids[side].clone())
            .on_scroll(move |v| {
                m(SftpMsg::Scrolled(
                    side,
                    v.absolute_offset().y,
                    v.bounds().height,
                ))
            })
            .height(Length::Fill)
            .width(Length::Fill);
        let body = widget::column::with_children(vec![header.into(), list.into()]).spacing(2);
        widget::mouse_area(
            widget::container(body)
                .width(Length::FillPortion(1))
                .height(Length::Fill)
                .class(theme::Container::Card),
        )
        .on_release(m(SftpMsg::Release(Some(side))))
        .into()
    }

    fn connect_host(&self) -> String {
        // `… -- host sftp`: the host is the next to last argument.
        let a = &self.connect.args;
        a.get(a.len().saturating_sub(2))
            .cloned()
            .unwrap_or_default()
    }

    fn queue_view(&self) -> Element<'_, Message> {
        let m = move |x| Message::Sftp(self.id, x);
        let rows: Vec<Element<'_, Message>> = self
            .queue
            .jobs()
            .iter()
            .rev()
            .map(|j| {
                let name = j
                    .src
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let to = j
                    .dst
                    .parent()
                    .map(|d| d.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let arrow = match j.dir {
                    Dir::Up => "↑",
                    Dir::Down => "↓",
                };
                let mut r = widget::row::with_capacity(5)
                    .spacing(8)
                    .align_y(Alignment::Center)
                    .push(
                        widget::text::body(format!("{arrow} {name}")).width(Length::FillPortion(3)),
                    )
                    .push(widget::text::caption(format!("→ {to}")).width(Length::FillPortion(3)));
                let state: Element<'_, Message> = match &j.state {
                    State::Running => {
                        let frac = match j.size {
                            0 => 0.0,
                            s => j.done as f32 / s as f32,
                        };
                        let speed = self
                            .started
                            .filter(|(id, _)| *id == j.id)
                            .map(|(_, t)| j.done as f64 / t.elapsed().as_secs_f64().max(0.001))
                            .unwrap_or(0.0);
                        let left = match speed > 0.0 {
                            true => ((j.size.saturating_sub(j.done)) as f64 / speed) as u64,
                            false => 0,
                        };
                        widget::row::with_children(vec![
                            widget::progress_bar::determinate_linear(frac)
                                .width(Length::Fixed(120.0))
                                .into(),
                            widget::text::caption(format!(
                                "{}%  {}/s  {}:{:02}",
                                (frac * 100.0) as u32,
                                size_text(speed as u64),
                                left / 60,
                                left % 60
                            ))
                            .into(),
                        ])
                        .spacing(8)
                        .align_y(Alignment::Center)
                        .into()
                    }
                    State::Waiting => widget::text::caption(fl!("sftp-waiting")).into(),
                    State::Done => widget::text::caption(fl!("sftp-done")).into(),
                    State::Skipped => widget::text::caption(fl!("sftp-skipped")).into(),
                    State::Cancelled => widget::text::caption(fl!("sftp-cancelled")).into(),
                    State::Failed(e) => widget::text::caption(e.clone())
                        .class(theme::Text::Accent)
                        .into(),
                };
                r = r.push(widget::container(state).width(Length::FillPortion(4)));
                let action: Element<'_, Message> = match &j.state {
                    State::Waiting | State::Running => widget::button::text(fl!("cancel"))
                        .on_press(m(SftpMsg::Cancel(j.id)))
                        .into(),
                    State::Failed(_) | State::Cancelled => widget::button::text(fl!("sftp-retry"))
                        .on_press(m(SftpMsg::Retry(j.id)))
                        .into(),
                    _ => widget::Space::new().width(Length::Fixed(1.0)).into(),
                };
                r.push(action).into()
            })
            .collect();
        let head = widget::row::with_children(vec![
            widget::text::heading(fl!("sftp-queue")).into(),
            widget::Space::new().width(Length::Fill).into(),
            widget::button::text(fl!("sftp-clear-done"))
                .on_press(m(SftpMsg::ClearDone))
                .into(),
        ])
        .align_y(Alignment::Center);
        widget::container(
            widget::column::with_children(vec![
                head.into(),
                widget::scrollable(widget::column::with_children(rows).spacing(2))
                    .height(Length::Fixed(140.0))
                    .into(),
            ])
            .spacing(4),
        )
        .padding(6)
        .class(theme::Container::Card)
        .into()
    }
}

/// One file row: name, size, date; the cursor (`focused`: of the active panel) and the marks.
fn row(
    e: &Entry,
    focused: bool,
    cursor: bool,
    marked: bool,
    side: usize,
    i: usize,
    id: TabId,
) -> Element<'static, Message> {
    let m = move |x| Message::Sftp(id, x);
    // A newline or another control character in a name would break the row: shown as `?`.
    let name: String = (e.name.to_string_lossy().chars())
        .map(|c| if c.is_control() { '?' } else { c })
        .collect();
    let name = match (e.dir, e.is_parent()) {
        (_, true) => name,
        (true, _) => format!("[{name}]"),
        (false, _) => name,
    };
    let size = match e.dir {
        true => String::new(),
        false => size_text(e.size),
    };
    let date = e.mtime.map(time_text).unwrap_or_default();
    let text = |s: String| {
        let t = widget::text::body(s);
        match marked {
            true => t.class(theme::Text::Accent),
            false => t,
        }
    };
    let content = widget::row::with_children(vec![
        text(name).width(Length::Fill).into(),
        text(size)
            .width(Length::Fixed(90.0))
            .align_x(Alignment::End)
            .into(),
        text(date)
            .width(Length::Fixed(130.0))
            .align_x(Alignment::End)
            .into(),
    ])
    .spacing(8)
    .padding([0, 8])
    .height(Length::Fixed(ROW))
    .align_y(Alignment::Center);
    // The active panel's cursor: the accent; the other panel's: an outline.
    let cell = widget::container(content)
        .width(Length::Fill)
        .class(theme::Container::custom(move |t| {
            let c = t.cosmic();
            let mut s = widget::container::Style::default();
            if focused {
                s.background = Some(cosmic::iced::Color::from(c.accent_color()).into());
                s.text_color = Some(c.on_accent_color().into());
            } else if cursor {
                s.border.color = c.accent_color().into();
                s.border.width = 1.0;
            }
            s
        }));
    widget::mouse_area(cell)
        .on_press(m(SftpMsg::Click(side, i, Modifiers::empty())))
        .on_double_click(m(SftpMsg::Open(side, i)))
        .into()
}

fn read_local(path: &std::path::Path) -> std::io::Result<Vec<Entry>> {
    let mut out = Vec::new();
    for d in std::fs::read_dir(path)? {
        let Ok(d) = d else { continue };
        // Through a link: a link to a folder opens like one.
        let meta = std::fs::metadata(d.path()).or_else(|_| d.metadata());
        let Ok(meta) = meta else { continue };
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64);
        out.push(Entry {
            name: d.file_name(),
            dir: meta.is_dir(),
            size: meta.len(),
            mtime,
        });
    }
    Ok(out)
}

fn target(dir: Dir) -> usize {
    match dir {
        Dir::Up => REMOTE,
        Dir::Down => LOCAL,
    }
}

fn source(dir: Dir) -> usize {
    1 - target(dir)
}

/// The SFTP workers' events, as messages (one channel for every tab, as the terminals').
pub fn subscription() -> Subscription<Message> {
    struct Channel;
    Subscription::run_with(std::any::TypeId::of::<Channel>(), |_| {
        cosmic::iced::stream::channel(100, async |mut out| {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            let _ = out.send(Message::SftpTx(tx)).await;
            while let Some((id, tag, e)) = rx.recv().await {
                if out
                    .send(Message::Sftp(id, SftpMsg::Event(tag, e)))
                    .await
                    .is_err()
                {
                    break;
                }
            }
            std::future::pending::<()>().await;
        })
    })
}
