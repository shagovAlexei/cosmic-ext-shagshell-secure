# 43. Session Options → Terminal → Emulation → Mapped Keys

## Why

SecureCRT: a session (and `Default`) has its own key map — key → send string / menu function / script —
which wins over the global one (Tools → Keymap Editor). Here the Keymap Editor is global only. Network gear
wants different keys per device (a Cisco `Ctrl+Alt+X` = `exit\r`, a MikroTik one = `/quit\r`).

## What the user gets

Session Options (and `Default.toml`) → Terminal → Emulation → **Mapped Keys…** opens the same editor as
Tools → Keymap Editor, holding only this session's keys (no «Built-in keys» block).

- In a tab of that session its keys are looked up first, then the global ones. The same chord in both: the
  session's wins.
- The chord rules are the global ones (`Chord::check`: no plain / Ctrl / Ctrl+Shift letters, no Ctrl+Insert /
  Shift+Insert); a chord held by a built-in key is refused.
- Other tabs and other sessions are unaffected. A new session copies `Default`'s keys (like every field).
- Takes effect for the active tab at once, no reconnect.

## How

Core: `term_env::Term` gets `keys: Vec<keymap::Binding>` (`#[serde(rename = "key", skip_serializing_if =
"Vec::is_empty")]`). `Keymap::sanitized(&[Binding]) -> Vec<Binding>` (the filter `set_bindings` already does:
drop what `check` refuses and repeats) is reused for both lists. Tests: TOML round trip with `[[term.key]]`,
refused / duplicate chords dropped, empty = today's file byte for byte.

App: `keymap_editor` gets a second static `SESSION` (`set_session(&[Binding])`) next to `BINDINGS`;
`lookup` checks it first and returns `SESSION_BASE + i` (`0x8000`), `binding(i)` reads the right list.
The app sets it whenever the active tab (or its session) changes — one call after `update()` when
`tabs.active_session()` differs from the last one, reading that session's `term.keys`. `Editor::new` takes the
`Keymap` it edits; the session variant has no built-ins and saves into the Session Options draft instead of
`keymap.toml`. `fl!` en + ru for the button and the title.

## Not doing

Keys per tab (not per session), key-up/-down events, mapping the keypad in VT application mode, a
SecureCRT `.ckm` importer (the SecureCRT importer may read it later).
