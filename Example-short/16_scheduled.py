# 16_scheduled.py (short blocks / короткие блоки)
#
# EN: Example/16 with short blocks. A job for Script -> Scheduler... (no tab): connect() opens the
#     saved session named by the job's argument (arg(0) = crt.Arguments[0]); status() puts the
#     time into the status bar.
# RU: Example/16 на коротких блоках. Задача для Script -> Планировщик... (без вкладки): connect()
#     открывает сохранённую сессию, имя которой задано аргументом задачи (arg(0) = crt.Arguments[0]);
#     status() пишет время в строку состояния.
import time
from shag_blocks import *

if arg(0):
    connect(arg(0))
status("scheduled run at " + time.strftime("%H:%M"))
