# 13_pick_file_and_hosts.py (short blocks / короткие блоки)
#
# EN: Example/13 with short blocks. Choose a commands file, choose saved sessions from a list, run
#     every command on each chosen session. read_list() reads the file, connect() opens the
#     session, run() waits for the prompt before each command (no fixed pauses as in Example/13).
#     The tabs stay open. Put the names of YOUR saved sessions into SESSIONS.
# RU: Example/13 на коротких блоках. Выбрать файл с командами, выбрать из списка сохранённые сессии,
#     выполнить каждую команду на каждой выбранной. read_list() читает файл, connect() открывает
#     сессию, run() ждёт приглашения перед каждой командой (без постоянных пауз, как в Example/13).
#     Вкладки остаются открытыми. Впишите в SESSIONS имена СВОИХ сохранённых сессий.
from shag_blocks import *

SESSIONS = ["Servers/web-01", "Servers/web-02", "Servers/db-01"]    # EN: yours / RU: ваши

path = crt.Dialog.FileOpenDialog("Commands file / Файл с командами", "Use", "",
                                 "Text (*.txt)|*.txt|All (*.*)|*.*")
if path == "":
    crt.Quit()
commands = read_list(path)

chosen = crt.Dialog.List("Run %d commands on / Выполнить %d команд на:" % (len(commands), len(commands)),
                         "Pick sessions / Сессии", SESSIONS, 0, True)
if not chosen:
    crt.Quit()

for name in chosen:
    tab = connect(name)
    for cmd in commands:
        run(tab, cmd, timeout=60)
printbox("Sent to %d sessions / Отправлено в %d сессий" % (len(chosen), len(chosen)), "Example 13", ICON_INFO)
