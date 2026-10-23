//! ssh's prompts in the main window (SecureCRT's password / host key dialogs): the socket the
//! askpass helpers connect to, the queue of prompts, the dialog. What a prompt is and the protocol:
//! `shagshell_core::askpass`; the keyring: `shagshell_core::keyring`.

use crate::app::Message;
use crate::fl;
use cosmic::Element;
use cosmic::iced::{Subscription, futures::SinkExt};
use cosmic::widget;
use shagshell_core::askpass::{self, Kind, Reply, Request};
use shagshell_core::keyring::Secret;
use std::collections::VecDeque;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};

/// Text that may be a secret: never in `Debug` output (messages may be logged).
#[derive(Clone)]
pub struct Hidden(pub String);

impl std::fmt::Debug for Hidden {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("…")
    }
}

/// Where a helper waits for its answer. Dropped without one: the helper gets Cancel.
#[derive(Clone)]
pub struct Responder(Arc<Mutex<Option<mpsc::Sender<Reply>>>>);

impl std::fmt::Debug for Responder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Responder")
    }
}

impl Responder {
    fn send(&self, r: Reply) {
        if let Some(tx) = self.0.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = tx.send(r);
        }
    }
}

/// A prompt from a helper.
#[derive(Clone, Debug)]
pub struct Incoming {
    pub req: Request,
    pub reply: Responder,
}

#[derive(Clone, Debug)]
pub enum AskMsg {
    Incoming(Incoming),
    /// The keyring looked up for a prompt: the secret, none, or why it failed.
    Saved(Incoming, Result<Option<Hidden>, String>),
    Input(Hidden),
    Save(bool),
    Ok,
    Cancel,
    /// A save to the keyring failed.
    Stored(Result<(), String>),
}

struct Pending {
    inc: Incoming,
    kind: Kind,
    /// The saved / typed password was refused: ssh asks again.
    retry: bool,
    value: String,
    save: bool,
    /// Can't be saved: no saved session, not a secret, or the keyring failed (why).
    no_save: Option<String>,
}

#[derive(Default)]
pub struct Asks {
    /// This instance's socket: what our ssh processes have in their environment.
    sock: PathBuf,
    queue: VecDeque<Pending>,
    tries: askpass::Tries,
    input_id: Option<widget::Id>,
}

/// What the app does after an update.
#[derive(Default)]
pub struct Outcome {
    /// Look the secret up off the UI thread, then `AskMsg::Saved`.
    pub lookup: Option<(Incoming, String, Secret)>,
    /// Save it off the UI thread (session, kind, secret), then `AskMsg::Stored`.
    pub store: Option<(String, Secret, Hidden)>,
    /// Remove a saved one that turned out wrong (the user unticked «Save» on the retry).
    pub forget: Option<(String, Secret)>,
    /// A dialog came to the front: show its tab, focus its field.
    pub shown: Option<(Option<u64>, widget::Id)>,
    /// The last dialog went: the keys back to the active terminal.
    pub refocus: bool,
}

pub fn secret(kind: Kind) -> Option<Secret> {
    match kind {
        Kind::Password => Some(Secret::Password),
        Kind::Passphrase => Some(Secret::Passphrase),
        Kind::HostKey | Kind::Other => None,
    }
}

impl Asks {
    pub fn new(sock: PathBuf) -> Self {
        Self {
            sock,
            ..Default::default()
        }
    }

    pub fn update(&mut self, msg: AskMsg) -> Outcome {
        match msg {
            AskMsg::Incoming(inc) if self.tries.cancelled(inc.req.ppid) => {
                inc.reply.send(Reply::Cancel);
            }
            AskMsg::Incoming(mut inc) => {
                // A ProxyJump / -J ssh (ssh's child, another host) inherits the session's env: it
                // must never get, or save over, this session's password.
                if inc.req.session.is_some() && !askpass::direct(inc.req.ppid) {
                    inc.req.session = None;
                }
                let kind = askpass::classify(&inc.req.prompt);
                let first = self.tries.first(inc.req.ppid, kind);
                if let (Some(s), Some(session)) = (secret(kind), inc.req.session.clone())
                    && first
                {
                    return Outcome {
                        lookup: Some((inc, session, s)),
                        ..Default::default()
                    };
                }
                // The same ssh asking again: what it was given was wrong.
                let retry = !first && secret(kind).is_some();
                return self.push(inc, kind, retry, None);
            }
            AskMsg::Saved(inc, Ok(Some(Hidden(p)))) => inc.reply.send(Reply::Answer(p)),
            AskMsg::Saved(inc, found) => {
                let kind = askpass::classify(&inc.req.prompt);
                return self.push(inc, kind, false, found.err());
            }
            AskMsg::Input(Hidden(v)) => {
                if let Some(p) = self.queue.front_mut() {
                    p.value = v;
                }
            }
            AskMsg::Save(on) => {
                if let Some(p) = self.queue.front_mut() {
                    p.save = on;
                }
            }
            AskMsg::Ok => {
                let Some(p) = self.queue.pop_front() else {
                    return Outcome::default();
                };
                let answer = match p.kind {
                    Kind::HostKey => "yes".to_string(),
                    _ => p.value,
                };
                let target = (p.no_save.is_none())
                    .then(|| Some((p.inc.req.session.clone()?, secret(p.kind)?)))
                    .flatten();
                let ppid = p.inc.req.ppid;
                let was_saved = self.tries.was_saved(ppid, p.kind);
                if p.save {
                    self.tries.saved(ppid, p.kind);
                }
                p.inc.reply.send(Reply::Answer(answer.clone()));
                let (store, forget) = match target {
                    Some((session, s)) if p.save => (Some((session, s, Hidden(answer))), None),
                    // Saved a moment ago, refused by the server, not wanted any more.
                    Some(t) if was_saved => (None, Some(t)),
                    _ => (None, None),
                };
                return Outcome {
                    store,
                    forget,
                    ..self.front()
                };
            }
            AskMsg::Cancel => {
                if let Some(p) = self.queue.pop_front() {
                    self.tries.cancel(p.inc.req.ppid);
                    askpass::abort(p.inc.req.ppid, &self.sock);
                    p.inc.reply.send(Reply::Cancel);
                    // Its other prompts waiting (Connect all, one ssh): cancelled too.
                    let ppid = p.inc.req.ppid;
                    self.queue.retain(|q| {
                        let keep = q.inc.req.ppid != ppid;
                        if !keep {
                            q.inc.reply.send(Reply::Cancel);
                        }
                        keep
                    });
                }
                return self.front();
            }
            AskMsg::Stored(_) => {} // the app says what failed
        }
        Outcome::default()
    }

    fn push(&mut self, inc: Incoming, kind: Kind, retry: bool, failed: Option<String>) -> Outcome {
        let no_save = match (secret(kind), &inc.req.session) {
            (Some(_), Some(_)) => failed.map(|e| fl!("ask-keyring-failed", error = e)),
            _ => Some(String::new()),
        };
        // A retry after a saved answer: still meant to be saved (the correct one replaces it).
        let save = retry && self.tries.was_saved(inc.req.ppid, kind);
        self.queue.push_back(Pending {
            inc,
            kind,
            retry,
            value: String::new(),
            save,
            no_save,
        });
        match self.queue.len() {
            1 => self.front(),
            _ => Outcome::default(),
        }
    }

    fn front(&mut self) -> Outcome {
        match self.queue.front() {
            Some(p) => {
                let id = widget::Id::unique();
                self.input_id = Some(id.clone());
                Outcome {
                    shown: Some((p.inc.req.tab, id)),
                    ..Default::default()
                }
            }
            None => Outcome {
                refocus: true,
                ..Default::default()
            },
        }
    }

    /// A dialog is up (Enter / Esc are its).
    pub fn active(&self) -> bool {
        !self.queue.is_empty()
    }

    /// Prompts whose tab is gone or whose program ended: cancelled (the helper must not hang).
    pub fn prune(&mut self, alive: impl Fn(u64) -> bool) -> Outcome {
        self.tries.forget_dead(askpass::alive);
        let front = self.queue.front().map(|p| p.inc.req.ppid);
        self.queue.retain(|p| {
            let keep = p.inc.req.tab.is_none_or(&alive);
            if !keep {
                p.inc.reply.send(Reply::Cancel);
            }
            keep
        });
        match self.queue.front().map(|p| p.inc.req.ppid) == front {
            true => Outcome::default(),
            false => self.front(),
        }
    }

    /// `title`: the tab the prompt is about.
    pub fn dialog(&self, title: impl Fn(u64) -> Option<String>) -> Option<Element<'_, Message>> {
        let p = self.queue.front()?;
        let m = Message::Ask;
        let tab = p.inc.req.tab.and_then(&title);
        let heading = match p.kind {
            Kind::HostKey => fl!("ask-host-key-title"),
            Kind::Passphrase => fl!("ask-passphrase-title"),
            Kind::Password => fl!("ask-password-title"),
            Kind::Other => fl!("ask-other-title"),
        };
        let heading = match tab {
            Some(t) => format!("{heading} — {t}"),
            None => heading,
        };
        let mut body = widget::column::with_capacity(4).spacing(10);
        if p.retry {
            body =
                body.push(widget::text::body(fl!("ask-wrong")).class(cosmic::theme::Text::Accent));
        }
        body = body.push(widget::text::body(p.inc.req.prompt.trim_end().to_string()));
        if p.kind != Kind::HostKey {
            let mut input = widget::text_input("", p.value.clone())
                .on_input(|v| Message::Ask(AskMsg::Input(Hidden(v))))
                .on_submit(|_| Message::Ask(AskMsg::Ok));
            if let Some(id) = &self.input_id {
                input = input.id(id.clone());
            }
            if secret(p.kind).is_some() {
                input = input.password();
            }
            body = body.push(input);
        }
        if secret(p.kind).is_some() {
            let mut save = widget::checkbox(p.save).label(match p.kind {
                Kind::Passphrase => fl!("ask-save-passphrase"),
                _ => fl!("ask-save-password"),
            });
            if p.no_save.is_none() {
                save = save.on_toggle(|v| Message::Ask(AskMsg::Save(v)));
            }
            body = body.push(save);
            match &p.no_save {
                Some(why) if !why.is_empty() => {
                    body = body.push(widget::text::caption(why.clone()))
                }
                Some(_) => body = body.push(widget::text::caption(fl!("ask-no-session"))),
                None => {}
            }
        }
        let ok = match p.kind {
            Kind::HostKey => fl!("ask-accept"),
            _ => fl!("ok"),
        };
        Some(
            widget::dialog()
                .title(heading)
                .control(body)
                .primary_action(widget::button::suggested(ok).on_press(m(AskMsg::Ok)))
                .secondary_action(
                    widget::button::standard(fl!("cancel")).on_press(m(AskMsg::Cancel)),
                )
                .into(),
        )
    }
}

/// The socket: one thread accepting, one per helper waiting for its answer.
pub fn subscription(sock: PathBuf) -> Subscription<Message> {
    Subscription::run_with(sock, |sock| {
        let sock = sock.clone();
        cosmic::iced::stream::channel(16, async move |mut out| {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            if let Some(dir) = sock.parent() {
                askpass::clean_stale(dir); // left by runs closed by × or crashed
            }
            let _ = std::fs::remove_file(&sock); // ours by name: a dead run's with our pid
            match std::os::unix::net::UnixListener::bind(&sock) {
                Ok(l) => drop(std::thread::spawn(move || listen(l, tx))),
                Err(e) => log::error!("askpass socket {}: {e}", sock.display()),
            }
            while let Some(inc) = rx.recv().await {
                if out.send(Message::Ask(AskMsg::Incoming(inc))).await.is_err() {
                    break;
                }
            }
            std::future::pending::<()>().await;
        })
    })
}

fn listen(l: std::os::unix::net::UnixListener, tx: tokio::sync::mpsc::UnboundedSender<Incoming>) {
    for stream in l.incoming().flatten() {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let Ok(req) = askpass::read_request(&mut BufReader::new(&stream)) else {
                return;
            };
            let (reply_tx, reply_rx) = mpsc::channel();
            let reply = Responder(Arc::new(Mutex::new(Some(reply_tx))));
            if tx.send(Incoming { req, reply }).is_err() {
                return;
            }
            // Every Responder clone dropped unanswered: Cancel.
            let r = reply_rx.recv().unwrap_or(Reply::Cancel);
            let _ = askpass::write_reply(&mut &stream, &r);
        });
    }
}
