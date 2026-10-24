# Notes

This is your note: edit it, it is saved by itself. Rename it or open another text / `.md` file with the buttons above. The eye button shows a `.md` file as formatted text.

## ShagShell keys

- `Alt+Q` — Quick Connect, `Ctrl+Shift+T` — local shell
- Session Manager: `Enter` connect, `F2` rename, `Delete` delete
- SFTP tab: `F5` copy, `F6` move, `F7` new folder, `F8` delete, `Tab` other panel, `Ctrl+H` hidden files
- Command Window: `Enter` sends, `Ctrl+Enter` new line, `Up` / `Down` history

## ssh

```
ssh -J jump.example.com user@10.0.0.5        # through a jump host
ssh -L 8080:localhost:80 user@host            # local forward
ssh -D 1080 user@host                         # SOCKS proxy
ssh-copy-id -i ~/.ssh/id_ed25519.pub user@host
```

`~/.ssh/config` hosts can be imported: Session Manager → import.

## Scripts (Python, `crt`)

```python
crt.Screen.Send("show version\r")
crt.Screen.WaitForString("#", 10)
out = crt.Screen.ReadString("#", 10)
crt.Dialog.MessageBox("Done")
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
