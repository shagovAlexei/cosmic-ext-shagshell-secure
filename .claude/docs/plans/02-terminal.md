# 02. Terminal Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Local shell tabs with the cosmic-term terminal, tab groups in a `pane_grid`, SecureCRT exit behaviour, copy/paste (keys, copy on select, middle click) and a first Global Options drawer.

**Architecture:** Tabs live in one `segmented_button::SingleSelectModel` per group (entity data = `Tab`), groups in `pane_grid::State<Group>` — as cosmic-term. Terminal events go through one channel tagged with `TabId`. Pure rules (title numbering, cycling, shell choice) in core with tests.

**Tech Stack:** libcosmic (pinned), alacritty_terminal 0.25.1, cosmic-text 0.19, tokio.

**Spec:** `.claude/docs/specs/02-terminal.md`

`CMD` = `~/Projects/cosmic/cosmic-shagoff_commander`.

## Global Constraints

- All Ctrl+letter / Alt+letter go to the terminal except the app keys: Ctrl+Shift+T/W/C/V, Ctrl+Tab, Ctrl+Shift+Tab, Alt+1…9, Alt+M — matched by physical key (ru layout works).
- Scrollback 10 000 lines; `TERM=xterm-256color`; font 14 px.
- Copy on select and paste on middle click: config fields, default `true`.
- Exit line written into the terminal: `\r\n[Process exited with code N — press Enter to restart]\r\n` (en) — fixed English, it is terminal content, not UI.
- Every UI string via `fl!` (en + ru).

## Review Focus

1. Ctrl+C / Ctrl+D / Ctrl+Z / Ctrl+W (no Shift) and Alt+B / Alt+F reach the shell, not the app (headless: `cat -v` shows `^C`… / readline moves).
2. `$SHELL` unset, empty, pointing to a missing or non-executable file → `/bin/sh` (core tests).
3. Closing the active tab in the middle of 3 → right neighbour active; last tab of a group → group gone; last overall → placeholder, no panic (core test for neighbour; headless for the rest).
4. Window resized / divider dragged → `stty size` in the shell matches the visible area.
5. Enter on an exited tab restarts; any other key on an exited tab does nothing (no write to a dead PTY).

---

### Task 1: core — `shell::pick`, `tabs::{numbered, cycle}`

**Files:** Create `crates/core/src/shell.rs`, `crates/core/src/tabs.rs`; modify `crates/core/src/lib.rs`.

**Interfaces (Produces):**
- `pub fn shell::pick(env_shell: Option<&std::ffi::OsStr>) -> std::path::PathBuf` — the value if it is an executable regular file, else `/bin/sh`.
- `pub fn tabs::numbered(base: &str, taken: &[&str]) -> String` — `base` if free, else `"{base} (n)"` with the smallest free n ≥ 2.
- `pub fn tabs::cycle(pos: usize, len: usize, forward: bool) -> usize` — next / previous position, wrapping; `len == 0` → 0.
- `pub fn tabs::after_close(pos: usize, len_before: usize) -> Option<usize>` — position to activate after closing `pos` among `len_before`: same position (right neighbour slides in), else `pos - 1`; `None` when it was the only one.

- [ ] Step 1: failing tests — `pick_env_executable` (temp exe → itself), `pick_unset` (None → /bin/sh), `pick_empty` (`""` → /bin/sh), `pick_missing`, `regression_pick_not_executable` (0o644 → /bin/sh); `numbered_free` (`("Local Shell", [])` → `"Local Shell"`), `numbered_second` (taken `["Local Shell"]` → `"Local Shell (2)"`), `numbered_gap` (taken `["Local Shell", "Local Shell (3)"]` → `"Local Shell (2)"`); `cycle_wraps` (`(2,3,true)` → 0, `(0,3,false)` → 2), `cycle_empty` → 0; `after_close_middle` (`(1,3)` → `Some(1)`), `after_close_last` (`(2,3)` → `Some(1)`), `after_close_only` (`(0,1)` → `None`)
- [ ] Step 2: `cargo test -p shagshell-core` → FAIL
- [ ] Step 3: implement (reuse the `is_file && mode & 0o111` check from `check.rs` — extract `pub(crate) fn is_executable(&Path) -> bool` there)
- [ ] Step 4: tests pass
- [ ] Step 5: commit `feat(core): shell choice and tab rules`

### Task 2: terminal module

**Files:** Create `crates/app/src/term/{mod,terminal,terminal_box,terminal_theme,mouse_reporter,style}.rs` — copies of `CMD/crates/app/src/term/*` with the source line at the top. Modify `crates/app/Cargo.toml` (+ `alacritty_terminal = "0.25.1"`, `cosmic-text = { version = "0.19", features = ["monospace_fallback", "shape-run-cache"] }`, `indexmap = "2"`, `tokio = { version = "1", features = ["rt", "sync"] }`).

**Interfaces (Produces):**
- `pub type TabId = u64;`
- `terminal::EventProxy(TabId, UnboundedSender<(TabId, Event)>)`.
- `term::start(id: TabId, tx: UnboundedSender<(TabId, Event)>, shell: &Path, dark: bool, bg: u32, style: &Style) -> io::Result<Terminal>` — `tty::Options { shell, working_directory: home, env: TERM=xterm-256color }`, history 10 000.
- `term::restyle(&Mutex<Terminal>, &Style, dark, bg)`, `term::on_event(&Mutex<Terminal>, Event) -> Outcome { None, Exit(Option<i32>), Copy(String), Title(String) }` (Title from `Event::Title`, `ResetTitle` → `Title(String::new())`).
- `term::subscription() -> Subscription<Message>` → `Message::TermTx(UnboundedSender<(TabId, Event)>)` first, then `Message::Term(TabId, Event)`.
- `terminal_box(&Mutex<Terminal>, app_key: fn(Modifiers, &Key) -> bool)` with `.on_copy_selection(Fn(String) -> Message)` (fired on left release when a selection exists and copy-on-select is on — the box takes a `copy_on_select: bool`) and existing `.on_middle_click`, `.on_focus`.
- Keys inside the box: the existing Ctrl+Shift+C/V only (Ctrl+Insert / Shift+Insert are the user's system keys — never bound); `app_key` keys are not consumed.

- [ ] Step 1: copy files, apply the changes above; delete commander-only parts (`follow`, `waiting_tick`, `stop_waiting`, `shell_pid` users, `shell.rs` calls, Ctrl+O pass).
- [ ] Step 2: `cargo build -p shagshell` → OK (module unused for now: `#[allow(dead_code)]` only on `mod term;` until Task 3)
- [ ] Step 3: commit `feat(app): terminal module from cosmic-term`

### Task 3: tabs and groups

**Files:** Create `crates/app/src/tabs.rs`; modify `app.rs`, `menu.rs`, `status_bar.rs`, ftl files.

**Interfaces:**
- Consumes: Task 1 fns, Task 2 `term::*`.
- Produces:
  - `pub struct Tab { id: TabId, terminal: Mutex<Terminal>, title: String, alive: bool }`
  - `pub struct Group { model: segmented_button::SingleSelectModel }` (entity data: `Tab`)
  - `pub struct Tabs { panes: Option<pane_grid::State<Group>>, focus: Option<pane_grid::Pane>, next_id: TabId, tx: Option<UnboundedSender<(TabId, Event)>>, confirm: Option<(Pane, Entity)> }`
  - `pub enum TabsMsg { Activate(Pane, Entity), Close(Pane, Entity), CloseConfirmed, CloseCancelled, Context(Pane, Entity), Reorder(Pane, ReorderEvent), PaneClicked(Pane), PaneResized(ResizeEvent), Focused(Pane, bool), CopySelection(String), MiddlePaste }`
  - `impl Tabs { fn new_local(&mut self) ; fn act(&mut self, Action) -> Task; fn on_term(&mut self, TabId, Event) -> Task; fn update(&mut self, TabsMsg) -> Task; fn view(&self, &Config) -> Element; fn dialog(&self) -> Option<Element>; fn status(&self) -> Option<(bool, (u16,u16))> }`
- `menu::Action` gains: `NewLocalShell, CloseTab, NextTab, PrevTab, SelectTab(u8), SplitRight, SplitDown, Copy, Paste, SelectAll, Clone`. Tab context menu = `segmented_button` `.context_menu()` with Reconnect / Clone / Move to New Group → Right, Down / Close.
- `app::route_key` maps the app keys (Global Constraints); the terminal box lets them through via `app_key = crate::app::is_app_key`.

Behaviour per spec. Exit: `Outcome::Exit(code)` → `alive = false`, write the exit line via `term.lock().term.lock()`-side feed (`Processor::advance` with the line bytes — no PTY write), icon ✗. Enter on dead tab → restart in place (new `Terminal`, same entity). Close alive → `confirm = Some(..)` → `dialog()` renders `widget::dialog` with Close / Cancel. Split: `pane_grid.split(axis, focus, Group::with(tab moved out))`, refused if the group has one tab. Last tab closed → `panes.close(pane)`; none left → `panes = None` → placeholder.

- [ ] Step 1: implement `tabs.rs` + wire into `App` (field `tabs: Tabs`, `Message::Tabs(TabsMsg)`, `Message::TermTx`, `Message::Term`), menu items enabled (File → New Local Shell, Edit → Copy/Paste/Select All, Window → Split Right/Down, Next/Previous Tab), status bar from `tabs.status()`
- [ ] Step 2: `cargo clippy --all-targets -- -D warnings` clean
- [ ] Step 3: headless: Ctrl+Shift+T, `echo hi`, `shot`; second tab title «Local Shell (2)»; Ctrl+Tab; `exit` → ✗ + line; Enter → ✓; Review Focus 1 (`cat -v` + Ctrl+C), 4 (`stty size` before/after split)
- [ ] Step 4: commit `feat(app): local shell tabs and tab groups`

### Task 4: Global Options drawer

**Files:** Create `crates/app/src/options.rs`; modify `config.rs` (+ `copy_on_select: bool`, `paste_on_middle_click: bool`, defaults `true`), `app.rs` (drawer enum `Drawer { About, Options }` replaces the bool), ftl.

- [ ] Step 1: `options::view(&Config) -> ContextDrawer` with `settings::section().title(fl!("options-terminal"))` and two `toggler` items → `Message::SetCopyOnSelect(bool)` / `SetPasteOnMiddle(bool)` → write config
- [ ] Step 2: terminal box reads the flags (`copy_on_select`, middle click handler only when on)
- [ ] Step 3: headless: Options drawer opens (by `Action::GlobalOptions` key-less — check via menu in TESTING.md), `shot`
- [ ] Step 4: commit `feat(app): Global Options with terminal copy/paste toggles`

### Task 5: docs, review, PR

- [ ] TESTING.md phase 2 checklist (spec "Проверка"); ROADMAP tick 2; CLAUDE.md: `tabs.rs`, `options.rs`, `term/` lines; version 0.2.0
- [ ] `just verify`; `/code-review`, fix confirmed
- [ ] push `feat/terminal`, `gh pr create --base feat/skeleton` while #2 is open (retarget to `main` after it merges); `gh pr checks --watch`
