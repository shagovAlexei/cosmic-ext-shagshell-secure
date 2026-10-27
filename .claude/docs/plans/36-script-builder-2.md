# 36. Script Builder, part 2 (the tab) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans. Steps use `- [ ]`.

**Goal:** the Script Builder tab: palette (every `crt` member + statements, search, help), tape of blocks with fields, add by double click / drag, move, delete, copy / paste, undo; the code pane (two-way); open / save; run on a chosen terminal tab.

**Architecture:** editing logic in `core::builder::{tape,field}` (pure, tested); `crates/app/src/builder.rs` owns `BuilderView`, `BuilderMsg`, `update`, `view` (like `notes.rs`), shown as `tabs::Body::Builder`. Code ↔ blocks through part 1's `generate` / `parse` (parse off the UI thread).

**Spec:** `.claude/docs/specs/36-script-builder.md` («Как выглядит», «Сохранение»; run with highlighting / debug is part 4, big blocks and templates part 3).

## Global Constraints
- Branch `feat/script-builder-tab`, version `0.87.0`, CHANGELOG entry.
- Every UI string through `fl!`, en + ru. Never bind Ctrl+Insert / Shift+Insert.
- Anti-goal: clutter. Default: code pane folded («Код ▸»), help panel one line + «▸», rare params under «ещё ▸».
- New tab keys only when no widget took them (`Message::Key`, `status == Ignored`).

## Review Focus
- A text field holding a Python expression (`cmd + "\r"`) must stay an expression after an edit elsewhere; a field typed `"=x"` is the expression `x` (Task 2 `typed_*` tests).
- Moving a block into its own child is refused, not a lost block (Task 1 `move_into_self_refused`).
- Code edited with a syntax error never replaces the blocks (Task 6 manual + `parse` error path).
- Closing an unsaved builder tab asks (Task 6 manual).
- A number field with letters shows red and blocks Save / Run (Task 2 `check_*`).

---

### Task 1: `core::builder::tape` — positions and edits
**Files:** create `crates/core/src/builder/tape.rs`; `mod.rs` (`pub mod tape;`).
**Produces:**
```rust
/// (body, index) steps from the top list: [(0, i)] is a top block; (b, j) inside a compound
/// block = its b-th body (`bodies` order), j-th block.
pub type Pos = Vec<(usize, usize)>;
pub fn bodies(b: &Block) -> Vec<&Vec<Block>>;          // If: branches…, orelse; Try: body, handlers…, finally (if Some); For/While/With/Def: body; else []
pub fn bodies_mut(b: &mut Block) -> Vec<&mut Vec<Block>>;
pub fn get<'a>(top: &'a [Block], at: &[(usize, usize)]) -> Option<&'a Block>;
pub fn get_mut<'a>(top: &'a mut Vec<Block>, at: &[(usize, usize)]) -> Option<&'a mut Block>;
pub fn insert(top: &mut Vec<Block>, at: &[(usize, usize)], b: Block) -> bool; // index may equal len
pub fn remove(top: &mut Vec<Block>, at: &[(usize, usize)]) -> Option<Block>;
pub fn move_to(top: &mut Vec<Block>, from: &[(usize, usize)], to: &[(usize, usize)]) -> Option<Pos>; // the block's new place; None = refused (into itself)
pub struct Doc { pub blocks: Vec<Block>, undo: Vec<Vec<Block>>, redo: Vec<Vec<Block>> }
impl Doc { pub fn new(blocks) -> Self; pub fn edit(&mut self, f: impl FnOnce(&mut Vec<Block>) -> bool) -> bool; pub fn undo(&mut self) -> bool; pub fn redo(&mut self) -> bool }
```
Tests: `get_nested`, `insert_end_and_middle`, `remove_shifts`, `move_down_same_list` (from before to: index adjusts), `move_into_other_body`, `move_into_self_refused`, `undo_redo` (edit returning false records nothing; a new edit clears redo). Undo keeps whole copies: `// ponytail: a copy of the tape per edit, diffs if scripts get huge`.

### Task 2: `core::builder::field` — what a field shows and takes
**Files:** create `crates/core/src/builder/field.rs`; `mod.rs`.
**Produces:**
```rust
pub fn literal(text: &str) -> String;            // Python repr: '…' (") when text has ' and no "), \\ \n \r \t \xNN for controls
pub fn unliteral(value: &str) -> Option<String>; // a plain '…' / "…" literal (no prefixes) → its text
pub fn shown(ty: Ty, value: &str) -> String;     // Text/File/Session: unliteral or "=" + value; Number/Bool/Choice: value if it fits, else "=" + value; Expr: value
pub enum Bad { Required, NotNumber }
pub fn typed(ty: Ty, input: &str) -> Result<String, Bad>; // "=x" → x for any ty; Text/File/Session → literal; Number → int/float text; Bool/Choice/Expr → as is; "" → Required (Expr "" ok only when the param has a default — caller passes "" through for optional params)
pub fn new_call(e: &Entry) -> Block;            // func "crt.<path>" (Tab.* → "tab.<M>"), args: required params with an empty literal / 0 / False, defaults left out
pub fn new_statement(kind: &str) -> Option<Block>; // "if" "for" "while" "with" "try" "def" "assign" "comment" "raw": an empty block with placeholder text
```
Tests: `literal_round_trip` (`'a'`, `"it's"`, `'\\r\\n'`, `'тест'`, `'\x01'`), `unliteral_rejects_prefixes_and_expressions`, `typed_text_and_expression`, `typed_number`, `shown_number_expression`, `new_call_send` (`crt.Screen.Send('')`), `new_call_tab_member`.

### Task 3: `core::builder` line map
**Files:** `mod.rs`.
**Produces:** `pub fn generate_map(blocks: &[Block]) -> (String, Vec<(Pos, usize)>)` — every block's first line (1-based); `generate` = `.0`. `pub fn at_line(map: &[(Pos, usize)], line: usize) -> Option<&Pos>` — the deepest block starting at or before `line`.
Tests: `map_nested_lines`, `at_line_picks_deepest`.

### Task 4: the tab (app)
**Files:** create `crates/app/src/builder.rs`; modify `tabs.rs` (`Body::Builder`, `Launch.builder: Option<Option<PathBuf>>`, glyph = Note's script icon, `live()` = unsaved, title with `•`), `app.rs` (`Message::Builder(TabId, BuilderMsg)`, route keys when active), `menu.rs` (`Action::ScriptBuilder` «Конструктор скриптов…», `Action::OpenInBuilder` «Открыть в конструкторе…» in Script; `named` for keys), `i18n/{en,ru}`.
View: bar (New, Open, Save, Save As, «на: [вкладка ▾]», ▶ Run) · palette (search, «Частые», groups of `CATALOG` folded, «Прочее»), each item a `tooltip` with help · tape (blocks: header + fields; compound bodies indented with a left rule; selected block outlined; bad field red with its words) · help panel under the palette (one line + ▸ for params / returns / example) · code pane folded by default.
Messages: `Search`, `Fold(group)`, `Pick(PaletteItem)` (select → help), `Add(PaletteItem)` (double click → after the selected block or at the end), `Select(Pos)`, `Field(Pos, usize, String)`, `Delete`, `Copy`, `Paste`, `Undo`, `Redo`, `ToggleCode`, `CodeChanged`, `Parsed(u64, Result<Vec<Block>, Error>)`, `New`, `Open`, `Opened(Option<PathBuf>)`, `Save`, `SaveAs(Option<PathBuf>)`, `Target(usize)`, `Run`, drag: `Press(DragItem)`, `Over(Pos)`, `Release`.
Keys (no widget took them): Delete, Ctrl+C / Ctrl+V / Ctrl+Z / Ctrl+Shift+Z / Ctrl+Y, ↑ / ↓ move the selection.

### Task 5: drag
Press a palette item or a tape block's grip → held (app subscribes to pointer release while held, as `session_manager`); entering a drop slot (thin `mouse_area` between blocks and at each body's end) sets the target; release inserts / moves (`tape::insert` / `move_to`) and selects the result; release elsewhere cancels.

### Task 6: code pane, files, run
- Code pane: `editor` (new `editor::from_text(text, "py", size)`), refreshed from `generate` after every block edit; `CodeChanged` restarts a 1 s timer (the app's existing 1 s note tick), then `parse` runs in `Task::perform` (blocking thread); a reply whose sequence is stale is dropped; `Ok` → `Doc::edit` replace; `Err(Syntax)` → the error line under the code, Save / Run disabled. «Копировать всё».
- Open: portal file chooser (`.py`), parse; a `Syntax` error opens the file as a note instead with the error shown (`sessions.notice`). Save: atomic write (as notes); Save As chooser. Unsaved = blocks differ from the last saved / opened; the tab is «live» so closing asks.
- Run: the tab list = open terminal tabs; writes the code to `$XDG_RUNTIME_DIR/shagshell/builder-<id>.py` when unsaved (else runs the file), `Scripts::run(path, &[], tab)`.

### Task 7: verify, docs, release
`just verify`; headless: open the tab (key for `ScriptBuilder`), double-click a palette item, shot; TESTING.md rows (Russian, steps); CLAUDE.md (app list: `builder.rs`); ROADMAP 36.2 ticked; version / CHANGELOG.
