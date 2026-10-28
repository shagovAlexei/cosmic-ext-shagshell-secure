# 37. Menu editor — plan

Spec: `.claude/docs/specs/37-menu-editor.md`.

1. **core::menu** (`crates/core/src/menu.rs`, test-first): `Item`, `Menu`, `STOCK` (today's menu as
   names; transfers `SendZmodem…ReceiveXmodem`, submenus `ScriptsList` / `RecentScripts`, new
   `CustomizeMenu` at the end of Tools), `default_menus`, `merged`, `MenuEdit` + `MenuMsg` with the
   rules (stock function deleted only when it stands twice; `CustomizeMenu` and its menu never
   hidden; delete own menu → `merged` gives the stock back), `ok()` = empty when default.
2. **app menu.rs**: `bar` builds from `merged(&config.menus)`; `entry(cmd)` → `(Action, label)`
   for every stock name (test: every `STOCK` name has one); state-driven check / greyed as
   today; own items → `Action::OwnItem(n)`; `Action::CustomizeMenu`.
3. **app menu_editor.rs**: the dialog (menus | items | buttons | fields); `App.menu_edit`; OK saves
   `Config.menus`; own items run by the toolbar's path (`App::run_tool`, shared); Browse = `Slot::Menu`.
4. **options.rs**: section «Меню», «Сбросить меню по умолчанию» with inline Да / Нет.
5. i18n en / ru, TESTING.md, CHANGELOG, CLAUDE.md, version 0.97.0, headless check.
