# 08_send_to_all_tabs.py (short blocks / короткие блоки)
#
# EN: Example/08 with short blocks. Sends one command to every connected tab, after a
#     confirmation. all_tabs() = crt.GetTabCount + crt.GetTab, connected(tab) =
#     tab.Session.Connected, send(text, tab) = tab.Screen.Send(text + "\r").
# RU: Example/08 на коротких блоках. Отправляет одну команду во все подключённые вкладки после
#     подтверждения. all_tabs() = crt.GetTabCount + crt.GetTab, connected(tab) =
#     tab.Session.Connected, send(текст, tab) = tab.Screen.Send(текст + "\r").
from shag_blocks import *

initial = script_tab()
command = ask("Command for ALL tabs / Команда во ВСЕ вкладки:", "Example 08", "ls -al")
if command == "":
    stop()
if printbox("Send to all tabs? / Отправить во все вкладки?\n\n" + command,
            "Example 08", BUTTON_YESNO) != IDYES:
    stop()

skipped = []
for i, tab in enumerate(all_tabs(), 1):
    if connected(tab):
        send(command, tab)
    else:
        skipped.append(str(i))         # EN: not connected / RU: не подключена
activate(initial)
printbox("Sent. Skipped tabs / Пропущены вкладки: " + (", ".join(skipped) or "-"), "Example 08")
