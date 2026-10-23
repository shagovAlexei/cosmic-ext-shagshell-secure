# 4a — askpass, keyring, host key: Implementation Plan

**Goal:** ssh's password / passphrase / host-key / other prompts answered by the app (dialog or keyring).
**Spec:** `.claude/docs/specs/04-auth.md`. Branch `feat/auth` (on `feat/session-options`), version 0.5.0.

## Global Constraints

- Secrets never in files, logs, `Debug` output (`Reply` / dialog state implement `Debug` by hand, redacted).
- keyring attributes exactly: `app=io.github.shagovalexei.ShagShellSecure`, `kind=session`, `id=<session id>`, `secret=password|passphrase`.
- env names exactly: `SSH_ASKPASS`, `SSH_ASKPASS_REQUIRE=force`, `SHAGSHELL_ASKPASS_SOCK`, `SHAGSHELL_SESSION`, `SHAGSHELL_TAB`.

## Task 1: `core::askpass` (TDD)

- `enum Kind { Password, Passphrase, HostKey, Other }`, `fn classify(prompt: &str) -> Kind`.
  Tests with the real prompts from the spec table: `home@h's password: `, `(u@h) Password: `, `Enter passphrase for key '/k': `, the 4-line host-key prompt, `Verification code: `.
- `struct Request { prompt, session: Option<String>, tab: Option<u64>, ppid: u32 }`, `enum Reply { Answer(String), Cancel }` — one JSON line each (`serde_json`); `Debug` of `Reply` hides the answer.
- `fn socket_path(control_dir: &Path, pid: u32) -> PathBuf` (`askpass-<pid>.sock`).
- `fn ask(sock: &Path, req: &Request) -> io::Result<Reply>` — the helper's side (blocking); `fn serve_one(stream) -> io::Result<(Request, impl FnOnce(Reply))>`-like split is the app's: `read_request(&mut BufRead) -> io::Result<Request>`, `write_reply(&mut Write, &Reply)`.
- `struct Tries` — `fn first(&mut self, ppid: u32, kind: Kind) -> bool` (true once per ssh process and kind; the second is "wrong password").
- Tests: `classify_real_prompts`, `request_round_trip` (prompt with newlines), `reply_debug_hides_secret`, `ask_over_a_socket` (UnixListener in a thread), `tries_second_is_retry`.

## Task 2: `core::keyring` (from shagoff-commander, own schema)

- `enum Secret { Password, Passphrase }`; `fn attrs(session: &str, s: Secret) -> Vec<(&str, String)>` (pure, tested).
- `lookup(session, Secret) -> zbus::Result<Option<String>>`, `store(session, Secret, value)`, `delete(session, Secret)`, `retarget(old, new)` (every item whose `id` `store::retarget`s: read, store under the new id, delete the old). Blocking — the app calls them in `spawn_blocking`.
- Test: `attrs_schema` only (D-Bus is checked by hand / headless).

## Task 3: launch env + helper mode

- `launch::askpass_env(exe: &Path, sock: &Path, session: Option<&str>) -> Vec<(String, String)>` (tested); `SHAGSHELL_TAB` added by `Tabs::spawn` (the id is known there).
- `term::start(.., env)`; `Launch.env`.
- `main.rs`: `SHAGSHELL_ASKPASS_SOCK` set and exactly one argument → `askpass::ask`, print, exit 0 / 1.

## Task 4: app

- `askpass.rs` (app): the listener thread (std `UnixListener`) → subscription channel `Message::Askpass(Prompt)`; `Prompt { id, req, kind, retry, reply: Arc<Mutex<Option<Sender>>> }`; queue + dialog (password field secure, «Сохранить пароль» off by default, host key: [Принять и сохранить] / [Отмена], other: visible field).
- Routing: password / passphrase + session + `Tries::first` → keyring lookup task → answer, else dialog. OK with save → store task. Tab closed / ended → its prompts cancelled. Dialog raises its tab.
- Session renamed / moved → `keyring::retarget`; deleted → `delete` (SM `Outcome.deleted`).
- Session Options: «Пароль сохранён [Забыть]» (lookup on open, delete on click).
- `--check`: keyring line.

## Review Focus

- Second password prompt from the same ssh after a keyring answer → dialog «не подошёл», not the same password again (else 3 failures and a lockout on the server).
- Two tabs of one session connecting at once → two dialogs in turn, each answer to its own ssh.
- Tab closed while its dialog is up → the dialog goes, the helper gets Cancel (no hanging ssh).
- Keyring missing / locked and dismissed → dialog still works, Save disabled or its failure shown, never a crash.
- A second app instance → its own socket; the first one's tabs unaffected.

---

# 4b — Credentials Manager, master password: Implementation Plan

**Spec:** `.claude/docs/specs/04-auth.md` § 4b. Branch `feat/credentials` (on `feat/auth`), version 0.6.0.

## Task 1: core (TDD)

- `session::Session.credential: Option<String>`; `Form.credential` ("" = none).
- `credentials.rs`: `Credentials(BTreeMap<String, Set { user }>)` (unknown fields kept), `load(path) / save(path)` (atomic), `default_path()`; `resolve(&Session, &Credentials) -> Session` (user from the set; a missing set: unchanged).
- `Store::using(name) -> Vec<String>` (sessions referencing a set), `Store::relink(old, new: Option<&str>, user)` — renamed: point to `new`; deleted (`None`): drop the reference, copy `user`.
- Tests: `round_trip_keeps_unknown`, `resolve_user_from_set`, `resolve_missing_set`, `relink_rename`, `relink_delete_copies_user`.

## Task 2: core keyring

- `enum Owner { Session(String), Credential(String) }` → `attrs(&Owner, Secret)` (`kind=session|credential`); `lookup / store / delete / exists` take `&Owner`; `rename(&Owner, &Owner)`.
- Master password: `separate() -> bool` (a collection labelled «ShagShell Secure»), `set_separate(bool)` (create — gnome-keyring asks the password — and move our items in; or move them back and delete it). `store` writes into it when there.
- Live test (`--ignored`): credential round trip + rename. `set_separate` checked by hand (its prompt is on the user's desktop).

## Task 3: app

- `launch::askpass_env(.., credential)` → `SHAGSHELL_CREDENTIAL`; `Request.credential`; password prompts of a session with a set use `Owner::Credential` (lookup and «Save password to «set»»).
- `credentials_window.rs`: a window like Session Options: list + New, fields name / user / password, Save / Delete (confirm with the count), Close.
- Session Options: «Учётные данные» dropdown; user field disabled with the set's user.
- Global Options: «separate keyring» checkbox (state read when the drawer opens, applied off the UI thread, failure shown), «Credentials Manager…» button; Tools → Credentials Manager….
