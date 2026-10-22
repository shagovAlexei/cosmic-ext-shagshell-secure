# 00. Архитектура ShagShell Secure

## Зачем

Менеджер SSH/SFTP-сессий для Pop!_OS 24.04 COSMIC с UX SecureCRT для Windows. Пользователь работает и с Linux-серверами, и с сетевым железом, поэтому важны и SFTP/ключи/jump host, и Command Window/Button Bar/логи/Serial.

**Цели:** дерево сессий, вкладки и группы вкладок, сохранённые учётные данные, SFTP с очередью, отправка команд во много сессий, логи, проброс портов.
**Не цели:** свой SSH-стек, скриптинг на VBScript/Python, legacy-протоколы и эмуляции, Windows/macOS.
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
- Askpass: ssh вызывает `shagshell askpass "<prompt>"`, тот через unix-сокет спрашивает работающее приложение. Приложение берёт секрет из keyring по `SHAGSHELL_SESSION` или показывает диалог (пароль / passphrase / host key yes-no). Ответ — в stdout помощника.
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
┌ Menu: File Edit View Options Transfer Tools Window Help ─────────────┐
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

## Что берём из shagoff-commander

- `crates/app/src/term/*` — терминал (cosmic-term); `shell.rs`/follow не нужны.
- `crates/core/src/keyring.rs` — Secret Service по zbus; дописать `lookup`/`delete` и свою схему атрибутов.
- `crates/core/src/tabs.rs`, `crates/app/src/{menu,keymap,hotkeys,dialogs,jobs,config,i18n,drawer,main}.rs`.
- `justfile`, `.github/workflows/{build,release}.yml`, `res/`, `headless.sh`, формат `TESTING.md`.

cosmic-files не используем: его вкладки завязаны на `std::fs`, сеть — через gvfs. SFTP-панель делаем свою на `table`/`list`.

## Риски

- `openssh-sftp-client` поверх mux: проверить на этапе 5 спайком (докачка, большие каталоги, не-UTF-8 имена). Запасной вариант — второе ssh-соединение с `-s sftp`.
- Askpass не вызывается, если ssh видит TTY и `SSH_ASKPASS_REQUIRE` не `force` — поэтому всегда `force`.
- Keyboard-interactive (OTP) приходит в askpass как произвольный prompt — показываем как «другое» без сохранения.

## Проверка

- `just verify` на каждом PR.
- Headless (sway + grim): подключение к `localhost` sshd, пароль через askpass, SFTP put/get.
- `shagshell --check`: ssh, telnet, picocom, keyring, askpass-сокет → ok / MISSING.
