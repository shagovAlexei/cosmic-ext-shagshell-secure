//! X / Y / Zmodem in a terminal tab: the system's lrzsz does the protocol. This module knows when
//! a server starts sending (so the app can hand the stream to `rz`) and which program to run
//! for each transfer. The app moves the bytes (`term::log::Tap`).

use std::path::{Path, PathBuf};

/// A Zmodem sender (`sz file` on the server) announces itself with `rz\r` and a ZRQINIT hex
/// header: `**` ZDLE `B00` and zeros.
const START: &[u8] = b"**\x18B00";

/// Watches the output for the start of a Zmodem send; the header may arrive in pieces.
#[derive(Default)]
pub struct Detector {
    tail: Vec<u8>,
}

impl Detector {
    /// `true`: `bytes` (with what came before) hold the start. The whole chunk belongs to the
    /// transfer then.
    pub fn feed(&mut self, bytes: &[u8]) -> bool {
        let has = |b: &[u8]| b.windows(START.len()).any(|w| w == START);
        let n = START.len() - 1;
        // Inside the new bytes, or across the join with the end of the earlier ones.
        let hit = has(bytes) || {
            let mut join = self.tail.clone();
            join.extend_from_slice(&bytes[..bytes.len().min(n)]);
            has(&join)
        };
        // Only the end that could still grow into the start is kept.
        match bytes.len() >= n {
            true => self.tail = bytes[bytes.len() - n..].to_vec(),
            false => {
                self.tail.extend_from_slice(bytes);
                let keep = self.tail.len().saturating_sub(n);
                self.tail.drain(..keep);
            }
        }
        if hit {
            self.tail.clear();
        }
        hit
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Proto {
    Zmodem,
    Ymodem,
    Xmodem,
}

/// What to run, and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    pub program: &'static str,
    pub args: Vec<String>,
    /// Received files go here.
    pub cwd: Option<PathBuf>,
}

/// The server sends: receive into `dir`. Xmodem has no file names: `file` is the name to write.
/// Files that are there are not overwritten (`-E` renames the new one), nothing leads out of
/// `dir` (`-R`).
pub fn receive(proto: Proto, dir: &Path, file: Option<&Path>) -> Command {
    let (program, mut args): (&'static str, Vec<String>) = match proto {
        Proto::Zmodem => (
            "rz",
            vec!["-b".into(), "-E".into(), "-R".into(), "-v".into()],
        ),
        Proto::Ymodem => ("rb", vec!["-E".into(), "-R".into(), "-v".into()]),
        Proto::Xmodem => ("rx", vec!["-b".into()]),
    };
    let cwd = match (proto, file) {
        (Proto::Xmodem, Some(f)) => {
            args.push(
                f.file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
            );
            f.parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(dir)
        }
        _ => dir,
    };
    Command {
        program,
        args,
        cwd: Some(cwd.to_path_buf()),
    }
}

/// The server receives: send `files` (Xmodem takes one).
pub fn send(proto: Proto, files: &[PathBuf]) -> Command {
    let (program, mut args): (&'static str, Vec<String>) = match proto {
        Proto::Zmodem => ("sz", vec!["-b".into()]),
        Proto::Ymodem => ("sb", Vec::new()),
        Proto::Xmodem => ("sx", vec!["-b".into()]),
    };
    let files = match proto {
        Proto::Xmodem => &files[..files.len().min(1)],
        _ => files,
    };
    args.push("--".into());
    args.extend(files.iter().map(|f| f.display().to_string()));
    Command {
        program,
        args,
        cwd: None,
    }
}

/// The programs a protocol needs (`--check`, greying the menu).
pub fn programs() -> [&'static str; 6] {
    ["sz", "rz", "sb", "rb", "sx", "rx"]
}

/// What the server is told to stop a transfer: CAN ×8 and backspaces to wipe them.
pub const CANCEL: &[u8] =
    b"\x18\x18\x18\x18\x18\x18\x18\x18\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08";

/// The last line of the program's progress / error text (`rz` ends with «Bytes received…»).
pub fn last_line(stderr: &str) -> Option<String> {
    stderr
        .split(['\r', '\n'])
        .map(str::trim)
        .rfind(|l| !l.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What `sz` really prints first (captured from lrzsz 0.12.21).
    const SZ: &[u8] = b"rz\r**\x18B00000000000000\r\x8a\x11";

    #[test]
    fn sees_the_start_of_a_send() {
        assert!(Detector::default().feed(SZ));
        assert!(!Detector::default().feed(b"hello rz, ** and B00 apart"));
        assert!(!Detector::default().feed(b"**\x18B01000000000000")); // the server wants a file
    }

    #[test]
    fn sees_it_cut_at_any_place() {
        for cut in 1..SZ.len() {
            let mut d = Detector::default();
            let (a, b) = SZ.split_at(cut);
            assert!(d.feed(a) || d.feed(b), "cut at {cut}");
        }
    }

    #[test]
    fn the_start_is_seen_once() {
        let mut d = Detector::default();
        assert!(d.feed(SZ));
        assert!(!d.feed(b"more output"));
    }

    #[test]
    fn receive_commands() {
        let dir = Path::new("/home/u/Downloads");
        let z = receive(Proto::Zmodem, dir, None);
        assert_eq!(
            (z.program, z.args.as_slice()),
            (
                "rz",
                &["-b".to_string(), "-E".into(), "-R".into(), "-v".into()][..]
            )
        );
        assert_eq!(z.cwd.as_deref(), Some(dir));
        let x = receive(Proto::Xmodem, dir, Some(Path::new("/tmp/out.bin")));
        assert_eq!(
            (x.program, x.args.last().map(String::as_str)),
            ("rx", Some("out.bin"))
        );
        assert_eq!(x.cwd.as_deref(), Some(Path::new("/tmp")));
        assert_eq!(receive(Proto::Ymodem, dir, None).program, "rb");
    }

    #[test]
    fn send_commands() {
        let f = [PathBuf::from("/a/x"), PathBuf::from("/a/-y")];
        assert_eq!(send(Proto::Zmodem, &f).args, ["-b", "--", "/a/x", "/a/-y"]);
        assert_eq!(send(Proto::Ymodem, &f).program, "sb");
        // a file name starting with « - » is a file, not an option; Xmodem sends one file
        assert_eq!(send(Proto::Xmodem, &f).args, ["-b", "--", "/a/x"]);
    }

    #[test]
    fn last_line_of_progress() {
        assert_eq!(
            last_line("a\rb\rBytes received:  6   BPS:98\r\n\r\n"),
            Some("Bytes received:  6   BPS:98".into())
        );
        assert_eq!(last_line("  \n"), None);
    }
}
