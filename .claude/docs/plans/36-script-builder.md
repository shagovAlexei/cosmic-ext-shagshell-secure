# 36. Script Builder, part 1 (core) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `shagshell_core::builder`: the block catalog of every `crt` member, `.py` → blocks (parsed by Python's own `ast` + `tokenize`) and blocks → `.py`, stable on every `Example/*.py`.

**Architecture:** Python does the syntax (`res/python/shag_parse.py` prints the block tree as JSON), Rust does the model and the code generation (`builder::Block`, `builder::generate`). The catalog is a static Rust table checked against `crt.py` by introspection. No UI in this part.

**Tech Stack:** Rust (serde / serde_json already in core), python3 ≥ 3.9 (`ast.unparse`; Pop!_OS 24.04 has 3.12).

**Spec:** `.claude/docs/specs/36-script-builder.md` (sections «Разбор и генерация», «Каталог блоков»; parts 2–5 are later plans).

## Global Constraints

- Branch `feat/script-builder`; version `0.86.0` in root `Cargo.toml`; `CHANGELOG.md` entry `## 0.86.0 — <date>`.
- `crates/core` has no libcosmic dependency and no `fl!`: catalog help texts live in the catalog as `[en, ru]`.
- Generated code: 4 spaces per level, expressions exactly as `ast.unparse` writes them.
- Python helpers run as `python3 -I <python_dir()>/shag_parse.py` (`script::python_dir()`), source on stdin.
- Tests that need `python3` return early when `python3 -V` fails (as `script::tests::python_script_runs_against_serve`).
- New Python files start with a module docstring like `crt.py` and are installed by `just install` to `/usr/share/shagshell/python/`.
- `just verify` passes before the PR.

## Review Focus

- SecureCRT files saved on Windows: CRLF line endings and a UTF-8 BOM must parse like plain files (Task 3 test `parse_crlf_and_bom`).
- SecureCRT Python 2 scripts (`print "x"`): a `Syntax` error with the right line, never a panic or an empty tape (Task 3 test `parse_python2_is_syntax_error`).
- Russian text in comments and strings comes back byte-identical (Task 3 test `parse_keeps_unicode`).
- A multi-line string in an assignment (a here-doc command) keeps its value (`ast` equal) after a round trip (Task 4 invariant fixture `multiline.py`).
- `python3` missing: `Error::Python` with a message, not a panic (Task 3 test `parse_without_python`: a dir without the helper is the same code path).

---

### Task 1: The catalog

**Files:**
- Create: `crates/core/src/builder/mod.rs` (only `pub mod catalog;` for now), `crates/core/src/builder/catalog.rs`
- Modify: `crates/core/src/lib.rs` (`pub mod builder;`)

**Interfaces:**
- Produces:
  ```rust
  pub enum Kind { Method, Property }
  pub enum Ty { Text, Number, Bool, Choice(&'static [&'static str]), Session, File, Expr }
  pub struct Param { pub name: &'static str, pub ty: Ty, pub default: Option<&'static str>, pub more: bool }
  pub struct Entry {
      pub path: &'static str,      // "Screen.Send", "Dialog.Prompt", crt level: "Sleep"
      pub kind: Kind,
      pub params: &'static [Param],// names exactly as in crt.py, "*texts" / "**kw" for varargs
      pub returns: &'static str,   // "" = nothing; else a short type word ("bool", "str", "int", "Tab")
      pub help: [&'static str; 2], // [en, ru], one or two sentences
      pub example: &'static str,   // one Python line
  }
  pub static CATALOG: &[Entry];
  pub fn lookup(func: &str) -> Option<&'static Entry>;
  impl Entry { pub fn group(&self) -> &'static str } // "Screen", …, "crt" for crt-level
  ```
  Groups (class in `crt.py` → prefix): `_Crt` → none, `_Screen` → `Screen`, `_Session` → `Session`, `_Tab` → `Tab`, `_Dialog` → `Dialog`, `_Clipboard` → `Clipboard`, `_Arguments` → `Arguments`, `_Window` → `Window`, `_FileTransfer` → `FileTransfer`.

- [ ] **Step 1: Write the failing tests** in `catalog.rs`:
  - `catalog_matches_crt_py`: runs `python3 -I -c <SNIPPET> <python_dir>`; SNIPPET imports `crt` from `sys.argv[1]`, and for each class above prints `{path: null}` for a `property`, `{path: [param names without self, "*"/"**" prefixed for varargs]}` for a function, skipping names starting with `_`. Assert: the JSON object equals the map built from `CATALOG` (`Property` → null, `Method` → param names), so a member missing on either side fails with its path in the message.
  - `paths_unique`, and `every_entry_has_help`: both `help` strings non-empty, `example` non-empty.
  - `lookup_by_receiver`:
    ```rust
    assert_eq!(lookup("crt.Screen.Send").unwrap().path, "Screen.Send");
    assert_eq!(lookup("tab.Screen.Send").unwrap().path, "Screen.Send");
    assert_eq!(lookup("crt.GetTab(2).Screen.Send").unwrap().path, "Screen.Send");
    assert_eq!(lookup("crt.Sleep").unwrap().path, "Sleep");
    assert_eq!(lookup("crt.Window.Activate").unwrap().path, "Window.Activate");
    assert_eq!(lookup("tab.Activate").unwrap().path, "Tab.Activate");
    assert!(lookup("x.Sleep").is_none());
    assert!(lookup("os.path.join").is_none());
    assert!(lookup("foo").is_none());
    ```
- [ ] **Step 2:** `cargo test -p shagshell-core builder::catalog` → FAIL (does not compile).
- [ ] **Step 3: Implement.** `CATALOG` covers every member `crt.py` has (methods and properties of the classes above). `lookup`: `crt.<path>` exact; else the last two dot segments `G.M` with `G` a group other than crt-level and `Tab`; else a receiver that is not `crt` with last segment `M` matching `Tab.M`. Param types: texts `Text`, timeouts / ms / rows `Number`, `bCaseInsensitive` / `bPassword` / `multi` `Bool`, MessageBox `buttons` `Choice(&["BUTTON_OK", "BUTTON_OKCANCEL", "BUTTON_ABORTRETRYIGNORE", "BUTTON_YESNOCANCEL", "BUTTON_YESNO", "BUTTON_RETRYCANCEL"])`, `Session.Connect` `cmd` `Session`, file paths `File`, `items` `Expr`. `more: true` for `bCaseInsensitive` and the dialogs' `filter` / `buttonLabel`.
- [ ] **Step 4:** the same command → PASS.
- [ ] **Step 5: Commit** `feat(core): script builder catalog of crt members`.

### Task 2: Blocks and code generation

**Files:**
- Modify: `crates/core/src/builder/mod.rs`

**Interfaces:**
- Produces (serde `Serialize, Deserialize, Debug, Clone, PartialEq`; `#[serde(tag = "kind", rename_all = "snake_case")]` on `Block`):
  ```rust
  pub struct Arg { pub name: Option<String>, pub value: String }       // value: Python expression text
  pub struct Handler { pub ty: Option<String>, pub name: Option<String>, pub body: Vec<Block> }
  pub enum Block {
      Call { func: String, args: Vec<Arg>, assign: Option<String> },   // [assign =] func(args)
      Assign { target: String, value: String },
      If { branches: Vec<(String, Vec<Block>)>, orelse: Vec<Block> },  // if / elif…, else
      For { target: String, iter: String, body: Vec<Block> },
      While { cond: String, body: Vec<Block> },
      With { items: String, body: Vec<Block> },                         // "open(p) as f, …"
      Try { body: Vec<Block>, handlers: Vec<Handler>, finally: Vec<Block> },
      Def { name: String, params: String, body: Vec<Block> },
      Comment { text: String },                                          // without "# "
      Blank,
      Raw { code: String },                                              // may be several lines, level 0
  }
  pub fn generate(blocks: &[Block]) -> String;  // ends with "\n" unless empty
  ```
- [ ] **Step 1: Write the failing tests** (`builder::tests`, no Python needed):
  - `gen_call`: `Call { func: "crt.Screen.WaitForString", args: [Arg{None,"'#'"}, Arg{None,"10"}, Arg{Some("bCaseInsensitive"),"True"}], assign: Some("ok") }` → `"ok = crt.Screen.WaitForString('#', 10, bCaseInsensitive=True)\n"`.
  - `gen_nested`: `For{c in cmds}` holding `If{[("c", [Call send])], orelse:[Raw "break"]}` → exactly
    ```
    for c in cmds:
        if c:
            crt.Screen.Send(c)
        else:
            break
    ```
  - `gen_elif_try_def_with`: one fixture each, exact text (`elif x:`, `except OSError as e:`, bare `except:`, `finally:`, `def f(a, b=1):`, `with open(p) as f:`).
  - `gen_empty_body_is_pass`: `If{[("x", [])], orelse: []}` → `"if x:\n    pass\n"`; `Def` with empty body likewise.
  - `gen_comment_blank_raw`: `Comment{"a"}` → `"# a"`, `Comment{""}` → `"#"`, `Blank` → empty line, `Raw{"class A:\n    x = 1"}` inside a `Def` → every line indented one level.
- [ ] **Step 2:** `cargo test -p shagshell-core builder::tests` → FAIL.
- [ ] **Step 3: Implement `generate`** (a recursive writer with a level). `Try` with no handlers and no finally writes `finally:\n    pass` (Python needs one). Mark the Raw re-indent with `// ponytail: re-indents every line of Raw, a multi-line string literal inside a nested Raw gets the indent too; keep Raw's own line offsets if that bites`.
- [ ] **Step 4:** → PASS.
- [ ] **Step 5: Commit** `feat(core): script builder blocks and code generation`.

### Task 3: Parsing through Python

**Files:**
- Create: `res/python/shag_parse.py`, `crates/core/src/builder/parse.rs`
- Modify: `crates/core/src/builder/mod.rs` (`mod parse; pub use parse::{parse, Error};`), `justfile` (install `shag_parse.py` next to `crt.py`)

**Interfaces:**
- Consumes: `Block`, `Arg`, `Handler` (Task 2) as the JSON shape; `script::python_dir()`.
- Produces:
  ```rust
  pub enum Error { Syntax { line: usize, msg: String }, Python(String) }  // Debug, PartialEq, Display
  pub fn parse(src: &str) -> Result<Vec<Block>, Error>;
  ```
  Helper protocol: stdin = source; stdout = one JSON line, `{"ok": [<Block>…]}` or `{"err": {"line": N, "msg": "…"}}` for a `SyntaxError`. Rust strips a leading U+FEFF before sending. A non-zero exit / unreadable output → `Error::Python(stderr or the serde error)`.

Mapping rules for `shag_parse.py` (statements → blocks):
- expression statement whose value is a `Call` without `*a` / `**k` → `Call` (`func` = `ast.unparse(call.func)`, args unparsed, keyword `name`); `Assign` with one `Name` / `Attribute` target and a `Call` value (no starred) → `Call` with `assign`; other single-target `Assign` → `Assign`.
- `If` (an `orelse` that is exactly one `If` becomes the next branch) → `If`; `For` / `While` without `else` and not `async` → `For` / `While`; `With` (not async) → `With` (`items` = the items unparsed, joined `", "`); `Try` without `else` (not `TryStar`) → `Try`; `FunctionDef` without decorators, not async → `Def` (`params` = `ast.unparse(node.args)`).
- a body that is exactly one `Pass` → `[]`.
- anything else (incl. `Import`, `Return`, `AugAssign`, `ClassDef`, decorated / async defs, loops with `else`) → `Raw` with `ast.unparse(node)`.
- comments (`tokenize`) and blank lines: for each body, the lines between the previous statement's end (or the parent header's end) and the next statement's start become, in line order, `Comment` blocks and one `Blank` per run of empty lines; lines after a body's last statement belong to the enclosing body's next gap; blank lines before the first top-level statement and after the last are dropped. A comment on a statement's own lines (trailing, or inside a multi-line expression) becomes a `Comment` right before that statement's block. Header end of a compound statement = end line of its test / iter / items / args; the `else:` / `except:` / `finally:` line's comment goes to the start of that body. `Comment.text` = the comment without `#` and one following space.

- [ ] **Step 1: Write the failing tests** in `parse.rs` (skip without python3):
  - `parse_call_and_assign`: `"ok = crt.Screen.WaitForString('#', 10)\ncrt.Screen.Send('ls\\r')\nx = 5\n"` → `[Call{assign: Some("ok"), …}, Call{func: "crt.Screen.Send", args: [Arg{None, "'ls\\r'"}], assign: None}, Assign{"x","5"}]`.
  - `parse_elif_chain`: `if a:\n f()\nelif b:\n g()\nelse:\n h()\n` → one `If` with 2 branches and `orelse` `[Call h]`.
  - `parse_comments_and_blanks`: `"# top\n\n\nx = 1  # tail\n\n# lead\ny = 2\n"` → `[Comment "top", Blank, Comment "tail", Assign x, Blank, Comment "lead", Assign y]`.
  - `parse_pass_body_is_empty`: `"while x:\n    pass\n"` → `[While{cond: "x", body: []}]`.
  - `parse_raw`: `"import os\nx += 1\nclass A:\n    pass\n"` → three `Raw` with `"import os"`, `"x += 1"`, `"class A:\n    pass"`.
  - `parse_python2_is_syntax_error`: `"x = 1\nprint \"hi\"\n"` → `Err(Error::Syntax { line: 2, .. })`.
  - `parse_crlf_and_bom`: `"\u{feff}x = 1\r\n# c\r\ny = 2\r\n"` → `[Assign x, Comment "c", Assign y]`.
  - `parse_keeps_unicode`: `"# привет\ns = 'мир'\n"` → `[Comment "привет", Assign{"s", "'мир'"}]`.
  - `parse_without_python`: `parse_with(src, <empty temp dir>)` → `Err(Error::Python(_))`. (`parse` = `parse_with(src, &python_dir())`; the test calls `parse_with` with the temp dir, no env var.)
- [ ] **Step 2:** `cargo test -p shagshell-core builder::parse` → FAIL.
- [ ] **Step 3: Implement** `shag_parse.py` (rules above) and `parse` / `parse_with(src: &str, dir: &Path)`.
- [ ] **Step 4:** → PASS.
- [ ] **Step 5: Commit** `feat(core): parse .py into script builder blocks`.

### Task 4: Round-trip invariants, docs, release bits

**Files:**
- Create: `crates/core/src/builder/tests/multiline.py` (fixture: an assignment of a triple-quoted two-line string, a nested `def` with a docstring, a `try/except/finally`, a `with open`)
- Modify: `crates/core/src/builder/mod.rs` (tests), `CLAUDE.md` (core list: one `builder/` line), `.claude/docs/ROADMAP.md` (backlog item → «36. Конструктор скриптов (spec 36), часть 1 из 5 готова»), `CHANGELOG.md`, `Cargo.toml` (`0.86.0`)

**Interfaces:**
- Consumes: `parse`, `generate`.

- [ ] **Step 1: Write the failing test** `round_trip_examples` (skip without python3): for every `Example/*.py`, `multiline.py`, and `recorder::Recorder` output of a two-step recording (`output(b"host$ ")`, `input(b"ls\r")`, `script("t")`):
  ```rust
  let b = parse(&src).unwrap();
  let g = generate(&b);
  assert_eq!(parse(&g).unwrap(), b, "{name}: blocks change on the second pass");
  assert_eq!(generate(&parse(&g).unwrap()), g, "{name}: code changes on the second pass");
  assert_eq!(ast_dump(&src), ast_dump(&g), "{name}: meaning changed");
  ```
  `ast_dump(s)` = stdout of `python3 -I -c "import ast,sys; print(ast.dump(ast.parse(sys.stdin.read())))"` with `s` on stdin (BOM stripped). Also assert every comment text of `src` (from `tokenize`, or simply each `#…` line trimmed) appears in `g`.
- [ ] **Step 2:** run it → it fails or passes; every failure is a bug in Task 2 / 3 — fix there with a test in that task's style, not here.
- [ ] **Step 3: Docs and version:** the files listed above; CHANGELOG line: `- feat: script builder core: every crt member as a block (catalog), .py ↔ blocks (parsed by python3's ast), stable on all examples; no UI yet`.
- [ ] **Step 4:** `just verify` → passes; `git diff Cargo.lock` shows only the version.
- [ ] **Step 5: Commit** `feat(core): script builder round trip on the examples (v0.86.0)`.
