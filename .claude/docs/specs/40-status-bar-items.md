# 40. Status bar items

## Why

SecureCRT: Terminal → Appearance → Status Bar lets you pick which items the bar shows and in what
order. Here the bar is a fixed string (state · protocol · cols×rows · cursor · time up · log).

## What the user gets

- Global Options → Terminal → Appearance → **Status bar**: two lists, «Selected» and «Available»,
  buttons ▲ ▼ (reorder selected), → ← (move between lists). Reset to default.
- Items: Connection state, Protocol, Size (cols×rows), Cursor position, Connected time,
  Local time, Log file, Recording, Caps Lock, Num Lock. Default = today's set, in today's order.
- Checkbox **Show seconds** (connected time and local time).
- **Align**: left (as now) / right. The notice text («Script finished: …») follows the alignment.
- Applies at once to the open window; global, not per session.

## How

`core::status_items`: `enum Item` (stable string ids), `Layout { items: Vec<Item>, seconds: bool,
right: bool }` with `Default`, `move_up/down`, `add`, `remove`, `available()`; serde into `Config`
(unknown ids dropped on load, duplicates ignored). `status_bar::view` takes the layout and renders
`Item → Option<String>`, skipping the ones with nothing to say. `Status` gains `local time`; Caps/Num
lock come from the keyboard state libcosmic reports. Options dialog: a new section in `options`
with its own `Message`; `app.rs` only routes.

## Open question

Caps/Num lock: iced gives the lock state only with key events, not on demand. Default: track it
from the last key event (shows nothing until the first key). If that is wrong in practice, drop the
two items.

## Not doing

Per-session layout, colours per item, clickable items.
