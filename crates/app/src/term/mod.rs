//! cosmic-term's terminal (via shagoff-commander), one per tab. Every connection is a program in
//! its PTY: a local shell now, `ssh` / `telnet` / `picocom` later.

pub mod log;
mod mouse_reporter;
pub mod style;
pub mod terminal;
pub mod terminal_box;
pub mod terminal_theme;

use crate::app::Message;
use alacritty_terminal::{event::Event, term, tty};
use cosmic::iced::{Subscription, futures::SinkExt};
use std::sync::Mutex;
use std::sync::atomic::Ordering;
use terminal::Terminal;
use tokio::sync::mpsc::UnboundedSender;

pub type TabId = u64;
pub type Events = UnboundedSender<(TabId, Event)>;

/// Lines kept above the screen.
pub(crate) const HISTORY: usize = 10_000;

/// Start `program` with `args` in the home dir. `dark`: the COSMIC theme's colors; `bg`: the window's
/// background (0xAARRGGBB), the terminal's own.
#[allow(clippy::too_many_arguments)]
pub fn start(
    id: TabId,
    tx: Events,
    program: &str,
    args: &[String],
    env: &[(String, String)],
    dark: bool,
    bg: u32,
    style: &style::Style,
    log: log::Tap,
    scheme: &str,
) -> std::io::Result<Terminal> {
    terminal::WINDOW_BG_COLOR.store(bg, Ordering::SeqCst);
    let colors = colors(dark, scheme);
    let options = tty::Options {
        shell: Some(tty::Shell::new(program.to_string(), args.to_vec())),
        working_directory: std::env::home_dir(),
        env: std::iter::once(("TERM".to_string(), "xterm-256color".to_string()))
            .chain(env.iter().cloned())
            .collect(),
        ..tty::Options::default()
    };
    let config = term::Config {
        scrolling_history: HISTORY,
        ..term::Config::default()
    };
    let mut t = Terminal::new(id, tx, config, options, style, colors, log)?;
    t.set_config(style, &colors);
    Ok(t)
}

/// The scheme's colours, else the theme's own (dark / light).
fn colors(dark: bool, scheme: &str) -> term::color::Colors {
    if let Some(c) = terminal_theme::scheme(scheme) {
        return c;
    }
    match dark {
        true => terminal_theme::cosmic_dark(),
        false => terminal_theme::cosmic_light(),
    }
}

/// What a script sees of a terminal: the screen's rows as text, the cursor (0-based), the selection.
pub struct Screen {
    pub rows: Vec<String>,
    pub row: usize,
    pub col: usize,
    pub cols: usize,
    pub selection: String,
}

pub fn screen(t: &Mutex<Terminal>) -> Screen {
    use alacritty_terminal::grid::Dimensions;
    use alacritty_terminal::index::{Column, Line};
    use alacritty_terminal::term::cell::Flags;
    let t = t.lock().unwrap_or_else(|e| e.into_inner());
    let term = t.term.lock();
    let grid = term.grid();
    let rows = (0..grid.screen_lines())
        .map(|i| {
            let line = &grid[Line(i as i32)];
            (0..grid.columns())
                .map(|j| &line[Column(j)])
                // One char per grid column (a wide char's second cell is a blank), so that
                // column numbers are the screen's.
                .map(|c| {
                    match c
                        .flags
                        .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
                    {
                        true => ' ',
                        false => c.c,
                    }
                })
                .collect()
        })
        .collect();
    Screen {
        rows,
        row: usize::try_from(grid.cursor.point.line.0).unwrap_or(0),
        col: grid.cursor.point.column.0,
        cols: grid.columns(),
        selection: term.selection_to_string().unwrap_or_default(),
    }
}

/// A new text size or theme for a running terminal.
pub fn restyle(t: &Mutex<Terminal>, style: &style::Style, dark: bool, bg: u32, scheme: &str) {
    terminal::WINDOW_BG_COLOR.store(bg, Ordering::SeqCst);
    let mut t = t.lock().unwrap_or_else(|e| e.into_inner());
    t.set_config(style, &colors(dark, scheme));
}

/// Show `text` on the screen as if the program had printed it (no PTY write: it may be dead).
pub fn print(t: &Mutex<Terminal>, text: &str) {
    let mut t = t.lock().unwrap_or_else(|e| e.into_inner());
    let mut parser = alacritty_terminal::vte::ansi::Processor::<
        alacritty_terminal::vte::ansi::StdSyncHandler,
    >::new();
    parser.advance(&mut *t.term.lock(), text.as_bytes());
    t.needs_update = true;
}

/// What the app does about a terminal event.
pub enum Outcome {
    None,
    /// The program ended (its exit code, when known).
    Exit(Option<i32>),
    /// OSC 52: put this on the clipboard.
    Copy(String),
    /// OSC 0 / 2; empty: reset to ours.
    Title(String),
    /// Something new on screen.
    Output,
}

pub fn on_event(t: &Mutex<Terminal>, event: Event) -> Outcome {
    let mut t = t.lock().unwrap_or_else(|e| e.into_inner());
    match event {
        Event::Wakeup => {
            t.needs_update = true;
            return Outcome::Output;
        }
        Event::MouseCursorDirty => t.needs_update = true,
        Event::ChildExit(code) => return Outcome::Exit(Some(code)),
        Event::Exit => return Outcome::Exit(None),
        Event::Title(title) => return Outcome::Title(title),
        Event::ResetTitle => return Outcome::Title(String::new()),
        Event::PtyWrite(text) => t.input_no_scroll(text.into_bytes()),
        Event::ColorRequest(index, f) => {
            let text = f(t.effective_color(index));
            t.input_no_scroll(text.into_bytes());
        }
        Event::TextAreaSizeRequest(f) => {
            let text = f(t.size().into());
            t.input_no_scroll(text.into_bytes());
        }
        Event::ClipboardStore(term::ClipboardType::Clipboard, data) => return Outcome::Copy(data),
        _ => {}
    }
    Outcome::None
}

/// The channel every terminal sends its events through: its sender comes first (`TermTx`), then
/// the events (`Term`).
pub fn subscription() -> Subscription<Message> {
    struct Channel;
    Subscription::run_with(std::any::TypeId::of::<Channel>(), |_| {
        cosmic::iced::stream::channel(100, async |mut out| {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            let _ = out.send(Message::TermTx(tx)).await;
            while let Some((id, e)) = rx.recv().await {
                if out.send(Message::Term(id, e)).await.is_err() {
                    break;
                }
            }
            std::future::pending::<()>().await;
        })
    })
}
