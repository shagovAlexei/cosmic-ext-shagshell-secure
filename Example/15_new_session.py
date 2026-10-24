# 15_new_session.py
#
# EN: Creates a session from a script, opens it, and uses the newer crt calls:
#     Session.Create (no password is stored), Screen.SendKeys (text as it is, "\r" is not Enter),
#     Screen.Clear, Screen.MatchIndex (which string WaitForStrings saw), Session.SetStatusText.
# RU: Создаёт сессию из скрипта, открывает её и показывает новые вызовы crt:
#     Session.Create (пароль не сохраняется), Screen.SendKeys (текст как есть, "\r" — не Enter),
#     Screen.Clear, Screen.MatchIndex (какую строку увидел WaitForStrings), Session.SetStatusText.
path = crt.Session.Create("Example host", "127.0.0.1", "", 22, "Examples")
crt.Session.Connect('/S "%s"' % path)
tab = crt.GetActiveTab()
tab.Screen.WaitForStrings("assword:", "$ ", "# ", 15)
tab.Session.SetStatusText("matched string #%d" % tab.Screen.MatchIndex)
tab.Screen.Clear()
tab.Screen.SendKeys("echo \\r stays as typed")
tab.Screen.Send("\r")
