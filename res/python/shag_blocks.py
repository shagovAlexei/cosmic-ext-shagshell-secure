"""ShagShell Secure Script Builder: the big blocks (spec 36), plain functions over `crt`.

A script made with them starts with `from shag_blocks import *`. ShagShell keeps this file in
~/.config/shagshell/scripts (and finds it from any folder); for SecureCRT put it next to the script.
"""
import builtins
import os
import threading
import time

__all__ = ["prompt", "run", "read_list", "log_to", "append_file", "to_clipboard", "from_clipboard",
           "each_host", "printbox", "ask", "read_until", "connect", "active_tab", "disconnect",
           "send", "send_keys", "wait_for", "wait_any", "clear", "script_tab", "all_tabs", "connected",
           "activate", "status", "log_text", "choose", "pick_file", "arg", "args", "stop", "last_error",
           "new_session", "lock", "unlock", "upload", "download", "pause"]

crt = getattr(builtins, "crt", None)
if crt is None:  # imported before `crt` is defined (SecureCRT defines it as a builtin too)
    from crt import crt
try:  # ShagShell's crt.py: a tab by id, a ReadString that tells a timeout from empty output
    import crt as _crt
except ImportError:  # SecureCRT
    _crt = None

# The log file each tab's commands go to (`log_to`), by tab id (an index moves when a tab closes).
_logs = {}
_lock = threading.Lock()


def _tab(tab):
    return crt.GetScriptTab() if tab is None else tab


def prompt(tab=None, timeout=60):
    """The prompt the tab shows, once the login is over: text on the cursor's row and the cursor
    still for a second (askpass, host key, MOTD and slow links take time)."""
    s = _tab(tab).Screen
    end = time.time() + timeout
    while time.time() < end:
        row, col = s.CurrentRow, s.CurrentColumn
        line = s.Get(row, 1, row, max(col - 1, 1)).rstrip()
        if line and not s.WaitForCursor(1):
            return line.strip()
        crt.Sleep(200)
    raise TimeoutError("no prompt in %d s" % timeout)


def _key(tab):
    k = getattr(tab, "_tab", None)
    return tab.Index if k is None else k


def _read(tab, text, timeout):
    """The output up to `text`; None when it did not come in `timeout` s."""
    if _crt is not None and hasattr(_crt, "_call"):
        return _crt._call("Screen.ReadString", tab._tab, text, timeout, False)
    return tab.Screen.ReadString(text, timeout) or None


def run(tab, cmd, timeout=30, until=None):
    """Types `cmd` and Enter, waits for the prompt again; the output (without the echoed command
    and the prompt). Written to the tab's `log_to` file too. `tab` None: the script's tab.
    A command that changes the prompt (enable, conf t, cd): `until` is the text to wait for
    instead (`'#'`). Not seen in `timeout` s: TimeoutError (the host is left, not the next one)."""
    tab = _tab(tab)
    p = prompt(tab, timeout)
    tab.Screen.Send(cmd + "\r")
    tab.Screen.WaitForString("\n", timeout)
    wait = until or p
    out = _read(tab, wait, timeout)
    if out is None:
        raise TimeoutError("%s: no %r in %d s" % (cmd, wait, timeout))
    out = out.strip()
    path = _logs.get(_key(tab))
    if path:
        append_file(path, "%s %s\n%s" % (p, cmd, out))
    return out


def read_list(path):
    """The lines of a file, without empty ones and `#` comments."""
    with open(os.path.expanduser(path), encoding="utf-8") as f:
        return [l.strip() for l in f if l.strip() and not l.lstrip().startswith("#")]


def log_to(tab, path):
    """The commands `run` sends on this tab and their output go to `path` from now on.
    In the name: %S the tab's title, %Y %M %D the date, %h %m %s the time."""
    tab = _tab(tab)
    t = time.localtime()
    name = tab.Caption.replace("/", "_")
    for k, v in (("%S", name), ("%Y", "%04d" % t.tm_year), ("%M", "%02d" % t.tm_mon), ("%D", "%02d" % t.tm_mday),
                 ("%h", "%02d" % t.tm_hour), ("%m", "%02d" % t.tm_min), ("%s", "%02d" % t.tm_sec)):
        path = path.replace(k, v)
    path = os.path.expanduser(path)
    d = os.path.dirname(path)
    if d:
        os.makedirs(d, exist_ok=True)
    _logs[_key(tab)] = path
    return path


def append_file(path, text):
    """Adds `text` and a line end at the end of a file (made if missing)."""
    path = os.path.expanduser(path)
    with _lock, open(path, "a", encoding="utf-8") as f:
        f.write(str(text) + "\n")


def to_clipboard(text):
    """Puts `text` on the clipboard."""
    crt.Clipboard.Text = str(text)


def from_clipboard():
    """The clipboard's text."""
    return crt.Clipboard.Text


def printbox(text, title="", buttons=0):
    """A message box; returns the button pressed (short for crt.Dialog.MessageBox)."""
    return crt.Dialog.MessageBox(str(text), title, buttons)


def ask(message, title="", default="", password=False):
    """Asks for a line of text, "" when cancelled (short for crt.Dialog.Prompt)."""
    return crt.Dialog.Prompt(message, title, default, password)


def read_until(text, timeout=30, tab=None):
    """The output up to `text` (short for tab.Screen.ReadString); the tab: the script's own."""
    return _tab(tab).Screen.ReadString(text, timeout)


def connect(session):
    """Opens the saved session `session` in a new tab and returns that tab."""
    crt.Session.Connect('/S "%s"' % session)
    return crt.GetActiveTab()


def active_tab():
    """The tab in front (short for crt.GetActiveTab)."""
    return crt.GetActiveTab()


def disconnect(tab=None):
    """Disconnects the tab (the script's own by default)."""
    _tab(tab).Session.Disconnect()


def send(text, tab=None):
    """Types a line and Enter, without waiting (short for tab.Screen.Send(text + "\\r"))."""
    _tab(tab).Screen.Send(str(text) + "\r")


def send_keys(text, tab=None):
    """Types the text as it is, no Enter (short for tab.Screen.SendKeys)."""
    _tab(tab).Screen.SendKeys(str(text))


def wait_for(text, timeout=0, tab=None):
    """Waits for `text`; False after `timeout` s, 0 = no limit (short for Screen.WaitForString)."""
    return _tab(tab).Screen.WaitForString(text, timeout)


def wait_any(texts, timeout=0, tab=None):
    """Waits for one of `texts`; its number from 1, 0 after `timeout` s (short for
    Screen.WaitForStrings)."""
    return _tab(tab).Screen.WaitForStrings(list(texts), timeout)


def clear(tab=None):
    """Clears the tab's screen (short for Screen.Clear)."""
    _tab(tab).Screen.Clear()


def script_tab():
    """The tab the script runs on (short for crt.GetScriptTab)."""
    return crt.GetScriptTab()


def all_tabs():
    """Every tab, left to right (short for crt.GetTabCount + crt.GetTab)."""
    return [crt.GetTab(i) for i in range(1, crt.GetTabCount() + 1)]


def connected(tab=None):
    """Whether the tab is connected (short for tab.Session.Connected)."""
    return bool(_tab(tab).Session.Connected)


def activate(tab):
    """Brings the tab to the front (short for tab.Activate)."""
    tab.Activate()


def status(text, tab=None):
    """Text in the status bar (short for Session.SetStatusText)."""
    _tab(tab).Session.SetStatusText(str(text))


def log_text(text, tab=None):
    """Writes `text` into the tab's session log, starting the log (appending) if it is off;
    returns the log file (short for Session.Log + Session.LogText)."""
    s = _tab(tab).Session
    if s.LogFileName == "":
        s.Log(True, True)
    s.LogText(str(text))
    return s.LogFileName


def choose(message, items, title="", default=0, multi=False):
    """A list to choose from: the item, a list of items when `multi`, None if cancelled
    (short for crt.Dialog.List)."""
    return crt.Dialog.List(message, title, list(items), default, multi)


def pick_file(title="", filter="", button="", default=""):
    """A file chosen in a dialog, "" when cancelled; `filter`: "Text (*.txt)|*.txt|All (*.*)|*.*"
    (short for crt.Dialog.FileOpenDialog)."""
    return crt.Dialog.FileOpenDialog(title, button, default, filter)


def arg(n, default=""):
    """The script's argument `n` (from 0), `default` when there are fewer (short for
    crt.Arguments[n])."""
    a = list(crt.Arguments)
    return a[n] if 0 <= n < len(a) else default


def args():
    """Every argument of the script (short for crt.Arguments)."""
    return list(crt.Arguments)


def stop():
    """Ends the script here (short for crt.Quit)."""
    crt.Quit()
    raise SystemExit(0)  # SecureCRT's Quit may return; the script ends here all the same


def last_error():
    """The last error's text (short for crt.GetLastErrorMessage)."""
    return crt.GetLastErrorMessage()


def new_session(name, host, user="", port=22, folder=""):
    """Saves a new SSH session (no password); its path, for connect() (short for
    crt.Session.Create)."""
    return crt.Session.Create(name, host, user, port, folder)


def lock(tab=None):
    """The user can no longer type into the tab, until unlock() or the script's end (short for
    Session.Lock)."""
    _tab(tab).Session.Lock()


def unlock(tab=None):
    """Lets the user type into the tab again (short for Session.Unlock)."""
    _tab(tab).Session.Unlock()


def upload(local, remote=""):
    """A file to the server over the script tab's SFTP; `remote`: a name, a folder, or "" = home
    (short for crt.FileTransfer.Upload)."""
    return crt.FileTransfer.Upload(local, remote)


def download(remote, local=""):
    """A file from the server over the script tab's SFTP; `local` "" = ~/Downloads (short for
    crt.FileTransfer.Download)."""
    return crt.FileTransfer.Download(remote, local)


def pause(ms):
    """Waits `ms` milliseconds (short for crt.Sleep)."""
    crt.Sleep(ms)


def each_host(hosts, func, parallel=False, limit=10):
    """For every saved session in `hosts`: connect (a new tab), `func(tab, host)`, disconnect.
    `parallel`: at most `limit` hosts at the same time, each in a thread. A host that fails does
    not stop the others; the failed ones are returned as (host, error) and printed."""
    failed = []
    gate = threading.Semaphore(max(int(limit), 1) if parallel else 1)
    opening = threading.Lock()

    def one(host):
        with gate:
            try:
                # ShagShell answers with the new tab (an error when none opened: a host never
                # runs on someone else's tab); SecureCRT: the active tab, one connect at a time.
                with opening:
                    tid = crt.Session.Connect('/S "%s"' % host)
                    tab = _crt._Tab(tid) if tid is not None and _crt is not None else crt.GetActiveTab()
            except Exception as e:
                failed.append((host, str(e)))
                return
            try:
                func(tab, host)
            except Exception as e:
                failed.append((host, str(e)))
            finally:
                try:
                    tab.Session.Disconnect()
                except Exception:
                    pass

    if parallel:
        threads = [threading.Thread(target=one, args=(h,)) for h in hosts]
        for t in threads:
            t.start()
        for t in threads:
            t.join()
    else:
        for h in hosts:
            one(h)
    for host, e in failed:
        print("%s: %s" % (host, e))
    return failed
