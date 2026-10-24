# ShagShell Secure

SSH / SFTP session manager for Pop!_OS 24.04 COSMIC, in the spirit of SecureCRT for Windows. Written in Rust on [libcosmic](https://github.com/pop-os/libcosmic).

See [ROADMAP](.claude/docs/ROADMAP.md) and [architecture](.claude/docs/specs/00-architecture.md).

- Session Manager tree, tabs and tab groups, Quick Connect
- System OpenSSH under the hood: `~/.ssh/config`, keys, agent, jump hosts, port forwarding just work
- Saved passwords and credential sets in the GNOME keyring, never in files
- Dual-pane SFTP tab with a transfer queue
- Command Window (send to all sessions), Button Bar, session logging
- Telnet, Serial, local shell

## Install

Download the `.deb` and `SHA256SUMS` from the [latest release](https://github.com/shagovAlexei/cosmic-ext-shagshell-secure/releases/latest) — only from this repository — into one folder, check it there and install:

```sh
cd ~/Downloads        # the folder with both files
sha256sum -c SHA256SUMS
sudo apt install ./shagshell_*.deb
```

`sha256sum` must print `OK` for the `.deb`. If it says `No such file`, you are in the wrong folder or `SHA256SUMS` was not downloaded.

The app checks for a newer release at startup (Options → Global Options → Updates to turn it off).

## Support the project

ShagShell Secure is free. If it helps you, you can support it — the price of one lunch at McDonald's or a couple of cups of coffee:

**https://donate.stripe.com/eVqaEYdZz0Ot3bc1F567S05**

This is the only official donation link (also in Help → Support the Project and in the update window). Builds from elsewhere may carry another one.

License: GPL-3.0-only.
