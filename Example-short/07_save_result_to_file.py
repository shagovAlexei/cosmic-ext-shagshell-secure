# 07_save_result_to_file.py (short blocks / короткие блоки)
#
# EN: Example/07 with short blocks. Runs a command and saves its output to
#     ~/ShagShellResults/<command>_<date>.txt.
# RU: Example/07 на коротких блоках. Выполняет команду и сохраняет вывод в
#     ~/ShagShellResults/<команда>_<дата>.txt.
import datetime
import os
import re
from shag_blocks import *

command = arg(0, "ls -l")
folder = os.path.expanduser("~/ShagShellResults")
os.makedirs(folder, exist_ok=True)

output = run(command, timeout=60)

# EN: keep letters and digits in the file name / RU: в имени файла оставляем буквы и цифры
safe = re.sub(r"\W+", "_", command).strip("_") or "command"
path = os.path.join(folder, "%s_%s.txt" % (safe, datetime.datetime.now().strftime("%Y-%m-%d_%H%M%S")))
append_file(path, output.replace("\r\n", "\n"))
printbox("Saved / Сохранено:\n" + path, "Example 07")
