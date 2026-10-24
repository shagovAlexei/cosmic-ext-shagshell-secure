# 10_comment_in_log.py
#
# EN: Writes a timestamped comment into the tab's session log (File -> Log Session, or the
#     session's own log setting). If the log is off the script starts it.
#     SecureCRT: like AddCommentToLogFile.py (Session.Log / Session.LogFileName).
# RU: Пишет комментарий с отметкой времени в лог сессии вкладки (File -> Log Session или
#     собственная настройка лога сессии). Если лог выключен, скрипт его включает.
#     SecureCRT: как AddCommentToLogFile.py (Session.Log / Session.LogFileName).
import datetime

session = crt.GetScriptTab().Session
text = crt.Dialog.Prompt("Comment / Комментарий:", "Example 10")
if text == "":
    crt.Quit()

if session.LogFileName == "":
    session.Log(True, True)             # EN: start, append / RU: включить, дописывать
path = session.LogFileName

# EN: Writing to the file straight away: the log appends, so we add our own lines at the end.
# RU: Пишем в файл напрямую: лог дописывается, поэтому добавляем свои строки в конец.
with open(path, "a", encoding="utf-8") as f:
    f.write("\n/* %s: %s */\n" % (datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S"), text))
crt.Dialog.MessageBox("Added to / Добавлено в:\n" + path, "Example 10")
