# 10_comment_in_log.py (short blocks / короткие блоки)
#
# EN: Example/10 with short blocks. Writes a timestamped comment into the tab's session log and
#     starts the log if it is off. The session log itself stays `crt` (Session.Log / LogText).
# RU: Example/10 на коротких блоках. Пишет комментарий с отметкой времени в лог сессии вкладки и
#     включает лог, если он выключен. Сам лог сессии остаётся на `crt` (Session.Log / LogText).
import datetime
from shag_blocks import *

session = crt.GetScriptTab().Session
text = ask("Comment / Комментарий:", "Example 10")
if text == "":
    crt.Quit()

if session.LogFileName == "":
    session.Log(True, True)             # EN: start, append / RU: включить, дописывать
session.LogText("\n/* %s: %s */\n" % (datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S"), text))
printbox("Added to / Добавлено в:\n" + session.LogFileName, "Example 10")
