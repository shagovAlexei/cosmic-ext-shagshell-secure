# 15_new_session.py (short blocks / короткие блоки)
#
# EN: Example/15 with short blocks. Creates a session from a script and opens it:
#     new_session() = Session.Create (no password is stored), connect() opens it and returns the
#     tab, wait_any() = Screen.WaitForStrings (returns which string came, like MatchIndex),
#     status() = Session.SetStatusText, clear() = Screen.Clear, send_keys() = Screen.SendKeys
#     (text as it is, "\r" is not Enter), send("") = just Enter.
# RU: Example/15 на коротких блоках. Создаёт сессию из скрипта и открывает её:
#     new_session() = Session.Create (пароль не сохраняется), connect() открывает её и возвращает
#     вкладку, wait_any() = Screen.WaitForStrings (возвращает, какая строка пришла, как MatchIndex),
#     status() = Session.SetStatusText, clear() = Screen.Clear, send_keys() = Screen.SendKeys
#     (текст как есть, "\r" — не Enter), send("") = просто Enter.
from shag_blocks import *

path = new_session("Example host", "127.0.0.1", "", 22, "Examples")
tab = connect(path)
n = wait_any(["assword:", "$ ", "# "], 15, tab)
status("matched string #%d" % n, tab)
clear(tab)
send_keys("echo \\r stays as typed", tab)
send("", tab)
