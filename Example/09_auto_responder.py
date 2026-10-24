# 09_auto_responder.py
#
# EN: Watches the tab's output and answers known lines: when the text on the left appears, the
#     script types the text on the right. Stops after the line "all done" or on Script -> Cancel.
#     WaitForStrings returns the 1-based number of the line that came (0 on timeout).
#     SecureCRT: like AutoResponder.py.
# RU: Следит за выводом вкладки и отвечает на известные строки: появилась левая строка - скрипт
#     вводит правую. Останавливается после строки "all done" или по Script -> Cancel.
#     WaitForStrings возвращает номер пришедшей строки (с 1; 0 - таймаут).
#     SecureCRT: как AutoResponder.py.

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
crt.Dialog.MessageBox("Finished / Готово", "Example 09")
