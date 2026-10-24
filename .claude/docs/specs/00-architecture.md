# 00. Архитектура ShagShell Secure

## Зачем

Менеджер SSH/SFTP-сессий для Pop!_OS 24.04 COSMIC с UX SecureCRT для Windows. Пользователь работает и с Linux-серверами, и с сетевым железом, поэтому важны и SFTP/ключи/jump host, и Command Window/Button Bar/логи/Serial.

**Цели:** дерево сессий, вкладки и группы вкладок, сохранённые учётные данные, SFTP с очередью, отправка команд во много сессий, Button Bar, скрипты на Python (API как у SecureCRT), логи, проброс портов.
**Не цели:** свой SSH-стек, VBScript/JScript, legacy-протоколы и эмуляции, Windows/macOS.
**Анти-цели:** пароли в открытом виде на диске; приложение, которое «технически работает», но игнорирует `~/.ssh/config` и ключи пользователя.

## Решения и почему

### 1. SSH — системный OpenSSH, а не своя реализация

Рассматривали:

| | OpenSSH в PTY (выбрано) | russh в процессе |
|---|---|---|
| Совместимость | `~/.ssh/config`, ProxyJump, агент, FIDO, сертификаты, все форматы ключей — сразу | Разбор ssh_config и known_hosts пишем сами (~500 строк) |
| Надёжность | Проверенный стек | Известные баги: зависший канал блокирует соединение (russh#789), SFTP падает на не-UTF-8 именах (russh-sftp#42) |
| Контроль | Нет доступа к деталям протокола (шифр в статусе) | Полный |
| Объём | Мало | Много, API ломается в минорных версиях |

Как это работает:
- Терминал (cosmic-term через shagoff-commander) работает с локальным PTY; `ssh`, `telnet`, `picocom`, `$SHELL` — просто разные команды.
- `launch.rs` строит argv/env из сессии; это чистая функция, тесты сравнивают с ожидаемым argv.
- ControlMaster (`ControlPath=$XDG_RUNTIME_DIR/shagshell/%C`) даёт переиспользование соединения для SFTP и Clone, как «Connect SFTP Tab» в SecureCRT.
- Askpass: ssh вызывает `shagshell "<prompt>"` (режим помощника — по `SHAGSHELL_ASKPASS_SOCK`), тот через unix-сокет своего экземпляра приложения (`askpass-<pid>.sock`) спрашивает его. Приложение берёт секрет из keyring по `SHAGSHELL_SESSION` или показывает диалог (пароль / passphrase / host key yes-no). Ответ — в stdout помощника.
  - Сокет создаётся с правами 0600 в `$XDG_RUNTIME_DIR/shagshell/` (каталог 0700).
- Auto-reconnect: ssh завершился с кодом 255 → предложить/выполнить переподключение.
- Переход на russh остаётся дешёвым (терминалу всё равно, откуда байты), но trait «на будущее» не заводим.

### 2. Хранение — TOML-файл на сессию

Рассматривали один общий файл и SQLite. Выбрано дерево файлов, как `.ini` в SecureCRT: удобно сравнивать, держать в git, править руками, переносить папкой.

```
~/.config/shagshell/
  sessions/
    Default.toml          # шаблон
    prod/
      web-01.toml
    lab/cisco/
      sw-core.toml
  credentials.toml        # именованные наборы: только username
```

Пример сессии (поля добавляются по этапам, у каждого есть default, чтобы старые файлы читались):

```toml
protocol = "ssh"          # ssh | telnet | serial | local
host = "10.0.0.1"
port = 22
user = "admin"
credential = "lab-admin"  # ссылка на набор из credentials.toml
identity = "~/.ssh/id_ed25519"
jump = "prod/bastion"     # путь к другой сессии
keepalive = 30

[[forwards]]
kind = "local"            # local | remote | dynamic
listen = "127.0.0.1:8080"
target = "localhost:80"
```

- Путь файла относительно `sessions/` (без `.toml`) — id сессии; он же ключ в keyring.
- Новая сессия копирует `Default.toml` в момент создания (без живого наследования, как в SecureCRT).
- Секреты — только в Secret Service: атрибуты `app=io.github.shagovalexei.ShagShellSecure`, `kind=session|credential`, `id=…`. Переименование/перенос сессии переносит и её элемент в keyring.
- Мастер-пароль не делаем: связка ключей уже зашифрована и открывается при входе. Отложено в бэклог.

### 3. Структура кода

Workspace из двух крейтов, как в shagoff-commander:
- `crates/core` (`shagshell-core`) — без libcosmic, вся логика, unit-тесты.
- `crates/app` (`shagshell`) — UI, разбит по фичам: каждый модуль владеет своими `Message`/`update`/`view`, `app.rs` только маршрутизирует. Урок commander: один `app.rs` вырос до 15 тыс. строк.

Список модулей — в CLAUDE.md. Модульность — модули Rust, без плагинов.

### 4. UI

Раскладка SecureCRT (подробно — `securecrt-reference.md`):

```
┌ Menu: File Edit View Options Transfer Script Tools Window Help ──────┐
│ Toolbar: Connect | Quick Connect | Reconnect | Disconnect | SFTP | ⚙ │
├──────────────┬───────────────────────────────────────────────────────┤
│ Session      │ [✓ web-01] [✗ sw-core] [✓ lab]   │ [✓ db-01]          │
│ Manager      │                                  │                    │
│ 🔍 filter    │   terminal                       │   terminal         │
│ ▾ prod       │                                  │                    │
│   web-01     │          (pane_grid: группы вкладок)                  │
│ ▾ lab        │                                                       │
│   ▾ cisco    ├───────────────────────────────────────────────────────┤
│     sw-core  │ Command Window: [ show run      ] → (Current|All|Sel) │
│              │ Button Bar: [save] [show int] [reload]                │
├──────────────┴───────────────────────────────────────────────────────┤
│ Connected · SSH2 · 120×40                                            │
└──────────────────────────────────────────────────────────────────────┘
```

SFTP-вкладка: слева локальные файлы, справа удалённые, внизу очередь передач (ожидает / идёт / готово / ошибка, скорость, ETA, повтор).

### 5. Button Bar (как в SecureCRT, этап 6)

- Несколько **именованных панелей** кнопок; переключатель панели — выпадающий список слева на самой панели.
- Сессия может указать свою панель (`button_bar = "cisco"`); при переключении вкладки панель меняется.
- Действие кнопки:
  - **Send String** — строка с escape-последовательностями `\r \n \t \e \\ \p` (пауза 1 с), разбор в `core/macros.rs`;
  - **Run Script** — запуск Python-скрипта (раздел 6) с аргументами;
  - **Menu Function** — любое действие приложения (Connect, Reconnect, Clone, SFTP, …), тот же enum `Action`, что и у горячих клавиш;
  - **Launch Application** — внешняя программа, с подстановкой `%H %P %U %S`.
- Цель отправки (Send String / Menu Function): текущая вкладка или все вкладки — как в Command Window.
- Правка: ПКМ по панели → New / Edit / Delete / Move left/right; диалог как «Map Button» в SecureCRT.
- Хранение: `~/.config/shagshell/buttonbars.toml` (`[[bar]] name, [[bar.button]] label, action, arg, target`).

### 6. Скрипты на Python (как в SecureCRT, этап 10)

Рассматривали встраивание интерпретатора (PyO3) и отдельный процесс. Выбран **отдельный процесс `python3`**:
- падение или зависание скрипта не роняет приложение (скрипт можно убить);
- сборка не привязана к версии libpython, `.deb` не тянет её как зависимость;
- скрипты используют любые pip-пакеты и venv пользователя.

Как это работает:
- Приложение ставит модуль `crt` (`/usr/share/shagshell/python/crt.py`) и запускает `python3 <script> [args]` с `PYTHONPATH` на него и `SHAGSHELL_SCRIPT_SOCK=$XDG_RUNTIME_DIR/shagshell/script-<n>.sock` (0600).
- `crt.py` — тонкая обёртка: каждый вызов — JSON-строка в сокет, ответ — JSON-строка. Протокол описан и тестируется в `core/script.rs`.
- Объект `crt` повторяет объектную модель SecureCRT, чтобы скрипты переносились с минимальными правками:

```python
# $language = "python"   (строка SecureCRT игнорируется, оставлена для совместимости)
crt.Screen.Synchronous = True
crt.Session.Connect("/SSH2 /L admin 10.0.0.1")   # или crt.Session.Connect("lab/cisco/sw-core")
crt.Screen.WaitForString("#", 10)
crt.Screen.Send("show version\r")
out = crt.Screen.ReadString("#")
crt.Dialog.MessageBox(out[:200])
```

- Объём API (MVP): `crt.Screen.{Send, WaitForString, WaitForStrings, ReadString, Get, CurrentRow, CurrentColumn, Synchronous}`, `crt.Session.{Connect, Disconnect, Connected, Path, Log}`, `crt.Dialog.{MessageBox, Prompt, FileOpenDialog}`, `crt.Sleep`, `crt.Arguments`, `crt.GetScriptTab()`, `crt.GetTab(n)` / `crt.GetTabCount()` / `crt.GetActiveTab()` (у вкладки — свои `Screen` и `Session`).
- Ожидание (`WaitForString`, `ReadString`) делается на стороне приложения по потоку вывода вкладки, со своим таймаутом; скрипт просто блокируется на сокете.
- Запуск: меню Script → Run (последние скрипты), кнопка Button Bar, горячая клавиша, Logon Script сессии, `shagshell --script file.py`. Script → Cancel убивает процесс.
- Записи скрипта (recorder) нет, в бэклоге.

## Что берём из shagoff-commander

- `crates/app/src/term/*` — терминал (cosmic-term); `shell.rs`/follow не нужны.
- `crates/core/src/keyring.rs` — Secret Service по zbus; дописать `lookup`/`delete` и свою схему атрибутов.
- `crates/core/src/tabs.rs`, `crates/app/src/{menu,keymap,hotkeys,dialogs,jobs,config,i18n,drawer,main}.rs`.
- `justfile`, `.github/workflows/{build,release}.yml`, `res/`, `headless.sh`, формат `TESTING.md`.

cosmic-files не используем: его вкладки завязаны на `std::fs`, сеть — через gvfs. SFTP-панель делаем свою на `table`/`list`.

## Риски

- SFTP: спайк этапа 5 показал, что `openssh-sftp-client` рвёт соединение на имени не в UTF-8 и не конвейеризует чтение — свой клиент SFTP v3 поверх `ssh -s … sftp` (см. specs/05-sftp.md).
- Askpass не вызывается, если ssh видит TTY и `SSH_ASKPASS_REQUIRE` не `force` — поэтому всегда `force`.
- Keyboard-interactive (OTP) приходит в askpass как произвольный prompt — показываем как «другое» без сохранения.
- Скрипты: Python 3 должен быть в системе (на Pop!_OS есть всегда); `--check` проверяет `python3`. Скрипт выполняется с правами пользователя — запускаем только явно выбранные пользователем файлы.

## Проверка

- `just verify` на каждом PR.
- Headless (sway + grim): подключение к `localhost` sshd, пароль через askpass, SFTP put/get.
- `shagshell --check`: ssh, telnet, picocom, python3, keyring → ok / MISSING.
