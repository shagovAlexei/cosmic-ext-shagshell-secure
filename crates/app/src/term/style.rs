// Adapted from shagoff-commander (crates/app/src/term/style.rs).
//! What cosmic-term's `Config` gave its terminal: font weights, size and bright bold. Ours is
//! fixed but for the size (the panel font's), named as there so the adapted code reads the same.

use cosmic_text::{Family, Metrics, Stretch, Weight};
use std::{collections::HashSet, sync::Mutex};

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

impl Style {
    /// The family for `Attrs<'static>`: a name is leaked once (a handful at most, as typed in Options).
    pub fn family(&self) -> Family<'static> {
        static NAMES: Mutex<Option<HashSet<&'static str>>> = Mutex::new(None);
        let name = self.font_family.trim();
        if name.is_empty() {
            return Family::Monospace;
        }
        let mut names = NAMES.lock().unwrap_or_else(|e| e.into_inner());
        let names = names.get_or_insert_default();
        let leaked = match names.get(name) {
            Some(n) => *n,
            None => {
                let n: &'static str = Box::leak(name.to_string().into_boxed_str());
                names.insert(n);
                n
            }
        };
        Family::Name(leaked)
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
    fn family_is_monospace_when_empty_and_named_otherwise() {
        let mut s = Style::default();
        assert_eq!(s.family(), Family::Monospace);
        s.font_family = " Fira Code ".into();
        assert_eq!(s.family(), Family::Name("Fira Code"));
    }
}
