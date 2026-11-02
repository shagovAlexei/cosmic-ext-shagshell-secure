# 42. Session Options → Terminal → Emulation

## Why

SecureCRT: Session Options → Terminal → Emulation sets what the server is told (`TERM`), the colour depth,
the scrollback and the window-resize behaviour. Here every tab gets `TERM=xterm-256color` and 10 000 lines.
Old network gear (Cisco, MikroTik, switches) often wants `vt100` / `ansi`; big logs want more history.

## What the user gets (first step)

Session Options (and `Default.toml`) → **Terminal → Emulation**:
- **Terminal**: Xterm / VT100 / VT220 / Linux / ANSI → the `TERM` value (`xterm-256color`, `vt100`, `vt220`,
  `linux`, `ansi`). The drawing stays alacritty's; no separate emulators.
- **Colours**: Monochrome / 16 / 256 / True colour → what `TERM` / `COLORTERM` say:
  mono → `TERM` with no `-color` part where it exists (`vt100`), `COLORTERM` unset; 16 → `xterm-16color`;
  256 → `xterm-256color`; true → `xterm-256color` + `COLORTERM=truecolor`. For VT100/VT220/Linux/ANSI the colour
  choice only toggles `COLORTERM`; their `TERM` is fixed.
- **Scrollback**: lines (default 10 000, 100–1 000 000).
- Empty / default session: exactly today's behaviour (`xterm-256color`, 10 000).

Applies when the tab is opened (a running tab keeps its `TERM` and scrollback).

## Later (not in this step)

Rows × columns and «on window change» (fit / scale font / keep), Modes (Backspace = ^H / ^?, Alt as Meta,
Delete), Emacs / Mapped Keys. Each is its own item; the roadmap line stays open for them.

## How

`core::session::Session` gets `term: Term` (`#[serde(skip_serializing_if default)]`): `kind` (enum, default
Xterm), `colors` (enum, default 256), `scrollback: Option<u32>`.
`core::term_env`: `env(kind, colors) -> Vec<(String,String)>` (TERM, COLORTERM) and `scrollback(opt) -> usize`
(clamped). Unit tests: each kind × colour, clamp, default = today's values, TOML round trip with unknown keys kept.
App: `term::start` takes the env pairs and history instead of the hard-coded ones (`set_cursor_shape` reuses the
tab's history, not `HISTORY`); a «Emulation» block in the Session Options Terminal page; `fl!` en + ru.
Telnet / serial sessions get the same env; local shell too.

## Not doing

Per-kind key mappings, real VT220/ANSI feature limits, Wyse/TN3270.
