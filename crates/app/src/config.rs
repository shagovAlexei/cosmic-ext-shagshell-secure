//! Global options in cosmic-config (`~/.config/cosmic/<APP_ID>/v1/`, one file per field).

use cosmic::cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};
use i18n_embed::unic_langid::LanguageIdentifier;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppTheme {
    /// Follow the COSMIC appearance setting.
    #[default]
    System,
    Light,
    Dark,
}

impl AppTheme {
    pub const ALL: [AppTheme; 3] = [AppTheme::System, AppTheme::Light, AppTheme::Dark];

    pub fn theme(self) -> cosmic::Theme {
        match self {
            AppTheme::System => cosmic::theme::system_preference(),
            // The user's COSMIC palette (accent…), pinned to light / dark. `Custom`, not
            // `System` + prefer_dark: libcosmic replaces a System theme with the system's
            // own palette on its startup theme event.
            AppTheme::Light => pinned(cosmic::theme::system_light(), false),
            AppTheme::Dark => pinned(cosmic::theme::system_dark(), true),
        }
    }
}

/// Which side of the terminals a side pane sits on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    #[default]
    Left,
    Right,
}

impl Side {
    pub const ALL: [Side; 2] = [Side::Left, Side::Right];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
    Sessions,
    Commands,
}

/// The panes of the left and of the right column, top to bottom. On one side they are stacked:
/// the Session Manager above, unless `commands_above`.
pub fn panels(
    sessions: (Side, bool),
    commands: (Side, bool),
    commands_above: bool,
) -> (Vec<Panel>, Vec<Panel>) {
    let mut order = [Panel::Sessions, Panel::Commands];
    if commands_above {
        order.reverse();
    }
    let on = |side| {
        order
            .iter()
            .copied()
            .filter(|p| match p {
                Panel::Sessions => sessions == (side, true),
                Panel::Commands => commands == (side, true),
            })
            .collect::<Vec<_>>()
    };
    (on(Side::Left), on(Side::Right))
}

fn pinned(t: cosmic::Theme, dark: bool) -> cosmic::Theme {
    use cosmic::cosmic_theme::Theme as Palette;
    let mut p = t.cosmic().clone();
    // Outside COSMIC libcosmic hands back its default palette, dark: then COSMIC's stock one.
    if p.is_dark != dark {
        p = if dark {
            Palette::dark_default()
        } else {
            Palette::light_default()
        };
    }
    cosmic::Theme::custom(std::sync::Arc::new(p))
}

/// SFTP: what to do when the target file is already there.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExistsRule {
    #[default]
    Ask,
    Overwrite,
    Newer,
    Skip,
}

impl ExistsRule {
    pub const ALL: [ExistsRule; 4] = [
        ExistsRule::Ask,
        ExistsRule::Overwrite,
        ExistsRule::Newer,
        ExistsRule::Skip,
    ];

    /// The queue's answer to every conflict; `None`: ask.
    pub fn choice(self) -> Option<shagshell_core::sftp::queue::Choice> {
        use shagshell_core::sftp::queue::Choice;
        match self {
            ExistsRule::Ask => None,
            ExistsRule::Overwrite => Some(Choice::Overwrite),
            ExistsRule::Newer => Some(Choice::Newer),
            ExistsRule::Skip => Some(Choice::Skip),
        }
    }
}

/// Interface languages we ship; `""` in the config = the system's.
pub const LANGUAGES: [&str; 2] = ["en", "ru"];

/// What to ask the localizer for: the configured language, or the system's list.
pub fn languages(setting: &str, system: Vec<LanguageIdentifier>) -> Vec<LanguageIdentifier> {
    match setting.parse() {
        Ok(l) if LANGUAGES.contains(&setting) => vec![l],
        _ => system,
    }
}

#[derive(Clone, Debug, PartialEq, CosmicConfigEntry)]
#[version = 1]
pub struct Config {
    /// View → Session Manager (Alt+M).
    pub session_manager_visible: bool,
    /// Global Options → Terminal: a finished mouse selection goes to the clipboard.
    pub copy_on_select: bool,
    /// Global Options → Terminal: the middle mouse button pastes.
    pub paste_on_middle_click: bool,
    /// Options → Session Manager side.
    pub session_manager_side: Side,
    /// Options → Command Manager side.
    pub command_manager_side: Side,
    /// Script menu: the folder whose `*.py` files it lists ("" = `~/.config/shagshell/scripts`).
    pub scripts_folder: String,
    /// The folder Script → Run… and Stop Recording open in: where the last script was.
    pub script_dir: String,
    /// Script → Recent, newest first.
    pub recent_scripts: Vec<String>,
    /// Run on a local shell at startup ("" = none).
    pub startup_script: String,
    /// Script Console: every `crt` call is traced into it.
    pub script_debug: bool,
    /// Script Console: opens by itself when a script writes something.
    pub script_console_auto: bool,
    /// The sessions that had a tab at the last exit are opened again at the start.
    pub tabs_on_start: bool,
    /// The saved sessions (ids) with a tab open now, in the order they were opened.
    pub tabs_open: Vec<String>,
    /// Notes: the files open at the last exit come back at the start.
    pub notes_on_start: bool,
    /// The files the note tabs showed (newest state; written when one opens / closes).
    pub notes_open: Vec<String>,
    /// Both managers on one side: the Command Manager is the upper one.
    pub commands_above: bool,
    /// Only the header of the pane shows.
    pub session_manager_collapsed: bool,
    pub command_manager_collapsed: bool,
    /// Session Manager width, px (dragged).
    pub session_manager_width: u16,
    /// The Command Manager's width when it is alone on its side.
    pub command_manager_width: u16,
    /// Two panes stacked on a side: the top one's share, in thousandths (500: half).
    pub panes_split: u16,
    /// Open folders of the session tree (ids).
    pub expanded: Vec<String>,
    /// Interface language: "" (system), "en", "ru".
    pub language: String,
    pub app_theme: AppTheme,
    /// View → Command Window.
    pub command_window_visible: bool,
    /// View → Button Bar.
    pub button_bar_visible: bool,
    /// View → Command Manager.
    pub command_manager_visible: bool,
    /// Terminal colour scheme: "" (the theme's), or a key of `terminal_theme::schemes`.
    pub color_scheme: String,
    /// Keyword highlighting on (the words are in `highlights.toml`).
    pub highlight_enabled: bool,
    /// Where session logs go ("" = `~/ShagShellLogs`).
    pub log_dir: String,
    /// The log name when it is started by hand (File → Log Session).
    pub log_template: String,
    /// Where Zmodem downloads go ("" = `~/Downloads`).
    pub download_dir: String,
    /// A Zmodem send from the server is received without asking (needs lrzsz).
    pub zmodem_auto: bool,
    /// Terminal font family ("" = the system's monospace).
    pub font_family: String,
    /// The toolbar's buttons (`toolbar::TOOLS` names); empty: `toolbar::DEFAULT`.
    pub toolbar: Vec<String>,
    /// The tab bar under the tabs' content (SecureCRT's «tabs at the bottom»).
    pub tabs_bottom: bool,
    /// Ctrl+Tab goes back to the tab used before (SecureCRT's option), not the next one.
    pub ctrl_tab_recent: bool,
    /// No tab bar while there is one tab (in one group).
    pub tabs_hide_single: bool,
    /// Double-click on a tab: 0 nothing, 1 close, 2 clone, 3 reconnect, 4 a window of its own.
    pub tab_double_click: u8,
    /// Terminal cursor: 0 block, 1 underline, 2 bar; read through `cursor`.
    pub cursor_shape: u8,
    /// Terminal font size, px (0 = 14); read through `font_px`.
    pub font_size: u16,
    /// Tab header height, px (0 = the theme's); read through `tab_px`.
    pub tab_height: u16,
    /// Button Bar button height, px (0 = the theme's); read through `button_px`.
    pub button_height: u16,
    /// Ask GitHub for a newer release when the app starts.
    pub update_check: bool,
    /// The release the user chose to skip (its tag), "" = none.
    pub skipped_version: String,
    /// SFTP tabs open with dot files shown (Ctrl+H flips it in a tab).
    pub sftp_show_hidden: bool,
    /// SFTP: the target is already there.
    pub sftp_overwrite: ExistsRule,
}

impl Config {
    pub fn cursor(&self) -> alacritty_terminal::vte::ansi::CursorShape {
        use alacritty_terminal::vte::ansi::CursorShape as C;
        match self.cursor_shape {
            1 => C::Underline,
            2 => C::Beam,
            _ => C::Block,
        }
    }

    /// The folder of session logs: `~`, `~/x` or absolute; a relative one is under home too.
    pub fn log_dir(&self) -> std::path::PathBuf {
        let home = std::env::home_dir().unwrap_or_else(std::env::temp_dir);
        match self.log_dir.trim() {
            "" => home.join("ShagShellLogs"),
            "~" => home,
            d => match d.strip_prefix("~/") {
                Some(rest) => home.join(rest),
                None => home.join(d), // an absolute `d` replaces `home` in `join`
            },
        }
    }
}

impl Config {
    /// The folder of Zmodem downloads (same spelling as `log_dir`).
    pub fn download_dir(&self) -> std::path::PathBuf {
        let home = std::env::home_dir().unwrap_or_else(std::env::temp_dir);
        match self.download_dir.trim() {
            "" => home.join("Downloads"),
            "~" => home,
            d => match d.strip_prefix("~/") {
                Some(rest) => home.join(rest),
                None => home.join(d),
            },
        }
    }
}

impl Config {
    /// A tab header's height (None = the theme's; a value outside 20–64 is no height at all).
    pub fn tab_px(&self) -> Option<u16> {
        (20..=64)
            .contains(&self.tab_height)
            .then_some(self.tab_height)
    }

    /// A Button Bar button's height (same rule).
    pub fn button_px(&self) -> Option<u16> {
        (20..=64)
            .contains(&self.button_height)
            .then_some(self.button_height)
    }
}

impl Config {
    /// The terminal font size in px, kept to 6–48 (0 = the default 14).
    pub fn font_px(&self) -> f32 {
        match self.font_size {
            0 => 14.0,
            n => f32::from(n.clamp(6, 48)),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            session_manager_visible: true,
            copy_on_select: true,
            paste_on_middle_click: true,
            session_manager_side: Side::Left,
            command_manager_side: Side::Right,
            scripts_folder: String::new(),
            script_dir: String::new(),
            recent_scripts: Vec::new(),
            startup_script: String::new(),
            script_debug: false,
            script_console_auto: true,
            tabs_on_start: true,
            tabs_open: Vec::new(),
            notes_on_start: false,
            notes_open: Vec::new(),
            commands_above: false,
            session_manager_collapsed: false,
            command_manager_collapsed: false,
            session_manager_width: 240,
            command_manager_width: 260,
            panes_split: 500,
            expanded: Vec::new(),
            language: String::new(),
            app_theme: AppTheme::System,
            command_window_visible: false,
            button_bar_visible: true,
            command_manager_visible: false,
            color_scheme: String::new(),
            highlight_enabled: true,
            log_dir: String::new(),
            download_dir: String::new(),
            zmodem_auto: true,
            log_template: "%S_%Y-%M-%D_%h%m%s.log".into(),
            font_family: String::new(),
            cursor_shape: 0,
            tabs_bottom: false,
            toolbar: Vec::new(),
            tabs_hide_single: false,
            ctrl_tab_recent: false,
            tab_double_click: 0,
            font_size: 14,
            tab_height: 0,
            button_height: 0,
            update_check: true,
            skipped_version: String::new(),
            sftp_show_hidden: true,
            sftp_overwrite: ExistsRule::Ask,
        }
    }
}

pub fn handler() -> Option<cosmic_config::Config> {
    cosmic_config::Config::new(crate::app::APP_ID, Config::VERSION)
        .inspect_err(|e| log::warn!("config: {e}"))
        .ok()
}

/// Stored value, or the default for anything missing or unreadable.
pub fn read(h: Option<&cosmic_config::Config>) -> Config {
    h.map(|h| {
        Config::get_entry(h).unwrap_or_else(|(errs, c)| {
            for e in errs.iter().filter(|e| e.is_err()) {
                log::warn!("config: {e}");
            }
            c
        })
    })
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    #[test]
    fn panels_by_side() {
        use Panel::*;
        use Side::*;
        let p = |a, b, c, d, up| panels((a, b), (c, d), up);
        assert_eq!(
            p(Left, true, Right, true, false),
            (vec![Sessions], vec![Commands])
        );
        assert_eq!(
            p(Right, true, Left, true, false),
            (vec![Commands], vec![Sessions])
        );
        // stacked: the Session Manager on top, or the other way round
        assert_eq!(
            p(Left, true, Left, true, false),
            (vec![Sessions, Commands], vec![])
        );
        assert_eq!(
            p(Right, true, Right, true, true),
            (vec![], vec![Commands, Sessions])
        );
        assert_eq!(
            p(Right, false, Right, true, false),
            (vec![], vec![Commands])
        );
        assert_eq!(p(Left, false, Left, false, false), (vec![], vec![]));
    }

    use super::*;

    #[test]
    fn font_size_is_kept_in_range() {
        let px = |n| {
            Config {
                font_size: n,
                ..Config::default()
            }
            .font_px()
        };
        assert_eq!((px(0), px(3), px(14), px(500)), (14.0, 6.0, 14.0, 48.0));
    }

    #[test]
    fn heights_are_kept_in_range() {
        let c = |n| Config {
            tab_height: n,
            button_height: n,
            ..Config::default()
        };
        assert_eq!((c(0).tab_px(), c(0).button_px()), (None, None));
        // Was: a «2» typed on the way to «24» gave 20 px tabs.
        assert_eq!((c(2).tab_px(), c(500).button_px()), (None, None));
        assert_eq!((c(24).tab_px(), c(64).button_px()), (Some(24), Some(64)));
    }
}
