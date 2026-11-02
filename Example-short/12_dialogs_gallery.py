# 12_dialogs_gallery.py (short blocks / короткие блоки)
#
# EN: Example/12 with short blocks: printbox() and ask(); the list and file dialogs stay `crt`
#     (Dialog.List, Dialog.FileOpenDialog: no blocks for them). Shows every dialog a script can use
#     and prints what each returned into the last message. Flags: BUTTON_* + ICON_* + DEFBUTTON*.
# RU: Example/12 на коротких блоках: printbox() и ask(); список и диалоги файлов остаются на `crt`
#     (Dialog.List, Dialog.FileOpenDialog: блоков для них нет). Показывает все диалоги скрипта и в
#     последнем окне выводит, что каждый вернул. Флаги: BUTTON_* + ICON_* + DEFBUTTON*.
from shag_blocks import *

results = []

# EN: Information with an icon and one button. / RU: Сообщение со значком и одной кнопкой.
printbox("Hello!\nSecond line / Вторая строка", "Gallery 1", ICON_INFO)

# EN: Question, Yes/No/Cancel, the default button is No (DEFBUTTON2).
# RU: Вопрос, Да/Нет/Отмена, кнопка по умолчанию - «Нет» (DEFBUTTON2).
r = printbox("Save changes?", "Gallery 2", BUTTON_YESNOCANCEL + ICON_QUESTION + DEFBUTTON2)
names = {IDYES: "Yes", IDNO: "No", IDCANCEL: "Cancel"}
results.append("printbox -> " + names.get(r, str(r)))

# EN: Retry/Cancel with a warning: the usual "try again" loop.
# RU: Повторить/Отмена с предупреждением: обычный цикл «повторить».
r = printbox("The device is busy.", "Gallery 3", BUTTON_RETRYCANCEL + ICON_WARN)
results.append("Retry/Cancel -> " + ("Retry" if r == IDRETRY else "Cancel"))

# EN: ask: text; password=True hides what is typed. / RU: ask: текст; password=True скрывает ввод.
text = ask("Your name?", "Gallery 4", "world")
results.append("ask -> %r" % text)
secret = ask("Type a secret / Введите секрет", "Gallery 5", password=True)
results.append("Password -> %d characters / символов" % len(secret))   # EN: never print secrets / RU: секрет не печатаем

# EN: List: one item (a str) or, with multi=True, a list of items; None when cancelled.
# RU: List: один пункт (str) или при multi=True список пунктов; None при отмене.
one = crt.Dialog.List("Choose a server / Выберите сервер:", "Gallery 6", ["web-01", "web-02", "db-01"], 1)
results.append("List -> %r" % (one,))
many = crt.Dialog.List("Choose several / Выберите несколько:", "Gallery 7", ["a", "b", "c", "d"], 0, True)
results.append("List (multi) -> %r" % (many,))

# EN: File dialogs return a path, or "" when cancelled. The filter is "Name|*.ext;*.ext2|Name|*.*".
# RU: Диалоги файлов возвращают путь или "" при отмене. Фильтр: "Имя|*.расш;*.расш2|Имя|*.*".
f = crt.Dialog.FileOpenDialog("Open a text file", "Open", "", "Text (*.txt;*.log)|*.txt;*.log|All (*.*)|*.*")
results.append("FileOpenDialog -> %r" % f)

printbox("\n".join(results), "Gallery: results / результаты", ICON_INFO)
