# 08_send_to_all_tabs.py
#
# EN: Sends one command to every connected tab, after a confirmation.
#     Run from any tab. (A built-in alternative without a script: View -> Command Window.)
#     SecureCRT: the same as SendCommandToAllTabs.py.
# RU: Отправляет одну команду во все подключённые вкладки после подтверждения.
#     Запускать с любой вкладки. (Встроенная замена без скрипта: View -> Command Window.)
#     SecureCRT: то же, что SendCommandToAllTabs.py.

initial = crt.GetScriptTab()
command = crt.Dialog.Prompt("Command for ALL tabs / Команда во ВСЕ вкладки:", "Example 08", "ls -al")
if command == "":
    crt.Quit()
if crt.Dialog.MessageBox("Send to all tabs? / Отправить во все вкладки?\n\n" + command,
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
crt.Dialog.MessageBox("Sent. Skipped tabs / Пропущены вкладки: " + (", ".join(skipped) or "-"), "Example 08")
