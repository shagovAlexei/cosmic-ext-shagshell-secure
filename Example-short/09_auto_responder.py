# 09_auto_responder.py (short blocks / короткие блоки)
#
# EN: Example/09 with short blocks. Watches the tab's output and answers known lines; stops after
#     "all done" or on Script -> Cancel. Waiting for one of several lines stays
#     Screen.WaitForStrings (no block for it), the last message is printbox().
# RU: Example/09 на коротких блоках. Следит за выводом вкладки и отвечает на известные строки;
#     останавливается после "all done" или по Script -> Cancel. Ожидание одной из нескольких строк
#     остаётся Screen.WaitForStrings (блока для него нет), последнее сообщение - printbox().
from shag_blocks import *

ANSWERS = [
    ("Continue? [y/N]", "y"),
    ("Press ENTER", ""),
    ("login:", "demo"),
]
STOP = "all done"

screen = crt.GetScriptTab().Screen
waits = [q for q, _ in ANSWERS] + [STOP]
while True:
    n = screen.WaitForStrings(waits)    # EN: no timeout: waits until one of them / RU: без таймаута
    if n == len(waits):                 # EN: the stop line / RU: строка остановки
        break
    screen.Send(ANSWERS[n - 1][1] + "\r")
printbox("Finished / Готово", "Example 09")
