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

## Architecture (summary)

Details per module: `.claude/docs/modules.md` — read the section of the module you touch, not the whole file.

- **Every connection is a command in a local PTY.** We do not implement SSH: system `ssh` (ControlMaster under `$XDG_RUNTIME_DIR/shagshell/%C`), `telnet`, `picocom`, `$SHELL`. Passwords via askpass = our own binary talking to the app's socket. No russh, no "SSH backend" trait.
- **SFTP** = our own SFTP v3 client (`core::sftp::proto`) on `ssh -s … sftp`; names are bytes.
- **Scripts** = a separate `python3` with the built-in `crt.py` (SecureCRT-like `crt` object), JSON lines over a socket. Not PyO3.
- **Storage:** sessions `~/.config/shagshell/sessions/<folder>/<name>.toml` (`Default.toml` = template, copied at creation); secrets only in Secret Service, never in files; global options in cosmic-config.
- **`crates/core` (`shagshell-core`)**: all logic, no libcosmic, unit-tested.
- **`crates/app` (`shagshell`)**: libcosmic UI split by feature, each module owns its `Message` / `update` / `view`; `app.rs` only routes. Blocking work through `app::off_ui`. Never bind Ctrl+Insert / Shift+Insert.
- UI follows SecureCRT's layout (menu, toolbar, Session Manager left, tabs in a `pane_grid`, Command Window / Button Bar / status bar at the bottom).

## Rules

- Logic in `core`, test-first; `update()` arms stay thin (map message → call core → render).
- Every UI string through `fl!`, with both `en` and `ru`.
- libcosmic pinned by rev `ef490df50b0a05a21c494c3f75737581bf0b39d9` (see the `cosmic-applet` skill for known traps).
- Never log or write secrets; strip passwords from anything saved.
- Every bug: a `regression_<what>` test in core and a row in `TESTING.md`.

## Workflow

Through the `shagshell-feature` skill: talk it through → spec `.claude/docs/specs/NN-<topic>.md` (for big features) → branch `feat/<topic>` / `fix/<topic>` → test-first → `just verify` + headless check for UI → `/code-review` (low / medium) → PR → merge → tag.

Saving tokens: read only the part of a file you need (`grep`, `offset`/`limit`; never all of ROADMAP.md or `app.rs`); long build / test output to a file, read its tail; no subagents unless the user asks; screenshots only for UI changes. One task per session: after merge + tag, suggest `/clear`.

## Versions

Root `Cargo.toml` `version`: a feature bumps minor, a fix bumps patch.  Every PR adds its entry to `CHANGELOG.md` (releases beyond the last 5 are pruned from GitHub; their tags stay, so `git checkout vX.Y.Z && just deb` rebuilds any version). Workflow actions are pinned by commit SHA (Dependabot proposes newer ones). Tag `vX.Y.Z` on `main` after the merge: CI makes a **draft** release; the author publishes it with `just sign vX.Y.Z` (minisign, the key only on their machine; the app refuses an update whose `SHA256SUMS.minisig` does not check against `core::update::PUBLIC_KEY`). Tell the user to run it after each tag.
