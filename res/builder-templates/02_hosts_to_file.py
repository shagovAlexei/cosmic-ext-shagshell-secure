# RU: хосты из файла (имя сохранённой сессии в строке), на каждом — команды, всё в один файл.
# EN: hosts from a file (a saved session name a line), the commands on each, all into one file.
from shag_blocks import *

hosts = read_list('~/hosts.txt')
commands = ['hostname', 'uptime']
report = '~/hosts_report.txt'


def _host_1(tab, host):
    append_file(report, '=== ' + host + ' ===')
    for cmd in commands:
        out = run(tab, cmd)
        append_file(report, '$ ' + cmd)
        append_file(report, out)
each_host(hosts, _host_1, parallel=False, limit=10)
crt.Dialog.MessageBox('Готово / Done: ' + report)
