# 08_send_to_all_tabs.py (short blocks / короткие блоки)
#
# EN: Example/08 with short blocks. Sends one command to every connected tab, after a
#     confirmation. Going over the tabs stays `crt` (no block for it).
# RU: Example/08 на коротких блоках. Отправляет одну команду во все подключённые вкладки после
#     подтверждения. Перебор вкладок остаётся на `crt` (блока для него нет).
from shag_blocks import *

initial = active_tab()
command = ask("Command for ALL tabs / Команда во ВСЕ вкладки:", "Example 08", "ls -al")
if command == "":
    crt.Quit()
if printbox("Send to all tabs? / Отправить во все вкладки?\n\n" + command,
            "Example 08", BUTTON_YESNO) != IDYES:
    crt.Quit()

skipped = []
for i in range(1, crt.GetTabCount() + 1):
    tab = crt.GetTab(i)
    if tab.Session.Connected:
        tab.Screen.Send(command + "\r")
    else:
        skipped.append(str(i))         # EN: not connected / RU: не подключена
initial.Activate()
printbox("Sent. Skipped tabs / Пропущены вкладки: " + (", ".join(skipped) or "-"), "Example 08")
