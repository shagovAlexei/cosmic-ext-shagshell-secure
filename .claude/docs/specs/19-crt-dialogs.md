# 19. Диалоги `crt` как в SecureCRT

Сейчас диалоги скрипта (`Dialog.MessageBox` / `Prompt`) — плашка поверх главного окна. В SecureCRT это **отдельные окна** по центру с заголовком, значком, кнопками и (у Prompt) полем ввода. Делаем так же и добавляем недостающие диалоги.

## Что
- **Отдельное окно** (как Session Options: `window::open`, своя `header_bar`) для каждого диалога скрипта; очередь диалогов как сейчас (по одному). Закрытие окна крестиком = Cancel / No / IDCANCEL (как Esc). Заголовок = `title` вызова, по умолчанию «Script».
- **MessageBox(text, title, flags)**: `flags` = `BUTTON_*` | `ICON_*` | `DEFBUTTON1/2/3`, как в SecureCRT: `ICON_STOP` 16, `ICON_QUESTION` 32, `ICON_WARN` 48, `ICON_INFO` 64; `DEFBUTTON2` 256, `DEFBUTTON3` 512. Значок слева от текста; кнопка по умолчанию (Enter) — выбранная. Кнопки Retry / Abort / Ignore добавляются (`BUTTON_RETRYCANCEL`, `BUTTON_ABORTRETRYIGNORE`, результаты `IDRETRY`, `IDABORT`, `IDIGNORE`).
- **Prompt(message, title, default, bPassword)**: окно с многострочным сообщением (`\n`), полем ввода (маска при `bPassword`), OK / Cancel; Enter = OK.
- **`crt.Dialog.FileOpenDialog(title, buttonLabel, defaultFilename, filter)`** и **`FileSaveDialog(...)`**: системный выбор файла (портал), `filter` как в SecureCRT (`"Text (*.txt)|*.txt|All (*.*)|*.*"`); результат — путь или `""` при отмене.
- **`crt.Dialog.List(message, title, items, default=0, multi=False)`** (наше расширение; в SecureCRT такое делали через IE): окно со списком, поиск по набору, результат — выбранный пункт (строка) или список строк при `multi`, `None` при отмене.
- Примеры: `Example/12_dialogs_gallery.py` (все диалоги), `Example/13_pick_file_and_hosts.py` (выбор файла и хостов из списка); README.

## Как
- `core::script`: разбор `flags` (чистая функция `Flags { buttons, icon, default }`, тест), разбор `filter` в пары (имя, маска) (тест), результат List (выбор по индексам, тест).
- Приложение: `script.rs` хранит очередь `Ask` как сейчас, но показывает её окном (`App::view_window` → `scripts.dialog_window`); значки — системные символьные иконки COSMIC; файловые диалоги — `cosmic::dialog::file_chooser` (open / save) асинхронно, ответ скрипту по завершении.
- Строки интерфейса (Retry, Abort, Ignore, значки) через `fl!`, en + ru.

## Не сделано
- «Script → Cancel» не закрывает уже открытый системный выбор файла (его закрывает пользователь).
- В `FileOpenDialog` игнорируется `defaultFilename` (у портала нет такого параметра в этой версии libcosmic).

## Не делаем
- Нативные Windows-окна, `crt.Dialog.MessageBox` с таймаутом, пользовательские формы (HTML/IE).

## Проверка
- Тесты `core` на `Flags`, фильтр, List. Headless: окно MessageBox с каждым набором кнопок и значком, Prompt с паролем, List (Enter / Esc); файловые диалоги — вручную (TESTING.md).
