# 02_send_and_read.py
#
# EN: Sends one command to the current tab and shows its output.
#     Run it on a tab with a shell prompt (local or ssh).
#     How: the prompt is found from the screen (the text left of the cursor), then
#     ReadString(prompt) returns everything printed before the next prompt.
#     SecureCRT: the same idea as BasicReadStringExample.py.
# RU: Отправляет команду в текущую вкладку и показывает вывод.
#     Запускайте на вкладке с приглашением shell (локальный или ssh).
#     Как: приглашение берётся с экрана (текст левее курсора), затем ReadString(приглашение)
#     возвращает всё напечатанное до следующего приглашения.
#     SecureCRT: та же идея, что в BasicReadStringExample.py.

tab = crt.GetScriptTab()          # EN: the tab the script was started from / RU: вкладка запуска
screen = tab.Screen

if not tab.Session.Connected:
    crt.Dialog.MessageBox("Open a connection first / Сначала откройте соединение")
    crt.Quit()

command = crt.Dialog.Prompt("Command / Команда:", "Example 02", "uname -a")
if command == "":
    crt.Quit()

# EN: The prompt = the text of the cursor row left of the cursor. Do not `cd` between reading
#     it and the commands: the prompt would change and ReadString would wait for the old one.
# RU: Приглашение = текст строки курсора левее курсора. Не меняйте каталог (`cd`) между чтением
#     приглашения и командами: приглашение изменится, а ReadString будет ждать старое.
row, col = screen.CurrentRow, screen.CurrentColumn
prompt = screen.Get(row, 1, row, col - 1).strip()

screen.Send(command + "\r")
screen.WaitForString("\n")   # EN: end of the echoed command line / RU: конец строки с эхом команды
output = screen.ReadString(prompt, 30)   # EN: up to 30 s / RU: не дольше 30 с

crt.Dialog.MessageBox(output.strip() or "(empty / пусто)", "Example 02: " + command)
