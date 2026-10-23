# SecureCRT 9.x reference (how SecureCRT does it)

Source of truth for "behave like SecureCRT". Compiled mostly from knowledge of SecureCRT for Windows; per-version details are unverified — check vandyke.com release notes before relying on them. Update this file when a feature teaches us more.

Column **Phase** = ROADMAP phase where we implement it (— = not planned).

## 1. Main window

| Area | SecureCRT | Phase |
|---|---|---|
| Menu bar | File, Edit, View, Options, Transfer, Script, Tools, Window, Help | 1 |
| Toolbar | Connect, Quick Connect, Connect in Tab, Reconnect, Disconnect, Copy/Paste, Find, Session Options, Global Options, Key Map, SFTP | 1–3 |
| Session Manager | Dockable left/right pane: folder/session tree, filter box, toolbar (new/edit/delete/connect), drag to reorder/move, multi-select connect all / bulk edit, double-click connects, pin/auto-hide | 3 |
| Connect dialog (Alt+C) | Modal version of the tree; "Show on startup", "Open in a tab" | 3 |
| Quick Connect (Alt+Q) | Protocol, host, port, firewall, user, ordered auth methods; "Save session", "Open in a tab" | 3 |
| Tabs | Status icon (✓ connected, ✗ disconnected, activity). Menu: Rename, Clone, Clone in new window, Reconnect, Disconnect, Lock session, Connect SFTP Tab, Move to new window, Close. Drag out to a new window | 2–3 |
| Tab groups / tiling | Split tab area horizontally/vertically into groups, each with its own strip; drag tabs between; resize dividers; Window > Tile/Cascade | 2 |
| Command Window | Pane under the terminal: compose a line, send to current / all / visible tabs; history; send as line or char-by-char | 6 |
| Button Bar | Several named bars; button = send string (`\r` escapes), run script, menu function, launch app; bar tied to a session | 6 |
| Command Manager | Dockable tree of saved commands, double-click sends | backlog |
| Status bar | State, protocol, cipher, cursor row/col, rows×cols, emulation, CAP/NUM | 2–3 |

## 2. Session storage

- Config folder `%APPDATA%\VanDyke\Config\` (relocatable, may be a network share): `Global.ini`, `Sessions\Default.ini`, `Sessions\<Folder>\<Session>.ini`, `__FolderData__.ini` (order, expanded), `KnownHosts\`, `Keymaps\`, `Color Schemes.ini`, `Keywords\*.ini`, `ButtonBarV4.ini`, `Credentials.ini`.
- `.ini` line format: `S:"Hostname"=x`, `D:"Port"=00000016` (hex), `B:` binary, `Z:` multi-line. (Needed for import, phase 9.)
- Hierarchy: Global Options > Default session > per-session. A new session **copies** Default at creation; changing Default offers "apply to all sessions". Bulk edit of selected sessions.
- Global Options: config path, appearance, tabs, Session Manager, host keys, agent, editor, firewalls list, credentials.
- Session Options: Connection, Logon Actions, SSH2 (Auth, Port Forwarding, X11), Terminal (Emulation, Modes, Appearance), Log File, File transfer.
- Credentials Manager: named user+password sets; sessions reference by name, change once → all sessions.
- Configuration passphrase: AES-256 of saved secrets in `.ini`, asked at startup. **Ours: keyring instead (deferred).**
- Import/export: XML export/import, import from PuTTY etc.

## 3. Protocols and connections

| Feature | SecureCRT | Ours | Phase |
|---|---|---|---|
| Protocols | SSH2, SSH1, Telnet, Telnet/TLS, RLogin, Serial, TAPI, Raw, Local shell | SSH, Telnet, Serial (picocom), Local | 2, 3, 8 |
| SSH2 auth | password, publickey, keyboard-interactive, GSSAPI, PKCS#11; order per session | all through OpenSSH + askpass | 4 |
| Keys | Create Public Key wizard (RSA/ECDSA/Ed25519), Public Key Assistant (RFC 4819) | `ssh-keygen`, `ssh-copy-id` | backlog |
| Agent | built-in agent + forwarding | system agent, `-A` option | 3 |
| Jump host / firewall | named firewalls (SOCKS4/5, HTTP, "Session" = jump), chainable | `-J` from session path / ProxyCommand | 8 |
| Port forwarding | local (named, "launch app"), remote, dynamic SOCKS | `-L/-R/-D` | 8 |
| X11 | checkbox | — (backlog) | — |
| Keepalive | NO-OP every N s, TCP keepalive | `ServerAliveInterval` | 8 |
| Auto reconnect | on drop; Reconnect / Reconnect all | on exit 255 | 8 |
| Logon Actions | expect/send table (hide password), logon script | expect/send on terminal output | 9 |
| Host keys | first connect: fingerprint, Accept & Save / Accept Once / Cancel; changed key warning; DB viewer | OpenSSH known_hosts, prompt via askpass dialog | 4 |

## 4. Terminal

- Emulation: VT100/102/220/320, Linux, Xterm, ANSI, Wyse, TN3270/5250; 256-colour + true colour; UTF-8. **Ours: xterm (alacritty) only.**
- Scrollback up to 128k+ lines; clear on clear-screen; scroll to bottom on output/key.
- Colour schemes (named, per session) + tab colour. Phase 9.
- Fonts: normal + narrow/Unicode, line spacing, cursor style/blink.
- Keyword highlighting: named lists, colour/style per keyword, regex, per session. Phase 9.
- Logging: name template `%H` host, `%S` session, `%Y %M %D %h %m %s` date/time, `%P` port, `%U` user; start on connect; append/overwrite; new file per session/day; custom text at start/end/each line (timestamps); raw log. Phase 7.
- Copy/paste: copy on select, middle/right-click paste, confirm multi-line paste, line/char delay, block vs stream selection.
- Send to many: Command Window "send to all", Edit > Send to all tabs. Phase 6.
- Scripting: VBScript/JScript/Python, object model `crt.Screen.Send/WaitForString/ReadString`, `crt.Session.Connect`, `crt.Dialog`, `crt.GetTab`; run from menu, button, key, logon, command line; recorder. **Ours: Python only, same `crt` model via a separate process (phase 10); recorder — backlog.**
- Keymap editor: key → send-string / menu function / script; global + per session.
- Find (Ctrl+F): case, regex, direction, scrollback, highlight all.

## 5. SFTP

- **SFTP tab in SecureCRT** (Alt+P / tab menu "Connect SFTP Tab"): reuses the session's connection; command-line `ls cd get put lls lcd mkdir rm rename chmod -r`; drag files onto the tab to upload; downloads to a configured local dir. X/Y/Zmodem in shell tabs.
- **SecureFX** (what our SFTP tab copies, phase 5): dual pane local | remote, each tree + list, multiple remote tabs; shares the session DB; drag & drop between panes and from the file manager; **transfer queue** (pending / active / done / failed, progress, speed, ETA, pause/resume/retry/remove); bookmarks; folder sync with diff preview; edit remote file locally (re-upload on save); overwrite rules (ask / overwrite / only if newer / resume); chmod dialog.

## 6. Priorities

Top features power users rely on (our MVP is 1–8 + host keys + SFTP queue):
1. Session Manager tree with filter, drag, dock
2. Tabs + Clone + Reconnect
3. Credentials Manager + publickey with agent
4. Command Window "send to all"
5. Button Bar macros
6. Session logging with name templates
7. Jump host chaining
8. Port forwarding (local + dynamic SOCKS)
9. Logon Actions
10. Bulk edit / Default apply-to-all
11. Keyword highlighting
12. Colour schemes, fonts, tab colours
13. Tab groups / tiling
14. SFTP tab + drag-drop upload
15. Auto reconnect + keepalive

Also planned: Python scripting with the `crt` object model (phase 10).

Rarely used (skip): SSH1, RLogin, TAPI, Wyse/SCO, TN3270/5250, printing, VBScript/JScript/PerlScript, Public Key Assistant, GSSAPI, Telnet/TLS.


## Where the user's real SecureCRT config is (for the importer, phase 9c)

SecureCRT for Windows runs under wine; its config root is
`/home/shag/.wine/drive_c/users/shag/AppData/Local/VanDyke Software/SecureCRT/Config/`:

| What | Path under `Config/` |
|---|---|
| Button Bar | `ButtonBarV5.ini` |
| Sessions | `Sessions/` — one `.ini` per session, sub-directories are folders (`__FolderData__.ini` in each), `Default.ini` / `Default_LocalShell.ini` / `Default_Serial.ini` / `Default_RDP.ini` are templates |
| Global options, SSH2 defaults | `Global.ini`, `SSH2.ini` |
| Credentials, known hosts, menus, saved commands | `Credentials/`, `KnownHosts/`, `Menus/`, `Commands/` |

These hold the user's real hosts, script paths and (encrypted) passwords: use them to learn the formats, never copy their content into the repository (fixtures are anonymised) and never print secrets.
