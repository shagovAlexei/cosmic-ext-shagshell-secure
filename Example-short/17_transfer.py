# 17_transfer.py (short blocks / короткие блоки)
#
# EN: Example/17 with short blocks: the last message is printbox(); the SFTP copy and the tab
#     lock stay `crt` (FileTransfer, Session.Lock).
#     Run it on an SSH tab.
# RU: Example/17 на коротких блоках: последнее сообщение - printbox();
#     копирование по SFTP и блокировка вкладки остаются на `crt` (FileTransfer, Session.Lock).
#     Запускайте на SSH-вкладке.
import os
import tempfile
from shag_blocks import *

local = os.path.join(tempfile.gettempdir(), "shagshell_transfer_demo.txt")
with open(local, "w") as f:
    f.write("hello from a crt script\n")

crt.Session.Lock()
try:
    crt.FileTransfer.Upload(local, "shagshell_demo.txt")        # into the server's home
    crt.FileTransfer.Download("shagshell_demo.txt", "shagshell_demo_back.txt")  # ~/Downloads
finally:
    crt.Session.Unlock()
printbox("Done: shagshell_demo.txt is in the server's home, and a copy is in ~/Downloads")
