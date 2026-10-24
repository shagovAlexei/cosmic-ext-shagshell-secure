# 12_dialogs_gallery.py
#
# EN: Shows every dialog a script can use, one after another, and prints what each returned
#     into the last message. Each dialog is a window of its own (like in SecureCRT).
#     MessageBox flags are added together: BUTTON_* + ICON_* + DEFBUTTON*.
#     SecureCRT: MessageBox / Prompt / FileOpenDialog work the same; Dialog.List is ours
#     (SecureCRT scripts draw such lists with Internet Explorer).
# RU: Показывает по очереди все диалоги скрипта и в последнем окне выводит, что каждый вернул.
#     Каждый диалог - отдельное окно (как в SecureCRT). Флаги MessageBox складываются:
#     BUTTON_* + ICON_* + DEFBUTTON*.
#     SecureCRT: MessageBox / Prompt / FileOpenDialog работают так же; Dialog.List - наш
#     (в SecureCRT такие списки рисуют через Internet Explorer).

results = []

# EN: Information with an icon and one button. / RU: Сообщение со значком и одной кнопкой.
crt.Dialog.MessageBox("Hello!\nSecond line / Вторая строка", "Gallery 1", ICON_INFO)

# EN: Question, Yes/No/Cancel, the default button is No (DEFBUTTON2).
# RU: Вопрос, Да/Нет/Отмена, кнопка по умолчанию - «Нет» (DEFBUTTON2).
r = crt.Dialog.MessageBox("Save changes?", "Gallery 2", BUTTON_YESNOCANCEL + ICON_QUESTION + DEFBUTTON2)
names = {IDYES: "Yes", IDNO: "No", IDCANCEL: "Cancel"}
results.append("MessageBox -> " + names.get(r, str(r)))

# EN: Retry/Cancel with a warning: the usual "try again" loop.
# RU: Повторить/Отмена с предупреждением: обычный цикл «повторить».
r = crt.Dialog.MessageBox("The device is busy.", "Gallery 3", BUTTON_RETRYCANCEL + ICON_WARN)
results.append("Retry/Cancel -> " + ("Retry" if r == IDRETRY else "Cancel"))

# EN: Prompt: text; the 4th argument True hides what is typed (a password).
# RU: Prompt: текст; 4-й аргумент True скрывает ввод (пароль).
text = crt.Dialog.Prompt("Your name?", "Gallery 4", "world")
results.append("Prompt -> %r" % text)
secret = crt.Dialog.Prompt("Type a secret / Введите секрет", "Gallery 5", "", True)
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

crt.Dialog.MessageBox("\n".join(results), "Gallery: results / результаты", ICON_INFO)
