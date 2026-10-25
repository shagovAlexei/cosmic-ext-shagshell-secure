# 33. Tab icons

SecureCRT shows each tab's state as a coloured icon (Connected, Disconnected, New Input, Inactivity, Locked); ours were text marks (`✓ ✗ 🔒 ✎ »`) and ssh tabs were all titled `user@host:~`.

## What
- Each tab has a 16 px icon: a glyph for what it is, in the colour of its state.
  - Glyph: SSH = screen `>_` with a padlock, Telnet = screen, Serial = connector, Local shell = house, SFTP = folder, Note = page, Script Console = screen.
  - State: amber = connecting (started, nothing printed yet), green = connected, blue = new output while not in front, red = disconnected, purple (with a padlock) = locked by a script, grey = note / console.
- «New output» clears when the tab comes in front (its group's active tab, or its own window), also when the app brings it (askpass, `crt.GetTab(n).Activate()`).
- SFTP: amber while its ssh connects (or asks a password), red once it gave up.
- Title: the session's label, else for a running local shell the program's title, else ours (the session name, «(2)» when opened twice). A session tab no longer shows the `user@host:~` its shell sets.

## How
- `core::tab_icon`: `state(alive, seen, unread, locked)`, `colour`, `svg(glyph, state)` (full-colour SVG, so any icon theme).
- `term::Outcome::Output` on alacritty `Wakeup`; `Tab.seen` / `Tab.unread`; `Tabs::retitle` sets text and icon; `Tabs::mark_read` after every tab message / action.

## Not now
- Connected is set by the first output, so an ssh warning before the login turns it green early.
- A tab in a window of its own counts as in front even when that window is covered.
- Inactivity (output stopped for N s), status as background colour or colour rectangle, label templates `%H %S %U`.
