# Example scripts with short blocks / Примеры на коротких блоках

**EN.** The same scripts as in `examples/`, written with the Script Builder's short blocks
(`from shag_blocks import *`): `run`, `prompt`, `ask`, `printbox`, `connect`, `disconnect`,
`active_tab`, `read_list`, `log_to`, `append_file`, `to_clipboard`, `from_clipboard`, `each_host`.
What has no block (`Dialog.List`, file dialogs, `WaitForStrings`, the session log, SFTP…) stays
`crt`. Compare a file here with the one of the same number in `examples/` to see what each block
replaces. The Script Builder opens them as blocks: *From a template ▾ → Examples (short blocks)*.
In SecureCRT put `shag_blocks.py` (from `~/.config/shagshell/scripts`) next to the script.

**RU.** Те же скрипты, что в `examples/`, написанные короткими блоками конструктора скриптов
(`from shag_blocks import *`): `run`, `prompt`, `ask`, `printbox`, `connect`, `disconnect`,
`active_tab`, `read_list`, `log_to`, `append_file`, `to_clipboard`, `from_clipboard`, `each_host`.
То, для чего блока нет (`Dialog.List`, диалоги файлов, `WaitForStrings`, лог сессии, SFTP…),
остаётся на `crt`. Сравните файл отсюда с файлом того же номера в `examples/`, чтобы увидеть, что
заменяет каждый блок. Конструктор открывает их блоками: *Из шаблона ▾ → Примеры (короткие блоки)*.
В SecureCRT положите `shag_blocks.py` (из `~/.config/shagshell/scripts`) рядом со скриптом.

| Block / Блок | Long form / Длинная запись |
|---|---|
| `run(tab, cmd)` | find the prompt, `Screen.Send(cmd + "\r")`, `WaitForString("\n")`, `ReadString(prompt)` |
| `prompt()` | wait for the login to end, the text left of the cursor / ждать конца входа, текст левее курсора |
| `ask(msg, title, default, password)` | `crt.Dialog.Prompt(...)` |
| `printbox(text, title, buttons)` | `crt.Dialog.MessageBox(...)` |
| `connect(name)` | `crt.Session.Connect('/S "name"')` + `crt.GetActiveTab()` |
| `disconnect(tab)` | `tab.Session.Disconnect()` |
| `active_tab()` | `crt.GetActiveTab()` |
| `read_list(path)` | `open(...)`, lines without empty ones and `#` / строки без пустых и `#` |
| `log_to(tab, path)` + `run` | every command and its output into a file / каждая команда с выводом в файл |
| `append_file(path, text)` | `open(path, "a").write(text + "\n")` |
| `to_clipboard(t)` / `from_clipboard()` | `crt.Clipboard.Text = t` / `crt.Clipboard.Text` |
| `each_host(hosts, func)` | connect, `func(tab, host)`, disconnect for every host; a failed one does not stop the rest / для каждого хоста; ошибка одного не останавливает остальных |
