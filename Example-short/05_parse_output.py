# 05_parse_output.py (short blocks / короткие блоки)
#
# EN: Example/05 with short blocks. Runs `df -h`, parses the table with a regular expression and
#     reports file systems that are more than LIMIT percent full.
# RU: Example/05 на коротких блоках. Выполняет `df -h`, разбирает таблицу регулярным выражением и
#     сообщает о файловых системах, заполненных больше чем на LIMIT процентов.
import re
from shag_blocks import *

LIMIT = 80          # EN: percent / RU: проценты

text = run("df -h")

# EN: A line looks like "/dev/sda1  50G  40G  10G  80% /". Group 1 = device, 2 = percent, 3 = mount.
# RU: Строка выглядит как "/dev/sda1  50G  40G  10G  80% /". Группа 1 - устройство, 2 - проценты, 3 - точка монтирования.
line = re.compile(r"^(\S+)\s+\S+\s+\S+\s+\S+\s+(\d+)%\s+(\S+)\s*$")
full = []
for l in text.splitlines():
    m = line.match(l.strip())
    if m and int(m.group(2)) > LIMIT:
        full.append("%s  %s%%  %s" % (m.group(1), m.group(2), m.group(3)))

printbox("\n".join(full) if full else "All below %d%% / Всё ниже %d%%" % (LIMIT, LIMIT), "Example 05")
