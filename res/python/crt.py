"""ShagShell Secure scripting: the `crt` object of SecureCRT, over the app's socket.

Scripts run with `crt` already defined (as in SecureCRT); `import crt` works too. Every call
is one JSON line to the app and blocks for its answer.
"""
import builtins
import itertools
import json
import os
import socket
import sys
import threading

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


# A connection per thread: a script's threads (parallel hosts) wait for their own answers.
_local = threading.local()
_next = itertools.count(1)
_last_error = [""]


def _call(call, tab, *args):
    conn = getattr(_local, "conn", None)
    if conn is None:
        s = socket.socket(socket.AF_UNIX)
        s.connect(os.environ["SHAGSHELL_SCRIPT_SOCK"])
        conn = _local.conn = (s, s.makefile("rw", encoding="utf-8", newline="\n"))
    f = conn[1]
    try:
        f.write(json.dumps({"id": next(_next), "call": call, "tab": tab, "args": list(args)}) + "\n")
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
        self.MatchIndex = 0  # the string the last WaitForStrings saw (1-based, 0: none)

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
        found = bool(_call("Screen.WaitForString", self._tab, text, timeout, bCaseInsensitive))
        self.MatchIndex = int(found)
        return found

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
        self.MatchIndex = _call("Screen.WaitForStrings", self._tab, items, timeout, fold)
        return self.MatchIndex

    def SendKeys(self, text):
        """Like Send, but `\\r`, `\\p`… are not escapes: the text goes as it is."""
        _call("Screen.Send", self._tab, str(text).replace("\\", "\\\\"))

    def Clear(self):
        _call("Screen.Clear", self._tab)

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
        """`/S "name"` — a saved session — opens a new tab. ShagShell: returns the new tab's
        id; ScriptError when no tab opened (no such session)."""
        return _call("Session.Connect", self._tab, cmd)

    def Disconnect(self):
        _call("Session.Disconnect", self._tab)

    def SetStatusText(self, text):
        _call("Session.SetStatusText", self._tab, str(text))

    def Lock(self):
        """The user can no longer type or paste into the tab (Send still works); freed when the
        script ends."""
        _call("Session.Lock", self._tab)

    def Unlock(self):
        _call("Session.Unlock", self._tab)

    @property
    def Locked(self):
        return bool(_call("Session.Locked", self._tab))

    def Create(self, name, host, user="", port=22, folder=""):
        """Saves a new SSH session (the Default template, no password); returns its path."""
        return _call("Session.Create", self._tab, name, host, user, int(port), folder)


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


class _Window:
    @property
    def Caption(self):
        return _call("Window.Caption", 0)

    def Activate(self):
        _call("Window.Activate", 0)


def _abs(p):
    p = str(p)
    return p if p.startswith("~/") else os.path.abspath(p)


class _FileTransfer:
    """SFTP over the script's tab (an SSH session); each call waits until it is done."""

    def __init__(self, tab):
        self._tab = tab

    def Upload(self, local, remote=""):
        """A file to the server (`remote`: a name, a folder, or empty = home); a folder goes whole
        INTO `remote` (empty = home)."""
        # Relative to the script's folder of work, not the app's.
        return _call("FileTransfer.Upload", self._tab, _abs(local), str(remote))

    def Download(self, remote, local=""):
        """From the server (`local`: a name, a folder, or empty = ~/Downloads)."""
        return _call("FileTransfer.Download", self._tab, str(remote), _abs(local) if local else "")


class _Crt:
    def __init__(self):
        self._tab = int(os.environ.get("SHAGSHELL_SCRIPT_TAB", "0"))
        self.Screen = _Screen(self._tab)
        self.Session = _Session(self._tab)
        self.FileTransfer = _FileTransfer(self._tab)
        self.Dialog = _Dialog()
        self.Window = _Window()
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


def _vars(frame):
    """The frame's variables a person wants on a pause: no modules, functions, classes, dunders."""
    import types
    out = []
    for k, v in frame.f_locals.items():
        if k.startswith("__") or k == "crt" or isinstance(v, (types.ModuleType, types.FunctionType, type)):
            continue
        r = repr(v)
        out.append([k, r if len(r) <= 120 else r[:117] + "..."])
    return out


def _trace(path):
    """The Script Builder's run: every line of the script (in every thread) is reported; one the
    window wants to stop at (a breakpoint, a step, Pause) waits here with its variables."""
    names = {path, os.path.abspath(path)}

    def local(frame, event, arg):
        if event == "line" and _call("Builder.Line", 0, frame.f_lineno):
            _call("Builder.Paused", 0, frame.f_lineno, _vars(frame))
        return local

    def calls(frame, event, arg):
        return local if event == "call" and frame.f_code.co_filename in names else None

    sys.settrace(calls)
    threading.settrace(calls)


def main():
    """Runs the script named by argv[1] with `crt` defined."""
    import runpy
    import traceback

    path = sys.argv[1]
    sys.argv = sys.argv[1:]
    # `import shag_blocks` (or a module of the user's) next to the script, or in the scripts folder.
    config = os.environ.get("XDG_CONFIG_HOME") or os.path.expanduser("~/.config")
    # The script's own folder first; the scripts folder last (a csv.py there must not hide the
    # standard one from scripts elsewhere).
    sys.path.insert(0, os.path.dirname(os.path.abspath(path)))
    scripts = os.path.join(config, "shagshell", "scripts")
    if scripts not in sys.path:
        sys.path.append(scripts)
    crt.Arguments = _Arguments(sys.argv[1:])
    builtins.crt = crt
    for k, v in list(globals().items()):  # BUTTON_YESNO, IDYES … as in SecureCRT
        if k.startswith(("BUTTON_", "ICON_", "DEFBUTTON", "ID")) and k.isupper():
            setattr(builtins, k, v)
    if os.environ.get("SHAGSHELL_TRACE"):
        _trace(path)
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
