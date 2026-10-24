# 01_dialogs_and_arguments.py
#
# EN: Asks for a name, shows what was typed, lists the script's arguments and asks Yes/No.
#     Run: Script -> Run...   (arguments: put them in the Button Bar button or after the file
#     name in `shagshell --script 01_dialogs_and_arguments.py one two`).
#     SecureCRT: the same calls; here BUTTON_YESNO / IDYES are predefined for you.
# RU: Спрашивает имя, показывает введённое, перечисляет аргументы скрипта и задаёт вопрос Да/Нет.
#     Запуск: Script -> Run...  (аргументы: в кнопке Button Bar или после имени файла в
#     `shagshell --script 01_dialogs_and_arguments.py one two`).
#     SecureCRT: те же вызовы; BUTTON_YESNO / IDYES здесь уже определены.

# EN: Prompt(message, title, default, bPassword) returns the text, or "" if cancelled.
# RU: Prompt(сообщение, заголовок, по умолчанию, пароль?) возвращает текст или "" при отмене.
name = crt.Dialog.Prompt("Your name?", "Example 01", "world")
if name == "":
    crt.Quit()          # EN: stop the script / RU: завершить скрипт

# EN: crt.Arguments is a list-like object: Count, GetArg(i), len(), iteration.
# RU: crt.Arguments - как список: Count, GetArg(i), len(), перебор.
args = ", ".join(crt.Arguments) or "(none / нет)"
crt.Dialog.MessageBox("Hello, %s!\nArguments: %s" % (name, args), "Example 01")

# EN: MessageBox with buttons returns IDYES / IDNO / IDCANCEL...
# RU: MessageBox с кнопками возвращает IDYES / IDNO / IDCANCEL...
if crt.Dialog.MessageBox("Continue?", "Example 01", BUTTON_YESNO) == IDYES:
    crt.Dialog.MessageBox("You said yes / Вы ответили «Да»", "Example 01")
