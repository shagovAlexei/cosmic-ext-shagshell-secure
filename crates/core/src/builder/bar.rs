//! The Script Builder tab's bar (`Config.builder_buttons`, `core::bar`).

/// Every button, in the default order.
pub const BUTTONS: [&str; 11] = [
    "New",
    "Open",
    "Template",
    "SaveTemplate",
    "Save",
    "SaveAs",
    "Undo",
    "Redo",
    "Target",
    "Settings",
    "Run",
];

pub const BAR: crate::bar::Bar = crate::bar::Bar {
    all: &BUTTONS,
    keep: Some("Run"),
};

pub fn default_buttons() -> Vec<String> {
    BUTTONS.map(String::from).to_vec()
}
