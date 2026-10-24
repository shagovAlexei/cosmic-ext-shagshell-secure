//! A note: the body of a tab that edits a text file, or shows a `.md` one as formatted text.
//! The file is written a moment after the last edit (`tick`) and when the tab goes.

use crate::app::Message;
use crate::fl;
use cosmic::iced::Length;
use cosmic::widget::text_editor::{self, Action};
use cosmic::widget::{self, markdown};
use cosmic::{Element, theme};
use shagshell_core::notes;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub enum NoteMsg {
    Edit(Action),
    Preview,
    RenameStart,
    RenameEdit(String),
    RenameDo,
    RenameCancel,
    Open,
    Link(String),
}

pub struct NoteView {
    pub path: PathBuf,
    text: text_editor::Content,
    items: Vec<markdown::Item>,
    preview: bool,
    /// Edited, not written yet: since when.
    dirty: Option<Instant>,
    /// The new name being typed.
    rename: Option<String>,
    error: Option<String>,
    rename_id: widget::Id,
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
        let text = read(path)?;
        let mut v = Self {
            path: path.to_path_buf(),
            text: text_editor::Content::with_text(&text),
            items: Vec::new(),
            preview: false,
            dirty: None,
            rename: None,
            error: None,
            rename_id: widget::Id::unique(),
        };
        // A `.md` file is read first, written second.
        if notes::is_markdown(path) {
            v.set_preview(true);
        }
        Ok(v)
    }

    pub fn name(&self) -> String {
        (self.path.file_name()).map_or_else(String::new, |n| n.to_string_lossy().into_owned())
    }

    pub fn has_unsaved(&self) -> bool {
        self.dirty.is_some()
    }

    fn set_preview(&mut self, on: bool) {
        self.preview = on;
        if on {
            self.items = markdown::parse(&self.text.text()).collect();
        }
    }

    /// Writes the file now (a failure is shown in the tab and tried again at the next edit).
    pub fn save(&mut self) {
        if self.dirty.is_none() {
            return;
        }
        match std::fs::write(&self.path, self.text.text()) {
            Ok(()) => self.dirty = None,
            Err(e) => self.error = Some(format!("{}: {e}", self.path.display())),
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
            NoteMsg::Edit(a) => {
                let edit = a.is_edit();
                self.text.perform(a);
                if edit {
                    self.dirty.get_or_insert_with(Instant::now);
                    self.error = None;
                }
            }
            NoteMsg::Preview => self.set_preview(!self.preview),
            NoteMsg::RenameStart => self.rename = Some(self.name()),
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
            NoteMsg::Link(url) => {
                if let Err(e) = std::process::Command::new("xdg-open").arg(&url).spawn() {
                    self.error = Some(format!("xdg-open: {e}"));
                }
            }
        }
        false
    }

    pub fn rename_field(&self) -> widget::Id {
        self.rename_id.clone()
    }

    pub fn view(&self, id: crate::term::TabId, editor_id: widget::Id) -> Element<'_, Message> {
        let m = move |x| Message::Note(id, x);
        let mut bar = widget::row::with_capacity(6)
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
                .push(
                    widget::button::standard(fl!("note-rename")).on_press(m(NoteMsg::RenameStart)),
                )
                .push(widget::button::standard(fl!("note-open")).on_press(m(NoteMsg::Open))),
        };
        if notes::is_markdown(&self.path) {
            let label = match self.preview {
                true => fl!("note-edit"),
                false => fl!("note-view"),
            };
            bar = bar.push(widget::button::standard(label).on_press(m(NoteMsg::Preview)));
        }
        let body: Element<'_, Message> = match self.preview {
            true => {
                let style = markdown::Style::from(&cosmic::iced::Theme::Dark);
                widget::scrollable(
                    markdown::view(&self.items, markdown::Settings::with_style(style))
                        .map(move |u| m(NoteMsg::Link(u.to_string()))),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
            }
            false => text_editor::TextEditor::new(&self.text)
                .id(editor_id)
                .on_action(move |a| m(NoteMsg::Edit(a)))
                .padding(8)
                .height(Length::Fill)
                .into(),
        };
        let mut col = widget::column::with_capacity(3)
            .spacing(8)
            .padding(8)
            .push(bar);
        if let Some(e) = &self.error {
            col = col.push(widget::text::body(e.clone()).class(theme::Text::Accent));
        }
        col.push(body).into()
    }
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
