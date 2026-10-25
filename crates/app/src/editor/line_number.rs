// SPDX-License-Identifier: GPL-3.0-only
// From COSMIC Edit (src/line_number.rs at 4ac1cc8).

use cosmic_text::{
    Align, AttrsList, BufferLine, Ellipsize, FontSystem, LayoutLine, LineEnding, Shaping, Wrap,
};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct LineNumberKey {
    pub number: usize,
    pub width: usize,
}

#[derive(Debug)]
pub struct LineNumberCache {
    cache: HashMap<LineNumberKey, Vec<LayoutLine>>,
}

impl LineNumberCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn get(&mut self, font_system: &mut FontSystem, key: LineNumberKey) -> &Vec<LayoutLine> {
        self.cache.entry(key).or_insert_with(|| {
            let attrs = super::monospace_attrs();
            let text = format!("{:width$}", key.number, width = key.width);
            let mut buffer_line = BufferLine::new(
                text,
                LineEnding::default(),
                AttrsList::new(&attrs),
                Shaping::Advanced,
            );
            buffer_line.set_align(Some(Align::Left));
            buffer_line
                .layout(
                    font_system,
                    1.0, /* font size adjusted later */
                    None,
                    Wrap::None,
                    Ellipsize::None,
                    None,
                    8, /* default tab width */
                    Default::default(),
                )
                .to_vec()
        })
    }
}
