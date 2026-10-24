# Example scripts / Примеры скриптов

**EN.** Python scripts for ShagShell Secure. They use the same `crt` object as SecureCRT, so most
of them also run in SecureCRT with small changes (noted in each header). Run one with
*Script → Run…*, from a Button Bar button (kind «Run Script», arguments in the *Arguments*
field), as a session's logon script, or `shagshell --script file.py arg1 arg2`.
Every script starts with a header: what it does, how to run it, what differs from SecureCRT.
Hosts, paths and names in the examples are placeholders: change them to yours.
Most examples need an open terminal tab (the script works on the tab it was started from) or a
saved session (see `03_connect_saved_session.py`).

**RU.** Python-скрипты для ShagShell Secure. Они используют тот же объект `crt`, что и SecureCRT,
поэтому большинство запускается и в SecureCRT с небольшими правками (они указаны в шапке каждого
файла). Запуск: *Script → Run…*, кнопка Button Bar (вид «Run Script», аргументы в поле
«Аргументы»), скрипт входа сессии или `shagshell --script file.py арг1 арг2`.
В шапке каждого скрипта: что делает, как запускать, чем отличается от SecureCRT.
Хосты, пути и имена в примерах условные: подставьте свои.
Большинству примеров нужна открытая вкладка-терминал (скрипт работает с той вкладкой, из которой
запущен) или сохранённая сессия (см. `03_connect_saved_session.py`).

| File | What it shows / Что показывает |
|---|---|
| `01_dialogs_and_arguments.py` | Prompt, MessageBox, `crt.Arguments` |
| `02_send_and_read.py` | send a command, read its output up to the prompt / команда и чтение вывода до приглашения |
| `03_connect_saved_session.py` | open a saved session, run commands, disconnect / открыть сохранённую сессию |
| `04_many_hosts_to_file.py` | several sessions from a list, results into one file / несколько хостов → файл |
| `05_parse_output.py` | parse `df -h` with a regular expression / разбор вывода |
| `06_output_to_clipboard.py` | command output → clipboard / вывод в буфер обмена |
| `07_save_result_to_file.py` | a result to a file with a date in the name / результат в файл с датой |
| `08_send_to_all_tabs.py` | one command to every connected tab / команда во все вкладки |
| `09_auto_responder.py` | answer known lines of the output (`WaitForStrings`) / автоответчик |
| `10_comment_in_log.py` | a timestamped comment in the session log / комментарий в лог |
| `12_dialogs_gallery.py` | every dialog: MessageBox flags, Prompt, List, file dialogs / все диалоги |
| `13_pick_file_and_hosts.py` | choose a commands file and sessions from a list / выбор файла и сессий из списка |
| `11_network_device.py` | `enable`, `terminal length 0`, `show run` → file (Cisco-like) / сетевое устройство |

## Differences from SecureCRT / Отличия от SecureCRT
- `crt.Screen.Synchronous` and `IgnoreEscape` are always on / всегда включены.
- `Session.Connect` accepts only `/S "saved session name"` / только `/S "имя сохранённой сессии"`.
- Line ends: a terminal gets `\r` for Enter; `Screen.Get` joins rows with `\n` / `Screen.Get` соединяет строки `\n`.
- Dialogs are windows of their own; `crt.Dialog.List` is ours (SecureCRT: Internet Explorer) / диалоги - отдельные окна; `crt.Dialog.List` - наш.
- Only `python3` (no VBScript, JScript) / только Python.
