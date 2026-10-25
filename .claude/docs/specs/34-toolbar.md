# 34. Toolbar editor

After shagoff-commander's TC-style «Change button bar» (the user asked for it instead of 0.61.0's toggles).

## What
- The toolbar is a list of buttons and gaps (`Config.toolbar_buttons`; empty = `toolbar::default_bar`: Session Manager | Connect, Quick Connect, Local Shell | Reconnect, Disconnect | SFTP, Session Options).
- A button runs an app function (`toolbar::TOOLS`, 26) or a program: `cmd` + `params`, `%H %S %U %P` of the active session (as a Button Bar «Launch App»).
- Right click on the toolbar, or Global Options → Toolbar → Open: the editor. List with icons; Add a button / Add a gap / Delete / ↑ ↓ / Default; the selected button's Command (a program, or a function from the list), Parameters, Icon (typed, or Pick… from a grid of the icon theme's symbolic icons, filtered), Hint. OK saves, Cancel drops.

## How
- `core::toolbar`: `ToolButton`, `SEP`, `ToolEdit` + `ToolMsg` (the editor's state), `matching` (the picker's filter); `core::icons::symbolic` (from shagoff-commander) lists the theme's icons. Tested.
- `app::toolbar`: `TOOLS`, `default_bar`, `view` (bar + Connect Bar), `editor`, `picker_view`, `icon_names` (scanned once, off the UI thread); `App::toolbar` runs a button / the editor.

## Not now
- Shift+drag of a button on the bar (commander has it); a button's own working directory.

## 0.80.0: script buttons

A program whose command ends in `.py` is a script (`ToolButton::is_script`): pressing it runs it on the active tab like Script → Run…, Parameters split shell-style into its arguments (an unclosed quote: the status bar says so). The editor's «Browse…» beside Command picks the file; the function list has its own row. A script's default icon is `system-run-symbolic`, its default hint the file name.
