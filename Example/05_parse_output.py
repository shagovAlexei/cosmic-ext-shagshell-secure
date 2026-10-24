# 05_parse_output.py
#
# EN: Runs `df -h`, parses the table with a regular expression and reports file systems that
#     are more than LIMIT percent full. Shows how to turn command output into data.
#     Run on a tab with a Linux shell prompt.
#     SecureCRT: the same approach (ReadString + Python `re`).
# RU: Выполняет `df -h`, разбирает таблицу регулярным выражением и сообщает о файловых системах,
#     заполненных больше чем на LIMIT процентов. Пример превращения вывода команды в данные.
#     Запускать на вкладке с приглашением Linux shell.
#     SecureCRT: тот же подход (ReadString + Python `re`).
import re

LIMIT = 80          # EN: percent / RU: проценты

screen = crt.GetScriptTab().Screen
row, col = screen.CurrentRow, screen.CurrentColumn
prompt = screen.Get(row, 1, row, col - 1).rstrip()[-12:]

screen.Send("df -h\r")
screen.WaitForString("\n")   # EN: end of the echoed command line / RU: конец строки с эхом команды
text = screen.ReadString(prompt, 30)

# EN: A line looks like "/dev/sda1  50G  40G  10G  80% /". Group 1 = device, 2 = percent, 3 = mount.
# RU: Строка выглядит как "/dev/sda1  50G  40G  10G  80% /". Группа 1 - устройство, 2 - проценты, 3 - точка монтирования.
line = re.compile(r"^(\S+)\s+\S+\s+\S+\s+\S+\s+(\d+)%\s+(\S+)\s*$")
full = []
for l in text.splitlines():
    m = line.match(l.strip())
    if m and int(m.group(2)) > LIMIT:
        full.append("%s  %s%%  %s" % (m.group(1), m.group(2), m.group(3)))

crt.Dialog.MessageBox("\n".join(full) if full else "All below %d%% / Всё ниже %d%%" % (LIMIT, LIMIT), "Example 05")
