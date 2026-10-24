# 04_many_hosts_to_file.py
#
# EN: Connects to several saved sessions one after another, runs the same commands on each and
#     writes everything into ONE file.
#     The list: a text file, one session name per line (empty lines and lines starting with #
#     are ignored). Arguments: <list file> [<result file>].
#     Run: shagshell --script 04_many_hosts_to_file.py ~/hosts.txt ~/report.txt
#     SecureCRT: like ConnectToMultipleSessionsAndSendCommands.py; there the list is a fixed file.
# RU: По очереди подключается к нескольким сохранённым сессиям, выполняет на каждой одинаковые
#     команды и пишет всё в ОДИН файл.
#     Список: текстовый файл, одно имя сессии в строке (пустые строки и строки с # пропускаются).
#     Аргументы: <файл списка> [<файл результата>].
#     Запуск: shagshell --script 04_many_hosts_to_file.py ~/hosts.txt ~/report.txt
#     SecureCRT: как ConnectToMultipleSessionsAndSendCommands.py; там список - постоянный файл.
import os

COMMANDS = ["hostname", "uptime"]      # EN: change to yours / RU: подставьте свои

if len(crt.Arguments) < 1:
    crt.Dialog.MessageBox("Usage: <hosts list file> [result file]")
    crt.Quit()
hosts_file = os.path.expanduser(crt.Arguments[0])
result_file = os.path.expanduser(crt.Arguments[1] if len(crt.Arguments) > 1 else "~/hosts_report.txt")

with open(hosts_file, encoding="utf-8") as f:
    names = [l.strip() for l in f if l.strip() and not l.lstrip().startswith("#")]

report = []
for name in names:
    report.append("=== %s ===" % name)
    try:
        crt.Session.Connect('/S "%s"' % name)
    except ScriptError:
        # EN: one bad host does not stop the rest / RU: один плохой хост не останавливает остальных
        report.append("ERROR: " + crt.GetLastErrorMessage())
        continue
    tab = crt.GetTab(crt.GetTabCount())
    screen = tab.Screen
    while screen.WaitForCursor(1):      # EN: login finished / RU: вход завершён
        pass
    row, col = screen.CurrentRow, screen.CurrentColumn
    prompt = screen.Get(row, 1, row, col - 1).rstrip()[-12:]
    for cmd in COMMANDS:
        screen.Send(cmd + "\r")
        screen.WaitForString("\n")   # EN: end of the echoed command line / RU: конец строки с эхом команды
        report.append("$ " + cmd)
        report.append(screen.ReadString(prompt, 30).strip())
    tab.Session.Disconnect()

with open(result_file, "w", encoding="utf-8") as f:
    f.write("\n".join(report) + "\n")
crt.Dialog.MessageBox("%d hosts, report: %s" % (len(names), result_file), "Example 04")
