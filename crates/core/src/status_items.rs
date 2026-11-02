//! Which items the status bar shows, in what order (Terminal → Appearance → Status Bar).

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Item {
    State,
    Protocol,
    Size,
    Cursor,
    Up,
    Clock,
    Log,
    Recording,
    /// An id from a newer version: kept out of `available`, shown as nothing.
    #[serde(other)]
    Unknown,
}

impl Item {
    pub const ALL: [Item; 8] = [
        Item::State,
        Item::Protocol,
        Item::Size,
        Item::Cursor,
        Item::Up,
        Item::Clock,
        Item::Log,
        Item::Recording,
    ];
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Layout {
    pub items: Vec<Item>,
    /// Seconds in the connected time and the clock.
    pub seconds: bool,
    /// Text pressed to the right edge.
    pub right: bool,
}

impl Default for Layout {
    fn default() -> Self {
        Layout {
            items: vec![
                Item::State,
                Item::Protocol,
                Item::Size,
                Item::Cursor,
                Item::Up,
                Item::Log,
                Item::Recording,
            ],
            seconds: true,
            right: false,
        }
    }
}

impl Layout {
    /// The items not selected, in the canonical order.
    pub fn available(&self) -> Vec<Item> {
        (Item::ALL.into_iter())
            .filter(|i| !self.items.contains(i))
            .collect()
    }

    pub fn add(&mut self, item: Item) {
        if item != Item::Unknown && !self.items.contains(&item) {
            self.items.push(item);
        }
    }

    pub fn remove(&mut self, item: Item) {
        self.items.retain(|i| *i != item);
    }

    /// `by` -1 up, +1 down; at the ends nothing happens.
    pub fn shift(&mut self, item: Item, by: isize) {
        let Some(at) = self.items.iter().position(|i| *i == item) else {
            return;
        };
        if let Some(to) = at.checked_add_signed(by).filter(|t| *t < self.items.len()) {
            self.items.swap(at, to);
        }
    }
}

/// Connected time: `m:ss` / `h:mm:ss`, or without seconds `h:mm`.
pub fn up_text(secs: u64, seconds: bool) -> String {
    match seconds {
        true => crate::panel::duration_text(secs),
        false => format!("{}:{:02}", secs / 3600, secs / 60 % 60),
    }
}

/// The local time of day.
pub fn clock_text(seconds: bool) -> String {
    let fmt = if seconds { "%H:%M:%S" } else { "%H:%M" };
    jiff::Zoned::now().strftime(fmt).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_todays_bar() {
        let l = Layout::default();
        assert_eq!(l.items[0], Item::State);
        assert_eq!(l.available(), vec![Item::Clock]);
    }

    #[test]
    fn add_remove_shift() {
        let mut l = Layout::default();
        l.add(Item::Clock);
        l.add(Item::Clock);
        assert_eq!(l.items.iter().filter(|i| **i == Item::Clock).count(), 1);
        l.shift(Item::Clock, -1);
        assert_eq!(l.items[l.items.len() - 2], Item::Clock);
        l.shift(Item::State, -1); // at the top: stays
        assert_eq!(l.items[0], Item::State);
        l.remove(Item::State);
        assert!(l.available().contains(&Item::State));
    }

    #[test]
    fn unknown_id_is_read_and_never_offered() {
        let l: Layout = serde_json::from_str(r#"{"items":["Size","Future"]}"#).unwrap();
        assert_eq!(l.items, vec![Item::Size, Item::Unknown]);
        assert!(!l.available().contains(&Item::Unknown));
    }

    #[test]
    fn up_without_seconds() {
        assert_eq!(up_text(3725, false), "1:02");
        assert_eq!(up_text(65, true), "1:05");
    }
}
