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
    /// Session Manager width, px (dragged).
    pub session_manager_width: u16,
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
    /// Terminal font size, px (0 = 14); read through `font_px`.
    pub font_size: u16,
    /// SFTP tabs open with dot files shown (Ctrl+H flips it in a tab).
    pub sftp_show_hidden: bool,
    /// SFTP: the target is already there.
    pub sftp_overwrite: ExistsRule,
}

impl Config {
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
            session_manager_width: 240,
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
            font_size: 14,
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
}
