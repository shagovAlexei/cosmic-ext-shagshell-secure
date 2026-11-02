# 39. Active Sessions panel

## Why

SecureCRT's left dock has two vertical tabs: Session Manager and Active Sessions (every open
tab, with a filter). Here the open tabs are visible only in the tab strip, which gets long.

## What the user gets

- Inside the Session Manager pane, a two-button switch at the top: **Session Manager** /
  **Active Sessions**. Not a third pane: the pane system (side, fold, strip, stacking, widths) is
  built for two, a third would multiply the config. The choice is remembered (`Config`).
- Active Sessions: a filter box (by tab title), then two folders: **Sessions (N)** (terminals,
  SFTP) and **Editors (N)** (notes, Script Builder, console). Each row: the tab's state icon
  (as on the tab, `core::tab_icon`) and its title.
- Click: bring that tab to the front (its group, its window). A × on each row closes that tab
  (asks first if live). Right click and the tab menu are not here: libcosmic's tab menu belongs
  to the tab bar widget.
- Button under the list: Close all tabs (the same «live tabs» confirmation as «Close others»).

## How

`core::active_tabs`: `group(&[Row{id, kind, title}], filter) -> Vec<Section>` (case-insensitive
filter, sections «Sessions» / «Editors», empty ones dropped), unit-tested. App: `active_sessions.rs`
owns `view()` and `ActiveMsg`; `Tabs::active_rows()` lists the tabs, `show` / `close_tab` /
`close_all` act on them. `session_manager.rs` is untouched: the page switch is `Config::active_sessions`.

## Not doing (yet)

Multi-select with Shift / Ctrl, group buttons (open / new / close group: they live in the Session
Manager), grouping by session group.
