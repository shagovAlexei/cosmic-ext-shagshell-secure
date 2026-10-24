# 29. Расширение crt API

Что добавляем к `crt` (порт скриптов SecureCRT без правок). Ветка `feat/crt-more`, версия 0.40.0.

## Что
| Вызов | Поведение |
|---|---|
| `crt.Screen.MatchIndex` | номер сработавшей строки последнего `WaitForStrings` (1-based, 0 — таймаут) |
| `crt.Screen.SendKeys(text)` | как `Send`, но без escape-последовательностей (`\r`, `\p`… уходят буквально) |
| `crt.Screen.Clear()` | очищает экран вкладки (как `clear`, без PTY; буфер для `WaitFor*` не трогает) |
| `crt.Session.SetStatusText(text)` | текст в строке состояния (до следующего сообщения) |
| `crt.Window.Caption` / `crt.Window.Activate()` | заголовок главного окна / вынести его на передний план |
| `crt.Session.Create(name, host, user="", port=22, folder="")` | сохраняет новую SSH-сессию (копия Default, без пароля) и возвращает её путь; затем `crt.Session.Connect('/S "путь"')` |

`crt.ScriptFullName` уже есть (фаза 10).

## Как
- `MatchIndex`, `SendKeys` — только `crt.py` (запоминает результат `WaitForStrings`; удваивает `\`).
- `Screen.Clear`, `Session.SetStatusText`, `Window.*`, `Session.Create` — ответы приложения в `script::answer` / `Outcome`; `Session.Create` идёт через `Store::create` + `sessions_changed` (имя и папка проверяются как в Quick Connect; повтор имени — ошибка).
- Тесты: `crt.py`-логика через `core::script` (протокол), создание сессии — на `Store`.

## Не делаем сейчас (отдельная фаза 29b, если нужно)
- `crt.FileTransfer.Upload/Download` (нужен SFTP-воркер без вкладки);
- `Session.Lock` (в приложении нет «блокировки сессии»);
- пароли в параметрах `Session.Create` (секреты только в keyring).
