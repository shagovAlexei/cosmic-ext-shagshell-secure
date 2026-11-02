# 01_dialogs_and_arguments.py (short blocks / короткие блоки)
#
# EN: Example/01 with short blocks: ask() = crt.Dialog.Prompt, printbox() = crt.Dialog.MessageBox.
#     Asks for a name, shows what was typed, lists the script's arguments and asks Yes/No.
# RU: Example/01 на коротких блоках: ask() = crt.Dialog.Prompt, printbox() = crt.Dialog.MessageBox.
#     Спрашивает имя, показывает введённое, перечисляет аргументы скрипта и задаёт вопрос Да/Нет.
from shag_blocks import *

# EN: ask(message, title, default, password) returns the text, or "" if cancelled.
# RU: ask(сообщение, заголовок, по умолчанию, пароль?) возвращает текст или "" при отмене.
name = ask("Your name?", "Example 01", "world")
if name == "":
    crt.Quit()

args = ", ".join(crt.Arguments) or "(none / нет)"
printbox("Hello, %s!\nArguments: %s" % (name, args), "Example 01")

# EN: printbox with buttons returns IDYES / IDNO / IDCANCEL...
# RU: printbox с кнопками возвращает IDYES / IDNO / IDCANCEL...
if printbox("Continue?", "Example 01", BUTTON_YESNO) == IDYES:
    printbox("You said yes / Вы ответили «Да»", "Example 01")
