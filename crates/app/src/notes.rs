//! A note: the body of a tab that edits a text file in COSMIC Edit's editor (`editor`), with
//! syntax highlighting by the file's extension (markdown, code…) and line numbers.
//! The file is written a moment after the last edit (`tick`) and when the tab goes.

use crate::app::Message;
use crate::fl;
use crate::toolbar::tip_button;
use cosmic::iced::Length;
use cosmic::widget;
use cosmic::{Element, theme};
use cosmic_text::ViEditor;
use shagshell_core::notes;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// COSMIC Edit's default size.
const FONT_SIZE: f32 = 14.0;

/// Edit → Find in a note: the text typed (the match may grow in place), the next, the previous.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Seek {
    Typed,
    Next,
    Prev,
}

#[derive(Clone, Debug)]
pub enum NoteMsg {
    /// The editor's text changed.
    Changed,
    RenameStart,
    RenameEdit(String),
    RenameDo,
    RenameCancel,
    Open,
    /// The tab's menu: write now, write as another file (the path chosen, if any).
    Save,
    SaveAs(Option<PathBuf>),
    Wrap,
    LineNumbers,
    /// The bar's buttons (Copy / Cut / Paste / Lock / Save As go through the tabs: clipboard,
    /// dialogs).
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    PasteText(String),
    SelectAll,
    Lock,
    SaveAsAsk,
    /// The «⋯» list of which buttons the bar shows: open / closed.
    Picker,
}

pub struct NoteView {
    pub path: PathBuf,
    editor: Mutex<ViEditor<'static, 'static>>,
    /// Edited, not written yet: since when.
    dirty: Option<Instant>,
    /// The new name being typed.
    rename: Option<String>,
    error: Option<String>,
    rename_id: widget::Id,
    pub wrap: bool,
    pub numbers: bool,
    /// Lock Session: read only (moving, selecting, copying).
    pub locked: bool,
    /// The «⋯» list is open.
    picker: bool,
    /// Global Options → Notes: the font family applied ("" = the system's monospace) and size.
    family: String,
    size: f32,
    /// Ctrl+= / Ctrl+− on this note: px over `size`.
    zoom: i8,
}

impl Drop for NoteView {
    /// The tab goes (closed, or the app ends): what is not written yet is.
    fn drop(&mut self) {
        self.save();
    }
}

/// The file's text for a note, or the words for why not.
pub fn read(path: &Path) -> Result<String, String> {
    notes::read(path).map_err(|e| match e {
        notes::ReadError::TooBig => fl!(
            "note-too-big",
            mb = i64::try_from(notes::MAX_BYTES >> 20).unwrap_or(5)
        ),
        notes::ReadError::NotText => fl!("note-not-text"),
        notes::ReadError::Io(e) => format!("{}: {e}", path.display()),
    })
}

impl NoteView {
    pub fn open(path: &Path) -> Result<Self, String> {
        // Checked first (size, text): the editor would load anything.
        read(path)?;
        let editor =
            crate::editor::open(path, FONT_SIZE).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Self {
            path: path.to_path_buf(),
            editor,
            dirty: None,
            rename: None,
            error: None,
            rename_id: widget::Id::unique(),
            wrap: true,
            numbers: true,
            locked: false,
            picker: false,
            family: String::new(),
            size: FONT_SIZE,
            zoom: 0,
        })
    }

    /// Global Options → Notes: the font family ("" = the system's monospace) and size.
    pub fn set_font(&mut self, family: &str, size: f32) {
        self.size = size;
        if family != self.family {
            let style = crate::term::style::Style {
                font_family: family.to_string(),
                ..Default::default()
            };
            crate::editor::set_family(&self.editor, style.family());
            self.family = family.to_string();
        }
    }

    /// Ctrl+= / Ctrl+− (`by` px, 6–48 in all), Ctrl+0 (`None`): this note's size.
    pub fn zoom(&mut self, by: Option<i8>) {
        self.zoom = match by {
            None => 0,
            Some(by) => {
                let px = (self.size as i32 + i32::from(self.zoom) + i32::from(by)).clamp(6, 48);
                i8::try_from(px - self.size as i32).unwrap_or(0)
            }
        };
    }

    fn font_px(&self) -> f32 {
        (self.size + f32::from(self.zoom)).clamp(6.0, 48.0)
    }

    /// Edit → Find: the match of `needle` selected; `false` when there is none.
    pub fn find(&mut self, needle: &str, seek: Seek) -> bool {
        let lines = crate::editor::lines(&self.editor);
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        let from = crate::editor::find_from(&self.editor, seek == Seek::Next);
        match shagshell_core::find::next_in(&lines, from, needle, seek != Seek::Prev) {
            Some((l, start, end)) => {
                crate::editor::select(&self.editor, l, start, end);
                true
            }
            None => false,
        }
    }

    /// Replace: the selected match becomes `with`, then the next one is selected.
    pub fn replace(&mut self, needle: &str, with: &str) -> bool {
        if self.locked || needle.is_empty() {
            return false;
        }
        let on_match = crate::editor::selected(&self.editor)
            .is_some_and(|t| shagshell_core::find::is_match(&t, needle));
        if on_match {
            crate::editor::replace_selection(&self.editor, with);
            self.dirty = Some(Instant::now());
        }
        self.find(needle, Seek::Next)
    }

    /// Replace all: how many.
    pub fn replace_all(&mut self, needle: &str, with: &str) -> usize {
        if self.locked {
            return 0;
        }
        let (text, n) =
            shagshell_core::find::replace_all(&crate::editor::text(&self.editor), needle, with);
        if n > 0 {
            crate::editor::replace_text(&self.editor, &text);
            self.dirty = Some(Instant::now());
        }
        n
    }

    pub fn name(&self) -> String {
        (self.path.file_name()).map_or_else(String::new, |n| n.to_string_lossy().into_owned())
    }

    pub fn has_unsaved(&self) -> bool {
        self.dirty.is_some()
    }

    /// Writes the file now (a failure is shown in the tab and tried again at the next edit).
    pub fn save(&mut self) {
        if self.dirty.is_none() {
            return;
        }
        // Written beside, then moved over: a crash leaves the old file, not half of the new.
        let tmp = self.path.with_file_name(format!(".{}.tmp", self.name()));
        match std::fs::write(&tmp, crate::editor::text(&self.editor))
            .and_then(|()| std::fs::rename(&tmp, &self.path))
        {
            Ok(()) => {
                self.dirty = None;
                (self.editor.lock().unwrap_or_else(|e| e.into_inner())).set_changed(false);
            }
            Err(e) => {
                let _ = std::fs::remove_file(&tmp);
                // Tried again in a second.
                self.dirty = Some(Instant::now());
                self.error = Some(format!("{}: {e}", self.path.display()));
            }
        }
    }

    /// The clock: what has been still for a second is written.
    pub fn tick(&mut self) {
        if self
            .dirty
            .is_some_and(|t| t.elapsed() >= Duration::from_secs(1))
        {
            self.save();
        }
    }

    /// `true`: the tab wants the file chooser.
    pub fn update(&mut self, msg: NoteMsg) -> bool {
        match msg {
            NoteMsg::Changed => {
                let changed = (self.editor.lock().unwrap_or_else(|e| e.into_inner())).changed();
                if changed {
                    // Restarted by every edit: written a second after the last one.
                    self.dirty = Some(Instant::now());
                    self.error = None;
                }
            }
            NoteMsg::RenameStart if !self.locked => self.rename = Some(self.name()),
            NoteMsg::RenameStart => {}
            NoteMsg::Save => {
                self.dirty.get_or_insert_with(Instant::now);
                self.save();
            }
            NoteMsg::SaveAs(None) => {}
            NoteMsg::SaveAs(Some(p)) => {
                self.save();
                match std::fs::write(&p, crate::editor::text(&self.editor)) {
                    Ok(()) => self.path = p,
                    Err(e) => self.error = Some(format!("{}: {e}", p.display())),
                }
            }
            NoteMsg::Wrap => {
                self.wrap = !self.wrap;
                crate::editor::set_wrap(&self.editor, self.wrap);
            }
            NoteMsg::LineNumbers => self.numbers = !self.numbers,
            // Edits by a button: no `Changed` from the editor, so written from here.
            NoteMsg::Undo if !self.locked => {
                crate::editor::undo(&self.editor);
                self.dirty = Some(Instant::now());
            }
            NoteMsg::Redo if !self.locked => {
                crate::editor::redo(&self.editor);
                self.dirty = Some(Instant::now());
            }
            NoteMsg::PasteText(t) if !self.locked => {
                crate::editor::paste(&self.editor, &t);
                self.dirty = Some(Instant::now());
            }
            NoteMsg::SelectAll => crate::editor::select_all(&self.editor),
            NoteMsg::Undo
            | NoteMsg::Redo
            | NoteMsg::PasteText(_)
            | NoteMsg::Cut
            | NoteMsg::Copy
            | NoteMsg::Paste
            | NoteMsg::Lock
            | NoteMsg::SaveAsAsk => {}
            NoteMsg::Picker => self.picker = !self.picker,
            NoteMsg::RenameEdit(v) => self.rename = Some(v),
            NoteMsg::RenameCancel => self.rename = None,
            NoteMsg::RenameDo => {
                if let Some(name) = self.rename.take() {
                    self.save();
                    match notes::rename(&self.path, &name) {
                        Ok(p) => self.path = p,
                        Err(e) => {
                            self.error = Some(match e.kind() {
                                std::io::ErrorKind::AlreadyExists => fl!("name-taken"),
                                std::io::ErrorKind::InvalidFilename => fl!("note-bad-name"),
                                _ => e.to_string(),
                            });
                            self.rename = Some(name);
                        }
                    }
                }
            }
            NoteMsg::Open => return true,
        }
        false
    }

    /// A bar button by its name (`core::notes::BUTTONS`); disabled when it would edit a locked
    /// note.
    fn button<'a>(
        &self,
        name: &str,
        m: impl Fn(NoteMsg) -> Message,
    ) -> Option<Element<'a, Message>> {
        let edit = !self.locked;
        let (icon, tip, msg, on) = match name {
            "Save" => (
                "document-save-symbolic",
                fl!("note-save"),
                NoteMsg::Save,
                true,
            ),
            "SaveAs" => (
                "document-save-as-symbolic",
                fl!("note-save-as"),
                NoteMsg::SaveAsAsk,
                true,
            ),
            "Undo" => ("edit-undo-symbolic", fl!("note-undo"), NoteMsg::Undo, edit),
            "Redo" => ("edit-redo-symbolic", fl!("note-redo"), NoteMsg::Redo, edit),
            "Cut" => ("edit-cut-symbolic", fl!("note-cut"), NoteMsg::Cut, edit),
            "Copy" => ("edit-copy-symbolic", fl!("menu-copy"), NoteMsg::Copy, true),
            "Paste" => (
                "edit-paste-symbolic",
                fl!("menu-paste"),
                NoteMsg::Paste,
                edit,
            ),
            "SelectAll" => (
                "edit-select-all-symbolic",
                fl!("menu-select-all"),
                NoteMsg::SelectAll,
                true,
            ),
            "Wrap" => (
                "format-justify-fill-symbolic",
                fl!("note-wrap"),
                NoteMsg::Wrap,
                true,
            ),
            "LineNumbers" => (
                "format-justify-left-symbolic",
                fl!("note-numbers"),
                NoteMsg::LineNumbers,
                true,
            ),
            "Rename" => (
                "edit-symbolic",
                fl!("note-rename"),
                NoteMsg::RenameStart,
                edit,
            ),
            "Open" => (
                "document-open-symbolic",
                fl!("note-open"),
                NoteMsg::Open,
                true,
            ),
            "Lock" => (
                "system-lock-screen-symbolic",
                match self.locked {
                    true => fl!("menu-unlock"),
                    false => fl!("menu-lock"),
                },
                NoteMsg::Lock,
                true,
            ),
            _ => return None,
        };
        Some(tip_button(icon, tip, on.then(|| m(msg))))
    }

    /// «⋯» at the end of the bar: a list that ticks which buttons the bar shows (every note's).
    fn picker_view<'a>(
        &self,
        buttons: &[String],
        m: impl Fn(NoteMsg) -> Message,
    ) -> Element<'a, Message> {
        let more = tip_button(
            "view-more-symbolic",
            fl!("note-buttons"),
            Some(m(NoteMsg::Picker)),
        );
        let mut p = widget::popover(more)
            .position(widget::popover::Position::Bottom)
            .on_close(m(NoteMsg::Picker));
        if self.picker {
            let rows = (notes::BUTTONS.iter().enumerate()).map(|(i, b)| -> Element<'a, Message> {
                let on = buttons.iter().any(|n| n == b);
                widget::checkbox(on)
                    .label(button_label(b))
                    .on_toggle(move |_| Message::NoteButton(u8::try_from(i).unwrap_or(0)))
                    .into()
            });
            p = p.popup(
                widget::container(
                    widget::column::with_children(rows.collect::<Vec<_>>()).spacing(8),
                )
                .padding(12)
                .class(cosmic::theme::Container::Dropdown),
            );
        }
        p.into()
    }

    /// The selection to the clipboard; cut takes it out (not in a locked note).
    pub fn copy(&self) -> Option<String> {
        crate::editor::copy(&self.editor)
    }

    pub fn cut(&mut self) -> Option<String> {
        if self.locked {
            return None;
        }
        let t = crate::editor::cut(&self.editor)?;
        self.dirty = Some(Instant::now());
        Some(t)
    }

    pub fn rename_field(&self) -> widget::Id {
        self.rename_id.clone()
    }

    pub fn view(
        &self,
        id: crate::term::TabId,
        editor_id: widget::Id,
        buttons: &[String],
    ) -> Element<'_, Message> {
        let m = move |x| Message::Note(id, x);
        let mut bar = widget::row::with_capacity(4)
            .spacing(8)
            .align_y(cosmic::iced::Alignment::Center);
        bar = match &self.rename {
            Some(name) => bar
                .push(
                    widget::text_input("", name.clone())
                        .id(self.rename_id.clone())
                        .on_input(move |v| m(NoteMsg::RenameEdit(v)))
                        .on_submit(move |_| m(NoteMsg::RenameDo))
                        .width(Length::Fill),
                )
                .push(
                    widget::button::suggested(fl!("note-rename-ok")).on_press(m(NoteMsg::RenameDo)),
                )
                .push(widget::button::standard(fl!("cancel")).on_press(m(NoteMsg::RenameCancel))),
            None => bar
                .push(widget::text::heading(self.name()).width(Length::Fill))
                .push_maybe(self.locked.then(|| {
                    widget::row::with_children(vec![
                        widget::icon::from_name("system-lock-screen-symbolic")
                            .size(16)
                            .into(),
                        widget::text::caption(fl!("lock-locked")).into(),
                    ])
                    .spacing(4)
                    .align_y(cosmic::iced::Alignment::Center)
                }))
                .extend(buttons.iter().filter_map(|b| self.button(b, m)))
                .push(self.picker_view(buttons, m)),
        };
        crate::editor::follow_theme(&self.editor);
        // The tab's widget id: the app's «focus the tab» then gives it the keys.
        let body: Element<'_, Message> =
            crate::editor::text_box::text_box(&self.editor, crate::editor::metrics(self.font_px()))
                .id(editor_id)
                .on_changed(m(NoteMsg::Changed))
                .highlight_current_line()
                .line_numbers(self.numbers)
                .read_only(self.locked)
                .padding(8)
                .into();
        let mut col = widget::column::with_capacity(3)
            .spacing(8)
            .height(Length::Fill)
            .padding(8)
            .push(bar);
        if let Some(e) = &self.error {
            col = col.push(widget::text::body(e.clone()).class(theme::Text::Accent));
        }
        col.push(body).into()
    }
}

/// A bar button's name (the «⋯» list).
fn button_label(name: &str) -> String {
    match name {
        "Save" => fl!("note-save"),
        "SaveAs" => fl!("note-save-as"),
        "Undo" => fl!("note-undo"),
        "Redo" => fl!("note-redo"),
        "Cut" => fl!("note-cut"),
        "Copy" => fl!("menu-copy"),
        "Paste" => fl!("menu-paste"),
        "SelectAll" => fl!("menu-select-all"),
        "Wrap" => fl!("note-wrap"),
        "LineNumbers" => fl!("note-numbers"),
        "Rename" => fl!("note-rename"),
        "Open" => fl!("note-open"),
        "Lock" => fl!("menu-lock"),
        other => other.to_string(),
    }
}

/// Save As…: where the note's text goes (its folder first).
pub async fn save_as(title: String, dir: PathBuf) -> Option<PathBuf> {
    use ashpd::desktop::file_chooser::SaveFileRequest;
    let mut r = SaveFileRequest::default()
        .title(Some(title.as_str()))
        .modal(true);
    if dir.is_dir() {
        let d: &Path = &dir;
        r = r.current_folder::<&Path>(Some(d)).ok()?;
    }
    let picked = r.send().await.ok()?.response().ok()?;
    picked.uris().first()?.to_file_path().ok()
}

/// The file chooser of «Open note…»: text and markdown first, anything else too.
pub async fn pick(title: String, dir: PathBuf) -> Option<PathBuf> {
    use ashpd::desktop::file_chooser::{FileFilter, OpenFileRequest};
    let mut r = OpenFileRequest::default()
        .title(Some(title.as_str()))
        .modal(true)
        .filters([
            FileFilter::new(&fl!("note-filter-text"))
                .glob("*.md")
                .glob("*.markdown")
                .glob("*.txt"),
            FileFilter::new(&fl!("script-filter-all")).glob("*"),
        ]);
    if dir.is_dir() {
        let d: &Path = &dir;
        r = r.current_folder::<&Path>(Some(d)).ok()?;
    }
    let picked = r.send().await.ok()?.response().ok()?;
    picked.uris().first()?.to_file_path().ok()
}
