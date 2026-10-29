# 38. Export / Import Settings

## Why

Audit item 43: a reinstalled system or a second computer means building every session, bar,
key and command again. One file should carry them.

## What the user gets

- **Файл → Экспорт настроек…**: a save dialog (`shagshell-settings-YYYY-MM-DD.tar.gz`); the
  archive holds `~/.config/shagshell` (sessions, credentials' user names, Button Bars, commands,
  keys, highlights, schedule, bookmarks, notes, own scripts and templates — not `scripts/.lib`,
  which the program writes itself) and the Global Options (`~/.config/cosmic/<APP_ID>`).
  **Passwords are not in it**: they stay in the keyring (said in the notice).
- **Файл → Импорт настроек…**: pick an archive; the current settings are first saved to
  `~/.local/state/shagshell/before-import-<time>.tar.gz`, then the archive's files replace
  those of the same name (nothing else is deleted). The notice says how many files came, where the
  old ones are, and to restart the program.
- An archive with anything but plain files and folders under `shagshell/` and `<APP_ID>/`
  (a link, a device, a path out) is refused whole, nothing written.

## How

`core::backup`: `export(config_home, out)` and `import(archive, config_home)` over the system
`tar` (gzip); import extracts into a private temp folder, checks every entry
(`symlink_metadata`: only files and folders, only the two tops), then copies. App: two
`Action`s in the File menu (`core::menu::STOCK`), the work off the UI thread.

## Not doing

- Passwords in the archive (they would need their own encryption; the keyring has its export).
- Choosing what to import (all or nothing).
