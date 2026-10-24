//! Notes: text / markdown files shown in a tab. The first one (a cheat sheet) is made on first
//! use; the rest is plain file work, checked here and tested.

use crate::session::store::valid_name;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

/// A bigger file is not a note.
pub const MAX_BYTES: u64 = 5 * 1024 * 1024;

const CHEAT_EN: &str = include_str!("../notes/cheatsheet.en.md");
const CHEAT_RU: &str = include_str!("../notes/cheatsheet.ru.md");

/// `~/.config/shagshell/notes`.
pub fn dir() -> PathBuf {
    crate::buttonbar::default_path().with_file_name("notes")
}

/// The main note in `dir`, made with the cheat sheet when missing (never written over).
pub fn ensure_default(dir: &Path, russian: bool) -> io::Result<PathBuf> {
    let (name, text) = match russian {
        true => ("Заметки.md", CHEAT_RU),
        false => ("Notes.md", CHEAT_EN),
    };
    let path = dir.join(name);
    // A folder with something in it (the note renamed, others added) gets no second cheat sheet.
    let empty = std::fs::read_dir(dir).is_ok_and(|mut d| d.next().is_none()) || !dir.exists();
    if !path.exists() && empty {
        std::fs::create_dir_all(dir)?;
        std::fs::write(&path, text)?;
    }
    Ok(path)
}

#[derive(Debug, PartialEq, Eq)]
pub enum ReadError {
    TooBig,
    NotText,
    Io(String),
}

/// A file's text; refused when big, binary or not UTF-8.
pub fn read(path: &Path) -> Result<String, ReadError> {
    let io = |e: io::Error| ReadError::Io(e.to_string());
    let meta = std::fs::metadata(path).map_err(io)?;
    if meta.len() > MAX_BYTES {
        return Err(ReadError::TooBig);
    }
    let bytes = std::fs::read(path).map_err(io)?;
    match String::from_utf8(bytes) {
        Ok(t) if !t.contains('\0') => Ok(t),
        _ => Err(ReadError::NotText),
    }
}

pub fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "md" | "markdown"))
}

/// The file renamed to `name` in the same folder; a name without an extension keeps the old one.
/// A taken name is `AlreadyExists`, a bad one `InvalidFilename`.
pub fn rename(path: &Path, name: &str) -> io::Result<PathBuf> {
    let name = name.trim();
    valid_name(name, false)
        .map_err(|e| io::Error::new(ErrorKind::InvalidFilename, format!("{e:?}")))?;
    let mut file = name.to_string();
    let known = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "md" | "markdown" | "txt"));
    if !known && let Some(ext) = path.extension() {
        file = format!("{name}.{}", ext.to_string_lossy());
    }
    let to = path.with_file_name(file);
    if to == path {
        return Ok(to);
    }
    if to.exists() {
        return Err(ErrorKind::AlreadyExists.into());
    }
    std::fs::rename(path, &to)?;
    Ok(to)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("shagshell-notes-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn default_note_is_made_once_and_never_overwritten() {
        let d = tmp("def");
        let p = ensure_default(&d, false).unwrap();
        assert!(p.ends_with("Notes.md"));
        assert!(std::fs::read_to_string(&p).unwrap().contains("ssh -J"));
        std::fs::write(&p, "mine").unwrap();
        ensure_default(&d, false).unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "mine");
        // Something is in the folder: no second cheat sheet next to it.
        let ru = ensure_default(&d, true).unwrap();
        assert!(!ru.exists());
        std::fs::remove_file(&p).unwrap();
        std::fs::write(d.join("Work.md"), "x").unwrap();
        assert!(!ensure_default(&d, false).unwrap().exists());
        let fresh = d.join("sub");
        assert!(
            ensure_default(&fresh, true)
                .unwrap()
                .ends_with("Заметки.md")
        );
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn read_refuses_binary_big_and_not_utf8() {
        let d = tmp("read");
        let f = d.join("a.txt");
        std::fs::write(&f, "héllo").unwrap();
        assert_eq!(read(&f).as_deref(), Ok("héllo"));
        std::fs::write(&f, [0xff, 0xfe, 0x41]).unwrap();
        assert_eq!(read(&f), Err(ReadError::NotText));
        std::fs::write(&f, b"a\0b").unwrap();
        assert_eq!(read(&f), Err(ReadError::NotText));
        std::fs::write(&f, vec![b'a'; MAX_BYTES as usize + 1]).unwrap();
        assert_eq!(read(&f), Err(ReadError::TooBig));
        assert!(matches!(read(&d.join("none")), Err(ReadError::Io(_))));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn markdown_by_extension() {
        assert!(is_markdown(Path::new("/x/a.md")));
        assert!(is_markdown(Path::new("a.MARKDOWN")));
        assert!(!is_markdown(Path::new("a.txt")));
        assert!(!is_markdown(Path::new("md")));
    }

    #[test]
    fn rename_keeps_extension_and_refuses_bad_or_taken() {
        let d = tmp("ren");
        let a = d.join("a.md");
        std::fs::write(&a, "x").unwrap();
        std::fs::write(d.join("b.md"), "y").unwrap();
        assert_eq!(
            rename(&a, "b").unwrap_err().kind(),
            ErrorKind::AlreadyExists
        );
        assert_eq!(
            rename(&a, "").unwrap_err().kind(),
            ErrorKind::InvalidFilename
        );
        assert_eq!(
            rename(&a, "x/y").unwrap_err().kind(),
            ErrorKind::InvalidFilename
        );
        assert_eq!(
            rename(&a, ".hid").unwrap_err().kind(),
            ErrorKind::InvalidFilename
        );
        assert_eq!(rename(&a, "a").unwrap(), a);
        let v = rename(&a, "ideas v1.2").unwrap();
        assert_eq!(v, d.join("ideas v1.2.md"));
        let a = rename(&v, "a.md").unwrap();
        let c = rename(&a, "plan").unwrap();
        assert_eq!(c, d.join("plan.md"));
        assert!(c.exists() && !a.exists());
        let t = rename(&c, "plan.txt").unwrap();
        assert_eq!(t, d.join("plan.txt"));
        let _ = std::fs::remove_dir_all(d);
    }
}
