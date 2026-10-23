"""ShagShell Secure scripting: the `crt` object of SecureCRT, over the app's socket.

Scripts run with `crt` already defined (as in SecureCRT); `import crt` works too. Every call
is one JSON line to the app and blocks for its answer.
"""
import builtins
import json
import os
import socket
import sys

__all__ = ["crt", "ScriptError"]
Version = "1.0"


class ScriptError(Exception):
    """The app refused a call (no such tab, bad argument…)."""


class Cancelled(SystemExit):
    """The user cancelled the script, or the app went away."""


_conn = None
_next = 0


def _call(call, tab, *args):
    global _conn, _next
    if _conn is None:
        s = socket.socket(socket.AF_UNIX)
        s.connect(os.environ["SHAGSHELL_SCRIPT_SOCK"])
        _conn = (s, s.makefile("rw", encoding="utf-8", newline="\n"))
    _next += 1
    f = _conn[1]
    try:
        f.write(json.dumps({"id": _next, "call": call, "tab": tab, "args": list(args)}) + "\n")
        f.flush()
        line = f.readline()
    except OSError:
        raise Cancelled(1)
    if not line:
        raise Cancelled(1)
    reply = json.loads(line)
    if "err" in reply:
        raise ScriptError(reply["err"])
    return reply.get("ok")


class _Screen:
    def __init__(self, tab):
        self._tab = tab
        self.Synchronous = True  # always, as if set

    def Send(self, text):
        _call("Screen.Send", self._tab, text)

    def WaitForString(self, text, timeout=0, bCaseInsensitive=False):
        return bool(_call("Screen.WaitForString", self._tab, text, timeout, bCaseInsensitive))

    def WaitForStrings(self, *texts, **kw):
        # WaitForStrings("a", "b"[, timeout[, bCaseInsensitive]]) or WaitForStrings(["a", "b"], …)
        texts = list(texts)
        timeout = kw.get("timeout", 0)
        fold = kw.get("bCaseInsensitive", False)
        if texts and isinstance(texts[-1], bool):
            fold = texts.pop()
        if texts and isinstance(texts[0], (list, tuple)):
            items = list(texts[0])
            timeout = texts[1] if len(texts) > 1 else timeout
        else:
            if texts and isinstance(texts[-1], (int, float)):
                timeout = texts.pop()
            items = texts
        return _call("Screen.WaitForStrings", self._tab, items, timeout, fold)

    def ReadString(self, text, timeout=0, bCaseInsensitive=False):
        r = _call("Screen.ReadString", self._tab, text, timeout, bCaseInsensitive)
        return "" if r is None else r


class _Session:
    def __init__(self, tab):
        self._tab = tab

    @property
    def Connected(self):
        return bool(_call("Session.Connected", self._tab))

    @property
    def Path(self):
        return _call("Session.Path", self._tab) or ""

    def Connect(self, cmd):
        """`/S "name"` — a saved session — opens a new tab."""
        _call("Session.Connect", self._tab, cmd)

    def Disconnect(self):
        _call("Session.Disconnect", self._tab)


class _Tab:
    def __init__(self, tab):
        self._tab = tab
        self.Screen = _Screen(tab)
        self.Session = _Session(tab)

    @property
    def Index(self):
        return _call("Tab.Index", self._tab)

    @property
    def Caption(self):
        return _call("Tab.Caption", self._tab)

    def Activate(self):
        _call("Tab.Activate", self._tab)


class _Dialog:
    def MessageBox(self, text, title="", buttons=0):
        _call("Dialog.MessageBox", 0, str(text), str(title))
        return 1

    def Prompt(self, message, title="", default="", bPassword=False):
        r = _call("Dialog.Prompt", 0, str(message), str(title), str(default), bool(bPassword))
        return "" if r is None else r


class _Arguments:
    def __init__(self, args):
        self._a = list(args)

    @property
    def Count(self):
        return len(self._a)

    def GetArg(self, i):
        return self._a[i]

    def __len__(self):
        return len(self._a)

    def __iter__(self):
        return iter(self._a)

    def __getitem__(self, i):
        return self._a[i]


class _Crt:
    def __init__(self):
        self._tab = int(os.environ.get("SHAGSHELL_SCRIPT_TAB", "0"))
        self.Screen = _Screen(self._tab)
        self.Session = _Session(self._tab)
        self.Dialog = _Dialog()
        self.Arguments = _Arguments(sys.argv[1:])
        self.ScriptFullName = os.environ.get("SHAGSHELL_SCRIPT", "")
        self.Version = Version

    def GetTab(self, n):
        return _Tab(_call("GetTab", 0, int(n)))

    def GetScriptTab(self):
        return _Tab(self._tab)

    def GetActiveTab(self):
        return _Tab(_call("GetActiveTab", 0))

    def GetTabCount(self):
        return _call("GetTabCount", 0)

    def Sleep(self, ms):
        _call("Sleep", 0, ms)

    def Quit(self):
        raise Cancelled(0)


crt = _Crt()


def __getattr__(name):  # `import crt; crt.Screen.Send(...)`
    return getattr(crt, name)


def main():
    """Runs the script named by argv[1] with `crt` defined."""
    import runpy
    import traceback

    path = sys.argv[1]
    sys.argv = sys.argv[1:]
    crt.Arguments = _Arguments(sys.argv[1:])
    builtins.crt = crt
    code = 0
    try:
        runpy.run_path(path, run_name="__main__")
    except Cancelled as e:
        code = e.code or 0
    except SystemExit as e:
        code = e.code if isinstance(e.code, int) else (0 if e.code is None else 1)
    except BaseException:
        traceback.print_exc()
        code = 1
    sys.stdout.flush()
    os._exit(code)
