# Changelog

Every release, newest first, rebuilt from the merged pull requests. GitHub keeps only the last 5 releases (see `release.yml`), so this file is the permanent history. Versions follow `Cargo.toml`: a feature bumps minor, a fix bumps patch. Add an entry for each PR when it is merged.

## 0.77.0 — 2026-10-25

- feat: Lock Session (SecureCRT) in every tab's menu: a password typed twice locks the tab — a terminal drops everything typed, pasted or sent to it, a note becomes read only (moving, selecting, copying); «Unlock Session…» with the same password frees it. The password lives in memory only; the tab icon shows the lock
- feat: a note tab's right-click menu has the editor's commands, as SecureCRT's scratchpad: Save, Save As…, Word Wrap, Line Numbers, Rename, Lock Session…

## 0.76.0 — 2026-10-25

- feat: notes are edited in COSMIC Edit's editor (its text box, adapted): syntax highlighting by the file's extension (Markdown, code, configs…), line numbers, the current line highlighted, COSMIC Dark / Light themes; Ctrl+C / X / V / A, Ctrl+Z, Ctrl+Shift+Z (or Ctrl+Y)
- the formatted `.md` view (eye button) is gone

## 0.75.0 — 2026-10-25

- feat: the program's own keys (Alt+M / Q / C / P, Ctrl+Shift+T / W / C / V / F / R, Ctrl+Tab, Ctrl+Shift+Tab, Ctrl+= / − / 0, F11) can be given another key: Tools → Keymap Editor lists them under «Built-in keys»; Edit… records a new chord, «Default key» puts the old one back. Stored in `keymap.toml` (`[builtin]`); the menus and the Connect Bar's hint show the key in force
- feat: Find, Connect Bar, Copy, Paste, Next / Previous Tab are menu functions for Button Bar buttons and own keys
- Alt+1…9 (tab N) stay fixed; a user key may not take a built-in key's chord

## 0.74.1 — 2026-10-25

- fix: Notes: the View / Edit button had no icon in the Cosmic icon theme (it lacks the Adwaita names); now uses icons the theme has

## 0.74.0 — 2026-10-25

- feat: View → Connect Bar shows / hides the toolbar's Connect Bar; Ctrl+Shift+R shows a hidden one and puts the cursor in it

## 0.73.0 — 2026-10-25

- feat: the Connect Bar is an item of the toolbar: the toolbar editor (right click on the bar) moves or removes it like a button
- feat: Ctrl+Shift+R (File → Connect Bar) puts the cursor in the Connect Bar; its hint shows the key

## 0.72.0 — 2026-10-25

- feat: Global Options: «Tabs: Top / Bottom» list instead of the «Tabs at the bottom» switch (top by default)
- feat: full screen (F11) hides the menu bar too; F11 again brings it back

## 0.71.0 — 2026-10-25

- feat: an icon for each action (send string, menu function, program, script) in the Keymap Editor's list and in the «Action» lists of the key and Button Bar editors

## 0.70.5 — 2026-10-25

- feat: Key Map: a «Run script» key has a Browse… button for the script file

## 0.70.4 — 2026-10-25

- fix: Button Bar: a button's label is centred vertically

## 0.70.3 — 2026-10-25

- fix: Scheduler list: the selected job's name was unreadable when the job was off (dimmed text on the accent colour); the toggler label has a gap

## 0.70.2 — 2026-10-25

- fix: the result of Check for Updates made in the About dialog (latest / error) shows in that dialog, not in the status bar

## 0.70.1 — 2026-10-25

- fix: Help → About is a dialog, not a side drawer (a drawer is at least 344 px wide and covered a narrow window)

## 0.70.0 — 2026-10-25

- feat: a side's Session / Command Manager folds into a thin strip of buttons (the « / » button in a pane's header); a click on a strip button unfolds it

## 0.69.0 — 2026-10-25

- feat: tabs are as wide as their text (SecureCRT's «fit to label», at most 240 px), not stretched over the whole bar

## 0.68.0 — 2026-10-25

- feat: a Button Bar button has a «Hint» field: shown when the pointer rests on the button
- feat: 24 more session icons (laptop, phone, printer, camera, Wi-Fi, storage, board / IoT, UPS, antenna, web, mail, VPN, key, service, monitoring, code, deploy, office, tools, lab, debugging, virtual machine, star, flag)

## 0.67.0 — 2026-10-25

- feat: the window opens maximized (Global Options, on by default)
- feat: «Open the last session at startup» brings back the tabs (saved sessions and local shells, in their order) and the notes; one option instead of two
- feat: the side panes reach down to the Button Bar, the Command Window and the status bar sit under the tabs
- feat: About: buttons to the latest release and to the issues instead of links
- fix: the tabs of the last time were never reopened at startup (they were opened before the terminals' channel was up)

## 0.66.0 — 2026-10-25

- feat: the toolbar is edited like shagoff-commander's button bar: buttons and gaps in any order, an app function or a program with `%H %S %U %P`, any icon of the theme (picker), its own hint; right click on the toolbar or Global Options → Toolbar

## 0.65.0 — 2026-10-25

- feat: Edit → Find (Ctrl+Shift+F) in the active terminal: Enter / ↓ next, ↑ previous, Esc closes
- feat: a session can pick its icon (server, router, switch, firewall, database, cloud, desktop, container…), shown in the tree and on its tab
- feat: the Connect Bar's list is walked with ↑ / ↓, Enter opens the row, Esc closes it
- feat: the Button Bar colour list shows each colour's dot
- fix: a long Button Bar label stays on one line, cut on the right; «Browse…» lines up with its field; clearer auto-reconnect labels; a thinner gap between the stacked side panes

## 0.64.1 — 2026-10-25

- fix: dragging a tab to another group moves the dragged tab (it moved the tab clicked last, or nothing on a second drag): the dragged one is now taken from libcosmic's drop hints

## 0.64.0 — 2026-10-25

- feat: Global Options → Blinking cursor (off by default: it redraws the terminal twice a second)

## 0.63.0 — 2026-10-25

- feat: Global Options → Ctrl+Tab goes back to the tab used before (SecureCRT's option); off: the next tab, as before

## 0.62.0 — 2026-10-25

- feat: the Connect Bar lists the sessions matching what is typed; a click opens one

## 0.61.0 — 2026-10-25

- feat: Global Options → Toolbar: pick the toolbar's buttons (Session Manager, Local Shell, Copy, Paste, Command Window, Run Script, Keymap, Create Key, Notes, Global Options besides the usual ones)

## 0.60.0 — 2026-10-25

- feat: the window title names the tab in front; tabs the Command Window sends to (All / Visible) are marked `*`; Global Options → hide the tab bar while there is one tab

## 0.59.0 — 2026-10-25

- feat: Session Manager → Duplicate: a copy of the session beside it, «name (2)» (its saved password is not copied)

## 0.58.0 — 2026-10-25

- feat: Connect Bar in the toolbar: a session's name (or part of it, `/S name`) or `[ssh2://][user@]host[:port]`, Enter connects
- fix: text typed in the Session Manager or Command Manager filter no longer also goes to the active terminal

## 0.57.0 — 2026-10-25

- feat: a session's tab label may use `%S` (session), `%H` (host), `%U` (user), `%P` (port), like SecureCRT's label

## 0.56.0 — 2026-10-25

- feat: a background tab whose new output stopped 5 s ago turns teal (SecureCRT's Inactivity: the command is likely done)

## 0.55.0 — 2026-10-25

- feat: Global Options → tabs at the bottom, and what a double-click on a tab does (nothing / close / clone / reconnect / its own window)

## 0.54.0 — 2026-10-25

- feat: Button Bar buttons can have a colour (SecureCRT's 11 dots), picked in the button's window and kept in `buttonbars.toml`

## 0.53.0 — 2026-10-25

- feat: zoom the active terminal (Ctrl+= / Ctrl+− / Ctrl+0, View menu), full screen (F11), cursor shape in Global Options (block / underline / bar)

## 0.52.0 — 2026-10-25

- feat: the status bar also shows the cursor position, how long the tab has been connected and the log file's name while logging

## 0.51.0 — 2026-10-25

- feat: SFTP panels show folder / file icons instead of `[name]`; a narrow panel drops the rights, then the date, then the size column; the F-key buttons wrap instead of being cut

## 0.50.0 — 2026-10-25

- feat: menu items have icons; Script → Cancel is enabled only while a script runs; the View → Toolbar / Status Bar placeholders are gone

## 0.49.0 — 2026-10-25

- feat: Session Manager shows each session with its protocol's icon (ssh, telnet, serial, local shell), the same glyphs as the tabs

## 0.48.0 — 2026-10-25

- feat: tabs show a coloured icon for their protocol and state (connecting, connected, new output, disconnected, locked); a session tab keeps the session's name instead of the shell's `user@host:~`

## 0.47.0 — 2026-10-25

- feat: a session's pre-script also runs before Reconnect and Clone of its tab; if it fails the tab is left as it was

## 0.46.1 — 2026-10-25

- fix: a `WaitForString` / `ReadString` / `WaitForStrings` without a timeout gives up after 60 s instead of hanging the script (a negative timeout waits forever)

## 0.46.0 — 2026-10-25

- Update downloads the new .deb and SHA256SUMS from the release, checks the SHA-256 (a mismatch deletes the package and installs nothing), then installs it with apt through polkit

## 0.45.1 — 2026-10-25

- Notes: no second frame around the text; notes open in the right-hand tab group like the Script Console
- README: the install block now downloads both files itself, so the checksum check always has its file

## 0.45.0 — 2026-10-24

- Close Other / All Tabs; reopen last sessions at startup (v0.45.0) ([#51](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/51))

## 0.44.1 — 2026-10-24

- Icon buttons with hints, readable selection, README fix (v0.44.1) ([#50](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/50))

## 0.44.0 — 2026-10-24

- Crt.FileTransfer and crt.Session.Lock (phase 29b, v0.44.0) ([#48](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/48))

## 0.43.0 — 2026-10-24

- UI polish — SFTP key buttons, heights in options, Script Console as a tab (v0.43.0) ([#47](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/47))

## 0.42.0 — 2026-10-24

- Notes — a tab with a text / markdown file (phase 31) ([#46](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/46))

## 0.41.0 — 2026-10-24

- Script scheduler (phase 30) ([#45](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/45))

## 0.40.0 — 2026-10-24

- Crt API — Clear / SendKeys / MatchIndex / SetStatusText / Session.Create / Window (phase 29) ([#44](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/44))

## 0.39.0 — 2026-10-24

- Session pre-script — before the connection, can cancel it (phase 28) ([#43](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/43))

## 0.38.0 — 2026-10-24

- Script menu — folder list, recent, last folder, startup script (phase 27) ([#42](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/42))

## 0.37.0 — 2026-10-24

- Script Console and debug trace (phase 26) ([#41](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/41))

## 0.36.0 — 2026-10-24

- Button Bar managed by right click (phase 25) ([#40](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/40))

## 0.35.0 — 2026-10-24

- Session / Command Manager side (phase 24) ([#39](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/39))

## 0.34.0 — 2026-10-24

- Multi-line Command Window (phase 23) ([#38](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/38))

## 0.33.0 — 2026-10-24

- A tab in a window of its own (phase 22) ([#37](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/37))

## 0.32.0 — 2026-10-24

- Script recording — Script → Record Script (phase 21) ([#35](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/35))

## 0.31.0 — 2026-10-24

- Update check at launch (Update / Skip) and donate button (phase 20) ([#34](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/34))

## 0.30.0 — 2026-10-24

- Colour icons for dialogs ([#32](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/32))

## 0.29.0 — 2026-10-24

- Crt dialogs as windows — MessageBox icons / buttons, List, file dialogs (phase 19) ([#31](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/31))

## 0.28.0 — 2026-10-24

- Example scripts and crt additions (phase 18) ([#30](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/30))

## 0.27.0 — 2026-10-24

- Terminal font and size in Global Options (phase 17) ([#29](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/29))

## 0.26.0 — 2026-10-24

- Move a tab to another group (phase 16) ([#28](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/28))

## 0.25.0 — 2026-10-24

- Command Manager (phase 15) ([#27](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/27))

## 0.24.0 — 2026-10-24

- X/Y/Zmodem in terminal tabs through lrzsz (phase 14) ([#26](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/26))

## 0.23.0 — 2026-10-24

- Synchronize Folders in the SFTP tab (phase 13) ([#25](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/25))

## 0.22.0 — 2026-10-24

- Tab label (Session Options → Appearance) ([#24](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/24))

## 0.21.0 — 2026-10-24

- Key Map — own keys for strings, functions, programs, scripts (phase 12) ([#23](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/23))

## 0.20.0 — 2026-10-24

- Create public keys and upload them (phase 11) ([#22](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/22))

## 0.19.0 — 2026-10-24

- Run scripts from buttons and sessions, import Run Script buttons (phase 10b) ([#21](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/21))

## 0.18.0 — 2026-10-24

- Python scripts with the crt object (phase 10a) ([#20](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/20))

## 0.17.0 — 2026-10-24

- Import sessions and Button Bar from SecureCRT (9c) ([#19](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/19))

## 0.16.0 — 2026-10-24

- Terminal colour schemes and keyword highlighting (9b) ([#18](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/18))

## 0.15.0 — 2026-10-24

- Logon Actions (9a) ([#17](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/17))

## 0.14.0 — 2026-10-24

- Telnet and Serial protocols (8b) ([#16](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/16))

## 0.13.0 — 2026-10-24

- Jump host, port forwarding, auto-reconnect (8a) ([#15](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/15))

## 0.12.0 — 2026-10-24

- Session logs (phase 7) ([#14](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/14))

## 0.11.0 — 2026-10-24

- Button Bar (6b) ([#13](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/13))

## 0.10.0 — 2026-10-24

- Command Window (6a) ([#12](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/12))

## 0.9.0 — 2026-10-24

- SFTP drop from file manager, edit remote file, bookmarks (5b-2) ([#11](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/11))

## 0.8.0 — 2026-10-24

- SFTP overwrite rules, resume, hidden files, chmod, column sorting (5b-1) ([#10](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/10))

## 0.7.1 — 2026-10-24

- Tab right-click menu shows and acts on the clicked tab ([#9](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/9))

## 0.7.0 — 2026-10-24

- SFTP tab as a commander (phase 5a) ([#8](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/8))

## 0.6.0 — 2026-10-24

- Credentials Manager and master password (phase 4b) ([#7](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/7))

## 0.5.0 — 2026-10-24

- Askpass dialogs, keyring passwords, host key (phase 4a) ([#6](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/6))

## 0.4.0 — 2026-10-24

- Session Options window (phase 3b) ([#5](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/5))

## 0.3.0 — 2026-10-24

- Phase 3a: sessions, Session Manager, Quick Connect, ssh ([#4](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/4))

## 0.2.0 — 2026-10-24

- Phase 2: terminal, tabs, tab groups ([#3](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/3))

## 0.1.0 — 2026-10-24

- Phase 1: application skeleton ([#2](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/2))

## Without a release

Documentation and CI changes that did not bump the version.

- No Flatpak: decision recorded in the roadmap (2026-10-25)
- Stripe: product icon and Radar rules set up, noted in spec 20 (2026-10-25)
- SecureCRT 9.7 main window notes from a screenshot (kept out of git) in the reference (2026-10-25)
- Keep only the last 5 releases ([#49](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/49), 2026-10-24)
- Roadmap backlog cleanup ([#36](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/36), 2026-10-24)
- Roadmap — update check and donate (priority) ([#33](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/33), 2026-10-24)
- Architecture, roadmap, SecureCRT reference ([#1](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/1), 2026-10-22)
