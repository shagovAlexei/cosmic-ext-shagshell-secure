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
