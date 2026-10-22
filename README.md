# ShagShell Secure

SSH / SFTP session manager for Pop!_OS 24.04 COSMIC, in the spirit of SecureCRT for Windows. Written in Rust on [libcosmic](https://github.com/pop-os/libcosmic).

Status: planning. See [ROADMAP](.claude/docs/ROADMAP.md) and [architecture](.claude/docs/specs/00-architecture.md).

- Session Manager tree, tabs and tab groups, Quick Connect
- System OpenSSH under the hood: `~/.ssh/config`, keys, agent, jump hosts, port forwarding just work
- Saved passwords and credential sets in the GNOME keyring, never in files
- Dual-pane SFTP tab with a transfer queue
- Command Window (send to all sessions), Button Bar, session logging
- Telnet, Serial, local shell

License: GPL-3.0-only.
