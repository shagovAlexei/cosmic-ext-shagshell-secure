# 11_network_device.py
#
# EN: Typical work with a Cisco-like device (also MikroTik with other commands): turn off paging,
#     read the whole configuration, save it to a file named after the device and the date.
#     Run on a tab connected to the device. `enable` asks for a password: the script asks you
#     for it in a masked dialog and never stores it.
#     SecureCRT: like CopyOutputToClipboard.py, but the result goes to a file.
# RU: Типичная работа с Cisco-подобным устройством (MikroTik - с другими командами): отключить
#     постраничный вывод, прочитать всю конфигурацию, сохранить в файл с именем устройства и
#     датой. Запускать на вкладке, подключённой к устройству. `enable` спрашивает пароль: скрипт
#     запрашивает его в скрытом диалоге и нигде не хранит.
#     SecureCRT: как CopyOutputToClipboard.py, но результат идёт в файл.
import datetime
import os

screen = crt.GetScriptTab().Screen

def send_expect(text, expect, timeout=30):
    """EN: send a line and wait for `expect` (False on timeout). RU: отправить строку и ждать `expect`."""
    screen.Send(text + "\r")
    return screen.WaitForString(expect, timeout)

# EN: The device name = the prompt without its last character ('>' or '#').
# RU: Имя устройства = приглашение без последнего символа ('>' или '#').
row, col = screen.CurrentRow, screen.CurrentColumn
prompt = screen.Get(row, 1, row, col - 1).strip()
device = prompt.rstrip(">#") or "device"

if prompt.endswith(">"):                    # EN: user mode -> privileged / RU: пользовательский -> привилегированный
    password = crt.Dialog.Prompt("enable password:", "Example 11", "", True)
    if not send_expect("enable", "assword:") or not send_expect(password, "#"):
        crt.Dialog.MessageBox("enable failed / не удалось войти в enable")
        crt.Quit()
    prompt = device + "#"

send_expect("terminal length 0", prompt)    # EN: no "--More--" / RU: без "--More--"
screen.Send("show running-config\r")
screen.WaitForString("\n")   # EN: end of the echoed command line / RU: конец строки с эхом команды
config = screen.ReadString(prompt, 120)

path = os.path.expanduser("~/%s_%s.cfg" % (device, datetime.date.today().isoformat()))
with open(path, "w", encoding="utf-8") as f:
    f.write(config.replace("\r\n", "\n"))
crt.Dialog.MessageBox("Saved / Сохранено:\n" + path, "Example 11")
