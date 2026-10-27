//! The Script Builder tab (spec 36): a palette of every `crt` member and statement, the tape of
//! blocks with their fields, and the code, edited either way. The logic is `core::builder`.

use crate::app::Message;
use crate::fl;
use crate::term::TabId;
use cosmic::iced::{Alignment, Length};
use cosmic::{Element, theme, widget};
use cosmic_text::ViEditor;
use shagshell_core::builder::catalog::{self, CATALOG, Entry, Ty};
use shagshell_core::builder::field::{self, Bad, F};
use shagshell_core::builder::tape::{self, Doc, Pos};
use shagshell_core::builder::{self, Block};
use shagshell_core::debugger::{Debugger, Event, Mode};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const FONT: f32 = 13.0;

/// The palette's «Frequent» members.
/// The palette's «Frequent»: the big blocks first (`stmt:` = a statement).
const FREQUENT: [&str; 15] = [
    "blocks.run",
    "stmt:for_host",
    "blocks.read_list",
    "blocks.log_to",
    "blocks.append_file",
    "blocks.to_clipboard",
    "Screen.Send",
    "Screen.WaitForString",
    "Screen.ReadString",
    "Session.Connect",
    "Session.Disconnect",
    "Dialog.MessageBox",
    "Dialog.Prompt",
    "Sleep",
    "Clipboard.Text",
];
const GROUPS: [&str; 10] = [
    "blocks",
    "Screen",
    "Session",
    "Dialog",
    "Tab",
    "Window",
    "Clipboard",
    "Arguments",
    "FileTransfer",
    "crt",
];
/// The palette's «Other»: statements (`field::new_statement`).
const STATEMENTS: [&str; 11] = [
    "for_host", "assign", "if", "for", "while", "with", "try", "def", "call", "comment", "raw",
];
/// The line the big blocks need (`res/python/shag_blocks.py`).
const IMPORT: &str = "from shag_blocks import *";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    Entry(&'static Entry),
    Stmt(&'static str),
}

impl Item {
    fn block(self) -> Option<Block> {
        match self {
            Item::Entry(e) => Some(field::new_block(e)),
            Item::Stmt(k) => field::new_statement(k),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Drag {
    New(Item),
    Move(Pos),
}

#[derive(Clone, Debug)]
pub enum BuilderMsg {
    Search(String),
    Fold(&'static str),
    Add(Item),
    Select(Pos),
    Field(Pos, F, String),
    AddBranch(Pos),
    AddElse(Pos),
    AddHandler(Pos),
    AddFinally(Pos),
    Delete,
    Copy,
    Paste,
    Undo,
    Redo,
    Up,
    Down,
    HelpMore,
    ToggleCode,
    CodeChanged,
    CopyCode,
    /// A parse came back: its number, whether it was of the edited code (else a check of the
    /// blocks' code).
    Parsed(u64, bool, Result<Vec<Block>, builder::Error>),
    New,
    Open,
    Save,
    SaveAsAsk,
    SaveAs(Option<PathBuf>),
    TargetsOpen,
    Target(Option<TabId>),
    /// «From a template ▾»: the list shown / one picked; «Save as template».
    TemplatesOpen,
    Template(PathBuf),
    SaveTemplate,
    Run,
    Press(Drag),
    Over(Option<Pos>),
    Release,
    /// A breakpoint on / off at a block.
    Break(Pos),
    /// The run started (its id), its debugger spoke, it ended (its error).
    Started(u64),
    Debug(Event),
    Ended(u64, Option<String>),
    DebugPause,
    DebugStep,
    DebugContinue,
    DebugStop,
    /// ⚙: the run's switches.
    Settings,
    SetTrace(bool),
    SetStops(bool),
}

/// What the tab asks of the app.
pub enum Out {
    None,
    Parse(u64, bool, String),
    Open,
    New,
    SaveAs,
    Clipboard(String),
    /// A template / example opened as a new unsaved tape.
    Template(PathBuf),
    /// Save As, starting in the templates folder.
    SaveTemplate,
    /// The script file, the terminal tab (`None`: the first one; `Some(None)`: none), the
    /// debugger (traced run) when its blocks are to light up / stop.
    Run(PathBuf, Option<Option<TabId>>, Option<Arc<Debugger>>),
    /// Stop that run.
    Stop(u64),
}

/// A run from the tab (or the last one: its marks stay until the next edit).
struct Run {
    id: Option<u64>,
    /// Every block's first line in the code that runs.
    map: Vec<(Pos, usize)>,
    dbg: Option<Arc<Debugger>>,
    seen: HashSet<Pos>,
    current: Option<Pos>,
    paused: Option<(Pos, Vec<(String, String)>)>,
    error: Option<(Pos, String)>,
    done: bool,
}

impl Run {
    /// The block of a line of the code that runs.
    fn pos(&self, line: u64) -> Option<Pos> {
        let line = usize::try_from(line).ok()?;
        (self.map.iter().find(|(_, l)| *l == line).map(|(p, _)| p))
            .or_else(|| builder::at_line(&self.map, line))
            .cloned()
    }

    fn live(&self) -> bool {
        !self.done
    }
}

pub struct BuilderView {
    pub path: Option<PathBuf>,
    doc: Doc,
    /// As opened / saved: unsaved = the tape differs.
    saved: Vec<Block>,
    selected: Option<Pos>,
    /// A palette item clicked: its help (until a block is selected).
    picked: Option<Item>,
    search: String,
    open: HashSet<&'static str>,
    help_more: bool,
    /// Fields typed with what is not a value yet (letters in a number): the text and why.
    bad: HashMap<(Pos, F), (String, Bad)>,
    /// What each field was typed as, shown as typed until another edit (`\` before its `r`).
    entered: HashMap<(Pos, F), String>,
    /// The block a syntax error of the code is in, and the words.
    broken: Option<(Pos, String)>,
    code_open: bool,
    editor: Mutex<ViEditor<'static, 'static>>,
    /// The code pane's syntax error (its blocks are kept).
    code_error: Option<String>,
    /// A parse is due at: and whether of the edited code.
    due: Option<(Instant, bool)>,
    seq: u64,
    clip: Option<Block>,
    pub drag: Option<Drag>,
    over: Option<Pos>,
    targets_open: bool,
    /// «From a template ▾» open: its files (templates, then examples).
    templates: Option<Vec<PathBuf>>,
    /// Where Run runs: `None` = the first terminal tab, `Some(None)` = no tab.
    target: Option<Option<TabId>>,
    error: Option<String>,
    /// Closed with «close without saving».
    discarded: bool,
    run: Option<Run>,
    breaks: HashSet<Pos>,
    /// ⚙: light the blocks up while running; stop at breakpoints / steps.
    trace: bool,
    stops: bool,
    settings_open: bool,
}

impl Drop for BuilderView {
    /// The app ends (window closed) with work not saved: it is kept in `scripts/unsaved/`.
    fn drop(&mut self) {
        if self.discarded || !self.unsaved() {
            return;
        }
        let dir = shagshell_core::script::default_dir().join("unsaved");
        let stem = (self.path.as_ref().and_then(|p| p.file_stem())).map_or_else(
            || "script".to_string(),
            |s| s.to_string_lossy().into_owned(),
        );
        let secs = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH))
            .map_or(0, |d| d.as_secs());
        let p = dir.join(format!("{stem}-{secs}.py"));
        if let Err(e) = std::fs::create_dir_all(&dir)
            .and_then(|()| std::fs::write(&p, builder::generate(&self.doc.blocks)))
        {
            log::warn!("{}: {e}", p.display());
        }
    }
}

fn tr(help: [&'static str; 2]) -> &'static str {
    help[usize::from(crate::i18n::russian())]
}

/// What a file holds as blocks (read and parsed).
fn load(path: &Path) -> Result<Vec<Block>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    builder::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
}

impl BuilderView {
    /// A new tape, or the blocks of `path`.
    pub fn new(path: Option<PathBuf>) -> Result<Self, String> {
        let blocks = match &path {
            Some(p) => load(p)?,
            None => vec![],
        };
        let editor = crate::editor::from_text(&builder::generate(&blocks), "py", FONT)
            .map_err(|e| e.to_string())?;
        Ok(Self {
            path,
            saved: blocks.clone(),
            doc: Doc::new(blocks),
            selected: None,
            picked: None,
            search: String::new(),
            open: HashSet::new(),
            help_more: false,
            bad: HashMap::new(),
            entered: HashMap::new(),
            broken: None,
            code_open: false,
            editor,
            code_error: None,
            due: None,
            seq: 0,
            clip: None,
            drag: None,
            over: None,
            targets_open: false,
            templates: None,
            target: None,
            error: None,
            discarded: false,
            run: None,
            breaks: HashSet::new(),
            trace: true,
            stops: true,
            settings_open: false,
        })
    }

    pub fn name(&self) -> String {
        match &self.path {
            Some(p) => {
                (p.file_name()).map_or_else(String::new, |n| n.to_string_lossy().into_owned())
            }
            None => fl!("builder-untitled"),
        }
    }

    /// A template's blocks as a new script: no file, nothing saved yet.
    pub fn untitled(mut self) -> Self {
        self.path = None;
        self.saved = vec![];
        self
    }

    /// Closed on purpose («close without saving»): nothing is kept.
    pub fn discard(&mut self) {
        self.discarded = true;
    }

    pub fn unsaved(&self) -> bool {
        self.doc.blocks != self.saved || self.path.is_none() && !self.doc.blocks.is_empty()
    }

    fn ready(&self) -> bool {
        self.bad.is_empty()
            && self.code_error.is_none()
            && self.broken.is_none()
            && self.due.is_none()
    }

    /// Fields show their values again (the tape changed under them).
    fn forget_typing(&mut self) {
        self.bad.clear();
        self.entered.clear();
        // ponytail: places shift with the tape, so breakpoints go; keep them by block identity
        // if that annoys
        self.breaks.clear();
    }

    /// The run (going, or its marks) is about this tape: an edit drops the marks of an ended one.
    pub fn run_id(&self) -> Option<u64> {
        self.run.as_ref().and_then(|r| r.id)
    }

    fn running(&self) -> bool {
        self.run.as_ref().is_some_and(Run::live)
    }

    /// The tape changed: the code follows, and is checked a moment later. A parse already
    /// running is of older code: its answer is dropped.
    fn changed(&mut self) {
        self.seq += 1;
        if self.run.as_ref().is_some_and(|r| r.done) {
            self.run = None;
        }
        crate::editor::replace_text(&self.editor, &builder::generate(&self.doc.blocks));
        crate::editor::scroll_home(&self.editor);
        (self.editor.lock().unwrap_or_else(|e| e.into_inner())).set_changed(false);
        self.code_error = None;
        self.due = Some((Instant::now() + Duration::from_millis(600), false));
    }

    fn edit(&mut self, f: impl FnOnce(&mut Vec<Block>) -> bool) -> bool {
        // Code typed and not parsed yet would be written over: it becomes the blocks first, or
        // (broken) the tape waits until it is fixed.
        if self.due.is_some_and(|(_, code)| code) || self.code_error.is_some() {
            match builder::parse(&crate::editor::text(&self.editor)) {
                Ok(blocks) => {
                    if blocks != self.doc.blocks {
                        self.doc.edit(|t| {
                            *t = blocks;
                            true
                        });
                    }
                    self.due = None;
                    self.code_error = None;
                }
                Err(builder::Error::Syntax { line, msg }) => {
                    self.code_error = Some(fl!("builder-line", line = line, msg = msg));
                    self.code_open = true;
                    return false;
                }
                Err(builder::Error::Python(e)) => {
                    self.error = Some(e);
                    return false;
                }
            }
        }
        let done = self.doc.edit(f);
        if done {
            self.changed();
        }
        done
    }

    /// A parse that is due now.
    pub fn tick(&mut self) -> Option<Out> {
        let (at, from_code) = self.due?;
        if Instant::now() < at {
            return None;
        }
        self.due = None;
        self.seq += 1;
        let code = match from_code {
            true => {
                // The editor tells of a change only when this flag rises: lowered now, a key
                // typed while this parse runs asks for the next one.
                (self.editor.lock().unwrap_or_else(|e| e.into_inner())).set_changed(false);
                crate::editor::text(&self.editor)
            }
            false => builder::generate(&self.doc.blocks),
        };
        Some(Out::Parse(self.seq, from_code, code))
    }

    pub fn due(&self) -> bool {
        self.due.is_some()
    }

    /// Where a new block goes: after the selected one, or at the end.
    fn after_selected(&self) -> Pos {
        match &self.selected {
            Some(p) if tape::get(&self.doc.blocks, p).is_some() => {
                let mut p = p.clone();
                if let Some(last) = p.last_mut() {
                    last.1 += 1;
                }
                p
            }
            _ => vec![(0, self.doc.blocks.len())],
        }
    }

    /// `fresh`: a new block from the palette (adapted to where it goes: the host's tab inside
    /// «for each host»); a pasted one goes in as it is.
    fn insert_selected(&mut self, mut at: Pos, mut b: Block, fresh: bool) {
        // A big block needs `from shag_blocks import *` at the top: put there with it (one undo).
        let big = match &b {
            Block::Call { func, .. } => {
                catalog::lookup(func).is_some_and(|e| e.group() == "blocks")
            }
            Block::ForHost { .. } => true,
            _ => false,
        };
        let import = big
            && !(self.doc.blocks.iter())
                .any(|b| matches!(b, Block::Raw { code } if code.trim() == IMPORT));
        if let Block::ForHost { func, .. } = &mut b {
            *func = builder::fresh_host_func(&self.doc.blocks);
        }
        let in_host = (1..at.len()).any(|n| {
            matches!(
                tape::get(&self.doc.blocks, &at[..n]),
                Some(Block::ForHost { .. })
            )
        });
        if fresh
            && in_host
            && let Block::Call { func, args, .. } = &mut b
        {
            // `crt.Screen.Send` would type into the script's tab: the host's is `tab`.
            for obj in ["Screen", "Session", "FileTransfer"] {
                if let Some(rest) = func.strip_prefix(&format!("crt.{obj}.")) {
                    *func = format!("tab.{obj}.{rest}");
                }
            }
            if catalog::lookup(func)
                .is_some_and(|e| e.params.first().is_some_and(|p| p.name == "tab"))
            {
                match args.first_mut() {
                    Some(a) if a.name.is_none() => a.value = "tab".into(),
                    _ => args.insert(
                        0,
                        shagshell_core::builder::Arg {
                            name: None,
                            value: "tab".into(),
                        },
                    ),
                }
            }
        }
        // After the leading comments (`# $language = "python3"`) and `from __future__`.
        let top = (self.doc.blocks.iter())
            .take_while(|b| match b {
                Block::Comment { .. } | Block::Blank => true,
                Block::Raw { code } => code.starts_with("from __future__"),
                _ => false,
            })
            .count();
        let nested = at.len() > 1;
        if import
            && let Some(first) = at.first_mut()
            && (nested || first.1 >= top)
        {
            first.1 += 1;
        }
        if self.edit(|t| {
            if import {
                t.insert(
                    top,
                    Block::Raw {
                        code: IMPORT.into(),
                    },
                );
            }
            tape::insert(t, &at, b)
        }) {
            self.selected = Some(at);
            self.picked = None;
            self.forget_typing();
        }
    }

    /// Blocks in the order written (↑ / ↓).
    fn order(&self) -> Vec<Pos> {
        builder::generate_map(&self.doc.blocks)
            .1
            .into_iter()
            .map(|(p, _)| p)
            .collect()
    }

    fn field(&mut self, pos: Pos, f: F, text: String) {
        let Some(b) = tape::get(&self.doc.blocks, &pos) else {
            return;
        };
        let value = match (b, f) {
            (Block::Call { func, .. }, F::Arg(i)) => {
                let Some(p) = catalog::lookup(func).and_then(|e| e.params.get(i)) else {
                    return;
                };
                match (text.trim().is_empty() && p.default.is_some(), p.ty) {
                    (true, _) => Ok(String::new()),
                    (false, ty) => field::typed(ty, &text),
                }
            }
            (
                _,
                F::Assign | F::Text | F::Args | F::Params | F::HandlerTy(_) | F::HandlerName(_),
            ) => Ok(text.clone()),
            (_, F::Limit) => field::typed(Ty::Number, &text),
            (_, F::Parallel) => Ok(text.clone()),
            _ if text.trim().is_empty() => Err(Bad::Required),
            _ => Ok(text.clone()),
        };
        let key = (pos.clone(), f);
        self.entered.insert(key.clone(), text.clone());
        match value {
            Ok(v) => {
                self.bad.remove(&key);
                // ponytail: one undo step per keystroke, group by field if it annoys
                self.edit(|t| tape::get_mut(t, &pos).is_some_and(|b| field::set(b, f, &v)));
            }
            Err(e) => {
                self.bad.insert(key, (text, e));
            }
        }
    }

    pub fn update(&mut self, msg: BuilderMsg) -> Out {
        match msg {
            BuilderMsg::Search(s) => self.search = s,
            BuilderMsg::Fold(g) => {
                if !self.open.remove(g) {
                    self.open.insert(g);
                }
            }
            BuilderMsg::Add(item) => {
                if let Some(b) = item.block() {
                    // A compound block selected with an empty body: into it.
                    let into = self.selected.as_ref().and_then(|p| {
                        let b = tape::get(&self.doc.blocks, p)?;
                        let first = tape::bodies(b).into_iter().next()?;
                        first.is_empty().then(|| [p.as_slice(), &[(0, 0)]].concat())
                    });
                    let at = into.unwrap_or_else(|| self.after_selected());
                    self.insert_selected(at, b, true);
                }
            }
            BuilderMsg::Select(p) => {
                self.selected = Some(p);
                self.picked = None;
            }
            BuilderMsg::Field(p, f, t) => self.field(p, f, t),
            BuilderMsg::AddBranch(p) => {
                // Bodies after it shift: typed-field state keyed by place is stale.
                self.forget_typing();
                self.edit(|t| match tape::get_mut(t, &p) {
                    Some(Block::If { branches, .. }) => {
                        branches.push(("True".into(), vec![]));
                        true
                    }
                    _ => false,
                });
            }
            BuilderMsg::AddElse(p) => {
                self.edit(|t| match tape::get_mut(t, &p) {
                    Some(Block::If { orelse, .. }) if orelse.is_empty() => {
                        orelse.push(Block::Raw {
                            code: "pass".into(),
                        });
                        true
                    }
                    _ => false,
                });
            }
            BuilderMsg::AddHandler(p) => {
                self.forget_typing();
                self.edit(|t| match tape::get_mut(t, &p) {
                    Some(Block::Try { handlers, .. }) => {
                        handlers.push(shagshell_core::builder::Handler {
                            ty: Some("Exception".into()),
                            name: Some("e".into()),
                            body: vec![],
                        });
                        true
                    }
                    _ => false,
                });
            }
            BuilderMsg::AddFinally(p) => {
                self.edit(|t| match tape::get_mut(t, &p) {
                    Some(Block::Try { finally, .. }) if finally.is_none() => {
                        *finally = Some(vec![]);
                        true
                    }
                    _ => false,
                });
            }
            BuilderMsg::Delete => {
                if let Some(p) = self.selected.clone()
                    && self.edit(|t| tape::remove(t, &p).is_some())
                {
                    self.forget_typing();
                    // The next one, or the one before, or none.
                    let mut prev = p.clone();
                    self.selected = match tape::get(&self.doc.blocks, &p) {
                        Some(_) => Some(p),
                        None if prev.last().is_some_and(|l| l.1 > 0) => {
                            if let Some(l) = prev.last_mut() {
                                l.1 -= 1;
                            }
                            Some(prev)
                        }
                        None => None,
                    };
                }
            }
            BuilderMsg::Copy => {
                if let Some(b) = self
                    .selected
                    .as_ref()
                    .and_then(|p| tape::get(&self.doc.blocks, p))
                {
                    self.clip = Some(b.clone());
                    return Out::Clipboard(builder::generate(std::slice::from_ref(b)));
                }
            }
            BuilderMsg::Paste => {
                if let Some(b) = self.clip.clone() {
                    let at = self.after_selected();
                    self.insert_selected(at, b, false);
                }
            }
            BuilderMsg::Undo => {
                if self.doc.undo() {
                    self.forget_typing();
                    self.changed();
                }
            }
            BuilderMsg::Redo => {
                if self.doc.redo() {
                    self.forget_typing();
                    self.changed();
                }
            }
            BuilderMsg::Up | BuilderMsg::Down => {
                let order = self.order();
                let i = self
                    .selected
                    .as_ref()
                    .and_then(|s| order.iter().position(|p| p == s));
                let next = match (msg, i) {
                    (BuilderMsg::Up, Some(i)) => i.checked_sub(1),
                    (_, Some(i)) => Some(i + 1),
                    (BuilderMsg::Up, None) => order.len().checked_sub(1),
                    (_, None) => Some(0),
                };
                if let Some(p) = next.and_then(|i| order.get(i)) {
                    self.selected = Some(p.clone());
                    self.picked = None;
                }
            }
            BuilderMsg::HelpMore => self.help_more = !self.help_more,
            BuilderMsg::ToggleCode => {
                self.code_open = !self.code_open;
                crate::editor::scroll_home(&self.editor);
            }
            BuilderMsg::CodeChanged => {
                let changed = (self.editor.lock().unwrap_or_else(|e| e.into_inner())).changed();
                if changed {
                    self.due = Some((Instant::now() + Duration::from_secs(1), true));
                }
            }
            BuilderMsg::CopyCode => return Out::Clipboard(crate::editor::text(&self.editor)),
            BuilderMsg::Parsed(seq, _, _) if seq != self.seq => {}
            BuilderMsg::Parsed(_, from_code, r) => match r {
                Ok(blocks) => {
                    self.code_error = None;
                    self.broken = None;
                    if from_code && blocks != self.doc.blocks {
                        self.doc.edit(|t| {
                            *t = blocks;
                            true
                        });
                        self.forget_typing();
                        self.selected = None;
                    }
                }
                Err(builder::Error::Syntax { line, msg }) => {
                    let words = fl!("builder-line", line = line, msg = msg);
                    match from_code {
                        true => self.code_error = Some(words),
                        false => {
                            let map = builder::generate_map(&self.doc.blocks).1;
                            self.broken = builder::at_line(&map, line).map(|p| (p.clone(), words));
                        }
                    }
                }
                Err(builder::Error::Python(e)) => self.error = Some(e),
            },
            BuilderMsg::New => return Out::New,
            BuilderMsg::Open => return Out::Open,
            BuilderMsg::Save if !self.ready() => {}
            BuilderMsg::Save => match self.path.clone() {
                Some(p) => self.write(&p),
                None => return Out::SaveAs,
            },
            BuilderMsg::SaveAsAsk if self.ready() => return Out::SaveAs,
            BuilderMsg::SaveAsAsk | BuilderMsg::SaveAs(None) => {}
            BuilderMsg::SaveAs(Some(mut p)) => {
                if p.extension().is_none() {
                    p.set_extension("py");
                }
                self.write(&p);
                if self.error.is_none() {
                    self.path = Some(p);
                }
            }
            BuilderMsg::TargetsOpen => self.targets_open = !self.targets_open,
            BuilderMsg::TemplatesOpen => {
                self.templates = match self.templates {
                    Some(_) => None,
                    None => Some(templates()),
                };
            }
            BuilderMsg::Template(p) => {
                self.templates = None;
                return Out::Template(p);
            }
            BuilderMsg::SaveTemplate if self.ready() => return Out::SaveTemplate,
            BuilderMsg::SaveTemplate => {}
            BuilderMsg::Target(t) => {
                self.target = Some(t);
                self.targets_open = false;
            }
            BuilderMsg::Run if !self.ready() || self.running() => {}
            BuilderMsg::Run => {
                let path = match (&self.path, self.unsaved()) {
                    (Some(p), false) => p.clone(),
                    _ => {
                        let dir = std::env::var_os("XDG_RUNTIME_DIR")
                            .map_or_else(std::env::temp_dir, PathBuf::from)
                            .join("shagshell");
                        let p = dir.join(format!("builder-{}.py", std::process::id()));
                        let code = builder::generate(&self.doc.blocks);
                        if let Err(e) =
                            std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&p, code))
                        {
                            self.error = Some(format!("{}: {e}", p.display()));
                            return Out::None;
                        }
                        p
                    }
                };
                let map = builder::generate_map(&self.doc.blocks).1;
                let dbg = (self.trace || self.stops).then(|| {
                    let lines = (self.stops)
                        .then(|| {
                            map.iter()
                                .filter(|(p, _)| self.breaks.contains(p))
                                .map(|(_, l)| *l as u64)
                        })
                        .into_iter()
                        .flatten();
                    Arc::new(Debugger::new(lines))
                });
                self.run = dbg.as_ref().map(|d| Run {
                    id: None,
                    map,
                    dbg: Some(d.clone()),
                    seen: HashSet::new(),
                    current: None,
                    paused: None,
                    error: None,
                    done: false,
                });
                return Out::Run(path, self.target, dbg);
            }
            BuilderMsg::Break(p) => {
                if !self.breaks.remove(&p) {
                    self.breaks.insert(p);
                }
                // A run going on hears of it (its lines are those of the code it runs).
                if let Some(r) = self.run.as_ref().filter(|r| r.live())
                    && let Some(d) = &r.dbg
                {
                    let lines = (r.map.iter())
                        .filter(|(p, _)| self.breaks.contains(p))
                        .map(|(_, l)| *l as u64);
                    d.set_breaks(lines);
                }
            }
            BuilderMsg::Started(n) => {
                if let Some(r) = &mut self.run {
                    r.id = Some(n);
                }
            }
            BuilderMsg::Debug(e) => {
                if let Some(r) = &mut self.run {
                    match e {
                        Event::Progress { current, seen } => {
                            for l in seen {
                                if let Some(p) = r.pos(l) {
                                    r.seen.insert(p);
                                }
                            }
                            r.current = r.pos(current);
                        }
                        Event::Paused { line, vars } => {
                            if let Some(p) = r.pos(line) {
                                r.current = Some(p.clone());
                                r.paused = Some((p, vars));
                            }
                        }
                    }
                }
            }
            BuilderMsg::Ended(n, failed) => {
                // `None`: it never started.
                if let Some(r) = &mut self.run
                    && (r.id.is_none() || r.id == Some(n))
                {
                    r.done = true;
                    r.paused = None;
                    r.dbg = None;
                    r.error = failed.and_then(|e| Some((r.current.clone()?, e)));
                    if r.error.is_none() {
                        r.current = None;
                    }
                }
            }
            BuilderMsg::DebugPause | BuilderMsg::DebugStep | BuilderMsg::DebugContinue => {
                if let Some(r) = &mut self.run
                    && let Some(d) = &r.dbg
                {
                    let mode = match msg {
                        BuilderMsg::DebugPause => Mode::Pause,
                        BuilderMsg::DebugStep => Mode::Step,
                        _ => Mode::Run,
                    };
                    if mode != Mode::Pause {
                        r.paused = None;
                    }
                    d.resume(mode);
                }
            }
            BuilderMsg::DebugStop => {
                if let Some(r) = &self.run
                    && let Some(n) = r.id
                {
                    return Out::Stop(n);
                }
            }
            BuilderMsg::Settings => self.settings_open = !self.settings_open,
            BuilderMsg::SetTrace(v) => self.trace = v,
            BuilderMsg::SetStops(v) => self.stops = v,
            BuilderMsg::Press(d) => {
                match &d {
                    // The selection stays: a double click adds after (or into) it.
                    Drag::New(item) => self.picked = Some(*item),
                    Drag::Move(p) => self.selected = Some(p.clone()),
                }
                self.drag = Some(d);
                self.over = None;
            }
            BuilderMsg::Over(p) => {
                if self.drag.is_some() {
                    self.over = p;
                }
            }
            BuilderMsg::Release => {
                let (drag, over) = (self.drag.take(), self.over.take());
                match (drag, over) {
                    (Some(Drag::New(item)), Some(at)) => {
                        if let Some(b) = item.block() {
                            self.insert_selected(at, b, true);
                        }
                    }
                    (Some(Drag::Move(from)), Some(to)) => {
                        let mut moved = None;
                        if self.edit(|t| {
                            moved = tape::move_to(t, &from, &to);
                            moved.is_some()
                        }) {
                            self.selected = moved;
                            self.forget_typing();
                        }
                    }
                    _ => {}
                }
            }
        }
        Out::None
    }

    /// Written beside, then moved over (as notes).
    fn write(&mut self, p: &Path) {
        let code = builder::generate(&self.doc.blocks);
        let name = (p.file_name()).map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let tmp = p.with_file_name(format!(".{name}.tmp"));
        match std::fs::write(&tmp, code).and_then(|()| std::fs::rename(&tmp, p)) {
            Ok(()) => {
                self.saved = self.doc.blocks.clone();
                self.error = None;
            }
            Err(e) => {
                let _ = std::fs::remove_file(&tmp);
                self.error = Some(format!("{}: {e}", p.display()));
            }
        }
    }

    pub fn view<'a>(&'a self, id: TabId, targets: &[(TabId, String)]) -> Element<'a, Message> {
        let m = move |x| Message::Builder(id, x);
        let palette = widget::container(
            widget::column::with_children(vec![
                widget::scrollable(self.palette(m))
                    .height(Length::Fill)
                    .into(),
                widget::divider::horizontal::default().into(),
                self.help(m),
            ])
            .spacing(6),
        )
        .width(Length::Fixed(270.0))
        .height(Length::Fill);
        let tape = widget::scrollable(
            widget::container(self.list(id, &self.doc.blocks, &[], 0, m)).padding([4, 12, 4, 4]),
        )
        .height(Length::Fill)
        .width(Length::Fill);
        let mut col = widget::column::with_capacity(5)
            .spacing(6)
            .padding(8)
            .height(Length::Fill)
            .push(self.bar(m, targets));
        if let Some(e) = &self.error {
            col = col.push(widget::text::body(e.clone()).class(theme::Text::Accent));
        }
        col = col.push(
            widget::row::with_children(vec![palette.into(), tape.into()])
                .spacing(8)
                .height(Length::FillPortion(3)),
        );
        col.push(self.code(m, id)).into()
    }

    fn bar<'a>(
        &'a self,
        m: impl Fn(BuilderMsg) -> Message + Copy + 'a,
        targets: &[(TabId, String)],
    ) -> Element<'a, Message> {
        use crate::toolbar::tip_button;
        let ready = self.ready();
        let target = match self.target {
            Some(t) => t.filter(|t| targets.iter().any(|(id, _)| id == t)),
            None => targets.first().map(|(id, _)| *id),
        };
        let label = match target.and_then(|t| targets.iter().find(|(id, _)| *id == t)) {
            Some((_, name)) => name.clone(),
            None => fl!("builder-no-tab"),
        };
        let pick = widget::button::standard(fl!("builder-on", tab = label))
            .on_press(m(BuilderMsg::TargetsOpen));
        let mut pop = widget::popover(pick)
            .position(widget::popover::Position::Bottom)
            .on_close(m(BuilderMsg::TargetsOpen));
        if self.targets_open {
            let mut rows: Vec<Element<'a, Message>> = vec![
                widget::button::text(fl!("builder-no-tab"))
                    .on_press(m(BuilderMsg::Target(None)))
                    .into(),
            ];
            rows.extend(targets.iter().map(|(t, name)| {
                widget::button::text(name.clone())
                    .on_press(m(BuilderMsg::Target(Some(*t))))
                    .into()
            }));
            pop = pop.popup(
                widget::container(widget::column::with_children(rows).spacing(2))
                    .padding(8)
                    .class(theme::Container::Dropdown),
            );
        }
        widget::row::with_children(vec![
            widget::text::heading(self.name())
                .width(Length::Fill)
                .into(),
            tip_button(
                "document-new-symbolic",
                fl!("builder-new"),
                Some(m(BuilderMsg::New)),
            ),
            tip_button(
                "document-open-symbolic",
                fl!("builder-open"),
                Some(m(BuilderMsg::Open)),
            ),
            self.templates_view(m),
            tip_button(
                "bookmark-new-symbolic",
                fl!("builder-save-template"),
                self.ready().then(|| m(BuilderMsg::SaveTemplate)),
            ),
            tip_button(
                "document-save-symbolic",
                fl!("note-save"),
                ready.then(|| m(BuilderMsg::Save)),
            ),
            tip_button(
                "document-save-as-symbolic",
                fl!("note-save-as"),
                ready.then(|| m(BuilderMsg::SaveAsAsk)),
            ),
            tip_button(
                "edit-undo-symbolic",
                fl!("note-undo"),
                Some(m(BuilderMsg::Undo)),
            ),
            tip_button(
                "edit-redo-symbolic",
                fl!("note-redo"),
                Some(m(BuilderMsg::Redo)),
            ),
            pop.into(),
            self.run_controls(m, ready),
        ])
        .spacing(6)
        .align_y(Alignment::Center)
        .into()
    }

    /// Run (⚙ its switches), or while it runs: Pause / Continue, Step, Stop.
    fn run_controls<'a>(
        &self,
        m: impl Fn(BuilderMsg) -> Message + Copy + 'a,
        ready: bool,
    ) -> Element<'a, Message> {
        use crate::toolbar::tip_button;
        if let Some(r) = self.run.as_ref().filter(|r| r.live()) {
            let paused = r.paused.is_some();
            let traced = r.dbg.is_some();
            let mut row = vec![];
            if traced {
                row.push(match paused {
                    true => tip_button(
                        "media-playback-start-symbolic",
                        fl!("builder-continue"),
                        Some(m(BuilderMsg::DebugContinue)),
                    ),
                    false => tip_button(
                        "media-playback-pause-symbolic",
                        fl!("builder-pause"),
                        Some(m(BuilderMsg::DebugPause)),
                    ),
                });
                row.push(tip_button(
                    "go-next-symbolic",
                    fl!("builder-step"),
                    Some(m(BuilderMsg::DebugStep)),
                ));
            }
            row.push(
                widget::button::destructive(fl!("builder-stop"))
                    .leading_icon(widget::icon::from_name("media-playback-stop-symbolic"))
                    .on_press_maybe(r.id.map(|_| m(BuilderMsg::DebugStop)))
                    .into(),
            );
            return widget::row::with_children(row)
                .spacing(6)
                .align_y(Alignment::Center)
                .into();
        }
        let gear = tip_button(
            "emblem-system-symbolic",
            fl!("builder-settings"),
            Some(m(BuilderMsg::Settings)),
        );
        let mut pop = widget::popover(gear)
            .position(widget::popover::Position::Bottom)
            .on_close(m(BuilderMsg::Settings));
        if self.settings_open {
            pop = pop.popup(
                widget::container(
                    widget::column::with_children(vec![
                        widget::checkbox(self.trace)
                            .label(fl!("builder-trace"))
                            .on_toggle(move |v| m(BuilderMsg::SetTrace(v)))
                            .into(),
                        widget::checkbox(self.stops)
                            .label(fl!("builder-stops"))
                            .on_toggle(move |v| m(BuilderMsg::SetStops(v)))
                            .into(),
                    ])
                    .spacing(8),
                )
                .padding(12)
                .class(theme::Container::Dropdown),
            );
        }
        widget::row::with_children(vec![
            pop.into(),
            widget::button::suggested(fl!("builder-run"))
                .leading_icon(widget::icon::from_name("media-playback-start-symbolic"))
                .on_press_maybe(ready.then(|| m(BuilderMsg::Run)))
                .into(),
        ])
        .spacing(6)
        .align_y(Alignment::Center)
        .into()
    }

    /// «From a template ▾»: a button and, open, the list of templates and examples.
    fn templates_view<'a>(
        &self,
        m: impl Fn(BuilderMsg) -> Message + Copy + 'a,
    ) -> Element<'a, Message> {
        let b = widget::button::standard(fl!("builder-from-template"))
            .on_press(m(BuilderMsg::TemplatesOpen));
        let mut pop = widget::popover(b)
            .position(widget::popover::Position::Bottom)
            .on_close(m(BuilderMsg::TemplatesOpen));
        if let Some(files) = &self.templates {
            let rows: Vec<Element<'a, Message>> = match files.is_empty() {
                true => vec![widget::text::caption(fl!("builder-no-templates")).into()],
                false => (files.iter())
                    .map(|p| {
                        let name = (p.file_stem())
                            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                        let dir = (p.parent().and_then(|d| d.file_name()))
                            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                        widget::button::text(format!("{dir} · {name}"))
                            .on_press(m(BuilderMsg::Template(p.clone())))
                            .into()
                    })
                    .collect(),
            };
            pop = pop.popup(
                widget::container(widget::scrollable(
                    widget::column::with_children(rows).spacing(2),
                ))
                .max_height(420)
                .padding(8)
                .class(theme::Container::Dropdown),
            );
        }
        pop.into()
    }

    fn item<'a>(
        &self,
        item: Item,
        m: impl Fn(BuilderMsg) -> Message + Copy + 'a,
    ) -> Element<'a, Message> {
        let (label, help) = match item {
            Item::Entry(e) => (entry_label(e), tr(e.help).to_string()),
            Item::Stmt(k) => (stmt_name(k), stmt_help(k)),
        };
        let on = self.picked == Some(item);
        let cell = widget::container(widget::text::body(label))
            .padding([3, 8])
            .width(Length::Fill)
            .class(theme::Container::custom(move |t| {
                let c = t.cosmic();
                let mut s = widget::container::Style::default();
                s.border.radius = c.corner_radii.radius_xs.into();
                if on {
                    let mut bg: cosmic::iced::Color = c.accent_color().into();
                    bg.a = 0.25;
                    s.background = Some(bg.into());
                }
                s
            }));
        widget::tooltip(
            widget::mouse_area(cell)
                .on_press(m(BuilderMsg::Press(Drag::New(item))))
                .on_double_click(m(BuilderMsg::Add(item)))
                .interaction(cosmic::iced::mouse::Interaction::Grab),
            widget::container(widget::text::caption(help)).max_width(320),
            widget::tooltip::Position::Right,
        )
        .into()
    }

    fn palette<'a>(&self, m: impl Fn(BuilderMsg) -> Message + Copy + 'a) -> Element<'a, Message> {
        let q = self.search.to_lowercase();
        let hit = |label: &str, help: &str| {
            q.is_empty() || label.to_lowercase().contains(&q) || help.to_lowercase().contains(&q)
        };
        let mut col = widget::column::with_capacity(40).spacing(2).push(
            widget::search_input(fl!("builder-search"), self.search.clone())
                .id(search_id())
                .on_focus(Message::FocusField(search_id()))
                .on_input(move |s| m(BuilderMsg::Search(s)))
                .on_clear(m(BuilderMsg::Search(String::new()))),
        );
        let entry_hit = |e: &Entry| hit(e.path, e.help[0]) || hit(e.path, e.help[1]);
        let section =
            |key: &'static str, title: String, items: Vec<Item>| -> Vec<Element<'a, Message>> {
                if items.is_empty() {
                    return vec![];
                }
                // Searching: everything found is shown; else a group shows when unfolded.
                let open = !q.is_empty() || key == "frequent" || self.open.contains(key);
                let arrow = if open { "▾" } else { "▸" };
                let mut out: Vec<Element<'a, Message>> = vec![
                    widget::button::text(format!("{arrow} {title}"))
                        .on_press(m(BuilderMsg::Fold(key)))
                        .width(Length::Fill)
                        .into(),
                ];
                if open {
                    out.extend(items.into_iter().map(|i| self.item(i, m)));
                }
                out
            };
        let frequent: Vec<Item> = match q.is_empty() {
            true => (FREQUENT.iter())
                .filter_map(|p| match p.strip_prefix("stmt:") {
                    Some(k) => STATEMENTS.iter().find(|s| *s == &k).map(|k| Item::Stmt(k)),
                    None => CATALOG.iter().find(|e| e.path == *p).map(Item::Entry),
                })
                .collect(),
            false => vec![],
        };
        col = col.extend(section("frequent", fl!("builder-frequent"), frequent));
        for g in GROUPS {
            let items: Vec<Item> = CATALOG
                .iter()
                .filter(|e| e.group() == g && entry_hit(e))
                .map(Item::Entry)
                .collect();
            let title = match g {
                "blocks" => fl!("builder-big"),
                g => g.to_string(),
            };
            col = col.extend(section(g, title, items));
        }
        let stmts: Vec<Item> = STATEMENTS
            .iter()
            .filter(|k| hit(&stmt_name(k), &stmt_help(k)))
            .map(|k| Item::Stmt(k))
            .collect();
        col = col.extend(section("other", fl!("builder-other"), stmts));
        col.into()
    }

    /// The help panel: the clicked palette item, or the selected block.
    fn help<'a>(&self, m: impl Fn(BuilderMsg) -> Message + Copy + 'a) -> Element<'a, Message> {
        if let Some((_, vars)) = self.run.as_ref().and_then(|r| r.paused.as_ref()) {
            let mut col = widget::column::with_capacity(vars.len() + 1)
                .spacing(2)
                .push(widget::text::heading(fl!("builder-variables")));
            if vars.is_empty() {
                col = col.push(widget::text::caption(fl!("builder-no-variables")));
            }
            for (k, v) in vars {
                col = col.push(widget::text::monotext(format!("{k} = {v}")));
            }
            return widget::scrollable(col).height(Length::Shrink).into();
        }
        let item = self.picked.or_else(|| {
            let b = tape::get(&self.doc.blocks, self.selected.as_ref()?)?;
            Some(match b {
                Block::Call { func, .. } => match catalog::lookup(func) {
                    Some(e) => Item::Entry(e),
                    None => Item::Stmt("call"),
                },
                b => Item::Stmt(stmt_kind(b)),
            })
        });
        let Some(item) = item else {
            return widget::text::caption(fl!("builder-help-none")).into();
        };
        let mut col = widget::column::with_capacity(6).spacing(4);
        match item {
            Item::Entry(e) => {
                col = col
                    .push(widget::text::heading(
                        match e.path.strip_prefix("blocks.") {
                            Some(f) => format!("{} · {f}()", entry_label(e)),
                            None => format!("crt.{}", e.path),
                        },
                    ))
                    .push(widget::text::caption(tr(e.help)));
                if self.help_more {
                    for p in e.params {
                        let d = p
                            .default
                            .map(|d| fl!("builder-default", value = d))
                            .unwrap_or_default();
                        col = col.push(widget::text::caption(format!(
                            "· {}: {}{d}",
                            p.name,
                            ty_name(p.ty)
                        )));
                    }
                    if !e.returns.is_empty() {
                        col = col.push(widget::text::caption(fl!(
                            "builder-returns",
                            ty = e.returns
                        )));
                    }
                    col = col.push(widget::text::monotext(e.example));
                }
                col = col.push(
                    widget::button::link(match self.help_more {
                        true => fl!("builder-less"),
                        false => fl!("builder-more"),
                    })
                    .on_press(m(BuilderMsg::HelpMore)),
                );
            }
            Item::Stmt(k) => {
                col = col
                    .push(widget::text::heading(stmt_name(k)))
                    .push(widget::text::caption(stmt_help(k)));
            }
        }
        col.into()
    }

    /// A drop place while something is dragged (else nothing).
    fn slot<'a>(
        &self,
        at: Pos,
        m: impl Fn(BuilderMsg) -> Message + Copy + 'a,
    ) -> Option<Element<'a, Message>> {
        self.drag.as_ref()?;
        let on = self.over.as_ref() == Some(&at);
        Some(
            widget::mouse_area(
                widget::container(
                    widget::Space::new()
                        .width(Length::Fill)
                        .height(Length::Fixed(if on { 6.0 } else { 10.0 })),
                )
                .width(Length::Fill)
                .class(theme::Container::custom(move |t| {
                    let mut s = widget::container::Style::default();
                    if on {
                        s.background =
                            Some(cosmic::iced::Color::from(t.cosmic().accent_color()).into());
                    }
                    s
                })),
            )
            .on_enter(m(BuilderMsg::Over(Some(at))))
            .into(),
        )
    }

    fn list<'a>(
        &'a self,
        id: TabId,
        blocks: &'a [Block],
        parent: &[(usize, usize)],
        body: usize,
        m: impl Fn(BuilderMsg) -> Message + Copy + 'a,
    ) -> Element<'a, Message> {
        let at = |i: usize| [parent, &[(body, i)]].concat();
        let mut col = widget::column::with_capacity(blocks.len() * 2 + 1).spacing(2);
        for (i, b) in blocks.iter().enumerate() {
            col = col.push_maybe(self.slot(at(i), m));
            col = col.push(self.block(id, b, at(i), m));
        }
        col = col.push_maybe(self.slot(at(blocks.len()), m));
        if blocks.is_empty() {
            // The words of an empty body are a drop place too (the biggest one there).
            let words = widget::text::caption(match parent.is_empty() {
                true => fl!("builder-empty"),
                false => fl!("builder-empty-body"),
            });
            col = col.push(widget::mouse_area(words).on_enter(m(BuilderMsg::Over(Some(at(0))))));
        }
        col.into()
    }

    fn input<'a>(
        &self,
        id: TabId,
        pos: &Pos,
        f: F,
        ty: Ty,
        value: Option<String>,
        hint: String,
    ) -> Element<'a, Message> {
        let key = (pos.clone(), f);
        let text = match self.entered.get(&key) {
            Some(t) => t.clone(),
            None => value.map(|v| field::shown(ty, &v)).unwrap_or_default(),
        };
        let fid = field_id(id, pos, f);
        let p = pos.clone();
        let width =
            (text.chars().count().max(hint.chars().count()) as f32 * 8.0 + 24.0).clamp(60.0, 420.0);
        widget::text_input(hint, text)
            .id(fid.clone())
            .on_focus(Message::FocusField(fid))
            .on_input(move |t| Message::Builder(id, BuilderMsg::Field(p.clone(), f, t)))
            .width(Length::Fixed(width))
            .into()
    }

    /// The field of a catalog parameter: a check box, a list of names, or text.
    fn param<'a>(
        &self,
        id: TabId,
        pos: &Pos,
        i: usize,
        p: &'static catalog::Param,
        value: Option<String>,
        m: impl Fn(BuilderMsg) -> Message + Copy + 'a,
    ) -> Element<'a, Message> {
        let f = F::Arg(i);
        let shown = value.as_deref().map(|v| field::shown(p.ty, v));
        let plain = shown.as_deref().is_none_or(|s| !s.starts_with('='))
            && !self.bad.contains_key(&(pos.clone(), f));
        let pos2 = pos.clone();
        let control: Element<'a, Message> = match p.ty {
            Ty::Bool if plain => {
                let on = value.as_deref().or(p.default) == Some("True");
                widget::checkbox(on)
                    .on_toggle(move |v| {
                        m(BuilderMsg::Field(
                            pos2.clone(),
                            f,
                            if v { "True" } else { "False" }.into(),
                        ))
                    })
                    .into()
            }
            Ty::Choice(names) if plain => {
                let v = value.as_deref().or(p.default).unwrap_or("0");
                let at = names
                    .iter()
                    .position(|n| *n == v)
                    .or_else(|| v.parse::<usize>().ok().filter(|i| *i < names.len()));
                widget::dropdown(names, at, move |k| {
                    Message::Builder(id, BuilderMsg::Field(pos2.clone(), f, names[k].to_string()))
                })
                .into()
            }
            ty => self.input(id, pos, f, ty, value, p.default.unwrap_or("").to_string()),
        };
        widget::row::with_children(vec![
            widget::text::caption(p.name.trim_start_matches('*')).into(),
            control,
        ])
        .spacing(4)
        .align_y(Alignment::Center)
        .into()
    }

    fn block<'a>(
        &'a self,
        id: TabId,
        b: &'a Block,
        pos: Pos,
        m: impl Fn(BuilderMsg) -> Message + Copy + 'a,
    ) -> Element<'a, Message> {
        let selected = self.selected.as_ref() == Some(&pos);
        let raw = |f: F| field::get(b, f);
        let mut head: Vec<Element<'a, Message>> = vec![];
        if self.stops {
            let on = self.breaks.contains(&pos);
            let dot = widget::text::body(if on { "●" } else { "○" }).class(match on {
                true => theme::Text::Color(cosmic::iced::Color::from_rgb(0.85, 0.2, 0.2)),
                false => theme::Text::Default,
            });
            head.push(
                widget::tooltip(
                    widget::mouse_area(dot).on_press(m(BuilderMsg::Break(pos.clone()))),
                    widget::text::caption(fl!("builder-break")),
                    widget::tooltip::Position::Top,
                )
                .into(),
            );
        }
        head.push(
            widget::mouse_area(widget::text::body("⠿"))
                .on_press(m(BuilderMsg::Press(Drag::Move(pos.clone()))))
                .interaction(cosmic::iced::mouse::Interaction::Grab)
                .into(),
        );
        let label = |t: String| -> Element<'a, Message> {
            widget::text::body(t).font(cosmic::font::bold()).into()
        };
        let word = |t: String| -> Element<'a, Message> { widget::text::caption(t).into() };
        let inp = |f: F, hint: String| self.input(id, &pos, f, Ty::Expr, raw(f), hint);
        let mut bodies: Vec<Sub<'a>> = vec![];
        let mut extra: Vec<Element<'a, Message>> = vec![];
        match b {
            Block::Call { func, .. } => match catalog::lookup(func) {
                Some(e) => {
                    head.push(label(entry_label(e)));
                    for (i, p) in e.params.iter().enumerate() {
                        let value = raw(F::Arg(i));
                        if p.more && value.is_none() && !selected {
                            continue;
                        }
                        head.push(self.param(id, &pos, i, p, value, m));
                    }
                    if !e.returns.is_empty() {
                        head.push(word(fl!("builder-save-to")));
                        head.push(inp(F::Assign, fl!("builder-variable")));
                    }
                }
                None => {
                    head.push(label(fl!("stmt-call")));
                    head.push(inp(F::Func, fl!("builder-function")));
                    head.push(word("(".into()));
                    head.push(inp(F::Args, String::new()));
                    head.push(word(")".into()));
                    head.push(word(fl!("builder-save-to")));
                    head.push(inp(F::Assign, fl!("builder-variable")));
                }
            },
            Block::Assign { .. } => {
                head.push(inp(F::Target, fl!("builder-variable")));
                head.push(word("=".into()));
                head.push(inp(F::Value, String::new()));
            }
            Block::If { branches, orelse } => {
                head.push(label(fl!("builder-if")));
                head.push(inp(F::Cond(0), String::new()));
                for (i, (_, body)) in branches.iter().enumerate() {
                    let title = (i > 0).then(|| -> Element<'a, Message> {
                        widget::row::with_children(vec![
                            label(fl!("builder-elif")),
                            inp(F::Cond(i), String::new()),
                        ])
                        .spacing(6)
                        .align_y(Alignment::Center)
                        .into()
                    });
                    bodies.push((title, body, i));
                }
                if !orelse.is_empty() {
                    bodies.push((Some(label(fl!("builder-else"))), orelse, branches.len()));
                }
                if selected {
                    extra.push(
                        widget::button::text(fl!("builder-add-elif"))
                            .on_press(m(BuilderMsg::AddBranch(pos.clone())))
                            .into(),
                    );
                    if orelse.is_empty() {
                        extra.push(
                            widget::button::text(fl!("builder-add-else"))
                                .on_press(m(BuilderMsg::AddElse(pos.clone())))
                                .into(),
                        );
                    }
                }
            }
            Block::For { body, .. } => {
                head.push(label(fl!("builder-for")));
                head.push(inp(F::Target, fl!("builder-variable")));
                head.push(word(fl!("builder-in")));
                head.push(inp(F::Iter, String::new()));
                bodies.push((None, body, 0));
            }
            Block::While { body, .. } => {
                head.push(label(fl!("builder-while")));
                head.push(inp(F::Cond(0), String::new()));
                bodies.push((None, body, 0));
            }
            Block::With { body, .. } => {
                head.push(label(fl!("builder-with")));
                head.push(inp(F::Items, String::new()));
                bodies.push((None, body, 0));
            }
            Block::Try {
                body,
                handlers,
                finally,
            } => {
                head.push(label(fl!("builder-try")));
                bodies.push((None, body, 0));
                for (i, h) in handlers.iter().enumerate() {
                    let title = widget::row::with_children(vec![
                        label(fl!("builder-except")),
                        inp(F::HandlerTy(i), "Exception".into()),
                        word(fl!("builder-as")),
                        inp(F::HandlerName(i), "e".into()),
                    ])
                    .spacing(6)
                    .align_y(Alignment::Center)
                    .into();
                    bodies.push((Some(title), &h.body, i + 1));
                }
                if let Some(f) = finally {
                    bodies.push((Some(label(fl!("builder-finally"))), f, handlers.len() + 1));
                }
                if selected {
                    extra.push(
                        widget::button::text(fl!("builder-add-except"))
                            .on_press(m(BuilderMsg::AddHandler(pos.clone())))
                            .into(),
                    );
                    if finally.is_none() {
                        extra.push(
                            widget::button::text(fl!("builder-add-finally"))
                                .on_press(m(BuilderMsg::AddFinally(pos.clone())))
                                .into(),
                        );
                    }
                }
            }
            Block::Def { body, .. } => {
                head.push(label(fl!("builder-def")));
                head.push(inp(F::Name, fl!("builder-function")));
                head.push(word("(".into()));
                head.push(inp(F::Params, String::new()));
                head.push(word(")".into()));
                bodies.push((None, body, 0));
            }
            Block::ForHost { parallel, body, .. } => {
                head.push(label(fl!("builder-for-host")));
                head.push(inp(F::Hosts, fl!("builder-hosts")));
                let p2 = pos.clone();
                head.push(
                    widget::checkbox(*parallel)
                        .label(fl!("builder-parallel"))
                        .on_toggle(move |v| {
                            Message::Builder(
                                id,
                                BuilderMsg::Field(
                                    p2.clone(),
                                    F::Parallel,
                                    if v { "True" } else { "False" }.into(),
                                ),
                            )
                        })
                        .into(),
                );
                if *parallel {
                    head.push(word(fl!("builder-limit")));
                    head.push(self.input(
                        id,
                        &pos,
                        F::Limit,
                        Ty::Number,
                        raw(F::Limit),
                        "10".into(),
                    ));
                }
                bodies.push((Some(word(fl!("builder-host-hint"))), body, 0));
            }
            Block::Comment { .. } => {
                head.push(word("#".into()));
                head.push(inp(F::Text, fl!("stmt-comment")));
            }
            Block::Blank => head.push(word(String::new())),
            Block::Raw { code } if code.contains('\n') => {
                head.push(label(fl!("stmt-raw")));
                extra.push(widget::text::monotext(code.clone()).into());
                extra.push(widget::text::caption(fl!("builder-raw-multi")).into());
            }
            Block::Raw { .. } => {
                head.push(label(fl!("stmt-raw")));
                head.push(inp(F::Text, String::new()));
            }
        }
        let mut col = widget::column::with_capacity(4).spacing(4).push(
            widget::flex_row(head)
                .spacing(6)
                .align_items(Alignment::Center),
        );
        for (title, list, n) in bodies {
            col = col.push_maybe(title);
            col = col.push(
                widget::container(self.list(id, list, &pos, n, m))
                    .padding([0, 0, 0, 18])
                    .class(theme::Container::custom(|t| {
                        let mut s = widget::container::Style::default();
                        s.border.color = t.cosmic().palette.neutral_5.into();
                        s.border.width = 0.0;
                        s
                    })),
            );
        }
        if !extra.is_empty() {
            col = col.push(widget::row::with_children(extra).spacing(6));
        }
        let broken = self
            .broken
            .as_ref()
            .filter(|(p, _)| *p == pos)
            .map(|(_, w)| w.clone());
        let bad = self
            .bad
            .iter()
            .find(|((p, _), _)| *p == pos)
            .map(|(_, (_, e))| match e {
                Bad::Required => fl!("builder-required"),
                Bad::NotNumber => fl!("builder-not-number"),
            });
        let run = self.run.as_ref();
        let run_error = run
            .and_then(|r| r.error.as_ref())
            .filter(|(p, _)| *p == pos)
            .map(|(_, e)| e.clone());
        let error = broken.or(bad).or(run_error);
        // Light-up of a run: where it is (paused: there), what it went through.
        let current = run.is_some_and(|r| r.current.as_ref() == Some(&pos) && r.live());
        let paused_here = run.is_some_and(|r| r.paused.as_ref().is_some_and(|(p, _)| *p == pos));
        let seen = run.is_some_and(|r| r.seen.contains(&pos));
        // A paused run's value of what this block keeps.
        let var = match b {
            Block::Call {
                assign: Some(t), ..
            }
            | Block::Assign { target: t, .. } => Some(t),
            _ => None,
        };
        if let Some((_, vars)) = run.and_then(|r| r.paused.as_ref())
            && let Some(v) = var.and_then(|t| vars.iter().find(|(k, _)| k == t))
        {
            col = col.push(widget::text::monotext(format!("{} = {}", v.0, v.1)));
        }
        let red = error.is_some();
        if let Some(e) = error {
            col = col.push(widget::text::caption(e).class(theme::Text::Accent));
        }
        let kind = stmt_kind(b);
        let select = m(BuilderMsg::Select(pos.clone()));
        // The whole block selects it (a nested block's click is its own: it takes it first).
        widget::mouse_area(
            widget::container(col)
                .padding([4, 8])
                .width(Length::Fill)
                .class(theme::Container::custom(move |t| {
                    let c = t.cosmic();
                    let mut s = widget::container::Style::default();
                    s.border.radius = c.corner_radii.radius_s.into();
                    s.border.width = if selected || red || current || paused_here {
                        2.0
                    } else {
                        1.0
                    };
                    s.border.color = match (red, paused_here, current, selected) {
                        (true, ..) => c.destructive_color().into(),
                        (_, true, ..) => c.warning_color().into(),
                        (_, _, true, _) | (.., true) => c.accent_color().into(),
                        _ => c.palette.neutral_4.into(),
                    };
                    let mut bg: cosmic::iced::Color = match (seen, kind) {
                        (true, _) => c.success_color().into(),
                        (_, "call") => c.accent_color().into(),
                        (_, "comment" | "blank") => c.palette.neutral_3.into(),
                        _ => c.success_color().into(),
                    };
                    bg.a = if seen { 0.22 } else { 0.07 };
                    s.background = Some(bg.into());
                    s
                })),
        )
        .on_press(select)
        .into()
    }

    fn code<'a>(
        &'a self,
        m: impl Fn(BuilderMsg) -> Message + Copy + 'a,
        id: TabId,
    ) -> Element<'a, Message> {
        let arrow = if self.code_open { "▾" } else { "▸" };
        let mut head = vec![
            widget::button::text(format!("{arrow} {}", fl!("builder-code")))
                .on_press(m(BuilderMsg::ToggleCode))
                .into(),
        ];
        if self.code_open {
            head.push(
                widget::button::standard(fl!("builder-copy-all"))
                    .on_press(m(BuilderMsg::CopyCode))
                    .into(),
            );
        }
        if let Some(e) = &self.code_error {
            head.push(
                widget::text::body(e.clone())
                    .class(theme::Text::Accent)
                    .into(),
            );
        }
        let mut col = widget::column::with_capacity(2).spacing(4).push(
            widget::row::with_children(head)
                .spacing(8)
                .align_y(Alignment::Center),
        );
        if self.code_open {
            crate::editor::follow_theme(&self.editor);
            // ponytail: lines wrap, so there is never a sideways scroll to keep; reset each draw
            // (the editor scrolls itself while it has no width yet). Keep it if wrap goes off.
            crate::editor::scroll_home(&self.editor);
            col = col.push(
                widget::container(
                    crate::editor::text_box::text_box(&self.editor, crate::editor::metrics(FONT))
                        // Not the tab's id: showing the tab must not give the code the keys.
                        .id(widget::Id::new(format!("builder-code-{id}")))
                        .on_changed(m(BuilderMsg::CodeChanged))
                        .line_numbers(true)
                        .padding(6),
                )
                .height(Length::Fixed(240.0)),
            );
        }
        col.into()
    }
}

/// A compound block's body: its title (`elif x:`, `except E as e:`), blocks, body number.
type Sub<'a> = (Option<Element<'a, Message>>, &'a [Block], usize);

/// The files «From a template ▾» lists: `scripts/templates/*.py`, then `scripts/examples/*.py`.
fn templates() -> Vec<PathBuf> {
    let dir = shagshell_core::script::default_dir();
    ["templates", "examples"]
        .iter()
        .flat_map(|d| shagshell_core::script::list_dir(&dir.join(d)))
        .collect()
}

fn search_id() -> widget::Id {
    widget::Id::new("builder-search")
}

fn field_id(id: TabId, pos: &Pos, f: F) -> widget::Id {
    widget::Id::new(format!("builder-{id}-{pos:?}-{f:?}"))
}

/// A catalog entry as the palette and the tape name it: a big block by its words, a `crt`
/// member by its path.
fn entry_label(e: &Entry) -> String {
    match e.path {
        "blocks.prompt" => fl!("block-prompt"),
        "blocks.run" => fl!("block-run"),
        "blocks.read_list" => fl!("block-read-list"),
        "blocks.log_to" => fl!("block-log-to"),
        "blocks.append_file" => fl!("block-append-file"),
        "blocks.to_clipboard" => fl!("block-to-clipboard"),
        "blocks.each_host" => fl!("block-each-host"),
        p => p.to_string(),
    }
}

fn stmt_kind(b: &Block) -> &'static str {
    match b {
        Block::Call { .. } => "call",
        Block::Assign { .. } => "assign",
        Block::If { .. } => "if",
        Block::For { .. } => "for",
        Block::While { .. } => "while",
        Block::With { .. } => "with",
        Block::Try { .. } => "try",
        Block::Def { .. } => "def",
        Block::Comment { .. } => "comment",
        Block::Blank => "blank",
        Block::Raw { .. } => "raw",
        Block::ForHost { .. } => "for_host",
    }
}

fn stmt_name(k: &str) -> String {
    match k {
        "assign" => fl!("stmt-assign"),
        "if" => fl!("stmt-if"),
        "for" => fl!("stmt-for"),
        "while" => fl!("stmt-while"),
        "with" => fl!("stmt-with"),
        "try" => fl!("stmt-try"),
        "def" => fl!("stmt-def"),
        "call" => fl!("stmt-call"),
        "for_host" => fl!("stmt-for-host"),
        "comment" => fl!("stmt-comment"),
        "blank" => fl!("stmt-blank"),
        _ => fl!("stmt-raw"),
    }
}

fn stmt_help(k: &str) -> String {
    match k {
        "assign" => fl!("stmt-assign-help"),
        "if" => fl!("stmt-if-help"),
        "for" => fl!("stmt-for-help"),
        "while" => fl!("stmt-while-help"),
        "with" => fl!("stmt-with-help"),
        "try" => fl!("stmt-try-help"),
        "def" => fl!("stmt-def-help"),
        "call" => fl!("stmt-call-help"),
        "for_host" => fl!("stmt-for-host-help"),
        "comment" => fl!("stmt-comment-help"),
        "blank" => fl!("stmt-blank-help"),
        _ => fl!("stmt-raw-help"),
    }
}

fn ty_name(t: Ty) -> String {
    match t {
        Ty::Text => fl!("ty-text"),
        Ty::Number => fl!("ty-number"),
        Ty::Bool => fl!("ty-bool"),
        Ty::Choice(_) => fl!("ty-choice"),
        Ty::Session => fl!("ty-session"),
        Ty::File => fl!("ty-file"),
        Ty::Expr => fl!("ty-expr"),
    }
}

/// The tab's keys when no field has them.
pub fn key(
    k: &cosmic::iced::keyboard::key::Physical,
    m: cosmic::iced::keyboard::Modifiers,
) -> Option<BuilderMsg> {
    use cosmic::iced::keyboard::key::{Code, Physical};
    let Physical::Code(code) = k else {
        return None;
    };
    let ctrl = m.control() && !m.alt();
    Some(match (code, ctrl, m.shift()) {
        (Code::Delete, false, false) => BuilderMsg::Delete,
        (Code::ArrowUp, false, false) => BuilderMsg::Up,
        (Code::ArrowDown, false, false) => BuilderMsg::Down,
        (Code::KeyC, true, false) => BuilderMsg::Copy,
        (Code::KeyV, true, false) => BuilderMsg::Paste,
        (Code::KeyZ, true, false) => BuilderMsg::Undo,
        (Code::KeyZ, true, true) | (Code::KeyY, true, false) => BuilderMsg::Redo,
        (Code::KeyS, true, false) => BuilderMsg::Save,
        _ => return None,
    })
}

/// «Open in builder…»: a `.py`.
pub async fn pick(title: String, dir: PathBuf) -> Option<PathBuf> {
    crate::script::pick(title, dir).await
}
