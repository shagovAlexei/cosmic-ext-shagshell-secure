# 16_scheduled.py (short blocks / короткие блоки)
#
# EN: Example/16 with short blocks. A job for Script -> Scheduler... (no tab): connect() opens the
#     saved session named by the job's argument; the time goes to the status bar.
# RU: Example/16 на коротких блоках. Задача для Script -> Планировщик... (без вкладки): connect()
#     открывает сохранённую сессию, имя которой задано аргументом задачи; время - в строку состояния.
import sys
import time
from shag_blocks import *

if len(sys.argv) > 1:
    connect(sys.argv[1])
crt.Session.SetStatusText("scheduled run at " + time.strftime("%H:%M"))
