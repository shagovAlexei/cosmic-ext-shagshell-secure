# Заметки

Это ваша заметка: правьте, она сохраняется сама. Кнопками сверху её можно переименовать или открыть другой текстовый / `.md` файл. Кнопка с глазом показывает `.md` как оформленный текст.

## Клавиши ShagShell

- `Alt+Q` — быстрое подключение, `Ctrl+Shift+T` — локальная оболочка
- Менеджер сессий: `Enter` подключить, `F2` переименовать, `Delete` удалить
- Вкладка SFTP: `F5` копировать, `F6` переместить, `F7` новая папка, `F8` удалить, `Tab` другая панель, `Ctrl+H` скрытые файлы
- Командное окно: `Enter` отправляет, `Ctrl+Enter` новая строка, `Вверх` / `Вниз` история

## ssh

```
ssh -J jump.example.com user@10.0.0.5        # через промежуточный хост
ssh -L 8080:localhost:80 user@host            # локальный проброс
ssh -D 1080 user@host                         # SOCKS-прокси
ssh-copy-id -i ~/.ssh/id_ed25519.pub user@host
```

Хосты из `~/.ssh/config` можно импортировать: Менеджер сессий → импорт.

## Скрипты (Python, `crt`)

```python
crt.Screen.Send("show version\r")
crt.Screen.WaitForString("#", 10)
out = crt.Screen.ReadString("#", 10)
crt.Dialog.MessageBox("Готово")
```

## Cisco

```
show running-config | include hostname
show ip interface brief
terminal length 0
copy running-config startup-config
```

## MikroTik

```
/system resource print
/interface print
/ip address print
/export file=backup
```
