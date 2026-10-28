# 37. Menu editor

## Why

The user wants to arrange the main menu themselves: rename, add, delete, move items between menus,
hide them; and get the stock menu back with one button in Global Options. We give the means, not
a new layout: the default menu stays as it is today.

## What the user gets

- **Сервис → Настройка меню…** (`Tools → Customize Menu…`) opens a dialog (like the toolbar
  editor: OK / Cancel, nothing changes until OK).
- **Left: the menus** (Файл, Правка, … Справка) in bar order:
  - a check box = the menu is shown on the bar;
  - ↑ / ↓ move it;
  - its name can be edited (empty = the stock name, which follows the language);
  - «Добавить меню» adds a menu of one's own; «Удалить» removes one's own menu (a stock menu can
    only be hidden; its items go back to it on Reset only).
- **Right: the items of the chosen menu**, in order:
  - a check box = the item is shown;
  - ↑ / ↓ move it inside the menu;
  - «В меню ▾» moves it to the end of another menu;
  - its label can be edited (empty = the stock label);
  - «Удалить» for an item of one's own (a program, a script, a divider). A stock function is
    never deleted, only hidden: it always stays somewhere, so the editor can always show it again.
- **Add to the chosen menu:**
  - «Функция ▾» — any app function (the same list as the toolbar / Button Bar «Menu Function»);
    a function may stand in several menus;
  - «Программа…» / «Скрипт…» — a command line or a `.py` with its arguments and a label, run like
    a toolbar button (a script on the active tab, `%H %S %U %P` for a program);
  - «Разделитель».
- **Сброс** in the dialog and **«Сбросить меню по умолчанию»** in Сервис → Общие параметры
  (section «Меню»): back to the stock menu (one's own items and menus are gone; the button asks
  once inline: «Сбросить? Да / Нет»).
- What stays as it is: key hints next to items, check marks (View → …, Log Session, Record
  Script), greyed items (no tab, no lrzsz…), the Script menu's «Скрипты ▸» / «Недавние ▸»
  submenus (they are items too: they can be moved or hidden as a whole), Donate only when built in.
- «Настройка меню…» itself can't be hidden or deleted (its check box is greyed), so the dialog is
  always reachable; the menu holding it can't be hidden either (its check box greyed while it's
  there). It can be moved to another menu.

## Model (core)

`core::menu` (no libcosmic):

- `Item { cmd: String, params: String, label: String, shown: bool }` — `cmd` = a function name,
  `-` a divider, a program, or a `.py` (the toolbar's conventions, `toolbar::SEP` / `is_script`);
  `shown` default true.
- `Menu { id: String, title: String, shown: bool, items: Vec<Item> }` — `id` = a stock id (`File`,
  `Edit`, `View`, `Options`, `Transfer`, `Script`, `Tools`, `Window`, `Help`) or `own-N`.
- `pub fn default_menus() -> Vec<Menu>` — today's menu (Tools: Global Options, divider, …; the
  new `CustomizeMenu` after Keymap Editor).
- `pub fn merged(saved: &[Menu]) -> Vec<Menu>` — what is drawn: empty saved = default; a stock
  function present in the default but nowhere in `saved` (a new version added it) is appended to
  its default menu (shown), so updates reach a customised menu (and a deleted own menu gives its
  stock functions back to their menus).
- `MenuEdit` — the dialog state, like `ToolEdit`: `new(list)`, `update(MenuMsg)`, the chosen menu
  / item, `list` on OK. Rules above (no delete of a stock function, `CustomizeMenu` and its menu
  can't be hidden) live here and are tested.
- Stored in the config: `Config.menus: Vec<Menu>` (empty = stock, so a later stock change reaches
  users who never edited it; OK with a list equal to the default stores empty, as the toolbar).

## App

- `menu::bar` builds the bar from `core::menu::merged(&config.menus)`: per item, `cmd` → the
  `menu::Item` it is today (check box / greyed / folder / plain), own label if set; a program /
  script item → `Action::MenuItem(n)` run by the toolbar's path (`App::run_tool`, shared).
- `menu_editor.rs`: the dialog's view, `Message::MenuEdit(MenuMsg)`; `App` saves on OK.
- `options.rs`: section «Меню» with the reset button.
- Every label through `fl!` (en / ru).

## Not doing

- Submenus of one's own (one level of items under each menu). Add when asked.
- Editing the tab context menu or the Session Manager's menus.
- Per-session menus (SecureCRT's `.mnu` files): not asked.

## Tests

- `merged`: empty → default; a new stock function appears in its menu; hidden items stay hidden.
- `MenuEdit`: move ↑/↓ at the ends, move to another menu, delete own / refuse stock,
  `CustomizeMenu` stays shown, add function / program / divider / menu, Reset, OK of the default
  gives empty.
- App: every stock `cmd` in `default_menus` maps to an item (a test over the list).
