# 02_send_and_read.py (short blocks / короткие блоки)
#
# EN: Example/02 with short blocks. Sends one command to the current tab and shows its output.
#     run(None, command) does what Example/02 does by hand: finds the prompt on the screen, sends
#     the command and returns everything printed before the next prompt.
# RU: Example/02 на коротких блоках. Отправляет команду в текущую вкладку и показывает вывод.
#     run(None, команда) делает то, что Example/02 делает руками: находит приглашение на экране,
#     отправляет команду и возвращает всё напечатанное до следующего приглашения.
from shag_blocks import *

if not active_tab().Session.Connected:
    printbox("Open a connection first / Сначала откройте соединение")
    crt.Quit()

command = ask("Command / Команда:", "Example 02", "uname -a")
if command == "":
    crt.Quit()

output = run(None, command)          # EN: None = the script's tab / RU: None = вкладка скрипта
printbox(output or "(empty / пусто)", "Example 02: " + command)
