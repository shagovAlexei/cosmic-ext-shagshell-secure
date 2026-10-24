//! Global options in cosmic-config (`~/.config/cosmic/<APP_ID>/v1/`, one file per field).

use cosmic::cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};

#[derive(Clone, Debug, PartialEq, CosmicConfigEntry)]
#[version = 1]
pub struct Config {
    /// View → Session Manager (Alt+M).
    pub session_manager_visible: bool,
    /// Global Options → Terminal: a finished mouse selection goes to the clipboard.
    pub copy_on_select: bool,
    /// Global Options → Terminal: the middle mouse button pastes.
    pub paste_on_middle_click: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            session_manager_visible: true,
            copy_on_select: true,
            paste_on_middle_click: true,
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
