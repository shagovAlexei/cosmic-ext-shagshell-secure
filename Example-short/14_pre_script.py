# 14_pre_script.py (short blocks / короткие блоки)
#
# EN: Example/14 with short blocks. A PRE-script (Session Options -> Connection -> "Script run
#     before connecting"): no tab yet; arguments: session name, host, user. Checks that the host
#     answers and asks to go on if not; sys.exit(1) = the session is NOT opened.
# RU: Example/14 на коротких блоках. PRE-скрипт (Session Options -> Connection -> "Скрипт до
#     подключения"): вкладки ещё нет; аргументы: имя сессии, хост, пользователь. Проверяет, что хост
#     отвечает, и спрашивает, продолжать ли, если нет; sys.exit(1) = сессия НЕ открывается.
import socket
import sys
from shag_blocks import *

name, host = arg(0), arg(1)

try:
    socket.create_connection((host, 22), timeout=3).close()
except OSError as e:
    answer = printbox(
        f"{host} does not answer on port 22 ({e}).\nConnect anyway?\n\n"
        f"{host} не отвечает на порту 22. Всё равно подключаться?",
        name, BUTTON_YESNO | ICON_WARN)
    if answer != IDYES:
        sys.exit(1)
