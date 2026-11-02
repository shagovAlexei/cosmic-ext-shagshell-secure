# 04_many_hosts_to_file.py (short blocks / короткие блоки)
#
# EN: Example/04 with short blocks. Connects to several saved sessions one after another, runs the
#     same commands on each and writes everything into ONE file.
#     read_list() reads the list (empty lines and # comments skipped), each_host() connects,
#     calls one_host and disconnects; log_to() sends every run() with its output to the file.
#     A host that fails does not stop the others: it is named at the end of the file.
#     Arguments: <list file> [<result file>].
#     Run: shagshell --script 04_many_hosts_to_file.py ~/hosts.txt ~/report.txt
# RU: Example/04 на коротких блоках. По очереди подключается к нескольким сохранённым сессиям,
#     выполняет на каждой одинаковые команды и пишет всё в ОДИН файл.
#     read_list() читает список (пустые строки и комментарии # пропускаются), each_host()
#     подключается, вызывает one_host и отключается; log_to() пишет каждый run() с выводом в файл.
#     Неудачный хост не останавливает остальных: он назван в конце файла.
#     Аргументы: <файл списка> [<файл результата>].
#     Запуск: shagshell --script 04_many_hosts_to_file.py ~/hosts.txt ~/report.txt
import os
from shag_blocks import *

COMMANDS = ["hostname", "uptime"]      # EN: change to yours / RU: подставьте свои

if not args():
    printbox("Usage: <hosts list file> [result file]")
    stop()
names = read_list(arg(0))
result_file = os.path.expanduser(arg(1, "~/hosts_report.txt"))
open(result_file, "w").close()         # EN: start with an empty file / RU: начать с пустого файла


def one_host(host, tab):
    append_file(result_file, "=== %s ===" % host)
    log_to(result_file, tab=tab)
    for cmd in COMMANDS:
        run(cmd, timeout=60, tab=tab)


failed = each_host(names, one_host)
for host, error in failed:
    append_file(result_file, "ERROR %s: %s" % (host, error))
printbox("%d hosts, report: %s" % (len(names), result_file), "Example 04")
