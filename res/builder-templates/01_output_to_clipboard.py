# RU: команда в открытой сессии, её вывод — в буфер обмена.
# EN: a command on the open session, its output to the clipboard.
from shag_blocks import *

out = run(None, 'uname -a')
to_clipboard(out)
crt.Dialog.MessageBox('Скопировано в буфер обмена / Copied to the clipboard')
