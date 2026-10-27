# RU: серверы и команды из файлов; все серверы сразу (параллельно), у каждого свой лог.
# EN: servers and commands from files; every server at once (in parallel), each its own log.
from shag_blocks import *

hosts = read_list('~/servers.txt')
commands = read_list('~/commands.txt')


def _host_1(tab, host):
    log_to(tab, '~/logs/%S_%Y%M%D.log')
    for cmd in commands:
        run(tab, cmd)
each_host(hosts, _host_1, parallel=True, limit=10)
crt.Dialog.MessageBox('Готово, логи в ~/logs / Done, the logs are in ~/logs')
