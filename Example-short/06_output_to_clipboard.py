# 06_output_to_clipboard.py (short blocks / короткие блоки)
#
# EN: Example/06 with short blocks. Runs several commands and puts the whole output into the
#     clipboard: to_clipboard() = crt.Clipboard.Text = ..., from_clipboard() reads it back.
# RU: Example/06 на коротких блоках. Выполняет несколько команд и кладёт весь вывод в буфер обмена:
#     to_clipboard() = crt.Clipboard.Text = ..., from_clipboard() читает обратно.
from shag_blocks import *

COMMANDS = ["date", "whoami", "pwd"]

parts = ["$ %s\n%s" % (cmd, run(None, cmd)) for cmd in COMMANDS]

to_clipboard("\n\n".join(parts))
printbox("In the clipboard / В буфере обмена:\n\n" + from_clipboard(), "Example 06")
