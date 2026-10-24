# 03a. Sessions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** PR 3a of the spec — session files, ssh launch, Session Manager tree (filter, multi-select, connect all, drag, splitter), Quick Connect, `~/.ssh/config` import, rename / delete / new folder. (3b — Session Options window — gets its own plan section later.)

**Architecture:** All file and tree logic in `shagshell-core::session` (pure, tested on temp dirs); `launch::ssh_argv` pure; the app's `session_manager.rs` owns its `Message`/`update`/`view`; tabs take a `Launch` instead of always a shell.

**Tech Stack:** serde + toml 0.8 + shlex 1 in core; libcosmic widgets (`mouse_area`, `text_input`, `scrollable`, `dialog`).

**Spec:** `.claude/docs/specs/03-sessions.md`

## Global Constraints

- Sessions root `~/.config/shagshell/sessions/` (`$XDG_CONFIG_HOME` honoured); id = relative path without `.toml`; `Default.toml` at the root is the template, hidden from the tree.
- Name rules: non-empty, no `/`, not starting with `.`, not `Default` at the root, unique in its folder.
- Writes: temp file in the same dir + `rename`.
- ssh argv order exactly as in the spec; `ControlPath=<control_dir>/%C`, `ControlPersist=60`.
- Control dir `$XDG_RUNTIME_DIR/shagshell` (else `/tmp/shagshell-$UID`), mode 0700, created at startup.
- Panel width 160…600 px, default 240; expanded folders and width in cosmic-config.
- Every UI string via `fl!` (en + ru). No Ctrl+Insert / Shift+Insert.

## Review Focus

1. A session file with unknown fields or missing optional fields still loads; a broken TOML file is skipped with a warning, not fatal for the tree (core test).
2. Moving a folder into its own subfolder is refused (core test), moving onto a name taken in the target refuses without touching either (core test).
3. `extra` with quotes (`-o "ProxyCommand=ssh -W %h:%p jump"`) splits like a shell; an unbalanced quote → error shown, no launch (core test).
4. Filter with no matches shows an empty tree with a hint, never panics; clearing restores the previous expansion (core test on rows).
5. Connect all on a folder with 20 sessions opens 20 tabs without freezing the UI (headless with prepared files).

---

### Task 1: core — session model, names, store

**Files:** Create `crates/core/src/session/{mod,model,store}.rs`; modify `crates/core/Cargo.toml` (`serde = { version = "1", features = ["derive"] }`, `toml = "0.8"`), `lib.rs`.

**Interfaces (Produces):**
- `model::Session { host: String, port: Option<u16>, user: Option<String>, identity: Option<String>, keepalive: Option<u32>, extra: String }` — `#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)] #[serde(default)]`, `None`/empty fields not serialized.
- `store::valid_name(name: &str, at_root: bool) -> Result<(), NameError>`; `NameError { Empty, Slash, Dot, Reserved }`.
- `store::Node { Folder { name: String, children: Vec<Node> }, Session { name: String, session: Session } }` — sorted: folders first, then sessions, case-insensitive.
- `store::Store { root: PathBuf }` with:
  - `load(&self) -> Vec<Node>` (missing root → empty; bad files skipped, `log::warn`)
  - `get(&self, id: &str) -> io::Result<Session>`
  - `create(&self, folder: &str, name: &str, host: &str) -> io::Result<String>` (copies Default, returns id; `AlreadyExists` on duplicate)
  - `save(&self, id: &str, s: &Session) -> io::Result<()>` (atomic)
  - `new_folder(&self, parent: &str, name: &str) -> io::Result<String>`
  - `rename(&self, id: &str, new_name: &str) -> io::Result<String>` (session or folder)
  - `move_to(&self, id: &str, folder: &str) -> io::Result<String>` (refuses self / own subfolder → `InvalidInput`, taken → `AlreadyExists`)
  - `delete(&self, id: &str) -> io::Result<()>` (folder recursive)
  - `count_sessions(&self, id: &str) -> usize`
  - `pub fn default_root() -> PathBuf`
- `log` added to core deps.

- [ ] Step 1: failing tests (tempdir): `names_valid`, `names_invalid` (each `NameError`), `create_copies_default` (Default has `keepalive = 30` → created has it and the given host), `create_duplicate`, `load_sorted_and_skips_bad` (a broken `x.toml` skipped, `Default` hidden, folder before session, `b` before `C`), `unknown_fields_load`, `rename_session_and_folder`, `regression_move_into_own_subfolder`, `move_name_taken_untouched`, `delete_folder_recursive` + `count_sessions`, `save_is_atomic` (no `.tmp` left; content round-trips)
- [ ] Step 2: run → FAIL; Step 3: implement; Step 4: pass; Step 5: commit `feat(core): session files and tree store`

### Task 2: core — launch, ssh_config import, tree rows

**Files:** Create `crates/core/src/launch.rs`, `crates/core/src/sshconfig.rs`, `crates/core/src/session/rows.rs`; core dep `shlex = "1"`.

**Interfaces (Produces):**
- `launch::ssh_argv(s: &Session, control_dir: &Path, home: &Path) -> Result<Vec<String>, LaunchError>` — first item `"ssh"`; `LaunchError::BadExtra` on unbalanced quotes.
- `launch::control_dir() -> PathBuf` + `launch::ensure_control_dir() -> io::Result<PathBuf>` (0700).
- `sshconfig::hosts(text: &str) -> Vec<String>` — aliases from `Host` lines (any case, `Host=a`, several per line), skipping patterns with `* ? !`, deduplicated, in order.
- `sshconfig::import(store: &Store, text: &str) -> (usize, usize)` — (imported, skipped) into folder `ssh config`.
- `rows::Row { depth: u16, id: String, name: String, folder: bool, expanded: bool }`; `rows::visible(tree: &[Node], expanded: &BTreeSet<String>, filter: &str) -> Vec<Row>` (filter: substring of name or host, case-insensitive; matching sessions' folders forced open); `rows::range(rows: &[Row], from: &str, to: &str) -> Vec<String>`; `rows::sessions_under(tree, ids: &[String]) -> Vec<String>` (ids of sessions, folders expanded recursively, order of the tree, no duplicates).

- [ ] Step 1: failing tests — `argv_minimal` (`["ssh","-o","ControlMaster=auto","-o","ControlPath=/c/%C","-o","ControlPersist=60","h"]`), `argv_all` (port/user/identity `~/k` → `/home/u/k`/keepalive/extra in spec order), `regression_extra_quotes` (`-o "ProxyCommand=ssh -W %h:%p j"` → one arg), `extra_unbalanced`; `hosts_basic`, `hosts_skips_patterns`, `hosts_case_and_equals`, `import_skips_existing`; `rows_collapsed`, `rows_expanded`, `rows_filter_opens_parents`, `rows_filter_none`, `range_either_direction`, `sessions_under_folder_recursive_dedup`
- [ ] Step 2–4: fail → implement → pass; Step 5: commit `feat(core): ssh launch, ssh_config import, tree rows`

### Task 3: tabs launch anything

**Files:** Modify `crates/app/src/term/mod.rs` (`start` takes `program: &str, args: &[String]`), `crates/app/src/tabs.rs`, `status_bar.rs`, `app.rs` (ensure control dir at init).

**Interfaces:**
- `tabs::Launch { program: String, args: Vec<String>, title: String, protocol: Protocol }`, `enum Protocol { Local, Ssh { host: String } }`.
- `Tabs::open(&mut self, launch: Launch, look: &Look) -> Task<Message>` (replaces `new_local`; `NewLocalShell` builds a Local launch); `Tab` keeps its `Launch` for Reconnect.
- `Tabs::status()` → `Option<(bool, String /*protocol label, e.g. "SSH · host" / "Local Shell"*/, usize, usize)>`.

- [ ] Steps: implement, `just verify`, headless: Ctrl+Shift+T still works; commit `refactor(app): tabs run any program`

### Task 4: Session Manager tree

**Files:** Rewrite `crates/app/src/session_manager.rs`; modify `config.rs` (+ `session_manager_width: u16` default 240, `expanded: Vec<String>`), `app.rs` (route `Message::Sessions(SmMsg)`), `menu.rs` (Alt+C → `Action::Connect` shows the panel and focuses the filter; File → Import from ~/.ssh/config), ftl.

**Interfaces:**
- `SessionManager { store: Store, tree: Vec<Node>, rows: Vec<Row>, selected: BTreeSet<String>, anchor: Option<String>, filter: String, filter_id: widget::Id, drag: Option<String>, hover: Option<String>, dialog: Option<SmDialog>, notice: Option<String> }`
- `SmMsg { Click(String, Modifiers), DoubleClick(String), Toggle(String), Filter(String), ClearFilter, Connect, DragStart(String), DragOver(Option<String>), DragEnd, Context(String), Menu(SmCmd), Dialog(SmDialogMsg), SplitStart, SplitMove(f32), SplitEnd }`
- `SmCmd { Connect, Rename, Delete, NewSession, NewFolder }` (`menu::Action` → `Message::Sessions(SmMsg::Menu(..))`)
- `update` returns `(Task<Message>, Vec<String> /*session ids to open*/)`; the app turns ids into `Launch`es via `store.get` + `launch::ssh_argv` (error → notice).
- Rows: `mouse_area` per row (press → Click with current modifiers kept by the app from `keyboard::Event::ModifiersChanged`, double click, right press → Context + `widget::context_menu`), indent `depth * 16`, icons `folder-symbolic` / `folder-open-symbolic` / `network-server-symbolic`, selected rows `Container::Primary`-like highlight.
- Drag: press + move over another row (≥ 4 px) → `drag = Some(id)`, row under pointer highlighted if it's a folder; release → `store.move_to`, reload; Esc cancels.
- Splitter: 6 px `mouse_area` with `ResizingHorizontally`, width clamped 160…600, saved on release.
- Dialogs (in `App::dialog` when no tab dialog): Rename / New Folder (text input + validation message), Delete confirm (folder: «and N sessions»).
- Keys when the tree has focus (filter not focused): Enter → Connect, Delete → Delete…, F2 → Rename…, ↑/↓ move selection (add to `app::key_action`? no — handled only while the tree is focused, via `SmMsg` from the panel's own key listener; terminal keeps all keys when it has focus).

- [ ] Steps: implement; `just verify`; headless with prepared session files under the headless `XDG_CONFIG_HOME` + a user-space sshd (Task 6): tree, filter, expand, double click → ssh tab, Ctrl+click two + Enter → two tabs, drag session onto folder (file moved), splitter drag; commit `feat(app): Session Manager tree`

### Task 5: Quick Connect and import

**Files:** Create `crates/app/src/quick_connect.rs`; modify `app.rs`, `menu.rs` (File → Quick Connect… enabled, Alt+Q in `key_action`), toolbar (Connect → `Action::Connect`, Quick Connect → `Action::QuickConnect` enabled), ftl.

- `QuickConnect { host, port, user, save: bool, name, folder }`, `QcMsg`, `view() -> Element` as `widget::dialog`; Connect: validate (host required, port 1…65535 or empty) → `Launch` (title = name if saved else `user@host`/`host`), save → `store.create` + `store.save` + select in tree.
- Import: read `~/.ssh/config` (missing → notice), `sshconfig::import`, reload, notice «Imported N, skipped M».

- [ ] Steps: implement; headless: Alt+Q → dialog, fill `127.0.0.1` + test port → tab with ssh; with «Save» → file exists; commit `feat(app): Quick Connect and ~/.ssh/config import`

### Task 6: test sshd + docs + review + PR

- [ ] `headless.sh sshd start|stop`: user-space `/usr/sbin/sshd -D -f $DIR/sshd/sshd_config -p 2222` (temp host key, temp client key `id_test` in `authorized_keys`, `UsePAM no`, `StrictModes no`, `ListenAddress 127.0.0.1`), prints the `ssh` options to use; sessions in headless point to it with `identity = $DIR/sshd/id_test`, `extra = "-o StrictHostKeyChecking=accept-new -o UserKnownHostsFile=$DIR/sshd/known_hosts"`.
- [ ] TESTING.md phase 3a checklist; ROADMAP (3a); CLAUDE.md (session module, launch); version 0.3.0
- [ ] `/code-review`, fix confirmed; PR based on `feat/terminal`

---

# 3b — Session Options window

**Goal:** SecureCRT's Session Options as a window of its own: new session, edit, the Default template.
**Spec:** `.claude/docs/specs/03-sessions.md` § Session Options (3b). Branch `feat/session-options` (on `feat/sessions`), version 0.4.0.

## Task 1: `core::session::form` (TDD)

- `pub struct Form { name, host, port, user, identity, keepalive, extra: String }` — the window's fields as typed.
- `Form::from_session(name: &str, s: &Session) -> Form`; `Form::apply(&self, need_host: bool) -> Result<Session, FormError>` — fields trimmed, empty → `None` (every `Session` field is in the form; a later field not in it will need a `base`).
- `pub enum FormError { Host, Port, Keepalive, Extra }`; `pub fn valid_host(&str) -> bool` (non-empty, no whitespace, no leading `-`), Quick Connect uses it too.
- Tests: `round_trip`, `empty_fields_are_none`, `bad_port` (`0`, `70000`, `x`), `bad_keepalive`, `bad_extra` (unbalanced quote), `host_required_unless_default`, `host_with_dash_rejected`.

## Task 2: `app::session_options` + wiring

- `SessionOptions { window: window::Id, target: Target, form, categories: segmented_button model, error }`, `Target::{New { folder }, Edit { id }, Default}`; `SoMsg::{Field(Field, String), Category(Entity), Browse, Browsed(Option<String>), Ok, Cancel}`.
- Window: `window::open` 760×540, `decorations: false`, `exit_on_close_request: false`; `view_window(id)` draws `header_bar` (close = Cancel, drag) + categories left + fields right + [Cancel] [OK]; window background class. A second call raises it (`gain_focus`).
- OK (in the app, it owns the store): New → `check` name, `Store::create` + apply + save, select it; Edit → `store.get(id)` as base (gone → error, no file recreated); Default → `get("Default")` or `Session::default()`, `need_host = false`.
- «Обзор…»: `cosmic::dialog::file_chooser::open::Dialog` in `~/.ssh` (libcosmic feature `xdg-portal`).
- Entry points: tree menu «Изменить…» (`SmCmd::Edit`, sessions only), «Новая сессия…» (now this window, not Quick Connect), toolbar / Options → Session Options… (active tab's session, else the tree's one selected session), Options → Edit Default Session… (`Action::EditDefault`). `Protocol::Ssh` carries the session id.
- Keys: the app's key subscriptions act only on the main window's events (Alt+Q in the options window must not open Quick Connect).
- `close_requests` for the options window = Cancel.

## Review Focus

- Session renamed / deleted while its window is open → OK says so, writes nothing.
- Default.toml missing → the window opens empty, OK creates it.
- Keys typed in the options window never reach the main window's terminal or tree.
- Editing a session that is open in a tab: Reconnect replays the tab's stored command; the new options apply on the next connect from the tree.
- Main window closed while options is open → the app exits.
