# 14_pre_script.py
#
# EN: A PRE-script: set it in Session Options -> Connection -> "Script run before connecting".
#     It runs before ssh starts, with no tab (so no crt.Screen); arguments: session name, host, user.
#     Here it checks that the host answers and asks to go on if not. If the script ends with an
#     error or sys.exit(1), the session is NOT opened.
# RU: PRE-скрипт: задаётся в Session Options -> Connection -> "Скрипт до подключения".
#     Запускается до ssh, без вкладки (crt.Screen недоступен); аргументы: имя сессии, хост, пользователь.
#     Здесь он проверяет, что хост отвечает, и спрашивает, продолжать ли, если нет. Если скрипт
#     завершился ошибкой или sys.exit(1), сессия НЕ открывается.
import socket
import sys

name, host = crt.Arguments[0], crt.Arguments[1]

try:
    socket.create_connection((host, 22), timeout=3).close()
except OSError as e:
    answer = crt.Dialog.MessageBox(
        f"{host} does not answer on port 22 ({e}).\nConnect anyway?\n\n"
        f"{host} не отвечает на порту 22. Всё равно подключаться?",
        name, BUTTON_YESNO | ICON_WARNING)
    if answer != IDYES:
        sys.exit(1)
