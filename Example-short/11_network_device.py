# 11_network_device.py (short blocks / короткие блоки)
#
# EN: Example/11 with short blocks. A Cisco-like device: `enable`, turn off paging, read the whole
#     configuration, save it to a file named after the device and the date.
#     prompt() is the prompt on the screen; run(..., until="#") waits for "#" instead of the
#     prompt (a command that changes it). The password is asked in a masked dialog and never
#     stored. Do not call log_to() here: run() would write the password into the log.
# RU: Example/11 на коротких блоках. Cisco-подобное устройство: `enable`, отключить постраничный
#     вывод, прочитать всю конфигурацию, сохранить в файл с именем устройства и датой.
#     prompt() - приглашение на экране; run(..., until="#") ждёт "#" вместо приглашения (команда,
#     которая его меняет). Пароль спрашивается в скрытом диалоге и нигде не хранится.
#     Не вызывайте здесь log_to(): run() записал бы пароль в лог.
import datetime
import os
from shag_blocks import *

# EN: The device name = the prompt without its last character ('>' or '#').
# RU: Имя устройства = приглашение без последнего символа ('>' или '#').
p = prompt()
device = p.rstrip(">#") or "device"

if p.endswith(">"):                    # EN: user mode -> privileged / RU: пользовательский -> привилегированный
    password = ask("enable password:", "Example 11", "", True)
    try:
        run(None, "enable", until="assword:")
        run(None, password, until="#")
    except TimeoutError:
        printbox("enable failed / не удалось войти в enable")
        crt.Quit()

run(None, "terminal length 0")         # EN: no "--More--" / RU: без "--More--"
config = run(None, "show running-config", timeout=120)

path = os.path.expanduser("~/%s_%s.cfg" % (device, datetime.date.today().isoformat()))
with open(path, "w", encoding="utf-8") as f:
    f.write(config.replace("\r\n", "\n"))
printbox("Saved / Сохранено:\n" + path, "Example 11")
