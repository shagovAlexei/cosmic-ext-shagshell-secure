# 10_comment_in_log.py (short blocks / короткие блоки)
#
# EN: Example/10 with short blocks. Writes a timestamped comment into the tab's session log.
#     log_text() starts the log if it is off (Session.Log), writes (Session.LogText) and returns
#     the log file (Session.LogFileName).
# RU: Example/10 на коротких блоках. Пишет комментарий с отметкой времени в лог сессии вкладки.
#     log_text() включает лог, если он выключен (Session.Log), пишет (Session.LogText) и
#     возвращает файл лога (Session.LogFileName).
import datetime
from shag_blocks import *

text = ask("Comment / Комментарий:", "Example 10")
if text == "":
    stop()

log = log_text("\n/* %s: %s */\n" % (datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S"), text))
printbox("Added to / Добавлено в:\n" + log, "Example 10")
