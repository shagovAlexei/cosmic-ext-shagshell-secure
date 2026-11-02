# 44. Terminal colour schemes: SecureCRT's built-ins, New / Edit, import

## Why

SecureCRT: Session Options → Terminal → Appearance → «Current color scheme» + **Edit…** / **New…**; the
user's schemes live in `Global.ini`. Here the list is fixed in `app/term/terminal_theme.rs` (18 schemes
since 0.119.0, several of them guessed: our «Windows» is grey on black, SecureCRT's is black on white).

## What SecureCRT stores (read from the user's real `Config/Global.ini`)

```
Z:"Color Schemes"=00000017
 Windows,010101,00000000,…(8 fg),FFFFFF00,…(8 bg),Standard
Z:"ANSI Color Palettes"=0000000f
 Standard,00000000,A0000000,00A00000,…(16 colours)
S:"Current ANSI Palette"=Standard
```

- A scheme: name, flags `010101` (meaning unknown, ignored), 8 foreground and 8 background colours
  `RRGGBB00` (one per text attribute combination; the first pair is plain text), and the name of an ANSI
  palette (16 colours, black … bright white) from the second list.
- 23 schemes ship with SecureCRT 9.7: Black / Cyan, Black / Floral White, Birds of Paradise, Chalkboard,
  Ciapre, Dark Pastel, Desert, Espresso, Floral White / Dark Cyan, Idletoes, Invisibone, Monochrome, Novel,
  Solarized Dark / Light / Darcula, Tomorrow, Traditional, White / Black, White / Blue, Yellow / Black,
  Zenburn, Windows; 15 palettes.
- The session keeps the scheme's name: `S:"Color Scheme"=White / Black`.

## What the user gets

1. **Built-ins = SecureCRT's 23** (exact colours from that file), plus ours that SecureCRT has not: Dracula,
   Nord, Gruvbox Dark, Monokai, Tango Light, Classic. Ours that were guesses of SecureCRT's (Windows,
   White / Black, Black / White, Yellow / Black, Black / Cyan, Tomorrow Night, Zenburn, Espresso,
   Chalkboard, Desert, Solarized Dark / Light) take SecureCRT's colours. Black / White and Tomorrow Night
   (not in SecureCRT) stay. Keys already saved in sessions keep working.
2. Session Options → Appearance, next to «Цветовая схема»: **«Новая…»**, **«Изменить…»**, **«Удалить»**.
   - The editor (dialog): name; «Текст», «Фон», «Курсор»; the 16 ANSI colours in two rows (normal /
     bright). Each colour = swatch + `#RRGGBB` field (a wrong value is shown red, OK greyed). Preview below:
     a few lines of text in every colour on the background, with a cursor block.
   - «Новая…» starts from the scheme selected now. «Изменить…» on a built-in saves an own scheme under the
     same key that replaces it everywhere; «Удалить» on it then reads «Вернуть встроенную». «Удалить» of an
     own scheme: sessions using it fall back to «как в глобальных настройках» (key not found = today's
     rule).
   - Saved at once to `~/.config/shagshell/color_schemes.toml`; every open tab using the scheme repaints.
3. Global Options lists the same merged list (no buttons there — like SecureCRT, editing is in the
   session's Appearance).
4. **Import from SecureCRT** (File → Import from SecureCRT…) gets a «Цветовые схемы» checkbox: reads
   `Global.ini`, takes each scheme's plain-text fg / bg and its palette. A scheme equal to a built-in is
   skipped; a different one with a built-in's name becomes its override; others are added. The imported
   sessions' `S:"Color Scheme"` maps to the key by name (unknown name → empty).

## How

Core, new `core::color_scheme` (tests first):

- `Scheme { key, name, fg, bg, cursor: Option<u32>, ansi: [u32; 16] }`; colours as `u32` RGB, in TOML as
  `"#RRGGBB"` strings.
- `builtins() -> Vec<Scheme>` (the data moves here from `terminal_theme.rs`; the SecureCRT values typed in
  from the file, it is colour data, nothing private).
- `Schemes { own: Vec<Scheme> }`: `load(path)` / `save(path)` (missing file = empty, broken file = error
  shown once, like `schedule.rs`); `all()` = built-ins with overrides applied + own, sorted by name;
  `get(key)`; `put(scheme)`; `remove(key)`; `new_key(name)` — slug of the name, unique against all keys.
- `parse_hex("#a0b1c2" | "A0B1C2") -> Option<u32>`.
- `securecrt::read_color_schemes(&Ini) -> Vec<Scheme>` (missing palette → Standard); `session_from_ini`
  maps `Color Scheme` through a name → key lookup passed in.
- Tests: TOML round trip; override hides the built-in and `remove` brings it back; `new_key` unique; hex
  parse; the SecureCRT block (anonymised copy of the two lists) → schemes; every built-in resolves.

App: `terminal_theme::scheme(key)` takes the colours from `Schemes::get` (held by the app, passed where
`schemes()` is called today) and keeps building alacritty `Colors` (dim = mix toward bg, as now).
`session_options.rs` gets the three buttons; the editor is its own module `color_scheme_editor.rs`
(`Message` / `update` / `view`) shown as a dialog over Session Options. `fl!` en + ru for every string.

## Not doing

SecureCRT's separate ANSI palettes as their own list (each scheme carries its 16 colours), the other 7
attribute colour pairs (bold / blink / underline colours), selection colour, rename (= edit the name),
iTerm2 / Alacritty / other formats, writing back to SecureCRT, colour schemes for the Command Window and
the editor.
