//! Files dropped on the window by a file manager arrive as a `text/uri-list`.

use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;

/// The local paths of a `text/uri-list` (`file://` lines; comments and other schemes are left).
pub fn paths_of(list: &[u8]) -> Vec<PathBuf> {
    list.split(|&b| b == b'\n')
        .map(|l| l.strip_suffix(b"\r").unwrap_or(l))
        .filter_map(|l| l.strip_prefix(b"file://"))
        .filter_map(|rest| {
            // `file://host/path`: the path starts at the next slash; an empty host is this one.
            let at = rest.iter().position(|&b| b == b'/')?;
            match &rest[..at] {
                b"" | b"localhost" => Some(PathBuf::from(OsString::from_vec(decode(&rest[at..])))),
                _ => None,
            }
        })
        .collect()
}

/// `%XX` escapes undone (bytes: a name need not be UTF-8).
fn decode(s: &[u8]) -> Vec<u8> {
    let hex = |b: u8| (b as char).to_digit(16).map(|d| d as u8);
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        match (
            s[i],
            s.get(i + 1).and_then(|&b| hex(b)),
            s.get(i + 2).and_then(|&b| hex(b)),
        ) {
            (b'%', Some(h), Some(l)) => {
                out.push(h << 4 | l);
                i += 3;
            }
            (b, ..) => {
                out.push(b);
                i += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_of_a_uri_list() {
        let list = b"# a comment\r\nfile:///home/u/a%20b.txt\r\nfile://localhost/tmp/x\r\nfile://other/y\r\nhttp://z/w\r\n\r\nfile:///caf%C3%A9\n";
        assert_eq!(
            paths_of(list),
            [
                PathBuf::from("/home/u/a b.txt"),
                PathBuf::from("/tmp/x"),
                PathBuf::from("/café"),
            ]
        );
    }

    #[test]
    fn non_utf8_names_and_bad_escapes_survive() {
        let got = paths_of(b"file:///a%FFb%zz%4");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].as_os_str().as_encoded_bytes(), b"/a\xFFb%zz%4");
    }
}
