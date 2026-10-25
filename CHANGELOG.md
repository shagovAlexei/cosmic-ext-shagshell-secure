# Changelog

Every release, newest first, rebuilt from the merged pull requests. GitHub keeps only the last 5 releases (see `release.yml`), so this file is the permanent history. Versions follow `Cargo.toml`: a feature bumps minor, a fix bumps patch. Add an entry for each PR when it is merged.

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

- Keep only the last 5 releases ([#49](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/49), 2026-10-24)
- Roadmap backlog cleanup ([#36](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/36), 2026-10-24)
- Roadmap — update check and donate (priority) ([#33](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/33), 2026-10-24)
- Architecture, roadmap, SecureCRT reference ([#1](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/pull/1), 2026-10-22)
