# 01. Skeleton Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A buildable, installable `shagshell` with the SecureCRT window layout (menu, toolbar, Session Manager, placeholder, status bar), `--check`, `.deb`, CI, headless check.

**Architecture:** Two-crate workspace. `shagshell-core` holds the tested `check::probe`. `shagshell` (libcosmic) has `app.rs` routing only, one module per screen area; one `Action` enum for every user action.

**Tech Stack:** Rust 2024, libcosmic (git rev `ef490df50b0a05a21c494c3f75737581bf0b39d9`), i18n-embed 0.16 + fluent, cosmic-config, just, GitHub Actions.

**Spec:** `.claude/docs/specs/01-skeleton.md`

**Reference project** (copy, adapt, mark the source at the top of copied files): `~/Projects/cosmic/cosmic-shagoff_commander` — referred to as `CMD` below.

## Global Constraints

- APP_ID `io.github.shagovalexei.ShagShellSecure`; binary `shagshell`; core crate `shagshell-core`; menu name «ShagShell Secure»; repo `shagovAlexei/cosmic-ext-shagshell-secure`.
- version `0.1.0`, edition 2024, `license = "GPL-3.0-only"`, authors `Shagov Alexei <shagov.alexei@gmail.com>`.
- libcosmic: `default-features = false`, features `["tokio", "winit", "wayland", "wgpu", "multi-window", "about"]`.
- Every UI string via `fl!`, keys present in both `i18n/en/shagshell.ftl` and `i18n/ru/shagshell.ftl`.
- No Ctrl+letter app shortcuts. Only Alt+M (toggle Session Manager) in this phase.
- Window 1200×800; Session Manager width 240 px.
- `just verify` = `cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test`.

## Review Focus

1. A file named `ssh` in PATH that is not executable, or a directory named `ssh` → `MISSING` (test in Task 1).
2. PATH unset or empty, or with empty components (`a::b`) → every tool `MISSING`, no panic (test in Task 1).
3. Missing / corrupt cosmic-config → Session Manager visible (default `true`), no crash (Task 2, `Config::default()` + `unwrap_or_else` read like `CMD/crates/app/src/config.rs:371`).
4. Russian UI at 1200 px: the 9 menu titles + toolbar fit without clipping (headless shot with `LANG=ru_RU.UTF-8`, Task 4).
5. Software renderer (`ICED_BACKEND=tiny-skia`) still draws the window (headless shot, Task 4).

---

### Task 1: Workspace + `check::probe`

**Files:**
- Create: `Cargo.toml` (workspace: `resolver = "3"`, members `crates/core`, `crates/app`, `[workspace.package]` per Global Constraints, `[profile.release] strip = true`)
- Create: `crates/core/Cargo.toml` (no deps; dev-dep `tempfile = "3"`), `crates/core/src/lib.rs` (`pub mod check;`), `crates/core/src/check.rs`
- Create: `crates/app/Cargo.toml` + `crates/app/src/main.rs` with an empty `fn main() {}` placeholder so the workspace builds (filled in Task 2)

**Interfaces:**
- Produces: `pub const TOOLS: [&str; 4] = ["ssh", "telnet", "picocom", "python3"];` and `pub fn probe(names: &[&str], path: Option<&std::ffi::OsStr>) -> Vec<(String, bool)>` — order of `names` kept; `true` = an executable regular file `<dir>/<name>` exists in some PATH dir (`std::env::split_paths`, skip empty components, `metadata.is_file() && mode & 0o111 != 0`).

- [ ] **Step 1: Write failing tests** in `check.rs` `#[cfg(test)]`, using a `tempfile::TempDir` with helper `exe(dir, name, mode)`:
  - `found_executable`: `ssh` mode 0o755 → `[("ssh", true)]`
  - `regression_not_executable_is_missing`: `ssh` mode 0o644 → `false`
  - `directory_is_missing`: dir named `ssh` → `false`
  - `no_path_all_missing`: `probe(&TOOLS, None)` → 4 entries, all `false`, names in `TOOLS` order
  - `empty_components_skipped`: PATH `"::<tmp>"` with executable `ssh` → `true`
- [ ] **Step 2:** `cargo test -p shagshell-core` → FAIL (not defined)
- [ ] **Step 3:** implement `probe`
- [ ] **Step 4:** `cargo test -p shagshell-core` → 5 passed
- [ ] **Step 5:** commit `feat(core): workspace and tool probe for --check`

### Task 2: App window skeleton

**Files:**
- Modify: `crates/app/Cargo.toml` — deps `shagshell-core` (path), libcosmic (Global Constraints), `i18n-embed = { version = "0.16", features = ["fluent-system", "desktop-requester"] }`, `i18n-embed-fl = "0.10"`, `rust-embed = "8"`, `log = "0.4"`, `simple_logger = "5"`, `wgpu = { version = "28", default-features = false }`, `pollster = "0.4"`, `serde = { version = "1", features = ["derive"] }`
- Create: `crates/app/src/{main,i18n,app,menu,toolbar,session_manager,status_bar,config,about}.rs`, `crates/app/i18n/{en,ru}/shagshell.ftl`, `i18n.toml` in `crates/app` (copy from `CMD/crates/app/i18n.toml`, `fallback_language = "en"`)

**Interfaces:**
- Consumes: `shagshell_core::check::{probe, TOOLS}`
- Produces (used by later phases):
  - `menu::Action` — `#[derive(Clone, Copy, Debug, PartialEq, Eq)] enum Action { Exit, ToggleSessionManager, About, Connect, QuickConnect, Reconnect, Disconnect, ConnectSftp, SessionOptions, GlobalOptions }` implementing libcosmic `menu::action::MenuAction<Message = Message>` (→ `Message::Action(self)`).
  - `app::Message { Action(menu::Action), Config(config::Config), OpenUrl(String), CloseDrawer }`.
  - `config::Config` — `#[derive(Clone, Debug, PartialEq, CosmicConfigEntry)] #[version = 1] struct Config { session_manager_visible: bool }`, `Default` → `true`.
  - Each area module exposes `pub fn view(..) -> Element<'_, Message>`.

Steps:
- [ ] **Step 1:** `i18n.rs` — copy `CMD/crates/app/src/i18n.rs` unchanged (macro `fl!`).
- [ ] **Step 2:** `main.rs`:
  - `--version` → print `shagshell <CARGO_PKG_VERSION>`, exit 0.
  - `--check` → for each `probe(&TOOLS, env::var_os("PATH").as_deref())` print `{name:<8} ok|MISSING`; exit 0 if `ssh` found else 1. No window, no GPU probe.
  - Else: logger (warn, `.env()`), `pick_renderer()` copied from `CMD/crates/app/src/main.rs` (keep its doc comment), `i18n::init`, `Settings::default().size(Size::new(1200.0, 800.0)).transparent(!software)`, `cosmic::app::run::<app::App>`.
- [ ] **Step 3:** `config.rs` — `Config` above; `read(h)` as `CMD/crates/app/src/config.rs:371`; watched via `core.watch_config::<Config>(APP_ID)` → `Message::Config`.
- [ ] **Step 4:** `menu.rs` — `pub fn bar(..) -> Element<Message>` via `menu::bar` (pattern `CMD/crates/app/src/menu.rs:750`). Roots and items (disabled = `menu::Item::ButtonDisabled`):
  - File: Connect…, Quick Connect…, Reconnect, Disconnect, ─, Exit (enabled)
  - Edit: Copy, Paste, Select All, Find…
  - View: Session Manager (enabled, `CheckBox` bound to config, shows `Alt+M`), Toolbar, Status Bar
  - Options: Session Options…, Global Options…
  - Transfer: Connect SFTP Tab, Send File…
  - Script: Run…, Cancel
  - Tools: Create Public Key…, Keymap Editor…
  - Window: Tile Horizontally, Tile Vertically, Next Tab, Previous Tab
  - Help: About (enabled)
  - Key binds map: only `Alt+M` → `ToggleSessionManager`.
- [ ] **Step 5:** `toolbar.rs` — row of `button::icon` with tooltips, all without `on_press` (disabled): Connect (`network-server-symbolic`), Quick Connect (`media-playback-start-symbolic`), Reconnect (`view-refresh-symbolic`), Disconnect (`process-stop-symbolic`), SFTP (`folder-remote-symbolic`), Session Options (`emblem-system-symbolic`).
- [ ] **Step 6:** `session_manager.rs` — container width 240, title «Session Manager», centered text `fl!("sessions-empty")`. `status_bar.rs` — `fl!("status-disconnected")`. Center placeholder in `app.rs::view`: `fl!("no-sessions")` + `fl!("no-sessions-hint")`.
- [ ] **Step 7:** `about.rs` — `About` built like `CMD/crates/app/src/drawer.rs:106` (name, version, GPL-3.0-only, links Source / Issues to the repo); shown via `context_drawer`.
- [ ] **Step 8:** `app.rs` — `impl Application`: `APP_ID`, `init` (read config), `header_start` → `menu::bar`, `view` = column[toolbar, row[session_manager if visible, placeholder], status_bar], `update` arms one line each: `Action(Exit)` → `cosmic::iced::exit()`, `ToggleSessionManager` → flip + `write_entry`, `About` → open drawer, `OpenUrl` → `open::that` or `xdg-open`, `Config` → store. Unused actions: no-op arm `_ => Task::none()`.
- [ ] **Step 9:** ftl keys (en + ru): menu titles and items, `sessions-empty` (No sessions / Нет сессий), `no-sessions` (No open sessions / Нет открытых сессий), `no-sessions-hint` (Use Quick Connect (Alt+Q) / Используйте Quick Connect (Alt+Q)), `status-disconnected` (Not connected / Не подключено), toolbar tooltips, about strings. Menu item names stay English-SecureCRT in `en`; `ru` translates them.
- [ ] **Step 10:** `cargo build` → OK; `cargo run -p shagshell -- --check` → 4 lines, exit 0 here; `just verify` (after Task 3) — for now `cargo clippy --all-targets -- -D warnings` clean.
- [ ] **Step 11:** commit `feat(app): SecureCRT-style window skeleton`

### Task 3: Resources, justfile, CI, .deb

**Files:**
- Create: `res/io.github.shagovalexei.ShagShellSecure.desktop` (Name=ShagShell Secure, Comment / Comment[ru], `Exec=shagshell`, Icon=APP_ID, `Categories=COSMIC;Network;RemoteAccess;System;TerminalEmulator;`, Keywords ssh;sftp;telnet;serial;terminal;securecrt;)
- Create: `res/io.github.shagovalexei.ShagShellSecure.metainfo.xml` (pattern of `CMD/res/*.metainfo.xml`, GPL-3.0-only, no screenshots yet)
- Create: `res/icons/io.github.shagovalexei.ShagShellSecure.svg` — 128×128 viewBox: rounded dark terminal window, `>_` prompt, a shield bottom-right
- Create: `justfile` — copy of `CMD/justfile` with `name := 'shagshell'`, `appid := 'io.github.shagovalexei.ShagShellSecure'`; drop `rpm` and man page lines; deb `Depends: $libs, openssh-client`, `Recommends: telnet, picocom, python3, gnome-keyring`, URLs to this repo, Description «SecureCRT-style SSH/SFTP session manager for COSMIC»
- Create: `.github/workflows/build.yml` (copy of `CMD`), `.github/workflows/release.yml` (copy of `CMD`, drop rpm steps and `cargo-generate-rpm`, `paths` without `res/*.1`)
- Create: `Cargo.lock` (via build; committed — CI uses `--locked`)

- [ ] **Step 1:** create files
- [ ] **Step 2:** `just verify` → all green
- [ ] **Step 3:** `just deb` → `target/deb/shagshell_0.1.0_amd64.deb`; `dpkg-deb -c` lists bin, .desktop, metainfo, icon; `dpkg-deb -f … Depends` contains `openssh-client`
- [ ] **Step 4:** `desktop-file-validate res/*.desktop` and `appstreamcli validate --no-net res/*.metainfo.xml` (if installed) → no errors
- [ ] **Step 5:** commit `build: justfile, .deb, CI, desktop entry, icon`

### Task 4: Headless check + TESTING.md

**Files:**
- Create: `.claude/skills/shagshell-feature/headless.sh` (copy `CMD/.claude/skills/shagoff-feature/headless.sh`, binary `target/debug/shagshell`, temp dir `/tmp/shagshell-headless`), copy `CMD/.claude/skills/shagoff-feature/vpointer/` (source only, not `target/`)
- Create: `TESTING.md` (format of `CMD/TESTING.md`: manual checks table + regression table with `regression_not_executable_is_missing`)

- [ ] **Step 1:** `headless.sh start` → `shot main`; Read png: menu (9 titles), toolbar, Session Manager with «No sessions», placeholder, status bar visible
- [ ] **Step 2:** `vkey alt m` → `shot hidden`: Session Manager gone; `vkey alt m` again → back
- [ ] **Step 3:** click «File» → `shot file-menu`: items visible, Exit enabled, others greyed
- [ ] **Step 4:** restart with `LANG=ru_RU.UTF-8` → `shot ru`: nothing clipped (Review Focus 4); restart with `ICED_BACKEND=tiny-skia` → `shot sw` (Review Focus 5)
- [ ] **Step 5:** TESTING.md manual rows: install from `.deb`, launch from COSMIC launcher by name «ShagShell Secure», icon shown, Alt+M persists across restart
- [ ] **Step 6:** commit `test: headless check and TESTING.md`

### Task 5: Docs + PR

- [ ] **Step 1:** ROADMAP: tick 0 and 1; CLAUDE.md "Commands (from phase 1)" → "Commands", keep in sync with the justfile
- [ ] **Step 2:** `/code-review`, fix confirmed findings
- [ ] **Step 3:** commit `docs: phase 1 done`; `git push -u origin feat/skeleton`; `gh pr create --fill` (body links spec + plan); `gh pr checks --watch` green
- [ ] **Step 4:** merge only after the user says so; then tag `v0.1.0` on `main`, push tag, `gh run watch` the release, `gh release view v0.1.0` shows the .deb
