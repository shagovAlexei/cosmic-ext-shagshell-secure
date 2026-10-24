# CLAUDE.md

Guidance for Claude Code in this repository. Reply to the user in Russian; code, identifiers, commits and PRs are in English.

## Project

ShagShell Secure is an SSH / SFTP session manager for Pop!_OS 24.04 COSMIC, written in Rust on libcosmic. The goal is to behave as close as possible to **SecureCRT for Windows** (plus a SecureFX-style SFTP tab). Users: Linux servers and network gear (Cisco, MikroTik, …).

- `.claude/docs/securecrt-reference.md` is the source of truth for "how SecureCRT does it". Check it before implementing a feature; update it when you learn more.
- Full design and its rationale: `.claude/docs/specs/00-architecture.md`.
- Phases and backlog: `.claude/docs/ROADMAP.md`.
- Every feature goes through the `shagshell-feature` skill (`.claude/skills/shagshell-feature/SKILL.md`).

Fixed names (never change these):

| | |
|---|---|
| Repo | `shagovAlexei/cosmic-ext-shagshell-secure` |
| Binary | `shagshell` (crate `shagshell`, core crate `shagshell-core`) |
| Menu name | ShagShell Secure |
| APP_ID | `io.github.shagovalexei.ShagShellSecure` (config path, `.desktop`, metainfo, icon, keyring attribute) |
| License | GPL-3.0-only (code adapted from cosmic-term / shagoff-commander is allowed; mark the source at the top of the file) |

## Commands

```bash
just verify          # fmt --check + clippy -D warnings + test — must pass before every PR
just build-debug / build-release
just run-logs        # RUST_LOG=debug cargo run
sudo just install    # binary, .desktop, metainfo, icon into /usr (rootdir=… to stage)
just deb             # target/deb/*.deb; tag vX.Y.Z → GitHub release with it
shagshell --check    # probes ssh, telnet, picocom, python3, keyring: ok / MISSING
```

## Architecture

### Key decision: every connection is a command in a local PTY

We do **not** implement SSH. The system OpenSSH does it; we drive it.

- SSH = `ssh` spawned in the PTY of the embedded terminal, with:
  - `-o ControlMaster=auto -o ControlPath=$XDG_RUNTIME_DIR/shagshell/%C -o ControlPersist=60` (so SFTP and clones reuse the connection);
  - `-p`, `-l`, `-i`, `-J <jump>`, `-L/-R/-D`, `-o ServerAliveInterval=N` built from the session.
  - env: `SSH_ASKPASS=<path to shagshell>`, `SSH_ASKPASS_REQUIRE=force`, `SHAGSHELL_SESSION=<session id>`.
- Telnet = `telnet host port`; Serial = `picocom -b <baud> <dev>`; Local shell = `$SHELL`.
- **Askpass:** ssh runs `shagshell "<prompt>"` (one argument; `SHAGSHELL_ASKPASS_SOCK` set = helper mode), which connects to this app instance's socket `$XDG_RUNTIME_DIR/shagshell/askpass-<pid>.sock` (one JSON line each way). The app classifies the prompt (password / passphrase / host-key yes-no / other), answers from the keyring on the first try of that ssh (its pid = the helper's ppid) or shows a dialog in the main window ("Save password", off by default; a second try = «wrong password»), and the helper prints the answer to stdout. Cancel kills that ssh (checked ours by its `/proc/<pid>/environ`): ssh would send empty passwords for its other tries.
- **SFTP** = our own SFTP v3 client (`core::sftp::proto`) on the pipes of `ssh <the tab's options> -s -- host sftp` (same ControlPath: an open connection is reused, else askpass asks). Not `openssh-sftp-client`: one non-UTF-8 file name kills its connection (`ssh_format` reads paths with `String::from_utf8`), and its reads aren't pipelined (15 MB/s). Names are bytes; READ / WRITE keep 16 × 64 KiB in flight.
- Why not russh: own known_hosts / ssh_config parsing, known russh bugs (one stalled channel freezes the connection, SFTP breaks on non-UTF-8 names), API churn. Switching later stays cheap: the terminal does not care where bytes come from. Do **not** add an "SSH backend" trait in advance.

### Button Bar and scripting (SecureCRT style)

- **Button Bar:** several named bars, a session may pick its own; a button = Send String (escapes), Run Script (`kind = "script"`: `arg` the .py, `args` its arguments), Menu Function (same `Action` enum as key bindings) or Launch App; target current / all tabs. Stored in `~/.config/shagshell/buttonbars.toml`.
- **Python scripts:** a separate `python3` process (not PyO3: a crash can't take the app down, no libpython dependency, user's pip packages work). We ship `crt.py` (`/usr/share/shagshell/python/`), started with `PYTHONPATH` to it and `SHAGSHELL_SCRIPT_SOCK=$XDG_RUNTIME_DIR/shagshell/script-<n>.sock`; every `crt.*` call is a JSON line over that socket. The `crt` object mirrors SecureCRT's model (`crt.Screen.Send / WaitForString / ReadString`, `crt.Session.Connect`, `crt.Dialog.MessageBox / Prompt`, `crt.GetTab(n)`, `crt.Sleep`, `crt.Arguments`, `crt.Clipboard.Text`, `Screen.Get / CurrentRow / Selection`, `Session.Log`) so SecureCRT Python scripts port with minimal edits. Waits run app-side on the tab's output stream. Run from Script menu, Button Bar, a key, a session's logon script, or `shagshell --script file.py`. Example scripts (RU+EN comments) live in `Example/` (installed to `/usr/share/shagshell/examples/`); spec `specs/18-script-examples.md`.

### Storage

- Sessions: `~/.config/shagshell/sessions/<folder>/…/<name>.toml`, one file per session; the folder tree is the Session Manager tree.
- `sessions/Default.toml` is the template: a new session copies it at creation (SecureCRT behaviour, no live inheritance).
- Credentials: `~/.config/shagshell/credentials.toml` holds named sets (username only); sessions reference a set by name.
- **Secrets only in Secret Service** (gnome-keyring), never in files: attributes `app=io.github.shagovalexei.ShagShellSecure`, `kind=session|credential`, `id=<session path or credential name>`, `secret=password|passphrase`; they follow a session renamed / moved / deleted (`keyring::retarget` / `forget_under`). A session with a credential set: its password is the set's (`askpass::owner`), its user the set's (`Credentials::resolve` at launch).
- **Master password** = a keyring of our own labelled «ShagShell Secure» (gnome-keyring asks its password when made and on first use after login); its existence is the setting (no config flag); `keyring::set_separate` moves our items in / out. New secrets go there when it exists.
- Keys: `~/.ssh` + ssh-agent (gnome-keyring); generation through `ssh-keygen`.
- Global options: cosmic-config (`~/.config/cosmic/<APP_ID>/v1/`). Window state / open tabs: `~/.local/state/…`.

### Workspace

- **`crates/core` (`shagshell-core`)** has no libcosmic dependency. All logic lives here and is unit-tested:
  - `session/{model,store,rows,form}.rs` — `Session` (every field defaulted, unknown ones kept in `rest` and written back), the tree on disk (`Store`: load / create from Default / atomic save / rename / move / delete), the rows on screen (`rows::visible` with filter, Shift ranges, `sessions_under` for Connect all), the Session Options fields → checked `Session` (`form`);
  - `sshconfig.rs` — `~/.ssh/config` hosts → sessions (alias only; ssh applies the rest);
  - `launch.rs` — `Session` → argv + env (pure, tested against expected argv);
  - `askpass.rs` — prompt classification, the socket protocol, the helper, `Tries` (first / retry / cancelled per ssh), `abort`;
  - `keyring.rs` — Secret Service over zbus (from shagoff-commander, + `lookup` / `delete`);
  - `credentials.rs` — `credentials.toml` (name → user, unknown fields kept), `put` (checked add / rename), `resolve`; `Store::using` / `relink` follow a renamed / deleted set;
  - `sftp/{proto,queue}.rs` — the SFTP v3 client (tested against `/usr/lib/openssh/sftp-server` run directly) and the transfer queue (a pure state machine: one job at a time, retry, cancel, per-batch «for all»);
  - `sftp/sync.rs` — Synchronize Folders: `diff(local, remote, mode, mirror)` → ops (copy / mkdir / delete / conflict), trees read by `scan_local` / `ops::scan_remote`; copies keep the source's mtime (`ops::preserve_times`) so a second sync sees equal files. App side: `sftp_sync.rs` (dialog), batches queued with «overwrite» answered, deletes after the copies all went well.
  - `bookmarks.rs` — SFTP bookmarks (`bookmarks.toml`, per place); `uri.rs` — `text/uri-list` of dropped files → paths;
  - `panel.rs` — a file panel (listing, cursor, marks, targets) and size / time text, after shagoff-commander;
  - `logfile.rs` — log name templates `%H %S %Y %M %D %h %m %s %P %U`, `Cleaner` (output → plain text), `Logger`; the app copies a PTY's output to it through `term::log::TeePty`;
  - `securecrt.rs` — import from SecureCRT's `Config/` (sessions `.ini`, `ButtonBarV5.ini`); reading only, what can't come across is reported;
  - `logon.rs` — Logon Actions (expect → send automaton over the session's output; `%user%` / `%password%`); the app feeds it from the PTY reader (`term::log::Tap`) and types the answers in;
  - `macros.rs` — Button Bar / Command Window escapes (`\r \n \t \e \\ \p`);
  - `buttonbar.rs` — named bars, buttons (Send String / Run Script / Menu Function / Launch App), `buttonbars.toml`;
  - `keygen.rs` — Tools → Create Public Key (`ssh-keygen` arguments, the passphrase through `SSH_ASKPASS` = our own binary printing `SHAGSHELL_ASKPASS_FIXED`, never an argument) and the `ssh-copy-id` arguments.
  - `keymap.rs` — Key Map (`keymap.toml`): chords (`Ctrl+Alt+F5`, physical key code names), `check` (what may be bound: no plain / Ctrl / Ctrl+Shift letters, no Ctrl+Insert / Shift+Insert), bindings = a chord + a `Button`. The app keeps them where `app::key_action` can see them (`keymap_editor::set_bindings`); built-in keys come first.
  - `zmodem.rs` — X / Y / Zmodem in a terminal tab through the system lrzsz: `Detector` (a server's `sz` announces itself with `**\x18B00`), `receive` / `send` commands. App side: `term::log::Tap` diverts the PTY's output to the program's stdin while a transfer runs (its stdout goes back as input), `transfer_pick.rs` (the file choosers of Transfer → …).
  - `commands.rs` — Command Manager (`commands.toml`): a tree of folders and commands (`Folder`, `Command`, `Addr`, `rows` with filter / open state, add / edit / remove / shift / merge); `securecrt::read_commands` reads SecureCRT's `Config/Commands`. App: `command_manager.rs` (the pane at the right of the tabs and its dialogs; the app sends with `Tabs::send` and saves).
  - `script.rs` — the `crt` JSON-lines socket protocol for Python scripts: `Buffer` (a tab's output as text for `WaitForString` / `ReadString`, collected only while a script runs), `serve` (requests → `Host`), `command` (argv + env of the `python3` process). Spec: `specs/10-scripting.md`.
- **`crates/app` (`shagshell`)** is the libcosmic UI, **split by feature**: each module (`session_manager`, `tabs`, `term/`, `sftp_view`, `command_window`, `button_bar`, `dialogs`, `menu`, `keymap`, `config`, `i18n`) owns its own `Message`, `update` and `view`. `app.rs` only routes. No growing a single 15k-line `app.rs` like shagoff-commander did.
  - `term/` is cosmic-term's terminal (via shagoff-commander `crates/app/src/term/`), driven by a local PTY. Events come through one channel tagged with `TabId`; `term::print` writes text on screen without the PTY (the exit line).
  - `tabs.rs`: tabs are kept by id (`HashMap<TabId, Tab>`); each group in the `pane_grid` has a `segmented_button` model holding the ids in order (moving a tab between groups = remove + insert the id). Exit leaves the tab (✗, Enter restarts), closing a live one asks. `app::key_action` is the one list of app keys (physical keys, any layout): the terminal lets exactly those through (`tabs::is_app_key`), everything else is the program's. Never bind Ctrl+Insert / Shift+Insert (the user's system uses them). The tab menu has its own `TabCmd`s, about the right-clicked tab; keys and the menu bar act on the active tab.
  - `options.rs`: Options → Global Options drawer.
  - `session_manager.rs`: the tree pane (own `SmMsg`, `Outcome { open, save, new_session }` back to the app), dialogs for rename / new folder / delete, drag (press a row, enter another, release anywhere — mouse events subscribed only while held), splitter. A click in the tree takes the keys from the terminal (`unfocus`), so Enter / F2 / Delete reach it.
  - `quick_connect.rs`: Alt+Q dialog; saving goes through `Store::create` (Default template).
  - `button_bar.rs`: View → Button Bar, bars from `core::buttonbar`; the app runs a button (`App::run_button`: Send → `Tabs::send`, Menu → `Action::named`, Launch → `launch_app`); a session's own bar (`Session.button_bar`) shows while its tab is in front; a broken `buttonbars.toml` is never overwritten.
  - `command_window.rs`: View → Command Window, a line sent to the current / visible / all terminals (`Tabs::send`, `\p` pauses go through `TabsMsg::Send`); its history stays in memory; it asks for the focus (`on_focus`) or the terminal keeps taking the keys.
  - `session_options.rs`: Session Options, a window of its own (`window::open`, drawn in `App::view_window`, `header_bar` with drag / close); `Target::{New, Edit, Default}`; fields checked by `core::session::form`; the app saves (it owns the store). One at a time, a second call raises it. «Новая сессия…» opens it, not Quick Connect.
  - A session saved, renamed or moved: `App::sessions_changed` rebuilds the `Launch` of the tabs opened from it (`store::retarget`), so Reconnect / Clone / Session Options follow it.
  - Key and pointer subscriptions are tagged with their window (`Message::Input`) and acted on only from the main one: keys typed in Session Options never reach the terminal or the tree.
  - App keys (`key_action`) are handled whatever the event status: a text field would take Alt+Q as a letter. Enter and the tree's keys only when no widget took them.
  - Popups (menus) are windows: `App::view_window` returns an empty `Space` for them, or libcosmic panics ("no view for window"). A `segmented_button` context menu is one root whose children are one menu per tab.
  - Script recording (`core::recorder::Recorder`): `term::log::Tap` feeds it the output (the prompt) and `Terminal::input_no_scroll` the user's keys; Stop gives the `crt` script text (`Tabs::toggle_recording`, the save dialog in `App::save_recording`); passwords become `crt.Dialog.Prompt`. Spec `specs/21-script-recording.md`.
  - Tab windows (`Tabs.detached`, spec `specs/22-tab-window.md`): a tab can live in a window of its own (`TabCmd::Detach`); `Tabs.front` is set while an event of such a window is handled (`App::handle` for `Message::Input(w, ..)`) so `Tabs::active_id` — what every key and menu action acts on — is that tab; closing the window brings the tab back.
  - `updates.rs`: the update window (`core::update`: GitHub `releases/latest` by `curl`, `SHAGSHELL_UPDATE_URL` replaces the address for tests) with Update (opens the release page) / Skip (`Config.skipped_version`) and the donate button (`DONATE_URL`, Stripe Payment Link); spec `specs/20-update-donate.md`.
  - `dialog_icons.rs`: the full-colour icons of our dialogs (`res/icons/dialog/*.svg` embedded: info, question, warning, stop, success, lock, key, keyboard, server, sync, import, bookmark, folder, edit); a dialog gets one with `.icon(Kind::X.icon(48))`, not a theme's grey `dialog-*-symbolic`.
  - `script.rs`: Script → Run… / Cancel and `--script file args…`; a run = a `python3` child + its socket + a supervising thread; calls about tabs are answered on the UI thread (`answer`), dialogs are queued like askpass prompts; the scripts' threads reach a tab through `Tabs::taps` (tab id → `term::log::Tap`).
  - Script Console (`Scripts.console_win`, `core::script::Console` ring of lines, spec `specs/26-script-console.md`): the child's stdout / stderr are pipes pumped into the console and `script.log`; debug mode (`Host::debug` / `trace`, `trace_line`) writes every `crt` call into the console only (never the file).
  - `scheduler.rs` / `core::schedule` (spec `specs/30-scheduler.md`): Script → Scheduler…, jobs in `schedule.toml`; `schedule::due(job, last, now)` is the whole clock, the app ticks every 15 s and runs a job like a pre-script (no tab).
  - `askpass.rs`: the socket (listener thread → subscription), the prompt queue and its dialog; keyring calls go through `App::ask_outcome` off the UI thread (`off_ui`). Prompts of a closed / ended tab are cancelled (`prune`). Secrets in messages are `Hidden` (redacted `Debug`).
  - `sftp_view.rs` / `sftp_worker.rs`: the SFTP tab (a `tabs::Body::Sftp`), a commander: local | server panels, F5–F8, Tab, Insert, the queue under them; its keys are routed by the app (`Message::Key`) when it is the active tab. Two workers per tab (browse, transfer: listing never waits for a copy), each a thread with its own ssh; events tagged by worker (a replaced one's are dropped). Local folders are read on the UI thread (fast), the server's in the worker. `Launch::sftp` makes the SFTP launch from a terminal's ssh args.
  - `credentials_window.rs`: Tools → Credentials Manager, a window like Session Options; the app saves (file, sessions relinked, keyring rename then store — in that order).
  - Blocking work (keyring, D-Bus) goes through `app::off_ui`: started when the task runs, so `Task::chain` orders it (a `spawn_blocking` made eagerly would race).
  - `main.rs` also dispatches the askpass helper and `--check`.
- Modularity = Rust modules. No plugin system.

### UI (SecureCRT layout)

- Menu: File, Edit, View, Options, Transfer, Script, Tools, Window, Help. Toolbar: Connect, Quick Connect, Reconnect, Disconnect, SFTP, Session Options.
- Left: Session Manager — folder tree with filter; double-click connects; drag between folders; multi-select "Connect all".
- Center: tabs inside a `pane_grid` (tab groups, as in cosmic-term); status icon per tab; tab menu: Clone, Reconnect, Connect SFTP Tab, Rename, Close.
- Bottom: Command Window (send to current / all / selected tabs, history), Button Bar, status bar (state, protocol, rows×cols).
- Dialogs: Quick Connect (Alt+Q), Connect (Alt+C), Session Options, Global Options (context drawer), askpass, host key.
- SFTP tab: dual pane (local | remote) + transfer queue at the bottom.

## Rules

- Logic in `core`, test-first; `update()` arms stay thin (map message → call core → render).
- Every UI string through `fl!`, with both `en` and `ru`.
- libcosmic pinned by rev `ef490df50b0a05a21c494c3f75737581bf0b39d9` (see the `cosmic-applet` skill for known traps).
- Never log or write secrets; strip passwords from anything saved.
- Every bug: a `regression_<what>` test in core and a row in `TESTING.md`.

## Workflow

brainstorm → spec `.claude/docs/specs/NN-<topic>.md` → plan `.claude/docs/plans/NN-<topic>.md` → branch `feat/<topic>` / `fix/<topic>` → TDD → `just verify` + headless check → `/code-review` → PR → merge only after the user approves → tag.

## Versions

Root `Cargo.toml` `version`: a feature bumps minor, a fix bumps patch. Tag `vX.Y.Z` on `main` after the merge.
