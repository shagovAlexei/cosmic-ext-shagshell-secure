//! Edit → Find in a terminal: the terminal searches by regex, the user types plain text.

/// `text` as a regex that matches it literally.
pub fn literal(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if "\\.+*?()|[]{}^$#&-~".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_text_is_matched_literally() {
        assert_eq!(literal("10.0.0.1"), r"10\.0\.0\.1");
        assert_eq!(literal("a(b)*"), r"a\(b\)\*");
        assert_eq!(literal("plain"), "plain");
        assert_eq!(literal("путь/к"), "путь/к");
    }
}
