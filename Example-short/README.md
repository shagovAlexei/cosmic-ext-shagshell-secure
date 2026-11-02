# Example scripts with short blocks / Примеры на коротких блоках

**EN.** The same scripts as in `examples/`, written with the Script Builder's short blocks
(`from shag_blocks import *`): no `crt.…` call is left in them. Compare a file here with the one of the same number in `examples/` to see what each block
replaces. The Script Builder opens them as blocks: *From a template ▾ → Examples (short blocks)*.
In SecureCRT put `shag_blocks.py` (from `~/.config/shagshell/scripts`) next to the script.

**RU.** Те же скрипты, что в `examples/`, написанные короткими блоками конструктора скриптов
(`from shag_blocks import *`): вызовов `crt.…` в них не осталось. Сравните файл отсюда с файлом того же номера в `examples/`, чтобы увидеть, что
заменяет каждый блок. Конструктор открывает их блоками: *Из шаблона ▾ → Примеры (короткие блоки)*.
В SecureCRT положите `shag_blocks.py` (из `~/.config/shagshell/scripts`) рядом со скриптом.

| Block / Блок | Long form / Длинная запись |
|---|---|
| `run(cmd, timeout, until, tab)` | find the prompt, `Screen.Send(cmd + "\r")`, `WaitForString("\n")`, `ReadString(prompt)` |
| `prompt()` | wait for the login to end, the text left of the cursor / ждать конца входа, текст левее курсора |
| `ask(msg, title, default, password)` | `crt.Dialog.Prompt(...)` |
| `printbox(text, title, buttons)` | `crt.Dialog.MessageBox(...)` |
| `connect(name)` | `crt.Session.Connect('/S "name"')` + `crt.GetActiveTab()` |
| `disconnect(tab)` | `tab.Session.Disconnect()` |
| `active_tab()` | `crt.GetActiveTab()` |
| `read_list(path)` | `open(...)`, lines without empty ones and `#` / строки без пустых и `#` |
| `log_to(path, tab)` + `run` | every command and its output into a file / каждая команда с выводом в файл |
| `append_file(path, text)` | `open(path, "a").write(text + "\n")` |
| `to_clipboard(t)` / `from_clipboard()` | `crt.Clipboard.Text = t` / `crt.Clipboard.Text` |
| `each_host(hosts, func)` | connect, `func(tab, host)`, disconnect for every host; a failed one does not stop the rest / для каждого хоста; ошибка одного не останавливает остальных |
| `send(text, tab)` / `send_keys(text, tab)` | `tab.Screen.Send(text + "\r")` / `tab.Screen.SendKeys(text)` |
| `wait_for(text, timeout, tab)` / `wait_any(texts, timeout, tab)` | `Screen.WaitForString` / `Screen.WaitForStrings` |
| `clear(tab)` | `tab.Screen.Clear()` |
| `script_tab()` / `all_tabs()` | `crt.GetScriptTab()` / `crt.GetTab(i)` for i in 1..`crt.GetTabCount()` |
| `connected(tab)` / `activate(tab)` | `tab.Session.Connected` / `tab.Activate()` |
| `status(text, tab)` | `tab.Session.SetStatusText(text)` |
| `log_text(text, tab)` | `Session.Log(True, True)` when off + `Session.LogText(text)`; returns `LogFileName` |
| `choose(message, items, title, default, multi)` | `crt.Dialog.List(message, title, items, default, multi)` |
| `pick_file(title, filter, button, default)` | `crt.Dialog.FileOpenDialog(title, button, default, filter)` |
| `arg(n, default)` / `args()` | `crt.Arguments[n] if len(crt.Arguments) > n else default` / `crt.Arguments` |
| `stop()` / `last_error()` | `crt.Quit()` / `crt.GetLastErrorMessage()` |
| `new_session(name, host, user, port, folder)` | `crt.Session.Create(...)` |
| `lock(tab)` / `unlock(tab)` | `Session.Lock()` / `Session.Unlock()` |
| `upload(local, remote)` / `download(remote, local)` | `crt.FileTransfer.Upload` / `Download` |
| `pause(ms)` | `crt.Sleep(ms)` |
