//! Edit → Find: in a terminal (it searches by regex, the user types plain text) and in a note
//! (`next_in` / `replace_all` over its lines, ignoring case).

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

/// Where `needle` ends if it starts at byte `at` of `hay`, letters compared ignoring case.
fn match_at(hay: &str, at: usize, needle: &str) -> Option<usize> {
    let mut rest = hay[at..].char_indices();
    let mut end = at;
    for n in needle.chars() {
        let (i, h) = rest.next()?;
        if !h.to_lowercase().eq(n.to_lowercase()) {
            return None;
        }
        end = at + i + h.len_utf8();
    }
    Some(end)
}

/// The matches of `needle` in one line: (start, end) bytes, not overlapping.
fn matches_in(line: &str, needle: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut at = 0;
    while at <= line.len() && !needle.is_empty() {
        match match_at(line, at, needle) {
            Some(end) => {
                out.push((at, end));
                at = end;
            }
            None => match line[at..].chars().next() {
                Some(c) => at += c.len_utf8(),
                None => break,
            },
        }
    }
    out
}

/// A note's Find: the match of `needle` (ignoring case) that starts at or after `from`
/// (line, byte) going forwards, or the last one before it going back; round the ends.
/// `(line, start, end)`.
pub fn next_in(
    lines: &[&str],
    from: (usize, usize),
    needle: &str,
    forwards: bool,
) -> Option<(usize, usize, usize)> {
    let all: Vec<(usize, usize, usize)> = (lines.iter().enumerate())
        .flat_map(|(l, line)| {
            matches_in(line, needle)
                .into_iter()
                .map(move |(s, e)| (l, s, e))
        })
        .collect();
    let found = match forwards {
        true => all.iter().find(|m| (m.0, m.1) >= from).or(all.first()),
        false => all.iter().rev().find(|m| (m.0, m.1) < from).or(all.last()),
    };
    found.copied()
}

/// `text` is a match of `needle` as a whole (Replace acts on a selection only if it is one).
pub fn is_match(text: &str, needle: &str) -> bool {
    !needle.is_empty() && match_at(text, 0, needle) == Some(text.len())
}

/// Replace all: every match of `needle` (ignoring case) in `text` becomes `with`; how many.
pub fn replace_all(text: &str, needle: &str, with: &str) -> (String, usize) {
    let mut out = String::with_capacity(text.len());
    let mut n = 0;
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let mut last = 0;
        for (s, e) in matches_in(line, needle) {
            out.push_str(&line[last..s]);
            out.push_str(with);
            last = e;
            n += 1;
        }
        out.push_str(&line[last..]);
    }
    (out, n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_note_finds_ignoring_case_round_the_ends() {
        let lines = ["host Router1", "ping router1", "Сервер и СЕРВЕР"];
        assert_eq!(next_in(&lines, (0, 0), "router", true), Some((0, 5, 11)));
        // From just after the first match: the second; after the last: round to the first.
        assert_eq!(next_in(&lines, (0, 6), "router", true), Some((1, 5, 11)));
        assert_eq!(next_in(&lines, (2, 0), "router", true), Some((0, 5, 11)));
        // Back from the second: the first; from the top: round to the last.
        assert_eq!(next_in(&lines, (1, 5), "router", false), Some((0, 5, 11)));
        assert_eq!(next_in(&lines, (0, 0), "router", false), Some((1, 5, 11)));
        // Cyrillic, any case: byte offsets of the line itself.
        assert_eq!(next_in(&lines, (2, 1), "сервер", true), Some((2, 16, 28)));
        assert_eq!(next_in(&lines, (0, 0), "nothing", true), None);
        assert_eq!(next_in(&lines, (0, 0), "", true), None);
        assert!(is_match("ROUTER", "router"));
        assert!(!is_match("routers", "router"));
        assert!(!is_match("", ""));
    }

    #[test]
    fn a_note_replaces_all() {
        let (t, n) = replace_all("a Router\nrouter b\nnone", "router", "sw");
        assert_eq!((t.as_str(), n), ("a sw\nsw b\nnone", 2));
        // The new text holding the old one is not replaced again.
        assert_eq!(replace_all("aa", "a", "aa"), ("aaaa".to_string(), 2));
        assert_eq!(replace_all("x", "", "y"), ("x".to_string(), 0));
        assert_eq!(replace_all("Ёж ёж\r\n", "ёж", "кот").0, "кот кот\r\n");
    }

    #[test]
    fn find_text_is_matched_literally() {
        assert_eq!(literal("10.0.0.1"), r"10\.0\.0\.1");
        assert_eq!(literal("a(b)*"), r"a\(b\)\*");
        assert_eq!(literal("plain"), "plain");
        assert_eq!(literal("путь/к"), "путь/к");
    }
}
