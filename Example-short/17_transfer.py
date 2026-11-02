# 17_transfer.py (short blocks / короткие блоки)
#
# EN: Example/17 with short blocks. Copies a file to the server and back over the SFTP of this SSH
#     tab (each call waits until it is done) and locks the tab for the user while it runs:
#     upload() / download() = FileTransfer.Upload / Download, lock() / unlock() = Session.Lock /
#     Unlock. Run it on an SSH tab.
# RU: Example/17 на коротких блоках. Копирует файл на сервер и обратно по SFTP этой SSH-вкладки
#     (каждый вызов ждёт конца) и блокирует вкладку для пользователя на время работы:
#     upload() / download() = FileTransfer.Upload / Download, lock() / unlock() = Session.Lock /
#     Unlock. Запускайте на SSH-вкладке.
import os
import tempfile
from shag_blocks import *

local = os.path.join(tempfile.gettempdir(), "shagshell_transfer_demo.txt")
with open(local, "w") as f:
    f.write("hello from a crt script\n")

lock()
try:
    upload(local, "shagshell_demo.txt")                         # EN: into the server's home / RU: в домашнюю папку
    download("shagshell_demo.txt", "shagshell_demo_back.txt")   # ~/Downloads
finally:
    unlock()
printbox("Done: shagshell_demo.txt is in the server's home, and a copy is in ~/Downloads")
