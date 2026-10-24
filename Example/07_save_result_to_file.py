# 07_save_result_to_file.py
#
# EN: Runs a command and saves its output to ~/ShagShellResults/<command>_<date>.txt.
#     Shows: building a file name, creating a folder, writing text (UTF-8).
#     Run on a tab with a shell prompt.
#     SecureCRT: like LogOutputOfSpecificCommand-UseReadString.py (one file per command).
# RU: Выполняет команду и сохраняет вывод в ~/ShagShellResults/<команда>_<дата>.txt.
#     Показано: составление имени файла, создание папки, запись текста (UTF-8).
#     Запускать на вкладке с приглашением shell.
#     SecureCRT: как LogOutputOfSpecificCommand-UseReadString.py (файл на команду).
import datetime
import os
import re

command = crt.Arguments[0] if len(crt.Arguments) else "ls -l"
folder = os.path.expanduser("~/ShagShellResults")
os.makedirs(folder, exist_ok=True)

screen = crt.GetScriptTab().Screen
row, col = screen.CurrentRow, screen.CurrentColumn
prompt = screen.Get(row, 1, row, col - 1).rstrip()[-12:]
screen.Send(command + "\r")
screen.WaitForString("\n")   # EN: end of the echoed command line / RU: конец строки с эхом команды
output = screen.ReadString(prompt, 60)

# EN: keep letters and digits in the file name / RU: в имени файла оставляем буквы и цифры
safe = re.sub(r"\W+", "_", command).strip("_") or "command"
path = os.path.join(folder, "%s_%s.txt" % (safe, datetime.datetime.now().strftime("%Y-%m-%d_%H%M%S")))
with open(path, "w", encoding="utf-8") as f:
    f.write(output.replace("\r\n", "\n"))
crt.Dialog.MessageBox("Saved / Сохранено:\n" + path, "Example 07")
