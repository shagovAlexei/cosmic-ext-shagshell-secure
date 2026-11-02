# 09_auto_responder.py (short blocks / короткие блоки)
#
# EN: Example/09 with short blocks. Watches the tab's output and answers known lines; stops after
#     "all done" or on Script -> Cancel. wait_any(texts) = Screen.WaitForStrings: the number of
#     the line that came (from 1); send(text) = Screen.Send(text + "\r").
# RU: Example/09 на коротких блоках. Следит за выводом вкладки и отвечает на известные строки;
#     останавливается после "all done" или по Script -> Cancel. wait_any(строки) =
#     Screen.WaitForStrings: номер пришедшей строки (с 1); send(текст) = Screen.Send(текст + "\r").
from shag_blocks import *

ANSWERS = [
    ("Continue? [y/N]", "y"),
    ("Press ENTER", ""),
    ("login:", "demo"),
]
STOP = "all done"

waits = [q for q, _ in ANSWERS] + [STOP]
while True:
    n = wait_any(waits)                 # EN: no timeout: waits until one of them / RU: без таймаута
    if n == len(waits):                 # EN: the stop line / RU: строка остановки
        break
    send(ANSWERS[n - 1][1])
printbox("Finished / Готово", "Example 09")
