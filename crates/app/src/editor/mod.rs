//! A note's editor: COSMIC Edit's text box (`text_box`, `line_number`) over a cosmic-text
//! `SyntaxEditor`, with COSMIC Edit's syntax themes. Never vi: plain typing.

pub mod line_number;
pub mod text_box;

use cosmic::iced::advanced::graphics::text::font_system;
use cosmic_text::{
    Attrs, Buffer, Edit, Family, Metrics, SwashCache, SyntaxEditor, SyntaxSystem, ViEditor, Wrap,
};
use line_number::LineNumberCache;
use std::path::Path;
use std::sync::{Arc, LazyLock, Mutex};

static LINE_NUMBER_CACHE: LazyLock<Mutex<LineNumberCache>> =
    LazyLock::new(|| Mutex::new(LineNumberCache::new()));
static SWASH_CACHE: LazyLock<Mutex<SwashCache>> = LazyLock::new(|| Mutex::new(SwashCache::new()));
static SYNTAX_SYSTEM: LazyLock<SyntaxSystem> = LazyLock::new(syntax_system);

pub fn monospace_attrs() -> Attrs<'static> {
    Attrs::new().family(Family::Monospace)
}

/// As COSMIC Edit: two-face's syntaxes and themes, plus COSMIC Dark / Light drawn on the
/// window's own background.
fn syntax_system() -> SyntaxSystem {
    let lazy = two_face::theme::LazyThemeSet::from(two_face::theme::extra());
    let mut theme_set = syntect::highlighting::ThemeSet::from(&lazy);
    for (name, data) in [
        ("COSMIC Dark", cosmic_syntax_theme::COSMIC_DARK_TM_THEME),
        ("COSMIC Light", cosmic_syntax_theme::COSMIC_LIGHT_TM_THEME),
    ] {
        let mut cursor = std::io::Cursor::new(data);
        if let Ok(mut theme) = syntect::highlighting::ThemeSet::load_from_reader(&mut cursor) {
            let clear = Some(syntect::highlighting::Color {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            });
            theme.settings.background = clear;
            theme.settings.gutter = clear;
            theme_set.themes.insert(name.to_string(), theme);
        }
    }
    SyntaxSystem {
        syntax_set: two_face::syntax::extra_no_newlines(),
        theme_set,
    }
}

/// The syntax theme of the light / dark look.
pub fn theme_name() -> &'static str {
    match cosmic::theme::active().cosmic().is_dark {
        true => "COSMIC Dark",
        false => "COSMIC Light",
    }
}

/// The font size of the text and its line height (COSMIC Edit's 1.4).
pub fn metrics(size: f32) -> Metrics {
    Metrics::new(size, (size * 1.4).ceil())
}

/// An editor holding `path`'s text (already read and checked), highlighted by its extension.
pub fn open(path: &Path, size: f32) -> std::io::Result<Mutex<ViEditor<'static, 'static>>> {
    let mut buffer = Buffer::new_empty(metrics(size));
    buffer.set_size(Some(0.0), Some(0.0));
    let syntax = SyntaxEditor::new(Arc::new(buffer), &SYNTAX_SYSTEM, theme_name())
        .ok_or_else(|| std::io::Error::other("no syntax theme"))?;
    let mut editor = ViEditor::new(syntax);
    {
        let mut fs = font_system().write().unwrap_or_else(|e| e.into_inner());
        let mut e = editor.borrow_with(fs.raw());
        e.set_passthrough(true);
        e.set_auto_indent(true);
        e.with_buffer_mut(|b| b.set_wrap(Wrap::WordOrGlyph));
        e.load_text(path, monospace_attrs())?;
        e.set_changed(false);
    }
    Ok(Mutex::new(editor))
}

/// The whole text, line endings kept.
pub fn text(editor: &Mutex<ViEditor<'static, 'static>>) -> String {
    let e = editor.lock().unwrap_or_else(|e| e.into_inner());
    e.with_buffer(|buffer| {
        let mut text = String::new();
        for line in &buffer.lines {
            text.push_str(line.text());
            text.push_str(line.ending().as_str());
        }
        text
    })
}

/// Follows the light / dark look (cheap when it did not change).
pub fn follow_theme(editor: &Mutex<ViEditor<'static, 'static>>) {
    let mut e = editor.lock().unwrap_or_else(|e| e.into_inner());
    e.update_theme(theme_name());
}

/// Long lines wrapped at the edge, or one screen line each.
pub fn set_wrap(editor: &Mutex<ViEditor<'static, 'static>>, on: bool) {
    let mut e = editor.lock().unwrap_or_else(|e| e.into_inner());
    let mut fs = font_system().write().unwrap_or_else(|e| e.into_inner());
    let wrap = match on {
        true => Wrap::WordOrGlyph,
        false => Wrap::None,
    };
    e.borrow_with(fs.raw())
        .with_buffer_mut(|b| b.set_wrap(wrap));
}

type Ed = Mutex<ViEditor<'static, 'static>>;

fn with<T>(editor: &Ed, f: impl FnOnce(&mut ViEditor<'static, 'static>) -> T) -> T {
    f(&mut editor.lock().unwrap_or_else(|e| e.into_inner()))
}

pub fn undo(editor: &Ed) {
    with(editor, |e| e.undo());
}

pub fn redo(editor: &Ed) {
    with(editor, |e| e.redo());
}

pub fn copy(editor: &Ed) -> Option<String> {
    with(editor, |e| e.copy_selection())
}

pub fn cut(editor: &Ed) -> Option<String> {
    with(editor, |e| {
        let t = e.copy_selection()?;
        e.start_change();
        e.delete_selection();
        e.finish_change();
        Some(t)
    })
}

pub fn paste(editor: &Ed, text: &str) {
    with(editor, |e| {
        e.start_change();
        e.insert_string(text, None);
        e.finish_change();
    });
}

pub fn select_all(editor: &Ed) {
    with(editor, |e| {
        e.set_cursor(cosmic_text::Cursor::new(0, 0));
        let end = e.with_buffer(|b| {
            let last = b.lines.len().saturating_sub(1);
            cosmic_text::Cursor::new(last, b.lines[last].text().len())
        });
        e.set_selection(cosmic_text::Selection::Normal(end));
    });
}
