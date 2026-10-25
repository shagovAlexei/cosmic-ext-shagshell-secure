//! Rules for tabs that don't depend on the UI: titles, cycling, what is active after a close.

/// `base` if no tab has it, else `"base (n)"` with the smallest free n ≥ 2.
pub fn numbered(base: &str, taken: &[&str]) -> String {
    if !taken.contains(&base) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base} ({n})"))
        .find(|t| !taken.contains(&t.as_str()))
        .expect("a free number exists")
}

/// The next (or previous) position among `len`, wrapping around.
pub fn cycle(pos: usize, len: usize, forward: bool) -> usize {
    match (len, forward) {
        (0, _) => 0,
        (_, true) => (pos + 1) % len,
        (_, false) => (pos + len - 1) % len,
    }
}

/// `id` came to the front: first in `recent` (most recent first), kept to the `alive` ones.
pub fn touch(recent: &mut Vec<u64>, id: u64, alive: impl Fn(u64) -> bool) {
    recent.retain(|r| *r != id && alive(*r));
    recent.insert(0, id);
}

/// Which position to activate after closing `pos` of `len_before` tabs: the right neighbour
/// (it slides into `pos`), else the left one; `None` when it was the only tab.
pub fn after_close(pos: usize, len_before: usize) -> Option<usize> {
    let left = len_before.checked_sub(1).filter(|&n| n > 0)?;
    Some(pos.min(left - 1))
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_most_recent_tab_comes_first() {
        let mut r = vec![];
        for id in [1, 2, 3, 2] {
            super::touch(&mut r, id, |_| true);
        }
        assert_eq!(r, [2, 3, 1]);
        super::touch(&mut r, 1, |id| id != 3); // 3 was closed
        assert_eq!(r, [1, 2]);
    }
    use super::*;

    #[test]
    fn numbered_free() {
        assert_eq!(numbered("Local Shell", &[]), "Local Shell");
    }

    #[test]
    fn numbered_second() {
        assert_eq!(numbered("Local Shell", &["Local Shell"]), "Local Shell (2)");
    }

    #[test]
    fn numbered_gap() {
        let taken = ["Local Shell", "Local Shell (3)"];
        assert_eq!(numbered("Local Shell", &taken), "Local Shell (2)");
    }

    #[test]
    fn cycle_wraps() {
        assert_eq!(cycle(2, 3, true), 0);
        assert_eq!(cycle(0, 3, false), 2);
        assert_eq!(cycle(0, 3, true), 1);
    }

    #[test]
    fn cycle_empty() {
        assert_eq!(cycle(0, 0, true), 0);
    }

    #[test]
    fn after_close_middle() {
        assert_eq!(after_close(1, 3), Some(1));
    }

    #[test]
    fn after_close_last() {
        assert_eq!(after_close(2, 3), Some(1));
    }

    #[test]
    fn after_close_only() {
        assert_eq!(after_close(0, 1), None);
    }
}
