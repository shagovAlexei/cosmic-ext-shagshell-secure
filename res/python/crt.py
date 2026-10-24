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
Version = "1.1"

# MessageBox buttons and results, as in SecureCRT.
BUTTON_OK, BUTTON_OKCANCEL, BUTTON_ABORTRETRYIGNORE = 0, 1, 2
BUTTON_YESNOCANCEL, BUTTON_YESNO, BUTTON_RETRYCANCEL = 3, 4, 5
ICON_STOP, ICON_QUESTION, ICON_WARN, ICON_INFO = 16, 32, 48, 64
DEFBUTTON1, DEFBUTTON2, DEFBUTTON3 = 0, 256, 512
IDOK, IDCANCEL, IDABORT, IDRETRY, IDIGNORE, IDYES, IDNO = 1, 2, 3, 4, 5, 6, 7


class ScriptError(Exception):
    """The app refused a call (no such tab, bad argument…)."""


class Cancelled(SystemExit):
    """The user cancelled the script, or the app went away."""


_conn = None
_next = 0
_last_error = [""]


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
        _last_error[0] = reply["err"]
        raise ScriptError(reply["err"])
    return reply.get("ok")


class _Screen:
    def __init__(self, tab):
        self._tab = tab
        self.Synchronous = True  # always, as if set
        self.IgnoreEscape = True  # the output is always plain text

    def _info(self):
        return _call("Screen.Info", self._tab)

    @property
    def CurrentRow(self):
        return self._info()[0]

    @property
    def CurrentColumn(self):
        return self._info()[1]

    @property
    def Rows(self):
        return self._info()[2]

    @property
    def Columns(self):
        return self._info()[3]

    @property
    def Selection(self):
        return _call("Screen.Selection", self._tab) or ""

    def Get(self, row1, col1, row2, col2):
        """The text of a block of the screen (1-based, ends included), lines joined by \\n."""
        return _call("Screen.Get", self._tab, int(row1), int(col1), int(row2), int(col2)) or ""

    def WaitForCursor(self, timeout=0):
        """True when the cursor moved within `timeout` seconds, False when it kept still."""
        import time

        start = self._info()[:2]
        end = time.time() + timeout
        while time.time() < end:
            time.sleep(0.05)
            if self._info()[:2] != start:
                return True
        return False

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

    @property
    def LogFileName(self):
        """The file of the log that is on ("" when it is off)."""
        return _call("Session.LogFileName", self._tab) or ""

    def LogText(self, text):
        """Writes `text` into the tab's log (it must be on), in order with the output."""
        _call("Session.LogText", self._tab, str(text))

    def Log(self, on=True, append=None):
        """Starts / stops the tab's log (name template of the session or Global Options)."""
        _call("Session.Log", self._tab, bool(on), None if append is None else bool(append))

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
        r = _call("Dialog.MessageBox", 0, str(text), str(title), int(buttons))
        return IDOK if r is None else r

    def FileOpenDialog(self, title="", buttonLabel="", defaultFilename="", filter=""):
        """The file chosen ("" when cancelled). filter: "Text (*.txt)|*.txt|All (*.*)|*.*"."""
        r = _call("Dialog.FileOpen", 0, str(title), str(buttonLabel), str(defaultFilename), str(filter))
        return "" if r is None else r

    def FileSaveDialog(self, title="", buttonLabel="", defaultFilename="", filter=""):
        """The file name chosen for saving ("" when cancelled)."""
        r = _call("Dialog.FileSave", 0, str(title), str(buttonLabel), str(defaultFilename), str(filter))
        return "" if r is None else r

    def List(self, message, title="", items=(), default=0, multi=False):
        """A list to choose from: the item (str), a list of items when `multi`, None if cancelled."""
        return _call("Dialog.List", 0, str(message), str(title), [str(i) for i in items], int(default), bool(multi))

    def Prompt(self, message, title="", default="", bPassword=False):
        r = _call("Dialog.Prompt", 0, str(message), str(title), str(default), bool(bPassword))
        return "" if r is None else r


class _Clipboard:
    @property
    def Text(self):
        return _call("Clipboard.Get", 0) or ""

    @Text.setter
    def Text(self, value):
        _call("Clipboard.Set", 0, str(value))

    Format = "CF_TEXT"  # accepted, ignored


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
        self.Clipboard = _Clipboard()
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

    def GetLastErrorMessage(self):
        return _last_error[0]

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
    for k, v in list(globals().items()):  # BUTTON_YESNO, IDYES … as in SecureCRT
        if k.startswith(("BUTTON_", "ICON_", "DEFBUTTON", "ID")) and k.isupper():
            setattr(builtins, k, v)
    code = 0
    try:
        runpy.run_path(path, run_name="__main__")
    except Cancelled as e:
        code = e.code or 0
    except SystemExit as e:
        code = e.code if isinstance(e.code, int) else (0 if e.code is None else 1)
    except BaseException as e:
        # Only the script's own frames: runpy's and ours say nothing to the author.
        tb = e.__traceback__
        mine = os.path.abspath(path)
        while tb is not None and os.path.abspath(tb.tb_frame.f_code.co_filename) != mine:
            tb = tb.tb_next
        # (a syntax error has no script frame: just its own file / line)
        keep = None if isinstance(e, SyntaxError) else tb or e.__traceback__
        traceback.print_exception(type(e), e, keep)
        code = 1
    sys.stdout.flush()
    os._exit(code)
