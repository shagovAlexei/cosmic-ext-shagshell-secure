# 15_new_session.py (short blocks / короткие блоки)
#
# EN: Example/15 with short blocks: connect() opens the new session and returns its tab; the
#     rest has no blocks and stays `crt`: Session.Create, Screen.WaitForStrings / MatchIndex,
#     Session.SetStatusText, Screen.Clear, Screen.SendKeys ("\r" is not Enter there).
# RU: Example/15 на коротких блоках: connect() открывает новую сессию и возвращает её вкладку;
#     для остального блоков нет, оно остаётся на `crt`: Session.Create, Screen.WaitForStrings /
#     MatchIndex, Session.SetStatusText, Screen.Clear, Screen.SendKeys ("\r" там - не Enter).
from shag_blocks import *

path = crt.Session.Create("Example host", "127.0.0.1", "", 22, "Examples")
tab = connect(path)
tab.Screen.WaitForStrings("assword:", "$ ", "# ", 15)
tab.Session.SetStatusText("matched string #%d" % tab.Screen.MatchIndex)
tab.Screen.Clear()
tab.Screen.SendKeys("echo \\r stays as typed")
tab.Screen.Send("\r")
