# 13_pick_file_and_hosts.py
#
# EN: Choose a text file with commands, choose the saved sessions to run it on from a list,
#     and send every command to each chosen session's tab. Shows FileOpenDialog + List(multi).
#     The sessions are the ones named in the list below: put the names of YOUR saved sessions.
#     SecureCRT: the same flow with FileOpenDialog; the multi-choice list is ours.
# RU: Выбрать текстовый файл с командами, выбрать из списка сохранённые сессии и отправить
#     каждую команду в вкладку каждой выбранной сессии. Показано: FileOpenDialog + List(multi).
#     Сессии - те, что названы в списке ниже: впишите имена СВОИХ сохранённых сессий.
#     SecureCRT: тот же порядок с FileOpenDialog; список с множественным выбором - наш.

SESSIONS = ["Servers/web-01", "Servers/web-02", "Servers/db-01"]    # EN: yours / RU: ваши

path = crt.Dialog.FileOpenDialog("Commands file / Файл с командами", "Use", "",
                                 "Text (*.txt)|*.txt|All (*.*)|*.*")
if path == "":
    crt.Quit()
with open(path, encoding="utf-8") as f:
    commands = [l.strip() for l in f if l.strip() and not l.startswith("#")]

chosen = crt.Dialog.List("Run %d commands on / Выполнить %d команд на:" % (len(commands), len(commands)),
                         "Pick sessions / Сессии", SESSIONS, 0, True)
if not chosen:
    crt.Quit()

for name in chosen:
    crt.Session.Connect('/S "%s"' % name)
    tab = crt.GetActiveTab()                 # EN: the new tab / RU: новая вкладка
    tab.Screen.WaitForString("\n", 1)        # EN: give the login a moment / RU: дать входу время
    crt.Sleep(2000)
    for cmd in commands:
        tab.Screen.Send(cmd + "\r")
        crt.Sleep(300)
crt.Dialog.MessageBox("Sent to %d sessions / Отправлено в %d сессий" % (len(chosen), len(chosen)), "Example 13", ICON_INFO)
