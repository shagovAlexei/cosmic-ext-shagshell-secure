//! The Script Builder tab's bar: which buttons it shows, in what order (`Config.builder_buttons`).

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

/// Never hidden: a builder that can't run its script is no use.
pub const ALWAYS: &str = "Run";

pub fn default_buttons() -> Vec<String> {
    BUTTONS.map(String::from).to_vec()
}

/// As saved, made sound: unknown names and repeats dropped, Run kept (at the end when missing).
pub fn clean(list: &[String]) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for b in list {
        if BUTTONS.contains(&b.as_str()) && !out.contains(b) {
            out.push(b.clone());
        }
    }
    if !out.iter().any(|b| b == ALWAYS) {
        out.push(ALWAYS.into());
    }
    out
}

/// `name` shown (at the end) or hidden.
pub fn toggle(list: &[String], name: &str) -> Vec<String> {
    let mut out = clean(list);
    match out.iter().position(|b| b == name) {
        Some(_) if name == ALWAYS => {}
        Some(i) => drop(out.remove(i)),
        None if BUTTONS.contains(&name) => out.push(name.into()),
        None => {}
    }
    out
}

/// `name` one place to the left (`left`) or to the right.
pub fn shift(list: &[String], name: &str, left: bool) -> Vec<String> {
    let mut out = clean(list);
    if let Some(i) = out.iter().position(|b| b == name) {
        let j = if left { i.checked_sub(1) } else { Some(i + 1) };
        if let Some(j) = j.filter(|j| *j < out.len()) {
            out.swap(i, j);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(l: &[&str]) -> Vec<String> {
        l.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn clean_drops_unknown_and_keeps_run() {
        assert_eq!(clean(&v(&["Save", "Nope", "Save"])), v(&["Save", "Run"]));
        assert_eq!(clean(&default_buttons()), default_buttons());
    }

    #[test]
    fn toggle_hides_and_shows_at_the_end() {
        let l = toggle(&v(&["New", "Save", "Run"]), "New");
        assert_eq!(l, v(&["Save", "Run"]));
        assert_eq!(toggle(&l, "New"), v(&["Save", "Run", "New"]));
        assert_eq!(toggle(&l, "Run"), l, "Run is never hidden");
    }

    #[test]
    fn shift_moves_one_place_and_stops_at_the_ends() {
        let l = v(&["New", "Save", "Run"]);
        assert_eq!(shift(&l, "Save", true), v(&["Save", "New", "Run"]));
        assert_eq!(shift(&l, "Save", false), v(&["New", "Run", "Save"]));
        assert_eq!(shift(&l, "New", true), l);
        assert_eq!(shift(&l, "Run", false), l);
        assert_eq!(shift(&l, "Undo", true), l, "a hidden one stays hidden");
    }
}
