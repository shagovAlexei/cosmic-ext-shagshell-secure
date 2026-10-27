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
        &[d("tab", Expr, "None"), d("timeout", Number, "60")],
        "str",
        [
            "The prompt the tab shows, once the login is over.",
            "Приглашение, которое показывает вкладка, когда вход закончен.",
        ],
        "p = prompt(tab)",
    ),
    m(
        "blocks.run",
        &[p("tab", Expr), p("cmd", Text), d("timeout", Number, "30")],
        "str",
        [
            "Runs a command: types it and Enter, waits for the prompt; its output (also into the tab's log_to file). tab None: the script's tab.",
            "Выполняет команду: печатает её и Enter, ждёт приглашения; возвращает вывод (и пишет в файл log_to вкладки). tab None — вкладка скрипта.",
        ],
        "out = run(tab, 'show version')",
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
        &[p("tab", Expr), p("path", File)],
        "str",
        [
            "From now on run writes this tab's commands and output to path (%S tab title, %Y %M %D date, %h %m %s time).",
            "Дальше run пишет команды и вывод этой вкладки в path (%S заголовок вкладки, %Y %M %D дата, %h %m %s время).",
        ],
        "log_to(tab, '~/logs/%S_%Y%M%D.log')",
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
        "blocks.each_host",
        &[
            p("hosts", Expr),
            p("func", Expr),
            d("parallel", Bool, "False"),
            d("limit", Number, "10"),
        ],
        "list",
        [
            "For every saved session in hosts: connect, func(tab, host), disconnect; the failed ones as (host, error).",
            "Для каждой сохранённой сессии из hosts: подключиться, func(tab, host), отключиться; возвращает неудачные (хост, ошибка).",
        ],
        "each_host(hosts, check)",
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
