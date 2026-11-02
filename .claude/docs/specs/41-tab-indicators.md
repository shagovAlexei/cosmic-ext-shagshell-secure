# 41. Tab look: state indicators

## Why

SecureCRT: Terminal → Tabs/Tiling lets you choose how a tab shows its state and which states show.
Here the state is a coloured glyph with fixed colours (`core::tab_icon::colour`) and always the same states.

## What the user gets

Global Options → **Tabs** (new section):
- **Indicator**: icons (as now) / neutral icon (grey, no state) — "none".
- **Colours** of the six states (connecting, connected, new output, idle, disconnected, script/locked):
  each a colour swatch with a hex field, «Default» resets one, reset-all button.
- Checkboxes: **Show «new output»**, **Show «idle» after N seconds** (N: 5 by default, 1–600),
  **Show «script running/locked»**. A hidden state falls back to «connected».
- **Tab width**: fit to label (as now) / fixed N px (80–400); a long name is cut at the end.

## Not possible here (decided)

SecureCRT's *background fill* and *coloured left stripe*: libcosmic's `tab_bar` cannot style a single
tab (one style for all). Doing it means our own tab widget; not now. The icon colour is the indicator.

## How

`core::tab_icon`: `Palette { connecting, connected, output, quiet, disconnected, locked: String }`
(hex, validated, bad value → default), `Options { indicator, show_output, show_quiet, quiet_secs,
show_locked }`; `colour(State, &Palette)` and `state(...)` take the options (a hidden state maps to
`Connected`); `svg(g, s, &Palette)`. Unit tests: palette fallback, hidden states, `quiet_secs`.
App: `Config` gets `tab_look: tab_icon::Look` (palette + options + width); `Tab::icon` and `tick`
read it; `options.rs` gets the section (colour = hex text field + swatch; no new colour picker);
`maximum_button_width` / `width(Fixed)` from the width option.

## Not doing

Per-session look, per-state glyph shapes, gradient/animation.
