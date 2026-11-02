//! Every member of `crt` (`res/python/crt.py`) as a block: its parameters, help and an example.
//! A test checks this table against `crt.py` itself.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Method,
    Property,
}

/// What a parameter's field takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ty {
    Text,
    Number,
    Bool,
    /// One of these names (constants of `crt.py`).
    Choice(&'static [&'static str]),
    /// `/S "name"`: a saved session.
    Session,
    File,
    /// Any Python expression.
    Expr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Param {
    /// As in `crt.py`; varargs as `*texts` / `**kw`.
    pub name: &'static str,
    pub ty: Ty,
    /// Python text of the default; `None` = required.
    pub default: Option<&'static str>,
    /// Shown under «more ▸» on the block.
    pub more: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    /// `Screen.Send`, `Dialog.Prompt`; a member of `crt` itself: `Sleep`.
    pub path: &'static str,
    pub kind: Kind,
    pub params: &'static [Param],
    /// `""` = nothing; else a short type word.
    pub returns: &'static str,
    /// `[en, ru]`.
    pub help: [&'static str; 2],
    /// One line of Python.
    pub example: &'static str,
}

impl Entry {
    /// `Screen`, `Session`…; `crt` for a member of `crt` itself.
    pub fn group(&self) -> &'static str {
        self.path.split_once('.').map_or("crt", |(g, _)| g)
    }
}

/// The palette's «Short blocks»: every `blocks.*` command (`shag_blocks.py`), in catalog order.
pub fn short_blocks() -> impl Iterator<Item = &'static Entry> {
    CATALOG.iter().filter(|e| e.group() == "blocks")
}

/// Whether a palette group shows its items: «Frequent» and «Short blocks» start unfolded, the
/// rest folded; `toggled` = the user clicked its title (an odd number of times). Searching
/// shows everything found.
pub fn group_open(key: &str, toggled: bool, searching: bool) -> bool {
    searching || (matches!(key, "frequent" | "short") != toggled)
}

const fn p(name: &'static str, ty: Ty) -> Param {
    Param {
        name,
        ty,
        default: None,
        more: false,
    }
}

const fn d(name: &'static str, ty: Ty, default: &'static str) -> Param {
    Param {
        name,
        ty,
        default: Some(default),
        more: false,
    }
}

const fn more(p: Param) -> Param {
    Param { more: true, ..p }
}

const fn m(
    path: &'static str,
    params: &'static [Param],
    returns: &'static str,
    help: [&'static str; 2],
    example: &'static str,
) -> Entry {
    Entry {
        path,
        kind: Kind::Method,
        params,
        returns,
        help,
        example,
    }
}

const fn prop(
    path: &'static str,
    returns: &'static str,
    help: [&'static str; 2],
    example: &'static str,
) -> Entry {
    Entry {
        path,
        kind: Kind::Property,
        params: &[],
        returns,
        help,
        example,
    }
}

use Ty::*;

const BUTTONS: &[&str] = &[
    "BUTTON_OK",
    "BUTTON_OKCANCEL",
    "BUTTON_ABORTRETRYIGNORE",
    "BUTTON_YESNOCANCEL",
    "BUTTON_YESNO",
    "BUTTON_RETRYCANCEL",
];
const TIMEOUT: Param = d("timeout", Number, "0");
const NOCASE: Param = more(d("bCaseInsensitive", Bool, "False"));

pub static CATALOG: &[Entry] = &[
    // crt
    m(
        "GetTab",
        &[p("n", Number)],
        "Tab",
        [
            "The tab number n (1 = the first).",
            "Вкладка номер n (1 — первая).",
        ],
        "tab = crt.GetTab(2)",
    ),
    m(
        "GetScriptTab",
        &[],
        "Tab",
        [
            "The tab the script was started on.",
            "Вкладка, на которой запущен скрипт.",
        ],
        "tab = crt.GetScriptTab()",
    ),
    m(
        "GetActiveTab",
        &[],
        "Tab",
        [
            "The tab in front now (a new connection's tab after Session.Connect).",
            "Вкладка, которая сейчас впереди (после Session.Connect — новая).",
        ],
        "tab = crt.GetActiveTab()",
    ),
    m(
        "GetTabCount",
        &[],
        "int",
        ["How many tabs are open.", "Сколько вкладок открыто."],
        "n = crt.GetTabCount()",
    ),
    m(
        "Sleep",
        &[p("ms", Number)],
        "",
        ["Waits ms milliseconds.", "Ждёт ms миллисекунд."],
        "crt.Sleep(1000)",
    ),
    m(
        "GetLastErrorMessage",
        &[],
        "str",
        [
            "The text of the last error of a crt call.",
            "Текст последней ошибки вызова crt.",
        ],
        "err = crt.GetLastErrorMessage()",
    ),
    m(
        "Quit",
        &[],
        "",
        ["Ends the script now.", "Сразу завершает скрипт."],
        "crt.Quit()",
    ),
    // Screen
    prop(
        "Screen.CurrentRow",
        "int",
        [
            "The cursor's row (1 = the top).",
            "Строка курсора (1 — верхняя).",
        ],
        "row = crt.Screen.CurrentRow",
    ),
    prop(
        "Screen.CurrentColumn",
        "int",
        [
            "The cursor's column (1 = the left).",
            "Столбец курсора (1 — левый).",
        ],
        "col = crt.Screen.CurrentColumn",
    ),
    prop(
        "Screen.Rows",
        "int",
        ["How many rows the screen has.", "Сколько строк на экране."],
        "rows = crt.Screen.Rows",
    ),
    prop(
        "Screen.Columns",
        "int",
        [
            "How many columns the screen has.",
            "Сколько столбцов на экране.",
        ],
        "cols = crt.Screen.Columns",
    ),
    prop(
        "Screen.Selection",
        "str",
        [
            "The text selected with the mouse.",
            "Выделенный мышью текст.",
        ],
        "text = crt.Screen.Selection",
    ),
    m(
        "Screen.Get",
        &[
            p("row1", Number),
            p("col1", Number),
            p("row2", Number),
            p("col2", Number),
        ],
        "str",
        [
            "The text of a block of the screen (1-based, ends included), lines joined by \\n.",
            "Текст прямоугольника экрана (с 1, края включены), строки через \\n.",
        ],
        "line = crt.Screen.Get(1, 1, 1, 80)",
    ),
    m(
        "Screen.WaitForCursor",
        &[TIMEOUT],
        "bool",
        [
            "True when the cursor moved within timeout seconds, False when it kept still.",
            "True, если курсор сдвинулся за timeout секунд, False — если стоял.",
        ],
        "moved = crt.Screen.WaitForCursor(1)",
    ),
    m(
        "Screen.Send",
        &[p("text", Text)],
        "",
        [
            "Types text into the tab; \\r is Enter.",
            "Печатает text во вкладку; \\r — Enter.",
        ],
        "crt.Screen.Send('show version\\r')",
    ),
    m(
        "Screen.WaitForString",
        &[p("text", Text), TIMEOUT, NOCASE],
        "bool",
        [
            "Waits until text appears in the output; timeout in seconds, 0 = no limit. False on timeout.",
            "Ждёт, пока text появится в выводе; timeout в секундах, 0 — без ограничения. False — не дождались.",
        ],
        "ok = crt.Screen.WaitForString('#', 10)",
    ),
    m(
        "Screen.WaitForStrings",
        &[p("*texts", Text), p("**kw", Expr)],
        "int",
        [
            "Waits for any of the texts; returns its number (1, 2…), 0 on timeout (timeout=… seconds).",
            "Ждёт любой из текстов; возвращает его номер (1, 2…), 0 — не дождались (timeout=… секунд).",
        ],
        "n = crt.Screen.WaitForStrings('#', '>', timeout=10)",
    ),
    m(
        "Screen.SendKeys",
        &[p("text", Text)],
        "",
        [
            "Like Send, but \\r, \\p… are not escapes: the text goes as it is.",
            "Как Send, но \\r, \\p… не спецсимволы: текст уходит как есть.",
        ],
        "crt.Screen.SendKeys('C:\\\\temp')",
    ),
    m(
        "Screen.Clear",
        &[],
        "",
        ["Clears the screen.", "Очищает экран."],
        "crt.Screen.Clear()",
    ),
    m(
        "Screen.ReadString",
        &[p("text", Text), TIMEOUT, NOCASE],
        "str",
        [
            "The output up to text (without it): the result of a command up to the prompt.",
            "Вывод до text (без него): результат команды до приглашения.",
        ],
        "out = crt.Screen.ReadString('#', 30)",
    ),
    // Session
    prop(
        "Session.Connected",
        "bool",
        [
            "True while the tab is connected.",
            "True, пока вкладка подключена.",
        ],
        "if crt.Session.Connected: ...",
    ),
    prop(
        "Session.Path",
        "str",
        [
            "The saved session of the tab (folder/name).",
            "Сохранённая сессия вкладки (папка/имя).",
        ],
        "path = crt.Session.Path",
    ),
    prop(
        "Session.LogFileName",
        "str",
        [
            "The file of the log that is on (\"\" when it is off).",
            "Файл включённого лога (\"\", если выключен).",
        ],
        "name = crt.Session.LogFileName",
    ),
    m(
        "Session.LogText",
        &[p("text", Text)],
        "",
        [
            "Writes text into the tab's log (it must be on), in order with the output.",
            "Пишет text в лог вкладки (он должен быть включён), по порядку с выводом.",
        ],
        "crt.Session.LogText('--- step 2 ---')",
    ),
    m(
        "Session.Log",
        &[d("on", Bool, "True"), more(d("append", Expr, "None"))],
        "",
        [
            "Starts / stops the tab's log (name template of the session or Global Options).",
            "Включает / выключает лог вкладки (шаблон имени из сессии или Global Options).",
        ],
        "crt.Session.Log(True)",
    ),
    m(
        "Session.Connect",
        &[p("cmd", Session)],
        "",
        [
            "/S \"name\" — a saved session — opens a new tab.",
            "/S \"имя\" — сохранённая сессия — открывает новую вкладку.",
        ],
        "crt.Session.Connect('/S \"router1\"')",
    ),
    m(
        "Session.Disconnect",
        &[],
        "",
        ["Disconnects the tab.", "Отключает вкладку."],
        "crt.Session.Disconnect()",
    ),
    m(
        "Session.SetStatusText",
        &[p("text", Text)],
        "",
        [
            "Shows text in the status bar.",
            "Показывает text в строке состояния.",
        ],
        "crt.Session.SetStatusText('step 2 of 5')",
    ),
    m(
        "Session.Lock",
        &[],
        "",
        [
            "The user can no longer type or paste into the tab (Send still works); freed when the script ends.",
            "Пользователь больше не может печатать во вкладку (Send работает); снимается в конце скрипта.",
        ],
        "crt.Session.Lock()",
    ),
    m(
        "Session.Unlock",
        &[],
        "",
        [
            "Lets the user type into the tab again.",
            "Снова даёт пользователю печатать во вкладку.",
        ],
        "crt.Session.Unlock()",
    ),
    prop(
        "Session.Locked",
        "bool",
        [
            "True while the tab is locked.",
            "True, пока вкладка заблокирована.",
        ],
        "if crt.Session.Locked: ...",
    ),
    m(
        "Session.Create",
        &[
            p("name", Text),
            p("host", Text),
            d("user", Text, "''"),
            d("port", Number, "22"),
            more(d("folder", Text, "''")),
        ],
        "str",
        [
            "Saves a new SSH session (the Default template, no password); returns its path.",
            "Сохраняет новую SSH-сессию (по шаблону Default, без пароля); возвращает её путь.",
        ],
        "crt.Session.Create('router1', '10.0.0.1', 'admin')",
    ),
    // Tab
    prop(
        "Tab.Index",
        "int",
        [
            "The tab's number (1 = the first).",
            "Номер вкладки (1 — первая).",
        ],
        "n = tab.Index",
    ),
    prop(
        "Tab.Caption",
        "str",
        ["The tab's title.", "Заголовок вкладки."],
        "title = tab.Caption",
    ),
    m(
        "Tab.Activate",
        &[],
        "",
        ["Brings the tab to the front.", "Выводит вкладку вперёд."],
        "tab.Activate()",
    ),
    // Dialog
    m(
        "Dialog.MessageBox",
        &[
            p("text", Text),
            d("title", Text, "''"),
            d("buttons", Choice(BUTTONS), "0"),
        ],
        "int",
        [
            "A message with buttons; returns the button pressed (IDOK, IDYES, IDNO…).",
            "Сообщение с кнопками; возвращает нажатую (IDOK, IDYES, IDNO…).",
        ],
        "r = crt.Dialog.MessageBox('Go on?', 'Check', BUTTON_YESNO)",
    ),
    m(
        "Dialog.FileOpenDialog",
        &[
            d("title", Text, "''"),
            more(d("buttonLabel", Text, "''")),
            d("defaultFilename", File, "''"),
            more(d("filter", Text, "''")),
        ],
        "str",
        [
            "The file chosen (\"\" when cancelled). filter: \"Text (*.txt)|*.txt|All (*.*)|*.*\".",
            "Выбранный файл (\"\" — отмена). filter: \"Text (*.txt)|*.txt|All (*.*)|*.*\".",
        ],
        "path = crt.Dialog.FileOpenDialog('Hosts list')",
    ),
    m(
        "Dialog.FileSaveDialog",
        &[
            d("title", Text, "''"),
            more(d("buttonLabel", Text, "''")),
            d("defaultFilename", File, "''"),
            more(d("filter", Text, "''")),
        ],
        "str",
        [
            "The file name chosen for saving (\"\" when cancelled).",
            "Имя файла для сохранения (\"\" — отмена).",
        ],
        "path = crt.Dialog.FileSaveDialog('Report', '', 'report.txt')",
    ),
    m(
        "Dialog.List",
        &[
            p("message", Text),
            d("title", Text, "''"),
            d("items", Expr, "()"),
            d("default", Number, "0"),
            more(d("multi", Bool, "False")),
        ],
        "str",
        [
            "A list to choose from: the item, a list of items when multi, None if cancelled.",
            "Список для выбора: пункт, список пунктов при multi, None — отмена.",
        ],
        "host = crt.Dialog.List('Host:', 'Pick', ['r1', 'r2'])",
    ),
    m(
        "Dialog.Prompt",
        &[
            p("message", Text),
            d("title", Text, "''"),
            d("default", Text, "''"),
            d("bPassword", Bool, "False"),
        ],
        "str",
        [
            "Asks for a line of text (\"\" when cancelled); bPassword hides it.",
            "Спрашивает строку текста (\"\" — отмена); bPassword скрывает ввод.",
        ],
        "name = crt.Dialog.Prompt('Name:', 'New user')",
    ),
    // Clipboard
    prop(
        "Clipboard.Text",
        "str",
        [
            "The clipboard's text; assign to put text there.",
            "Текст буфера обмена; присвойте, чтобы положить текст.",
        ],
        "crt.Clipboard.Text = out",
    ),
    // Arguments
    prop(
        "Arguments.Count",
        "int",
        [
            "How many arguments the script got.",
            "Сколько аргументов получил скрипт.",
        ],
        "n = crt.Arguments.Count",
    ),
    m(
        "Arguments.GetArg",
        &[p("i", Number)],
        "str",
        [
            "Argument number i (0 = the first).",
            "Аргумент номер i (0 — первый).",
        ],
        "path = crt.Arguments.GetArg(0)",
    ),
    // Window
    prop(
        "Window.Caption",
        "str",
        ["The main window's title.", "Заголовок главного окна."],
        "title = crt.Window.Caption",
    ),
    m(
        "Window.Activate",
        &[],
        "",
        [
            "Brings the main window to the front.",
            "Выводит главное окно вперёд.",
        ],
        "crt.Window.Activate()",
    ),
    // FileTransfer
    m(
        "FileTransfer.Upload",
        &[p("local", File), d("remote", Text, "''")],
        "",
        [
            "A file or folder to the server over SFTP (remote: a name, a folder, or empty = home).",
            "Файл или папка на сервер по SFTP (remote: имя, папка или пусто — домашняя).",
        ],
        "crt.FileTransfer.Upload('~/backup.cfg', '/tmp/')",
    ),
    m(
        "FileTransfer.Download",
        &[p("remote", Text), d("local", File, "''")],
        "",
        [
            "A file from the server over SFTP (local: a name, a folder, or empty = ~/Downloads).",
            "Файл с сервера по SFTP (local: имя, папка или пусто — ~/Downloads).",
        ],
        "crt.FileTransfer.Download('/etc/hostname')",
    ),
    // blocks: `res/python/shag_blocks.py` (`from shag_blocks import *`)
    m(
        "blocks.prompt",
        &[d("timeout", Number, "60"), d("tab", Expr, "None")],
        "str",
        [
            "The prompt the tab shows, once the login is over.",
            "Приглашение, которое показывает вкладка, когда вход закончен.",
        ],
        "p = prompt()",
    ),
    m(
        "blocks.run",
        &[
            p("cmd", Text),
            d("timeout", Number, "30"),
            more(d("until", Text, "None")),
            d("tab", Expr, "None"),
        ],
        "str",
        [
            "Runs a command: types it and Enter, waits for the prompt; its output (also into the tab's log_to file). tab None: the script's tab.",
            "Выполняет команду: печатает её и Enter, ждёт приглашения; возвращает вывод (и пишет в файл log_to вкладки). tab None — вкладка скрипта.",
        ],
        "out = run('show version')",
    ),
    m(
        "blocks.read_list",
        &[p("path", File)],
        "list",
        [
            "The lines of a file, without empty ones and # comments.",
            "Строки файла без пустых и комментариев #.",
        ],
        "hosts = read_list('~/hosts.txt')",
    ),
    m(
        "blocks.log_to",
        &[p("path", File), d("tab", Expr, "None")],
        "str",
        [
            "From now on run writes this tab's commands and output to path (%S tab title, %Y %M %D date, %h %m %s time).",
            "Дальше run пишет команды и вывод этой вкладки в path (%S заголовок вкладки, %Y %M %D дата, %h %m %s время).",
        ],
        "log_to('~/logs/%S_%Y%M%D.log')",
    ),
    m(
        "blocks.append_file",
        &[p("path", File), p("text", Expr)],
        "",
        [
            "Adds text and a line end at the end of a file.",
            "Дописывает text и перевод строки в конец файла.",
        ],
        "append_file('~/report.txt', out)",
    ),
    m(
        "blocks.to_clipboard",
        &[p("text", Expr)],
        "",
        ["Puts text on the clipboard.", "Кладёт text в буфер обмена."],
        "to_clipboard(out)",
    ),
    m(
        "blocks.from_clipboard",
        &[],
        "str",
        ["The clipboard's text.", "Текст из буфера обмена."],
        "out = from_clipboard()",
    ),
    m(
        "blocks.printbox",
        &[
            p("text", Text),
            d("title", Text, "''"),
            d("buttons", Choice(BUTTONS), "0"),
        ],
        "int",
        [
            "A message box (short for Dialog.MessageBox); returns the button pressed.",
            "Окно сообщения (коротко от Dialog.MessageBox); возвращает нажатую кнопку.",
        ],
        "printbox('Done', 'Check')",
    ),
    m(
        "blocks.ask",
        &[
            p("message", Text),
            d("title", Text, "''"),
            d("default", Text, "''"),
            d("password", Bool, "False"),
        ],
        "str",
        [
            "Asks for a line of text, \"\" when cancelled (short for Dialog.Prompt).",
            "Спрашивает строку текста, \"\" — отмена (коротко от Dialog.Prompt).",
        ],
        "name = ask('Name:', 'New user')",
    ),
    m(
        "blocks.read_until",
        &[
            p("text", Text),
            d("timeout", Number, "30"),
            d("tab", Expr, "None"),
        ],
        "str",
        [
            "The output up to text (short for Screen.ReadString); tab: the script's own by default.",
            "Вывод до text (коротко от Screen.ReadString); tab: по умолчанию вкладка скрипта.",
        ],
        "out = read_until('#', 30)",
    ),
    m(
        "blocks.connect",
        &[p("session", Text)],
        "Tab",
        [
            "Opens the saved session in a new tab and returns the tab (short for Session.Connect).",
            "Открывает сохранённую сессию в новой вкладке и возвращает её (коротко от Session.Connect).",
        ],
        "tab = connect('router1')",
    ),
    m(
        "blocks.active_tab",
        &[],
        "Tab",
        [
            "The tab in front (short for GetActiveTab).",
            "Вкладка впереди (коротко от GetActiveTab).",
        ],
        "tab = active_tab()",
    ),
    m(
        "blocks.disconnect",
        &[d("tab", Expr, "None")],
        "",
        [
            "Disconnects the tab, the script's own by default (short for Session.Disconnect).",
            "Отключает вкладку, по умолчанию скрипта (коротко от Session.Disconnect).",
        ],
        "disconnect(tab)",
    ),
    m(
        "blocks.each_host",
        &[
            p("hosts", Expr),
            p("func", Expr),
            d("parallel", Bool, "False"),
            d("limit", Number, "10"),
        ],
        "list",
        [
            "For every saved session in hosts: connect, func(host, tab), disconnect; the failed ones as (host, error).",
            "Для каждой сохранённой сессии из hosts: подключиться, func(host, tab), отключиться; возвращает неудачные (хост, ошибка).",
        ],
        "each_host(hosts, check)",
    ),
    m(
        "blocks.send",
        &[p("text", Text), d("tab", Expr, "None")],
        "",
        [
            "Types a line and Enter, without waiting (short for Screen.Send(text + '\\r')).",
            "Печатает строку и Enter, не ожидая (коротко от Screen.Send(text + '\\r')).",
        ],
        "send('ls -l')",
    ),
    m(
        "blocks.send_keys",
        &[p("text", Text), d("tab", Expr, "None")],
        "",
        [
            "Types the text as it is, no Enter (short for Screen.SendKeys).",
            "Печатает текст как есть, без Enter (коротко от Screen.SendKeys).",
        ],
        "send_keys('y')",
    ),
    m(
        "blocks.wait_for",
        &[
            p("text", Text),
            d("timeout", Number, "0"),
            d("tab", Expr, "None"),
        ],
        "bool",
        [
            "Waits for text in the output; False after timeout s (0: no limit) (short for Screen.WaitForString).",
            "Ждёт text в выводе; False через timeout с (0 — без предела) (коротко от Screen.WaitForString).",
        ],
        "wait_for('#', 10)",
    ),
    m(
        "blocks.wait_any",
        &[
            p("texts", Expr),
            d("timeout", Number, "0"),
            d("tab", Expr, "None"),
        ],
        "int",
        [
            "Waits for one of the texts; its number from 1, 0 after timeout (short for Screen.WaitForStrings).",
            "Ждёт одну из строк; её номер с 1, 0 — таймаут (коротко от Screen.WaitForStrings).",
        ],
        "n = wait_any(['$ ', '# '], 10)",
    ),
    m(
        "blocks.clear",
        &[d("tab", Expr, "None")],
        "",
        [
            "Clears the tab's screen (short for Screen.Clear).",
            "Очищает экран вкладки (коротко от Screen.Clear).",
        ],
        "clear()",
    ),
    m(
        "blocks.script_tab",
        &[],
        "Tab",
        [
            "The tab the script runs on (short for GetScriptTab).",
            "Вкладка, на которой запущен скрипт (коротко от GetScriptTab).",
        ],
        "tab = script_tab()",
    ),
    m(
        "blocks.all_tabs",
        &[],
        "list",
        [
            "Every tab, left to right (short for GetTabCount + GetTab).",
            "Все вкладки слева направо (коротко от GetTabCount + GetTab).",
        ],
        "for tab in all_tabs(): send('uptime', tab)",
    ),
    m(
        "blocks.connected",
        &[d("tab", Expr, "None")],
        "bool",
        [
            "Whether the tab is connected (short for Session.Connected).",
            "Подключена ли вкладка (коротко от Session.Connected).",
        ],
        "if connected(tab): send('exit', tab)",
    ),
    m(
        "blocks.activate",
        &[d("tab", Expr, "None")],
        "",
        [
            "Brings the tab to the front, the script's own by default (short for Tab.Activate).",
            "Делает вкладку активной, по умолчанию вкладку скрипта (коротко от Tab.Activate).",
        ],
        "activate(tab)",
    ),
    m(
        "blocks.status",
        &[p("text", Text), d("tab", Expr, "None")],
        "",
        [
            "Text in the status bar (short for Session.SetStatusText).",
            "Текст в строке состояния (коротко от Session.SetStatusText).",
        ],
        "status('backup done')",
    ),
    m(
        "blocks.log_text",
        &[p("text", Text), d("tab", Expr, "None")],
        "str",
        [
            "Writes text into the tab's session log, starting the log if it is off; the log file (short for Session.Log + LogText).",
            "Пишет text в лог сессии вкладки, включая лог, если он выключен; возвращает файл лога (коротко от Session.Log + LogText).",
        ],
        "log_text('--- before the upgrade ---')",
    ),
    m(
        "blocks.choose",
        &[
            p("message", Text),
            p("items", Expr),
            d("title", Text, "''"),
            d("default", Number, "0"),
            d("multi", Bool, "False"),
        ],
        "str",
        [
            "A list to choose from: the item, a list of items when multi, None if cancelled (short for Dialog.List).",
            "Список для выбора: пункт, при multi — список пунктов, None — отмена (коротко от Dialog.List).",
        ],
        "host = choose('Server:', ['web-01', 'db-01'])",
    ),
    m(
        "blocks.pick_file",
        &[
            d("title", Text, "''"),
            d("filter", Text, "''"),
            more(d("button", Text, "''")),
            more(d("default", File, "''")),
        ],
        "str",
        [
            "A file chosen in a dialog, \"\" when cancelled; filter: 'Text (*.txt)|*.txt' (short for Dialog.FileOpenDialog).",
            "Файл, выбранный в диалоге, \"\" — отмена; filter: 'Text (*.txt)|*.txt' (коротко от Dialog.FileOpenDialog).",
        ],
        "path = pick_file('Commands', 'Text (*.txt)|*.txt')",
    ),
    m(
        "blocks.arg",
        &[p("n", Number), d("default", Text, "''")],
        "str",
        [
            "The script's argument n (from 0), default when there are fewer (short for crt.Arguments[n]).",
            "Аргумент скрипта номер n (с 0), default — если их меньше (коротко от crt.Arguments[n]).",
        ],
        "host = arg(0, 'router1')",
    ),
    m(
        "blocks.args",
        &[],
        "list",
        [
            "Every argument of the script (short for crt.Arguments).",
            "Все аргументы скрипта (коротко от crt.Arguments).",
        ],
        "for host in args(): connect(host)",
    ),
    m(
        "blocks.stop",
        &[],
        "",
        [
            "Ends the script here (short for crt.Quit).",
            "Завершает скрипт здесь (коротко от crt.Quit).",
        ],
        "stop()",
    ),
    m(
        "blocks.last_error",
        &[],
        "str",
        [
            "The last error's text (short for GetLastErrorMessage).",
            "Текст последней ошибки (коротко от GetLastErrorMessage).",
        ],
        "printbox(last_error())",
    ),
    m(
        "blocks.new_session",
        &[
            p("name", Text),
            p("host", Text),
            d("user", Text, "''"),
            d("port", Number, "22"),
            d("folder", Text, "''"),
        ],
        "str",
        [
            "Saves a new SSH session (no password); its path for connect (short for Session.Create).",
            "Сохраняет новую SSH-сессию (без пароля); возвращает путь для connect (коротко от Session.Create).",
        ],
        "path = new_session('web1', '10.0.0.5', 'admin')",
    ),
    m(
        "blocks.lock",
        &[d("tab", Expr, "None")],
        "",
        [
            "The user can no longer type into the tab until unlock or the script's end (short for Session.Lock).",
            "Пользователь не может печатать во вкладку до unlock или конца скрипта (коротко от Session.Lock).",
        ],
        "lock()",
    ),
    m(
        "blocks.unlock",
        &[d("tab", Expr, "None")],
        "",
        [
            "Lets the user type into the tab again (short for Session.Unlock).",
            "Снова разрешает печатать во вкладку (коротко от Session.Unlock).",
        ],
        "unlock()",
    ),
    m(
        "blocks.upload",
        &[
            p("local", File),
            d("remote", Text, "''"),
            d("tab", Expr, "None"),
        ],
        "",
        [
            "A file to the server over the tab's SFTP (remote: a name, a folder, or empty = home) (short for FileTransfer.Upload).",
            "Файл на сервер по SFTP вкладки (по умолчанию — скрипта; remote: имя, папка или пусто — домашняя) (коротко от FileTransfer.Upload).",
        ],
        "upload('~/backup.cfg', '/tmp/')",
    ),
    m(
        "blocks.download",
        &[
            p("remote", Text),
            d("local", File, "''"),
            d("tab", Expr, "None"),
        ],
        "",
        [
            "A file from the server over the tab's SFTP (local: empty = ~/Downloads) (short for FileTransfer.Download).",
            "Файл с сервера по SFTP вкладки (по умолчанию — скрипта; local: пусто — ~/Downloads) (коротко от FileTransfer.Download).",
        ],
        "download('/etc/hostname')",
    ),
    m(
        "blocks.pause",
        &[p("ms", Number)],
        "",
        [
            "Waits ms milliseconds (short for crt.Sleep).",
            "Ждёт ms миллисекунд (коротко от crt.Sleep).",
        ],
        "pause(1000)",
    ),
];

/// The groups whose members are reached through any object holding them (`tab.Screen.Send`).
const OBJECTS: [&str; 7] = [
    "Screen",
    "Session",
    "Dialog",
    "Clipboard",
    "Arguments",
    "Window",
    "FileTransfer",
];

/// The entry a call's function text names: `crt.Screen.Send`, `tab.Screen.Send`, `tab.Activate`.
pub fn lookup(func: &str) -> Option<&'static Entry> {
    let find = |path: &str| CATALOG.iter().find(|e| e.path == path);
    if let Some(e) = func.strip_prefix("crt.").and_then(find) {
        return Some(e);
    }
    if !func.contains('.') {
        return find(&format!("blocks.{func}"));
    }
    let (recv, last) = func.rsplit_once('.')?;
    let group = recv.rsplit('.').next().unwrap_or(recv);
    if OBJECTS.contains(&group) {
        return find(&format!("{group}.{last}"));
    }
    (recv != "crt")
        .then(|| find(&format!("Tab.{last}")))
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn regression_frequent_folds() {
        // «Frequent» was always open: its title did nothing.
        assert!(group_open("frequent", false, false));
        assert!(!group_open("frequent", true, false));
        assert!(group_open("short", false, false));
        assert!(!group_open("short", true, false));
        assert!(!group_open("Screen", false, false));
        assert!(group_open("Screen", true, false));
        assert!(group_open("frequent", true, true));
    }

    #[test]
    fn short_blocks_are_all_blocks() {
        // «Big blocks» and «Short blocks» are one palette group: every `blocks.*` command.
        let short: Vec<_> = short_blocks().map(|e| e.path).collect();
        let all = CATALOG.iter().filter(|e| e.path.starts_with("blocks."));
        assert_eq!(short, all.map(|e| e.path).collect::<Vec<_>>());
        assert!(short.contains(&"blocks.run") && short.contains(&"blocks.printbox"));
    }

    #[test]
    fn short_blocks_call_crt() {
        use std::process::Command;
        if Command::new("python3").arg("-V").output().is_err() {
            return;
        }
        // A fake `crt` that records the calls; the short blocks must make the long ones.
        const SNIPPET: &str = r##"
import builtins, sys
sys.path.insert(0, sys.argv[1])
calls = []
class Rec:
    def __init__(self, name): self._n = name
    def __getattr__(self, a):
        return Rec(self._n + "." + a)
    def __call__(self, *a):
        calls.append((self._n,) + a)
        return {"crt.GetTabCount": 2, "crt.Screen.WaitForStrings": 2}.get(self._n)
class Tab:
    Caption = "t"
    def __init__(self, n):
        self.Index = n
        self.Screen, self.Session = Rec("tab%d.Screen" % n), Rec("tab%d.Session" % n)
        self.Activate = Rec("tab%d.Activate" % n)
        self.Session.LogFileName = ""
        self.Session.Connected = n == 1
class Crt(Rec):
    Arguments = ["a", "b"]
    def GetActiveTab(self): return Tab(7)
    def GetTab(self, n): return Tab(n)
    def GetScriptTab(self): return Tab(0)
crt = builtins.crt = Crt("crt")
import shag_blocks as b
b._crt = None
t1 = crt.GetTab(1)
b.send("ls"); b.send("ls", t1)
assert ("tab0.Screen.Send", "ls\r") in calls and ("tab1.Screen.Send", "ls\r") in calls, calls
assert [t.Session.Connected for t in b.all_tabs()] == [True, False]
assert b.connected(t1) and not b.connected()
assert b.arg(1) == "b" and b.arg(5, "x") == "x" and b.args() == ["a", "b"]
b.log_text("hi")
assert ("tab0.Session.Log", True, True) in calls and ("tab0.Session.LogText", "hi") in calls, calls
calls.clear()
b.wait_any(["$ ", "# "], 5)
b.choose("m", ["x"]); b.pick_file("t", "f")
b.new_session("n", "h"); b.upload("l"); b.download("r"); b.pause(10)
assert calls == [
    ("tab0.Screen.WaitForStrings", ["$ ", "# "], 5),
    ("crt.Dialog.List", "m", "", ["x"], 0, False),
    ("crt.Dialog.FileOpenDialog", "t", "", "", "f"),
    ("crt.Session.Create", "n", "h", "", 22, ""),
    ("crt.FileTransfer.Upload", "l", ""),
    ("crt.FileTransfer.Download", "r", ""),
    ("crt.Sleep", 10),
], calls
try:
    b.stop()
    assert False
except SystemExit:
    pass
# `tab` goes last; scripts written before (run(None, cmd), run(tab, cmd), log_to(tab, path),
# prompt(tab)) still work.
got = []
b.prompt = lambda timeout=60, tab=None: got.append(("prompt", timeout, tab)) or "$"
b._read = lambda tab, text, timeout: "out"
b.log_to("/tmp/x", t1); b.log_to(t1, "/tmp/y")
assert b._logs[1] == "/tmp/y", b._logs
b._logs.clear()
calls.clear()
b.run("ls", until="#"); b.run(None, "pwd"); b.run(t1, "id", 5); b.run("w", tab=t1)
assert [c for c in calls if c[0].endswith("Send")] == [
    ("tab0.Screen.Send", "ls\r"), ("tab0.Screen.Send", "pwd\r"),
    ("tab1.Screen.Send", "id\r"), ("tab1.Screen.Send", "w\r")], calls
assert [g[1] for g in got] == [30, 30, 5, 30], got
# each_host calls func(host, tab); a func written before as (tab, host) still gets them right.
calls.clear()
crt.Session = Rec("crt.Session")
crt.Session.Connect = lambda cmd: 7
seen = []
def new(host, tab): seen.append((host, tab.Session._n))
def old(tab, host): seen.append((host, tab.Session._n))
b.each_host(["h1"], new); b.each_host(["h2"], old)
assert seen == [("h1", "tab7.Session"), ("h2", "tab7.Session")], seen
calls.clear()
b.activate(); b.upload("l", tab=t1)
assert ("tab0.Activate",) in calls and ("crt.FileTransfer.Upload", "l", "") in calls, calls
"##;
        let o = Command::new("python3")
            .args(["-I", "-c", SNIPPET])
            .arg(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../res/python"))
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    }

    #[test]
    fn catalog_matches_crt_py() {
        use std::process::Command;
        if Command::new("python3").arg("-V").output().is_err() {
            return;
        }
        const SNIPPET: &str = r#"
import sys, inspect, json
sys.path.insert(0, sys.argv[1])
import crt
out = {}
for cls, g in [("_Crt", ""), ("_Screen", "Screen"), ("_Session", "Session"), ("_Tab", "Tab"),
               ("_Dialog", "Dialog"), ("_Clipboard", "Clipboard"), ("_Arguments", "Arguments"),
               ("_Window", "Window"), ("_FileTransfer", "FileTransfer")]:
    for n, v in vars(getattr(crt, cls)).items():
        if n.startswith("_"):
            continue
        path = g + "." + n if g else n
        if isinstance(v, property):
            out[path] = None
        elif inspect.isfunction(v):
            ps = list(inspect.signature(v).parameters.values())[1:]
            pre = {inspect.Parameter.VAR_POSITIONAL: "*", inspect.Parameter.VAR_KEYWORD: "**"}
            out[path] = [pre.get(p.kind, "") + p.name for p in ps]
import shag_blocks
for n in shag_blocks.__all__:
    ps = list(inspect.signature(getattr(shag_blocks, n)).parameters.values())
    out["blocks." + n] = [p.name for p in ps]
print(json.dumps(out))
"#;
        let o = Command::new("python3")
            .args(["-I", "-c", SNIPPET])
            .arg(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../res/python"))
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        let py: BTreeMap<String, Option<Vec<String>>> = serde_json::from_slice(&o.stdout).unwrap();
        let ours: BTreeMap<String, Option<Vec<String>>> = CATALOG
            .iter()
            .map(|e| {
                let ps = (e.kind == Kind::Method)
                    .then(|| e.params.iter().map(|p| p.name.to_string()).collect());
                (e.path.to_string(), ps)
            })
            .collect();
        for (k, v) in &py {
            assert_eq!(ours.get(k), Some(v), "crt.py has {k}");
        }
        for k in ours.keys() {
            assert!(py.contains_key(k), "crt.py has no {k}");
        }
    }

    #[test]
    fn paths_unique() {
        let mut seen = std::collections::HashSet::new();
        for e in CATALOG {
            assert!(seen.insert(e.path), "{} twice", e.path);
        }
    }

    #[test]
    fn every_entry_has_help() {
        assert!(!CATALOG.is_empty());
        for e in CATALOG {
            assert!(!e.help[0].is_empty() && !e.help[1].is_empty(), "{}", e.path);
            assert!(!e.example.is_empty(), "{}", e.path);
        }
    }

    #[test]
    fn lookup_by_receiver() {
        assert_eq!(lookup("crt.Screen.Send").unwrap().path, "Screen.Send");
        assert_eq!(lookup("tab.Screen.Send").unwrap().path, "Screen.Send");
        assert_eq!(
            lookup("crt.GetTab(2).Screen.Send").unwrap().path,
            "Screen.Send"
        );
        assert_eq!(lookup("crt.Sleep").unwrap().path, "Sleep");
        assert_eq!(
            lookup("crt.Window.Activate").unwrap().path,
            "Window.Activate"
        );
        assert_eq!(lookup("tab.Activate").unwrap().path, "Tab.Activate");
        assert!(lookup("x.Sleep").is_none());
        assert!(lookup("os.path.join").is_none());
        assert!(lookup("foo").is_none());
        assert_eq!(lookup("run").unwrap().path, "blocks.run");
        assert!(lookup("x.run").is_none());
    }
}
