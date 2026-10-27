//! A tab's bar of buttons the user picks and orders in its «⋯» (Script Builder, notes): the
//! names shown, in order, as kept in the config.

pub struct Bar {
    /// Every button, in the default order.
    pub all: &'static [&'static str],
    /// Never hidden (a builder that can't run its script is no use).
    pub keep: Option<&'static str>,
}

impl Bar {
    /// As saved, made sound: unknown names and repeats dropped, `keep` kept (at the end).
    pub fn clean(&self, list: &[String]) -> Vec<String> {
        let mut out: Vec<String> = vec![];
        for b in list {
            if self.all.contains(&b.as_str()) && !out.contains(b) {
                out.push(b.clone());
            }
        }
        if let Some(k) = self.keep.filter(|k| !out.iter().any(|b| b == k)) {
            out.push(k.into());
        }
        out
    }

    /// `name` shown (at the end) or hidden.
    pub fn toggle(&self, list: &[String], name: &str) -> Vec<String> {
        let mut out = self.clean(list);
        match out.iter().position(|b| b == name) {
            Some(_) if self.keep == Some(name) => {}
            Some(i) => drop(out.remove(i)),
            None if self.all.contains(&name) => out.push(name.into()),
            None => {}
        }
        out
    }

    /// `name` one place to the left (`left`) or to the right.
    pub fn shift(&self, list: &[String], name: &str, left: bool) -> Vec<String> {
        let mut out = self.clean(list);
        if let Some(i) = out.iter().position(|b| b == name) {
            let j = if left { i.checked_sub(1) } else { Some(i + 1) };
            if let Some(j) = j.filter(|j| *j < out.len()) {
                out.swap(i, j);
            }
        }
        out
    }

    /// The «⋯» list: the shown ones in their order, then the hidden ones.
    pub fn rows(&self, list: &[String]) -> Vec<(&'static str, bool)> {
        let shown = (list.iter()).filter_map(|n| self.all.iter().find(|b| *b == n).copied());
        let hidden = (self.all.iter().copied()).filter(|b| !list.iter().any(|n| n == b));
        (shown.map(|b| (b, true)))
            .chain(hidden.map(|b| (b, false)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const B: Bar = Bar {
        all: &["New", "Save", "Undo", "Run"],
        keep: Some("Run"),
    };
    const N: Bar = Bar {
        all: &["Save", "Undo"],
        keep: None,
    };

    fn v(l: &[&str]) -> Vec<String> {
        l.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn clean_drops_unknown_and_keeps_keep() {
        assert_eq!(B.clean(&v(&["Save", "Nope", "Save"])), v(&["Save", "Run"]));
        assert_eq!(N.clean(&v(&["Nope"])), v(&[]));
    }

    #[test]
    fn toggle_hides_and_shows_at_the_end() {
        let l = B.toggle(&v(&["New", "Save", "Run"]), "New");
        assert_eq!(l, v(&["Save", "Run"]));
        assert_eq!(B.toggle(&l, "New"), v(&["Save", "Run", "New"]));
        assert_eq!(B.toggle(&l, "Run"), l, "Run is never hidden");
        assert_eq!(N.toggle(&v(&["Save"]), "Save"), v(&[]));
    }

    #[test]
    fn shift_moves_one_place_and_stops_at_the_ends() {
        let l = v(&["New", "Save", "Run"]);
        assert_eq!(B.shift(&l, "Save", true), v(&["Save", "New", "Run"]));
        assert_eq!(B.shift(&l, "Save", false), v(&["New", "Run", "Save"]));
        assert_eq!(B.shift(&l, "New", true), l);
        assert_eq!(B.shift(&l, "Run", false), l);
        assert_eq!(B.shift(&l, "Undo", true), l, "a hidden one stays hidden");
    }

    #[test]
    fn rows_shown_in_order_then_hidden() {
        assert_eq!(
            B.rows(&v(&["Save", "New", "Run"])),
            [
                ("Save", true),
                ("New", true),
                ("Run", true),
                ("Undo", false)
            ]
        );
    }
}
