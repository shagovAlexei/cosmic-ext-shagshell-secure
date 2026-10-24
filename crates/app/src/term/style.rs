// Adapted from shagoff-commander (crates/app/src/term/style.rs).
//! What cosmic-term's `Config` gave its terminal: font weights, size and bright bold. Ours is
//! fixed but for the size (the panel font's), named as there so the adapted code reads the same.

use cosmic_text::{Family, Metrics, Stretch, Weight};
use std::{collections::HashMap, sync::Mutex};

#[derive(Clone, Debug, PartialEq)]
pub struct Style {
    pub font_size: f32,
    /// "" = the system's monospace.
    pub font_family: String,
    pub font_weight: u16,
    pub dim_font_weight: u16,
    pub bold_font_weight: u16,
    pub use_bright_bold: bool,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            font_size: 14.0,
            font_family: String::new(),
            font_weight: Weight::NORMAL.0,
            dim_font_weight: Weight::NORMAL.0,
            bold_font_weight: Weight::BOLD.0,
            use_bright_bold: false,
        }
    }
}

/// The installed monospace families, sorted (for the font choice).
pub fn monospace_families() -> Vec<String> {
    static ALL: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    ALL.get_or_init(|| {
        let mut fs = cosmic::iced::advanced::graphics::text::font_system()
            .write()
            .unwrap();
        let mut names: Vec<String> = fs
            .raw()
            .db()
            .faces()
            .filter(|f| f.monospaced)
            .filter_map(|f| f.families.first().map(|(n, _)| n.clone()))
            .collect();
        names.sort_by_key(|n| n.to_lowercase());
        names.dedup();
        names
    })
    .clone()
}

impl Style {
    /// The family for `Attrs<'static>`: the system's monospace when empty or not installed (cosmic-text
    /// would otherwise pick any face, maybe a proportional one). An installed name is leaked once.
    pub fn family(&self) -> Family<'static> {
        static NAMES: Mutex<Option<HashMap<String, Option<&'static str>>>> = Mutex::new(None);
        let name = self.font_family.trim();
        if name.is_empty() {
            return Family::Monospace;
        }
        let mut names = NAMES.lock().unwrap_or_else(|e| e.into_inner());
        let known = names.get_or_insert_default().get(name).copied();
        let found = known.unwrap_or_else(|| {
            let mut fs = cosmic::iced::advanced::graphics::text::font_system()
                .write()
                .unwrap();
            let installed = fs
                .raw()
                .db()
                .faces()
                .any(|f| f.families.iter().any(|(n, _)| n.eq_ignore_ascii_case(name)));
            let leaked = installed.then(|| &*Box::leak(name.to_string().into_boxed_str()));
            names
                .get_or_insert_default()
                .insert(name.to_string(), leaked);
            leaked
        });
        found.map_or(Family::Monospace, Family::Name)
    }

    pub fn typed_font_stretch(&self) -> Stretch {
        Stretch::Normal
    }

    /// Line height 1.4 × the size, as cosmic-term; `zoom_adj` steps of 1 px.
    pub fn metrics(&self, zoom_adj: i8) -> Metrics {
        let size = (self.font_size + f32::from(zoom_adj)).max(1.0);
        Metrics::new(size, (size * 1.4).ceil())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn family_is_monospace_when_empty_or_not_installed() {
        let mut s = Style::default();
        assert_eq!(s.family(), Family::Monospace);
        s.font_family = "No Such Font 123".into();
        assert_eq!(s.family(), Family::Monospace);
    }
}
