# Roadmap

Детали — в `.claude/docs/specs/00-architecture.md`. Каждый этап и каждую фичу из бэклога начинаем с брейншторма; один этап = один PR.

## Этапы

- [x] 0. Bootstrap: репозиторий, CLAUDE.md, архитектура, разбор SecureCRT, навык `shagshell-feature`
- [x] 1. Каркас: workspace, пустое окно libcosmic, justfile, CI, i18n en/ru, `.desktop`, metainfo, иконка
- [x] 2. Терминал: вкладка локального shell, вкладки + `pane_grid` (tab groups)
- [x] 3. Сессии: 3a — модель + хранилище, Session Manager, Quick Connect, запуск ssh, импорт ~/.ssh/config; 3b — окно Session Options
- [x] 4. Аутентификация: 4a — askpass, keyring, диалоги пароля / host key; 4b — Credentials Manager, мастер-пароль (своя связка keyring)
- [ ] 5. SFTP: двухпанельная вкладка + очередь передач (drag-drop, докачка, правила перезаписи)
- [ ] 6. Command Window (send to all) и Button Bar: именованные панели, Send String / Menu Function / Launch App, панель на сессию
- [ ] 7. Логирование сессий по шаблонам имён
- [ ] 8. Jump host / port forwarding в UI, Telnet и Serial, keepalive + auto-reconnect
- [ ] 9. Logon Actions (expect/send), keyword highlighting, color schemes, импорт из `~/.ssh/config` и SecureCRT `.ini`
- [ ] 10. Скрипты на Python: модуль `crt` (API как в SecureCRT), меню Script, запуск с Button Bar / клавиши / logon script / `--script`

## Бэклог

- Flatpak
- Генерация ключей в UI (обёртка над `ssh-keygen`), загрузка ключа на сервер (`ssh-copy-id`)
- Редактирование удалённого файла локально (SecureFX «Edit»)
- Синхронизация папок в SFTP
- X/Y/Zmodem (`rz`/`sz`)
- Command Manager (дерево сохранённых команд)
- Перетаскивание вкладки между группами и в новое окно
- Шрифт и его размер в Global Options
- Запись скрипта (Script → Start Recording)

Не делаем: SSH1, RLogin, TAPI, TN3270/5250, VBScript/JScript, legacy-эмуляции.
