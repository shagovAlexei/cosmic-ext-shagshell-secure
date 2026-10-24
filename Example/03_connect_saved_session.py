# 03_connect_saved_session.py
#
# EN: Opens a SAVED session (by its name in the Session Manager), runs a command, shows the
#     result and disconnects. The session name is the first argument (or asked).
#     Run: shagshell --script 03_connect_saved_session.py "Servers/web1"
#     SecureCRT: crt.Session.Connect('/S "name"') works the same. A session that needs a
#     password uses the one saved for it (askpass); the script never sees secrets.
# RU: Открывает СОХРАНЁННУЮ сессию (по имени из Session Manager), выполняет команду, показывает
#     результат и отключается. Имя сессии - первый аргумент (или спрашивается).
#     Запуск: shagshell --script 03_connect_saved_session.py "Servers/web1"
#     SecureCRT: crt.Session.Connect('/S "имя"') работает так же. Сессия с паролем берёт
#     сохранённый для неё (askpass); скрипт секретов не видит.

name = crt.Arguments[0] if len(crt.Arguments) else crt.Dialog.Prompt("Session / Сессия:", "Example 03")
if name == "":
    crt.Quit()

try:
    crt.Session.Connect('/S "%s"' % name)   # EN: a new tab opens / RU: открывается новая вкладка
except ScriptError:
    crt.Dialog.MessageBox("Cannot open: " + crt.GetLastErrorMessage(), "Example 03")
    crt.Quit()

tab = crt.GetTab(crt.GetTabCount())          # EN: the newest tab / RU: самая новая вкладка
screen = tab.Screen

# EN: Wait for the login to finish: the cursor stops moving for a second.
# RU: Ждём окончания входа: курсор не двигается секунду.
while screen.WaitForCursor(1):
    pass
row, col = screen.CurrentRow, screen.CurrentColumn
prompt = screen.Get(row, 1, row, col - 1).rstrip()[-12:]

command = "hostname; uptime"
screen.Send(command + "\r")
screen.WaitForString("\n")   # EN: end of the echoed command line / RU: конец строки с эхом команды
result = screen.ReadString(prompt, 30)
crt.Dialog.MessageBox(result.strip(), "Example 03: " + name)

tab.Session.Disconnect()
