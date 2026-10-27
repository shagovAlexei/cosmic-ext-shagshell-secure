"""ShagShell Secure Script Builder: the big blocks (spec 36), plain functions over `crt`.

A script made with them starts with `from shag_blocks import *`. ShagShell keeps this file in
~/.config/shagshell/scripts (and finds it from any folder); for SecureCRT put it next to the script.
"""
import builtins
import os
import threading
import time

__all__ = ["prompt", "run", "read_list", "log_to", "append_file", "to_clipboard", "each_host"]

crt = getattr(builtins, "crt", None)
if crt is None:  # imported before `crt` is defined (SecureCRT defines it as a builtin too)
    from crt import crt

# The log file each tab's commands go to (`log_to`), by tab index.
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


def run(tab, cmd, timeout=30):
    """Types `cmd` and Enter, waits for the prompt again; the output (without the echoed command
    and the prompt). Written to the tab's `log_to` file too. `tab` None: the script's tab."""
    tab = _tab(tab)
    p = prompt(tab, timeout)
    tab.Screen.Send(cmd + "\r")
    tab.Screen.WaitForString("\n", timeout)
    out = tab.Screen.ReadString(p, timeout).strip()
    path = _logs.get(tab.Index)
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
    _logs[tab.Index] = path
    return path


def append_file(path, text):
    """Adds `text` and a line end at the end of a file (made if missing)."""
    path = os.path.expanduser(path)
    with _lock, open(path, "a", encoding="utf-8") as f:
        f.write(str(text) + "\n")


def to_clipboard(text):
    """Puts `text` on the clipboard."""
    crt.Clipboard.Text = str(text)


def each_host(hosts, func, parallel=False, limit=10):
    """For every saved session in `hosts`: connect (a new tab), `func(tab, host)`, disconnect.
    `parallel`: at most `limit` hosts at the same time, each in a thread. A host that fails does
    not stop the others; the failed ones are returned as (host, error) and printed."""
    failed = []
    gate = threading.Semaphore(max(int(limit), 1))
    opening = threading.Lock()

    def one(host):
        with gate:
            try:
                # One connect at a time: the new tab is the active one only until the next.
                with opening:
                    crt.Session.Connect('/S "%s"' % host)
                    tab = crt.GetActiveTab()
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
