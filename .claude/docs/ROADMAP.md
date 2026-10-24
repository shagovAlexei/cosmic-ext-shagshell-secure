# Roadmap

Детали — в `.claude/docs/specs/00-architecture.md`. Каждый этап и каждую фичу из бэклога начинаем с брейншторма; один этап = один PR.

## Этапы

- [x] 0. Bootstrap: репозиторий, CLAUDE.md, архитектура, разбор SecureCRT, навык `shagshell-feature`
- [x] 1. Каркас: workspace, пустое окно libcosmic, justfile, CI, i18n en/ru, `.desktop`, metainfo, иконка
- [x] 2. Терминал: вкладка локального shell, вкладки + `pane_grid` (tab groups)
- [x] 3. Сессии: 3a — модель + хранилище, Session Manager, Quick Connect, запуск ssh, импорт ~/.ssh/config; 3b — окно Session Options
- [x] 4. Аутентификация: 4a — askpass, keyring, диалоги пароля / host key; 4b — Credentials Manager, мастер-пароль (своя связка keyring)
- [x] 5. SFTP: ~~5a — вкладка-коммандер (две панели, F5–F8, очередь, конфликты, перетаскивание между панелями)~~; ~~5b-1 — правила перезаписи, докачка, скрытые файлы, chmod, сортировка~~; ~~5b-2 — перетаскивание из файлового менеджера, правка удалённого файла, закладки~~
- [x] 6. ~~6a — Command Window (send to all)~~; ~~6b — Button Bar: именованные панели, Send String / Menu Function / Launch App, панель на сессию
- [x] 7. Логирование сессий по шаблонам имён
- [x] 8. ~~8a — jump host / port forwarding в UI, auto-reconnect~~; ~~8b — Telnet и Serial~~
- [x] 9. ~~9a — Logon Actions (expect/send)~~; ~~9b — keyword highlighting, color schemes~~; ~~9c — импорт из SecureCRT `.ini` (сессии, кнопки)~~
- [x] 10. ~~Скрипты на Python~~: ~~10a — модуль `crt`, меню Script, `--script`~~; ~~10b — запуск с кнопки / из сессии, импорт кнопок Run Script~~
- [x] 11. ~~Создание ключей (`ssh-keygen`) и загрузка на сервер (`ssh-copy-id`)~~
- [x] 12. ~~Key Map: свои горячие клавиши на строку / функцию меню / программу / скрипт~~
- [x] 13. ~~Синхронизация папок в SFTP (Ctrl+Y), сохранение времени файлов при копировании~~
- [x] 14. ~~X/Y/Zmodem через lrzsz (автоприём Zmodem, отправка и приём Y/X из меню Transfer)~~
- [x] 15. ~~Command Manager (панель справа, импорт из SecureCRT)~~
- [x] 16. ~~Перенос вкладок между группами (меню и перетаскивание)~~
- [x] 17. ~~Шрифт и его размер в Global Options~~
- [x] 18. ~~Примеры скриптов (`Example/`) и дополнения `crt` (Clipboard, Screen.Get, Session.Log, MessageBox с кнопками)~~

## Бэклог

- Flatpak
- Редактирование удалённого файла локально (SecureFX «Edit»)
- Импорт Button Bar из SecureCRT: кнопки Menu Function / Launch App (в настоящих файлах не встретились, формат строк неизвестен)
- Перетаскивание вкладки в новое окно (между группами готово)
- Запись скрипта (Script → Start Recording)

Не делаем: SSH1, RLogin, TAPI, TN3270/5250, VBScript/JScript, legacy-эмуляции.
