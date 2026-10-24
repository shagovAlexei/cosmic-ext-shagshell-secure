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

def wait_for_prompt(screen, timeout=60):
    """EN: wait until the login is over: the cursor row has text (the prompt) and the
    cursor stood still for a second (askpass / host key dialogs, MOTD and slow links take time).
    RU: ждать конца входа: в строке курсора есть текст (приглашение), а курсор секунду
    не двигался (диалоги askpass / ключа хоста, MOTD и медленный канал занимают время)."""
    import time
    end = time.time() + timeout
    while time.time() < end:
        row, col = screen.CurrentRow, screen.CurrentColumn
        line = screen.Get(row, 1, row, col - 1).rstrip()
        if line and not screen.WaitForCursor(1):
            return
        crt.Sleep(200)
    crt.Dialog.MessageBox("No prompt / Приглашения нет")
    crt.Quit()


name = crt.Arguments[0] if len(crt.Arguments) else crt.Dialog.Prompt("Session / Сессия:", "Example 03")
if name == "":
    crt.Quit()

try:
    crt.Session.Connect('/S "%s"' % name)   # EN: a new tab opens / RU: открывается новая вкладка
except ScriptError:
    crt.Dialog.MessageBox("Cannot open: " + crt.GetLastErrorMessage(), "Example 03")
    crt.Quit()

tab = crt.GetActiveTab()                           # EN: a new tab is the active one / RU: новая вкладка активна
screen = tab.Screen

wait_for_prompt(screen)
row, col = screen.CurrentRow, screen.CurrentColumn
prompt = screen.Get(row, 1, row, col - 1).strip()

command = "hostname; uptime"
screen.Send(command + "\r")
screen.WaitForString("\n")   # EN: end of the echoed command line / RU: конец строки с эхом команды
result = screen.ReadString(prompt, 30)
crt.Dialog.MessageBox(result.strip(), "Example 03: " + name)

tab.Session.Disconnect()
