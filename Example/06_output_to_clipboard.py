# 06_output_to_clipboard.py
#
# EN: Runs several commands and puts the whole output into the clipboard.
#     Run on a tab with a shell prompt, then paste anywhere (Ctrl+V).
#     SecureCRT: like CopyOutputToClipboard.py (crt.Clipboard.Text).
# RU: Выполняет несколько команд и кладёт весь вывод в буфер обмена.
#     Запускать на вкладке с приглашением shell, потом вставить где угодно (Ctrl+V).
#     SecureCRT: как CopyOutputToClipboard.py (crt.Clipboard.Text).

COMMANDS = ["date", "whoami", "pwd"]

screen = crt.GetScriptTab().Screen
row, col = screen.CurrentRow, screen.CurrentColumn
prompt = screen.Get(row, 1, row, col - 1).strip()

parts = []
for cmd in COMMANDS:
    screen.Send(cmd + "\r")
    screen.WaitForString("\n")   # EN: end of the echoed command line / RU: конец строки с эхом команды
    parts.append("$ %s\n%s" % (cmd, screen.ReadString(prompt, 30).strip()))

crt.Clipboard.Text = "\n\n".join(parts)        # EN: set / RU: записать
crt.Dialog.MessageBox("In the clipboard / В буфере обмена:\n\n" + crt.Clipboard.Text, "Example 06")   # EN: read back / RU: прочитать
