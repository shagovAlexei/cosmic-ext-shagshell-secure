# 12_dialogs_gallery.py (short blocks / короткие блоки)
#
# EN: Example/12 with short blocks: printbox() = Dialog.MessageBox, ask() = Dialog.Prompt,
#     choose() = Dialog.List, pick_file() = Dialog.FileOpenDialog. Shows every dialog a script can
#     use and prints what each returned into the last message. Flags: BUTTON_* + ICON_* + DEFBUTTON*.
# RU: Example/12 на коротких блоках: printbox() = Dialog.MessageBox, ask() = Dialog.Prompt,
#     choose() = Dialog.List, pick_file() = Dialog.FileOpenDialog. Показывает все диалоги скрипта и
#     в последнем окне выводит, что каждый вернул. Флаги: BUTTON_* + ICON_* + DEFBUTTON*.
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

# EN: choose: one item (a str) or, with multi=True, a list of items; None when cancelled.
# RU: choose: один пункт (str) или при multi=True список пунктов; None при отмене.
one = choose("Choose a server / Выберите сервер:", ["web-01", "web-02", "db-01"], "Gallery 6", 1)
results.append("choose -> %r" % (one,))
many = choose("Choose several / Выберите несколько:", ["a", "b", "c", "d"], "Gallery 7", multi=True)
results.append("choose (multi) -> %r" % (many,))

# EN: pick_file returns a path, or "" when cancelled. The filter is "Name|*.ext;*.ext2|Name|*.*".
# RU: pick_file возвращает путь или "" при отмене. Фильтр: "Имя|*.расш;*.расш2|Имя|*.*".
f = pick_file("Open a text file", "Text (*.txt;*.log)|*.txt;*.log|All (*.*)|*.*", "Open")
results.append("pick_file -> %r" % f)

printbox("\n".join(results), "Gallery: results / результаты", ICON_INFO)
