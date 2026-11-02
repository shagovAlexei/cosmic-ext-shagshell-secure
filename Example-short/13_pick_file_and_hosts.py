# 13_pick_file_and_hosts.py (short blocks / короткие блоки)
#
# EN: Example/13 with short blocks. Choose a commands file, choose saved sessions from a list, run
#     every command on each chosen session. pick_file() = Dialog.FileOpenDialog, read_list() reads
#     the file, choose(..., multi=True) = Dialog.List, connect() opens the session, run() waits for
#     the prompt before each command (no fixed pauses as in Example/13). The tabs stay open.
#     Put the names of YOUR saved sessions into SESSIONS.
# RU: Example/13 на коротких блоках. Выбрать файл с командами, выбрать из списка сохранённые сессии,
#     выполнить каждую команду на каждой выбранной. pick_file() = Dialog.FileOpenDialog,
#     read_list() читает файл, choose(..., multi=True) = Dialog.List, connect() открывает сессию,
#     run() ждёт приглашения перед каждой командой (без постоянных пауз, как в Example/13).
#     Вкладки остаются открытыми. Впишите в SESSIONS имена СВОИХ сохранённых сессий.
from shag_blocks import *

SESSIONS = ["Servers/web-01", "Servers/web-02", "Servers/db-01"]    # EN: yours / RU: ваши

path = pick_file("Commands file / Файл с командами", "Text (*.txt)|*.txt|All (*.*)|*.*", "Use")
if path == "":
    stop()
commands = read_list(path)

chosen = choose("Run %d commands on / Выполнить %d команд на:" % (len(commands), len(commands)),
                SESSIONS, "Pick sessions / Сессии", multi=True)
if not chosen:
    stop()

for name in chosen:
    tab = connect(name)
    for cmd in commands:
        run(tab, cmd, timeout=60)
printbox("Sent to %d sessions / Отправлено в %d сессий" % (len(chosen), len(chosen)), "Example 13", ICON_INFO)
