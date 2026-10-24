# 16_scheduled.py
#
# EN: A script for Script -> Scheduler... (no tab: crt.Screen is not available). Opens a saved
#     session by name given as the job's argument and notes the time in the status bar.
# RU: Скрипт для Script -> Планировщик... (без вкладки: crt.Screen недоступен). Открывает сохранённую
#     сессию, имя которой задано аргументом задачи, и пишет время в строку состояния.
import sys
import time

if len(sys.argv) > 1:
    crt.Session.Connect('/S "%s"' % sys.argv[1])
crt.Session.SetStatusText("scheduled run at " + time.strftime("%H:%M"))
