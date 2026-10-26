# 35. Notes: font, zoom, find and replace

Done in 0.84.0. It closes the «Заметки» items of the ROADMAP.

## What
- **Font.** Global Options → Notes has its own font family and size: the installed monospace families plus the system's, 8–48 px. They are stored in `Config.note_font_family` / `note_font_size` (`note_font_px`).
  - A change applies to every open note at once (`Tabs::set_note_font`) and to the ones opened later.
  - The note tab's menu has «Font…», which opens Global Options.
- **Zoom.** Ctrl+= / Ctrl+− / Ctrl+0 on a note change that note's size only (`NoteView::zoom`, 6–48 px), like a terminal's zoom. The zoom is not saved.
- **Find.** Edit → Find (Ctrl+Shift+F) with a note in front searches the note's text.
  - The search ignores case and goes round the ends (`core::find::next_in`).
  - The match is selected and the view scrolls to it. Enter or ↓ goes to the next match, ↑ to the previous one.
  - Typing extends the match in place.
  - «Not found: …» in the status bar.
- **Replace.** The same bar, with a note in front, adds «Replace with», «Replace» and «Replace all».
  - «Replace» replaces the selected match (`core::find::is_match`) and selects the next one.
  - «Replace all» rewrites the text in one undo step (`core::find::replace_all`) and reports «Replaced: N».
  - A locked note is not changed.

## How
- **Keys.** A note's editor takes every key, so Find, the zoom keys and F11 are in `app::over_fields`: they act even when a widget took them.
- **Font family** (`editor::set_family`). Every line's attrs get the new family. The syntax colours are then rebuilt by switching to the other COSMIC theme and back: a theme change re-highlights from the lines' own attrs.
- **Not done:** regex search, a case-sensitive switch, a font that is not monospace. Add them if they are asked for.
