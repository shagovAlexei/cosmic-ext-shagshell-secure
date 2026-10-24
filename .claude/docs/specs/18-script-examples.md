# 18. Примеры скриптов и дополнения `crt`

Цель: готовые разбираемые примеры в `Example/` (комментарии на русском и английском) и то, чего им не хватает в `crt`. Образцы задач — скрипты из `Scripts/` SecureCRT (CopyOutputToClipboard, LogOutputOfSpecificCommand, ConnectToMultipleSessionsAndSendCommands, SendCommandToAllTabs, AutoResponder, AddCommentToLogFile, SaveSelectedTextToFile); оттуда взяты только задачи, ни хостов, ни путей, ни паролей.

## Дополнения `crt` (по примерам SecureCRT)
| Что | Как |
|---|---|
| `crt.Clipboard.Text` (чтение и запись) | приложение: запись `clipboard::write`, чтение `clipboard::read` с ответом скрипту |
| `Screen.Get(r1,c1,r2,c2)`, `CurrentRow`, `CurrentColumn`, `Rows`, `Columns`, `Selection` | приложение читает сетку терминала; нарезка текста — чистая функция в `core::script` (тест) |
| `Screen.WaitForCursor(sec)` | Python: опрос положения курсора; `Synchronous` / `IgnoreEscape` принимаются (всегда включены) |
| `Session.Log(bool[, append])`, `Session.LogFileName` | приложение: старт / стоп лога вкладки (`Tap::start`) |
| `Dialog.MessageBox(text, title, buttons)` с `BUTTON_OK / OKCANCEL / YESNO / YESNOCANCEL`, результат `IDOK / IDCANCEL / IDYES / IDNO` | диалог с нужными кнопками |
| `crt.GetLastErrorMessage()` | Python: текст последней `ScriptError` |

Не делаем: `crt.FileDialog`, `Screen.Send` бинарных данных, VBScript.

## Примеры (`Example/`, ставятся в `/usr/share/shagshell/examples/`)
`README.md` (RU+EN) + по сценарию: 01 диалоги и аргументы; 02 команда → вывод (`ReadString` по приглашению); 03 открыть сохранённую сессию, выполнить, закрыть; 04 несколько хостов из списка → результаты в файл; 05 разбор вывода (`df -h`, предупреждение > 80 %); 06 вывод в буфер обмена; 07 сохранить результат в файл с датой; 08 команда во все вкладки; 09 автоответчик (`WaitForStrings`); 10 комментарий в лог сессии; 11 сетевое устройство (enable / `terminal length 0` / `show run` → файл).
Каждый пример самодостаточен, с шапкой «что делает / как запускать / отличия от SecureCRT».

## Проверка
Тесты `core` на нарезку `Screen.Get`; примеры 02–10 прогоняются headless на локальном sshd.
