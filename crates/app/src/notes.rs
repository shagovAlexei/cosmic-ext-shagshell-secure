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
        })
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

    pub fn rename_field(&self) -> widget::Id {
        self.rename_id.clone()
    }

    pub fn view(&self, id: crate::term::TabId, editor_id: widget::Id) -> Element<'_, Message> {
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
                .push(tip_button(
                    "edit-symbolic",
                    fl!("note-rename"),
                    (!self.locked).then(|| m(NoteMsg::RenameStart)),
                ))
                .push(tip_button(
                    "document-open-symbolic",
                    fl!("note-open"),
                    Some(m(NoteMsg::Open)),
                )),
        };
        crate::editor::follow_theme(&self.editor);
        // The tab's widget id: the app's «focus the tab» then gives it the keys.
        let body: Element<'_, Message> =
            crate::editor::text_box::text_box(&self.editor, crate::editor::metrics(FONT_SIZE))
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
