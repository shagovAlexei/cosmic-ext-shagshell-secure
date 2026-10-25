//! A note: the body of a tab that edits a text file, or shows a `.md` one as formatted text.
//! The file is written a moment after the last edit (`tick`) and when the tab goes.

use crate::app::Message;
use crate::fl;
use crate::toolbar::tip_button;
use cosmic::iced::Length;
use cosmic::widget;
use cosmic::widget::text_editor::{self, Action};
use cosmic::{Element, theme};
use shagshell_core::mdview::Look;
use shagshell_core::notes;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub enum NoteMsg {
    Edit(Action),
    /// The formatted view: moving and selecting only, never an edit.
    Look(Action),
    Preview,
    /// The whole text to the clipboard (the formatted view cannot be selected).
    Copy,
    RenameStart,
    RenameEdit(String),
    RenameDo,
    RenameCancel,
    Open,
}

pub struct NoteView {
    pub path: PathBuf,
    text: text_editor::Content,
    /// The formatted view: the markdown as plain text a mouse can select, and its looks.
    shown: text_editor::Content,
    spans: Spans,
    preview: bool,
    /// Edited, not written yet: since when.
    dirty: Option<Instant>,
    /// The new name being typed.
    rename: Option<String>,
    error: Option<String>,
    rename_id: widget::Id,
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
        let text = read(path)?;
        let mut v = Self {
            path: path.to_path_buf(),
            text: text_editor::Content::with_text(&text),
            shown: text_editor::Content::new(),
            spans: Spans::default(),
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
            let v = shagshell_core::mdview::render(&self.text.text());
            self.shown = text_editor::Content::with_text(&v.text);
            self.spans = Spans(std::sync::Arc::new(v.lines));
        }
    }

    /// Writes the file now (a failure is shown in the tab and tried again at the next edit).
    pub fn save(&mut self) {
        if self.dirty.is_none() {
            return;
        }
        // Written beside, then moved over: a crash leaves the old file, not half of the new.
        let tmp = self.path.with_file_name(format!(".{}.tmp", self.name()));
        match std::fs::write(&tmp, self.text.text())
            .and_then(|()| std::fs::rename(&tmp, &self.path))
        {
            Ok(()) => self.dirty = None,
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
    pub fn text(&self) -> String {
        self.text.text()
    }

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
            NoteMsg::Look(a) => {
                if !a.is_edit() {
                    self.shown.perform(a);
                }
            }
            NoteMsg::Preview => self.set_preview(!self.preview),
            NoteMsg::Copy => {}
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
                .push(tip_button(
                    "document-edit-symbolic",
                    fl!("note-rename"),
                    Some(m(NoteMsg::RenameStart)),
                ))
                .push(tip_button(
                    "document-open-symbolic",
                    fl!("note-open"),
                    Some(m(NoteMsg::Open)),
                )),
        };
        if notes::is_markdown(&self.path) {
            let (icon, label) = match self.preview {
                true => ("edit-symbolic", fl!("note-edit")),
                false => ("view-paged-symbolic", fl!("note-view")),
            };
            if self.preview {
                bar = bar.push(tip_button(
                    "edit-copy-symbolic",
                    fl!("note-copy"),
                    Some(m(NoteMsg::Copy)),
                ));
            }
            bar = bar.push(tip_button(icon, label, Some(m(NoteMsg::Preview))));
        }
        let body: Element<'_, Message> = match self.preview {
            // iced's own editor: libcosmic's wrapper cannot take a highlighter.
            // The tab's widget id too: the app's «focus the tab» then keeps it (and its selection).
            true => cosmic::iced::widget::text_editor(&self.shown)
                .id(editor_id)
                .on_action(move |a| m(NoteMsg::Look(a)))
                .highlight_with::<Looks>(self.spans.clone(), look_format)
                // A see-through selection: the coloured text (code, headings) stays readable.
                .style(|theme, status| {
                    use cosmic::iced::widget::text_editor::Catalog;
                    let mut st = theme.style(&<cosmic::Theme as Catalog>::default(), status);
                    st.selection.a = 0.3;
                    st.border.width = 0.0;
                    st
                })
                .padding(8)
                .height(Length::Fill)
                .into(),
            false => text_editor::TextEditor::new(&self.text)
                .id(editor_id)
                .on_action(move |a| m(NoteMsg::Edit(a)))
                // The tab's own frame is enough: no second one around the text.
                .style(|theme, status| {
                    use cosmic::iced::widget::text_editor::Catalog;
                    let mut st = theme.style(&<cosmic::Theme as Catalog>::default(), status);
                    st.border.width = 0.0;
                    st
                })
                .padding(8)
                .height(Length::Fill)
                .into(),
        };
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

/// The looks of every line of the formatted view (shared, compared by address).
#[derive(Clone, Default)]
struct Spans(std::sync::Arc<shagshell_core::mdview::Lines>);

impl PartialEq for Spans {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &other.0)
    }
}

/// Hands the editor the looks of the line it asks about, in order.
struct Looks {
    spans: Spans,
    line: usize,
}

impl cosmic::iced::advanced::text::Highlighter for Looks {
    type Settings = Spans;
    type Highlight = Look;
    type Iterator<'a> = std::vec::IntoIter<(std::ops::Range<usize>, Look)>;

    fn new(spans: &Spans) -> Self {
        Self {
            spans: spans.clone(),
            line: 0,
        }
    }

    fn update(&mut self, spans: &Spans) {
        self.spans = spans.clone();
        self.line = 0;
    }

    fn change_line(&mut self, line: usize) {
        self.line = self.line.min(line);
    }

    fn highlight_line(&mut self, _line: &str) -> Self::Iterator<'_> {
        let at = self.line;
        self.line += 1;
        self.spans
            .0
            .get(at)
            .cloned()
            .unwrap_or_default()
            .into_iter()
    }

    fn current_line(&self) -> usize {
        self.line
    }
}

/// Colour and font of a look: headings and links in the accent, code monospace.
fn look_format(
    look: &Look,
    theme: &cosmic::Theme,
) -> cosmic::iced::advanced::text::highlighter::Format<cosmic::iced::Font> {
    use cosmic::iced::font::{Style, Weight};
    let c = theme.cosmic();
    let mut font = cosmic::iced::Font::default();
    if look.code {
        font = cosmic::font::mono();
    }
    if look.bold || look.heading > 0 {
        font.weight = Weight::Bold;
    }
    if look.italic || look.quote {
        font.style = Style::Italic;
    }
    let color = if look.heading > 0 || look.link {
        Some(cosmic::iced::Color::from(c.accent_text_color()))
    } else if look.mark || look.quote {
        Some(cosmic::iced::Color::from(c.palette.neutral_7))
    } else if look.code {
        Some(cosmic::iced::Color::from(c.success_text_color()))
    } else {
        None
    };
    cosmic::iced::advanced::text::highlighter::Format {
        color,
        font: Some(font),
    }
}
