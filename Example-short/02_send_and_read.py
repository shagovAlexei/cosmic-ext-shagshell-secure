# 02_send_and_read.py (short blocks / короткие блоки)
#
# EN: Example/02 with short blocks. Sends one command to the current tab and shows its output.
#     run(command) does what Example/02 does by hand: finds the prompt on the screen, sends
#     the command and returns everything printed before the next prompt.
# RU: Example/02 на коротких блоках. Отправляет команду в текущую вкладку и показывает вывод.
#     run(команда) делает то, что Example/02 делает руками: находит приглашение на экране,
#     отправляет команду и возвращает всё напечатанное до следующего приглашения.
from shag_blocks import *

if not connected():
    printbox("Open a connection first / Сначала откройте соединение")
    stop()

command = ask("Command / Команда:", "Example 02", "uname -a")
if command == "":
    stop()

output = run(command)                # EN: on the script's tab / RU: на вкладке скрипта
printbox(output or "(empty / пусто)", "Example 02: " + command)
