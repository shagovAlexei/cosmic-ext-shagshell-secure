# 17_transfer.py
#
# EN: Copies a file to the server and back over the SFTP of this SSH tab (each call waits until
#     it is done), and locks the tab for the user while it runs. Run it on an SSH tab.
# RU: Копирует файл на сервер и обратно по SFTP этой SSH-вкладки (каждый вызов ждёт конца) и
#     блокирует вкладку для пользователя на время работы. Запускайте на SSH-вкладке.
import os
import tempfile

local = os.path.join(tempfile.gettempdir(), "shagshell_transfer_demo.txt")
with open(local, "w") as f:
    f.write("hello from a crt script\n")

crt.Session.Lock()
try:
    crt.FileTransfer.Upload(local, "shagshell_demo.txt")        # into the server's home
    crt.FileTransfer.Download("shagshell_demo.txt", "shagshell_demo_back.txt")  # ~/Downloads
finally:
    crt.Session.Unlock()
crt.Dialog.MessageBox("Done: shagshell_demo.txt is in the server's home, and a copy is in ~/Downloads")
