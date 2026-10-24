# 01. Каркас приложения

## Зачем

Получить собираемое, устанавливаемое приложение с раскладкой SecureCRT, в которую следующие этапы встраивают функции, не трогая общий каркас. Плюс вся инфраструктура: `just verify`, CI, `.deb`, i18n, headless-проверка.

## Что видит пользователь

`shagshell` открывает окно 1200×800 «ShagShell Secure»:

```
┌ [header] File Edit View Options Transfer Script Tools Window Help ───┐
│ Toolbar: Connect | Quick Connect | Reconnect | Disconnect | SFTP | ⚙ │   ← все неактивны
├──────────────┬───────────────────────────────────────────────────────┤
│ Session      │                                                       │
│ Manager      │            Нет открытых сессий                        │
│ (пусто:      │      Используйте Quick Connect (Alt+Q)                │
│ «Нет сессий»)│                                                       │
├──────────────┴───────────────────────────────────────────────────────┤
│ Не подключено                                                        │
└──────────────────────────────────────────────────────────────────────┘
```

- **Меню** — все 9 пунктов SecureCRT, с основными элементами каждого (по `securecrt-reference.md`). Работают только:
  - File → Exit (без своей клавиши: Ctrl+Q — это XON и уходит в терминал; окно закрывает Alt+F4 композитора, как в SecureCRT);
  - View → Session Manager (показать/скрыть панель, Alt+M; состояние запоминается);
  - Help → About (context drawer с версией, лицензией, ссылкой на репозиторий).
  - Остальные элементы видны, но неактивны.
- **Toolbar** — кнопки с иконками и подсказками, неактивны.
- **Session Manager** — панель слева шириной 240 px, пока с текстом «Нет сессий».
- **Центр** — заглушка «Нет открытых сессий» + подсказка про Quick Connect.
- **Строка статуса** — «Не подключено».
- Все строки — через `fl!`, en + ru.

## Что вне окна

- `shagshell --check` — печатает `ok` / `MISSING` для `ssh`, `telnet`, `picocom`, `python3` (поиск в `PATH`), код выхода 0, если `ssh` есть. Keyring и сокет добавят этапы 4 и 10.
- `shagshell --version`.

## Код

Каркас workspace — как в CLAUDE.md, но только то, что нужно сейчас:

```
Cargo.toml                 # workspace, version 0.1.0, edition 2024, GPL-3.0-only, release strip
crates/core/src/lib.rs     # check.rs: probe(names, PATH) -> Vec<(name, bool)>  (тесты)
crates/app/
  Cargo.toml               # libcosmic rev ef490df…, features tokio winit wayland wgpu multi-window about
  i18n/{en,ru}/shagshell.ftl
  src/main.rs              # --check / --version, pick_renderer (из commander), run
  src/i18n.rs              # из commander
  src/app.rs               # App, Message, Application impl — только маршрутизация
  src/menu.rs              # header menu bar + Action enum (общий для клавиш, Button Bar)
  src/toolbar.rs
  src/session_manager.rs   # пустая панель
  src/status_bar.rs
  src/config.rs            # cosmic-config: session_manager_visible
res/
  io.github.shagovalexei.ShagShellSecure.desktop
  io.github.shagovalexei.ShagShellSecure.metainfo.xml
  icons/io.github.shagovalexei.ShagShellSecure.svg
justfile                   # verify, build-debug/release, run-logs, install/uninstall, deb
.github/workflows/{build,release}.yml
.claude/skills/shagshell-feature/headless.sh
TESTING.md
```

- `Action` — один enum на все действия приложения (Exit, ToggleSessionManager, About, а дальше Connect, QuickConnect, …). Меню, горячие клавиши, а позже Button Bar «Menu Function» мапят в него.
- Из commander берём: `main.rs::pick_renderer` (фолбэк на tiny-skia без `SHADER_F16_IN_F32`), `i18n.rs`, `justfile` (без rpm и man), `build.yml` / `release.yml`, `headless.sh`. `desktop_icon_theme` не берём: целевая платформа — COSMIC.
- `.deb` Depends: библиотеки от `dpkg-shlibdeps` + `openssh-client`; Recommends: `telnet`, `picocom`, `python3`, `gnome-keyring`.

## Мои решения (проверь)

- Версия стартует с 0.1.0.
- Иконка — простая SVG: окно терминала с щитом.
- Неработающие пункты меню видны, но неактивны (а не спрятаны) — чтобы раскладка сразу была как в SecureCRT.
- Ширина Session Manager фиксированная 240 px; перетаскиваемый разделитель — на этапе 3, когда в панели появится дерево.
- Горячие клавиши приложения — как в SecureCRT, на Alt+буква (Alt+Q, Alt+C, Alt+M…), Ctrl+буква не трогаем: это управляющие символы для удалённой стороны. Переназначение клавиш — когда появится терминал (этап 2).

## Не делаем на этом этапе

Терминал, вкладки, сессии, любые подключения, настройки Global Options, горячие клавиши кроме Alt+M.

## Проверка

- `just verify` зелёный, CI зелёный на PR.
- `cargo test -p shagshell-core` — тесты `check::probe` (найден / не найден / пустой PATH).
- headless: `start`, `shot main` — видны меню, тулбар, Session Manager, заглушка, статус; `vkey alt m` → панель скрыта, `shot hidden`; меню File открывается.
- `shagshell --check` на этой машине.
- `just deb` собирает пакет; `TESTING.md` — ручная проверка установки из `.deb` и запуска из меню COSMIC.
