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

## Commands (from phase 1)

```bash
just verify          # fmt --check + clippy -D warnings + test — must pass before every PR
just build-debug / build-release
just run-logs        # RUST_LOG=debug cargo run
sudo just install    # binary, .desktop, metainfo, icon into /usr (rootdir=… to stage)
just deb             # target/deb/*.deb; tag vX.Y.Z → GitHub release with it
shagshell --check    # probes ssh, telnet, picocom, python3, keyring, askpass socket: ok / MISSING
```

## Architecture

### Key decision: every connection is a command in a local PTY

We do **not** implement SSH. The system OpenSSH does it; we drive it.

- SSH = `ssh` spawned in the PTY of the embedded terminal, with:
  - `-o ControlMaster=auto -o ControlPath=$XDG_RUNTIME_DIR/shagshell/%C -o ControlPersist=60` (so SFTP and clones reuse the connection);
  - `-p`, `-l`, `-i`, `-J <jump>`, `-L/-R/-D`, `-o ServerAliveInterval=N` built from the session.
  - env: `SSH_ASKPASS=<path to shagshell>`, `SSH_ASKPASS_REQUIRE=force`, `SHAGSHELL_SESSION=<session id>`.
- Telnet = `telnet host port`; Serial = `picocom -b <baud> <dev>`; Local shell = `$SHELL`.
- **Askpass:** `shagshell askpass "<prompt>"` connects to the running app's socket `$XDG_RUNTIME_DIR/shagshell/askpass.sock`. The app classifies the prompt (password / passphrase / host-key yes-no / other), answers from the keyring or shows a dialog in the main window ("Save password" checkbox), and the helper prints the answer to stdout.
- **SFTP** = `openssh` crate (native-mux) + `openssh-sftp-client` over the session's ControlMaster.
- Why not russh: own known_hosts / ssh_config parsing, known russh bugs (one stalled channel freezes the connection, SFTP breaks on non-UTF-8 names), API churn. Switching later stays cheap: the terminal does not care where bytes come from. Do **not** add an "SSH backend" trait in advance.

### Button Bar and scripting (SecureCRT style)

- **Button Bar:** several named bars, a session may pick its own; a button = Send String (escapes), Run Script, Menu Function (same `Action` enum as key bindings) or Launch App; target current / all tabs. Stored in `~/.config/shagshell/buttonbars.toml`.
- **Python scripts:** a separate `python3` process (not PyO3: a crash can't take the app down, no libpython dependency, user's pip packages work). We ship `crt.py` (`/usr/share/shagshell/python/`), started with `PYTHONPATH` to it and `SHAGSHELL_SCRIPT_SOCK=$XDG_RUNTIME_DIR/shagshell/script-<n>.sock`; every `crt.*` call is a JSON line over that socket. The `crt` object mirrors SecureCRT's model (`crt.Screen.Send / WaitForString / ReadString`, `crt.Session.Connect`, `crt.Dialog.MessageBox / Prompt`, `crt.GetTab(n)`, `crt.Sleep`, `crt.Arguments`) so SecureCRT Python scripts port with minimal edits. Waits run app-side on the tab's output stream. Run from Script menu, Button Bar, a key, a session's logon script, or `shagshell --script file.py`.

### Storage

- Sessions: `~/.config/shagshell/sessions/<folder>/…/<name>.toml`, one file per session; the folder tree is the Session Manager tree.
- `sessions/Default.toml` is the template: a new session copies it at creation (SecureCRT behaviour, no live inheritance).
- Credentials: `~/.config/shagshell/credentials.toml` holds named sets (username only); sessions reference a set by name.
- **Secrets only in Secret Service** (gnome-keyring), never in files: attributes `app=io.github.shagovalexei.ShagShellSecure`, `kind=session|credential`, `id=<session path or credential name>`. No master password (deferred).
- Keys: `~/.ssh` + ssh-agent (gnome-keyring); generation through `ssh-keygen`.
- Global options: cosmic-config (`~/.config/cosmic/<APP_ID>/v1/`). Window state / open tabs: `~/.local/state/…`.

### Workspace

- **`crates/core` (`shagshell-core`)** has no libcosmic dependency. All logic lives here and is unit-tested:
  - `session/{model,store}.rs` — `Session`, folder tree, load/save/rename/move, Default template;
  - `launch.rs` — `Session` → argv + env (pure, tested against expected argv);
  - `askpass.rs` — prompt classification + socket protocol;
  - `keyring.rs` — Secret Service over zbus (from shagoff-commander, + `lookup` / `delete`);
  - `credentials.rs`;
  - `sftp/{client,queue}.rs` — the transfer queue is a pure state machine;
  - `logfile.rs` — log name templates `%H %S %Y %M %D %h %m %s %P %U`;
  - `macros.rs` — Button Bar / Command Window escapes (`\r \n \t \e \\ \p`);
  - `buttonbar.rs` — named bars, buttons (Send String / Run Script / Menu Function / Launch App), `buttonbars.toml`;
  - `script.rs` — the `crt` JSON-lines socket protocol for Python scripts.
- **`crates/app` (`shagshell`)** is the libcosmic UI, **split by feature**: each module (`session_manager`, `tabs`, `term/`, `sftp_view`, `command_window`, `button_bar`, `dialogs`, `menu`, `keymap`, `config`, `i18n`) owns its own `Message`, `update` and `view`. `app.rs` only routes. No growing a single 15k-line `app.rs` like shagoff-commander did.
  - `term/` is cosmic-term's terminal (via shagoff-commander `crates/app/src/term/`), driven by a local PTY.
  - `main.rs` also dispatches the `askpass` subcommand and `--check`.
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
