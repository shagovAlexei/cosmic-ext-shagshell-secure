//! Strings sent to a session from the Command Window and Button Bar (SecureCRT's Send String):
//! `\r \n \t \e \\` are the bytes they say, `\p` a one-second pause, anything else after a
//! backslash stays as typed.

use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Piece {
    Bytes(Vec<u8>),
    Pause(Duration),
}

/// `text` as pieces: bytes to send, and pauses between them.
pub fn parse(text: &str) -> Vec<Piece> {
    let mut out = Vec::new();
    let mut cur: Vec<u8> = Vec::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            let mut b = [0; 4];
            cur.extend(c.encode_utf8(&mut b).as_bytes());
            continue;
        }
        match chars.next() {
            Some('r') => cur.push(b'\r'),
            Some('n') => cur.push(b'\n'),
            Some('t') => cur.push(b'\t'),
            Some('e') => cur.push(0x1b),
            Some('\\') => cur.push(b'\\'),
            Some('p') => {
                if !cur.is_empty() {
                    out.push(Piece::Bytes(std::mem::take(&mut cur)));
                }
                out.push(Piece::Pause(Duration::from_secs(1)));
            }
            // Not an escape: both characters as typed.
            Some(o) => {
                cur.push(b'\\');
                let mut b = [0; 4];
                cur.extend(o.encode_utf8(&mut b).as_bytes());
            }
            None => cur.push(b'\\'),
        }
    }
    if !cur.is_empty() {
        out.push(Piece::Bytes(cur));
    }
    out
}

/// A multi-line `text` (Command Window) as the pieces to send: every non-empty line parsed and
/// ended with Enter, in order. Nothing but blank lines is a bare Enter.
pub fn lines(text: &str) -> Vec<Piece> {
    let mut out = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        out.extend(parse(line));
        match out.last_mut() {
            Some(Piece::Bytes(b)) => b.push(b'\r'),
            _ => out.push(Piece::Bytes(b"\r".to_vec())),
        }
    }
    if out.is_empty() {
        out.push(Piece::Bytes(b"\r".to_vec()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_ends_each_with_enter_and_skips_blanks() {
        assert_eq!(lines(""), [bytes(b"\r")]);
        assert_eq!(lines("  \n\n"), [bytes(b"\r")]);
        assert_eq!(lines("ls"), [bytes(b"ls\r")]);
        assert_eq!(lines("a\r\n\nb\n"), [bytes(b"a\r"), bytes(b"b\r")]);
        // a pause stays inside its line, Enter after it
        assert_eq!(
            lines("x\\p"),
            [
                bytes(b"x"),
                Piece::Pause(Duration::from_secs(1)),
                bytes(b"\r")
            ]
        );
    }

    fn bytes(b: &[u8]) -> Piece {
        Piece::Bytes(b.to_vec())
    }

    #[test]
    fn escapes() {
        assert_eq!(parse(r"a\r\n\t\e\\b"), [bytes(b"a\r\n\t\x1b\\b")]);
    }

    #[test]
    fn pause_splits_the_string() {
        assert_eq!(
            parse(r"en\pconf t\r"),
            [
                bytes(b"en"),
                Piece::Pause(Duration::from_secs(1)),
                bytes(b"conf t\r")
            ]
        );
        assert_eq!(parse(r"\p"), [Piece::Pause(Duration::from_secs(1))]);
    }

    #[test]
    fn unknown_escape_and_lone_backslash_stay() {
        assert_eq!(parse(r"a\qb\"), [bytes(b"a\\qb\\")]);
        assert_eq!(parse("żółć\\r"), [bytes("żółć\r".as_bytes())]);
        assert_eq!(parse(""), []);
    }
}
