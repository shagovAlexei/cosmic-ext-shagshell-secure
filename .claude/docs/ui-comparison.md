# UI: SecureCRT 9.7 vs ShagShell Secure

Sources: SecureCRT 9.7.1 under wine (help `SecureCRT.chm`, `Menus/default.mnu`, icons from `SecureCRT.exe` / `ConnectDialog97U.dll`, `Global.ini`), vandyke.com. Our side: headless screenshots of 0.47.0 (2026-10-25).

Legend: ✅ have · 🟡 partly · ❌ missing · — not planned.

## Themes and look

| SecureCRT | ShagShell | Note |
|---|---|---|
| View → Themes (System, Native, Carbon, VS Blue/Dark/White, Office White, legacy) | 🟡 Global Options: System / Dark / Light | COSMIC draws the chrome; a theme = light/dark + accent. Enough for us. |
| Terminal colours = session colour scheme, «System Color Scheme» follows light/dark | ✅ colour schemes global + per session; the default one follows light / dark | |
| Flat monochrome toolbar glyphs 16/24/32 | ✅ symbolic icons | |
| Icons in menus | ✅ 0.50.0 | submenus (Scripts, Recent) have none: libcosmic folders take no icon |
| Window transparency (active / inactive) | — | iced on Wayland sets no window opacity |
| Zoom Ctrl +/−/0 | ✅ 0.53.0 (per tab) | |
| Cursor style (block / underline / bar), colour, blink | ✅ shape (0.53.0), blink (0.64.0, off by default) | colour = the scheme's |
| Full screen Alt+Enter, Always on Top, hide menu bar | 🟡 full screen F11 (0.53.0) | |

## Session tabs

| SecureCRT | ShagShell | Note |
|---|---|---|
| Status icons: Connected, Disconnected, **New Input**, **Inactivity** (N s), Locked overlay, script running | ✅ coloured icons (0.48.0), inactivity (0.56.0, fixed 5 s) | |
| Status as icon / background colour / colour rectangle | ❌ | |
| Tab text = session name, label template `%H %S %U %I %P` | ✅ session name (0.48.0), label template `%S %H %U %P` (0.57.0) | |
| «(n)» on a second tab of the same session | ❌ | |
| Tab width: fit / fixed / divide evenly, min width, tooltip | 🟡 divide evenly only | |
| Tabs at top or bottom; hide when only one | ✅ bottom (0.55.0), hidden when one (0.60.0) | |
| Close button on tabs | ✅ | |
| Double-click action (close / clone / reconnect / new window) | ✅ 0.55.0 | |
| Alt+1…0 to tab N; Ctrl+Tab most recent | ✅ Alt+1…9, Ctrl+Tab next / previous or most recent (0.63.0) | |
| Tab groups (drag to an edge), Tile / Cascade | ✅ groups (menu), drag between groups needs a live check | |
| Send to new window | ✅ Open in a New Window | |
| `*` before the name of a tab that gets Command Window input | ✅ 0.60.0 | |

## Session Manager

| SecureCRT | ShagShell | Note |
|---|---|---|
| Dockable pane, pin / auto-hide | 🟡 left / right side, collapsible | |
| Toolbar: Connect, Quick Connect, New, Cut/Copy/Paste, Delete, Properties, Find, New Folder | 🟡 New, New Folder, Properties, Delete, Connect; Duplicate (0.59.0) | no cut / copy / paste (Duplicate + drag covers it) |
| Filter box | ✅ | |
| Icon by protocol (SSH: terminal+lock, Telnet: terminal, Serial: phone, Local: house), folder variants | ✅ by protocol (0.49.0), folders: system icons | |

## Toolbar, status bar, other

| SecureCRT | ShagShell | Note |
|---|---|---|
| Toolbar: Session Manager, Quick Connect, Local Shell, Reconnect, Disconnect, Connect Bar, Copy, Paste, Find, Session Options, Keymap, Public-Key, SecureFX | ✅ pick from 16 buttons (0.61.0) | no drag to reorder |
| Connect Bar (Alt+R: `ssh2://user@host`, filtered session list) | ✅ toolbar field (0.58.0) with its list (0.62.0) | no Alt+R |
| Status bar: message + up to 5 columns (protocol+cipher, cursor, rows×cols, emulation, elapsed time, local time), log file, CAPS/NUM | 🟡 state · protocol · cols×rows · cursor · time up · log file (0.52.0) | no cipher, no local time, not configurable |
| Command Window: active / all / groups / visible, history, multi-line | ✅ | |
| Button Bar: several bars, coloured dot icons on buttons | ✅ colours (0.54.0) | |
| Command Manager | ✅ | |
| Title bar template | 🟡 «tab — ShagShell Secure» (0.60.0) | not configurable |
| Activator (tray) | — | |

## Found on our screenshots

1. Tab status = text marks, all ssh tabs titled `user@host:~` (can't tell them apart).
2. Menus: no icons; «Log Session» / «Record Script» indented, their neighbours not; Copy / Paste / Cancel enabled with nothing to act on.
3. SFTP: folders as `[name]`, no file / folder icons; in a narrow group the name column disappears, dates overlap, F-key buttons are cut. — fixed in 0.51.0.
4. Session Manager: one grey icon for every session and protocol.

## Order of work

1. **Tabs**: coloured status icons (connecting, connected, disconnected, new output, inactivity, locked), protocol icon, session name instead of the program title (label template `%S %H %U`), «(n)» for doubles.
2. **Session Manager**: icons by protocol, folders.
3. **Menus**: icons, even indents, disabled when nothing to act on.
4. **SFTP**: file / folder icons, narrow layout.
5. **Status bar**: elapsed time, cursor position, log file.
6. Cursor style, zoom, full screen.
7. Button colours on the Button Bar, tabs at bottom, double-click action, Alt+N.
