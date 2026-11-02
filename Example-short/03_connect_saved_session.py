# 03_connect_saved_session.py (short blocks / короткие блоки)
#
# EN: Example/03 with short blocks. Opens a SAVED session, runs a command, shows the result and
#     disconnects. The session name is the first argument (or asked). run() waits for the login
#     to end by itself, so no wait_for_prompt here.
#     Run: shagshell --script 03_connect_saved_session.py "Servers/web1"
# RU: Example/03 на коротких блоках. Открывает СОХРАНЁННУЮ сессию, выполняет команду, показывает
#     результат и отключается. Имя сессии - первый аргумент (или спрашивается). run() сам ждёт
#     конца входа, поэтому wait_for_prompt здесь не нужен.
#     Запуск: shagshell --script 03_connect_saved_session.py "Servers/web1"
from shag_blocks import *

name = arg(0) or ask("Session / Сессия:", "Example 03")
if name == "":
    stop()

try:
    tab = connect(name)              # EN: a new tab / RU: новая вкладка
except ScriptError:
    printbox("Cannot open: " + last_error(), "Example 03")
    stop()

try:
    result = run(tab, "hostname; uptime", timeout=60)
except TimeoutError:
    printbox("No prompt / Приглашения нет")
    stop()
printbox(result, "Example 03: " + name)

disconnect(tab)
