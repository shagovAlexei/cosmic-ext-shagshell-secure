//! An SFTP v3 client (draft-ietf-secsh-filexfer-02, what OpenSSH's sftp-server speaks) over any
//! byte pipe — here `ssh -s host sftp`. Blocking: one worker thread per SFTP tab owns it.
//!
//! Our own instead of `openssh-sftp-client`: there one file name that isn't UTF-8 kills the whole
//! connection (`ssh_format` reads paths with `String::from_utf8`), and reads aren't pipelined.
//! Names here are bytes, and transfers keep several requests in flight.

use std::collections::{BTreeMap, HashMap};
use std::ffi::OsString;
use std::io::{self, BufReader, BufWriter, ErrorKind, Read, Write};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

const INIT: u8 = 1;
const VERSION: u8 = 2;
const OPEN: u8 = 3;
const CLOSE: u8 = 4;
const READ: u8 = 5;
const WRITE: u8 = 6;
const LSTAT: u8 = 7;
const OPENDIR: u8 = 11;
const READDIR: u8 = 12;
const REMOVE: u8 = 13;
const MKDIR: u8 = 14;
const RMDIR: u8 = 15;
const REALPATH: u8 = 16;
const STAT: u8 = 17;
const RENAME: u8 = 18;
const STATUS: u8 = 101;
const HANDLE: u8 = 102;
const DATA: u8 = 103;
const NAME: u8 = 104;
const ATTRS: u8 = 105;

const FX_OK: u32 = 0;
const FX_EOF: u32 = 1;
const FX_NO_SUCH_FILE: u32 = 2;
const FX_PERMISSION_DENIED: u32 = 3;

const READ_FLAG: u32 = 0x01;
const WRITE_FLAG: u32 = 0x02;
const CREAT: u32 = 0x08;
const TRUNC: u32 = 0x10;

/// Bytes per READ / WRITE (OpenSSH's sftp-server takes up to 256 KiB).
const CHUNK: u32 = 64 * 1024;
/// Requests in flight during a transfer.
const WINDOW: usize = 16;
/// Largest packet we accept (a 64 KiB DATA, or a READDIR batch, is far below).
const MAX_PACKET: usize = 4 << 20;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Attrs {
    pub size: Option<u64>,
    pub perm: Option<u32>,
    pub mtime: Option<u32>,
}

impl Attrs {
    pub fn is_dir(&self) -> bool {
        self.perm.is_some_and(|p| p & 0o170000 == 0o040000)
    }

    pub fn is_link(&self) -> bool {
        self.perm.is_some_and(|p| p & 0o170000 == 0o120000)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteEntry {
    pub name: OsString,
    pub attrs: Attrs,
}

pub struct Client<R: Read, W: Write> {
    r: BufReader<R>,
    w: BufWriter<W>,
    next_id: u32,
}

/// A request's body being built.
struct Msg(Vec<u8>);

impl Msg {
    fn new(kind: u8, id: u32) -> Self {
        let mut m = Msg(vec![kind]);
        m.u32(id);
        m
    }
    fn u32(&mut self, v: u32) -> &mut Self {
        self.0.extend(v.to_be_bytes());
        self
    }
    fn u64(&mut self, v: u64) -> &mut Self {
        self.0.extend(v.to_be_bytes());
        self
    }
    fn bytes(&mut self, b: &[u8]) -> &mut Self {
        self.u32(b.len() as u32);
        self.0.extend(b);
        self
    }
    fn path(&mut self, p: &Path) -> &mut Self {
        self.bytes(p.as_os_str().as_bytes())
    }
}

/// An answer's body being read.
struct Body<'a>(&'a [u8]);

fn bad() -> io::Error {
    io::Error::new(ErrorKind::InvalidData, "bad SFTP packet")
}

impl Body<'_> {
    fn take(&mut self, n: usize) -> io::Result<&[u8]> {
        if self.0.len() < n {
            return Err(bad());
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a)
    }
    fn u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_be_bytes(
            self.take(4)?.try_into().map_err(|_| bad())?,
        ))
    }
    fn u64(&mut self) -> io::Result<u64> {
        Ok(u64::from_be_bytes(
            self.take(8)?.try_into().map_err(|_| bad())?,
        ))
    }
    fn bytes(&mut self) -> io::Result<Vec<u8>> {
        let n = self.u32()? as usize;
        Ok(self.take(n)?.to_vec())
    }
    fn attrs(&mut self) -> io::Result<Attrs> {
        let flags = self.u32()?;
        let mut a = Attrs::default();
        if flags & 0x1 != 0 {
            a.size = Some(self.u64()?);
        }
        if flags & 0x2 != 0 {
            self.take(8)?; // uid, gid
        }
        if flags & 0x4 != 0 {
            a.perm = Some(self.u32()?);
        }
        if flags & 0x8 != 0 {
            self.take(4)?; // atime
            a.mtime = Some(self.u32()?);
        }
        if flags & 0x8000_0000 != 0 {
            for _ in 0..self.u32()? * 2 {
                self.bytes()?;
            }
        }
        Ok(a)
    }
}

/// A STATUS answer that isn't OK, as an `io::Error` (EOF never gets here).
fn status_error(code: u32, msg: &[u8]) -> io::Error {
    let kind = match code {
        FX_NO_SUCH_FILE => ErrorKind::NotFound,
        FX_PERMISSION_DENIED => ErrorKind::PermissionDenied,
        _ => ErrorKind::Other,
    };
    let text = String::from_utf8_lossy(msg);
    match text.is_empty() {
        true => io::Error::new(kind, format!("SFTP error {code}")),
        false => io::Error::new(kind, text.into_owned()),
    }
}

fn cancelled() -> io::Error {
    io::Error::new(ErrorKind::Interrupted, "cancelled")
}

impl<R: Read, W: Write> Client<R, W> {
    /// INIT / VERSION: the server speaks v3.
    pub fn new(r: R, w: W) -> io::Result<Self> {
        let mut c = Self {
            r: BufReader::with_capacity(256 * 1024, r),
            w: BufWriter::with_capacity(256 * 1024, w),
            next_id: 1,
        };
        let mut init = Msg(vec![INIT]);
        init.u32(3);
        c.send(&init)?;
        c.w.flush()?;
        let (kind, _) = c.recv_raw()?;
        if kind != VERSION {
            return Err(bad());
        }
        Ok(c)
    }

    fn id(&mut self) -> u32 {
        self.next_id = self.next_id.wrapping_add(1);
        self.next_id
    }

    fn send(&mut self, m: &Msg) -> io::Result<()> {
        self.w.write_all(&(m.0.len() as u32).to_be_bytes())?;
        self.w.write_all(&m.0)
    }

    fn recv_raw(&mut self) -> io::Result<(u8, Vec<u8>)> {
        let mut len = [0; 4];
        self.r.read_exact(&mut len)?;
        let len = u32::from_be_bytes(len) as usize;
        if len == 0 || len > MAX_PACKET {
            return Err(bad());
        }
        let mut buf = vec![0; len];
        self.r.read_exact(&mut buf)?;
        let kind = buf[0];
        buf.remove(0);
        Ok((kind, buf))
    }

    /// An answer: its kind, its request id, the rest.
    fn recv(&mut self) -> io::Result<(u8, u32, Vec<u8>)> {
        let (kind, buf) = self.recv_raw()?;
        let mut b = Body(&buf);
        let id = b.u32()?;
        let rest = b.0.to_vec();
        Ok((kind, id, rest))
    }

    /// One request, its one answer.
    fn call(&mut self, m: &Msg) -> io::Result<(u8, Vec<u8>)> {
        self.send(m)?;
        self.w.flush()?;
        let (kind, _, body) = self.recv()?;
        Ok((kind, body))
    }

    fn expect_ok(&mut self, m: &Msg) -> io::Result<()> {
        match self.call(m)? {
            (STATUS, body) => {
                let mut b = Body(&body);
                match b.u32()? {
                    FX_OK => Ok(()),
                    code => Err(status_error(code, &b.bytes().unwrap_or_default())),
                }
            }
            _ => Err(bad()),
        }
    }

    fn expect(&mut self, m: &Msg, want: u8) -> io::Result<Vec<u8>> {
        match self.call(m)? {
            (k, body) if k == want => Ok(body),
            (STATUS, body) => {
                let mut b = Body(&body);
                let code = b.u32()?;
                Err(status_error(code, &b.bytes().unwrap_or_default()))
            }
            _ => Err(bad()),
        }
    }

    pub fn realpath(&mut self, p: &Path) -> io::Result<PathBuf> {
        let id = self.id();
        let body = self.expect(Msg::new(REALPATH, id).path(p), NAME)?;
        let mut b = Body(&body);
        if b.u32()? == 0 {
            return Err(bad());
        }
        Ok(PathBuf::from(OsString::from_vec(b.bytes()?)))
    }

    fn attrs_of(&mut self, kind: u8, p: &Path) -> io::Result<Attrs> {
        let id = self.id();
        let body = self.expect(Msg::new(kind, id).path(p), ATTRS)?;
        Body(&body).attrs()
    }

    /// Follows a symlink.
    pub fn stat(&mut self, p: &Path) -> io::Result<Attrs> {
        self.attrs_of(STAT, p)
    }

    pub fn lstat(&mut self, p: &Path) -> io::Result<Attrs> {
        self.attrs_of(LSTAT, p)
    }

    fn close(&mut self, handle: &[u8]) -> io::Result<()> {
        let id = self.id();
        self.expect_ok(Msg::new(CLOSE, id).bytes(handle))
    }

    /// The directory's entries, without `.` and `..`.
    pub fn read_dir(&mut self, p: &Path) -> io::Result<Vec<RemoteEntry>> {
        let id = self.id();
        let handle = self.expect(Msg::new(OPENDIR, id).path(p), HANDLE)?;
        let handle = Body(&handle).bytes()?;
        let mut out = Vec::new();
        let listed = loop {
            let id = self.id();
            match self.call(Msg::new(READDIR, id).bytes(&handle)) {
                Ok((NAME, body)) => {
                    let mut b = Body(&body);
                    for _ in 0..b.u32()? {
                        let name = b.bytes()?;
                        b.bytes()?; // longname
                        let attrs = b.attrs()?;
                        if name != b"." && name != b".." {
                            out.push(RemoteEntry {
                                name: OsString::from_vec(name),
                                attrs,
                            });
                        }
                    }
                }
                Ok((STATUS, body)) => {
                    let mut b = Body(&body);
                    break match b.u32()? {
                        FX_EOF => Ok(()),
                        code => Err(status_error(code, &b.bytes().unwrap_or_default())),
                    };
                }
                Ok(_) => break Err(bad()),
                Err(e) => return Err(e),
            }
        };
        let closed = self.close(&handle);
        listed.and(closed)?;
        Ok(out)
    }

    pub fn mkdir(&mut self, p: &Path) -> io::Result<()> {
        let id = self.id();
        self.expect_ok(Msg::new(MKDIR, id).path(p).u32(0))
    }

    pub fn rmdir(&mut self, p: &Path) -> io::Result<()> {
        let id = self.id();
        self.expect_ok(Msg::new(RMDIR, id).path(p))
    }

    pub fn remove(&mut self, p: &Path) -> io::Result<()> {
        let id = self.id();
        self.expect_ok(Msg::new(REMOVE, id).path(p))
    }

    pub fn rename(&mut self, from: &Path, to: &Path) -> io::Result<()> {
        let id = self.id();
        self.expect_ok(Msg::new(RENAME, id).path(from).path(to))
    }

    fn open(&mut self, p: &Path, flags: u32) -> io::Result<Vec<u8>> {
        let id = self.id();
        let body = self.expect(Msg::new(OPEN, id).path(p).u32(flags).u32(0), HANDLE)?;
        Body(&body).bytes()
    }

    /// The remote file into `out`; `progress(bytes so far)` false cancels (`Interrupted`).
    pub fn download(
        &mut self,
        p: &Path,
        out: &mut impl Write,
        mut progress: impl FnMut(u64) -> bool,
    ) -> io::Result<u64> {
        let handle = self.open(p, READ_FLAG)?;
        let r = self.pull(&handle, out, &mut progress);
        let closed = self.close(&handle);
        let n = r?;
        closed?;
        Ok(n)
    }

    fn pull(
        &mut self,
        handle: &[u8],
        out: &mut impl Write,
        progress: &mut impl FnMut(u64) -> bool,
    ) -> io::Result<u64> {
        let mut inflight: HashMap<u32, (u64, u32)> = HashMap::new();
        let mut ready: BTreeMap<u64, Vec<u8>> = BTreeMap::new();
        let (mut next, mut written) = (0u64, 0u64);
        let mut eof: Option<u64> = None;
        let mut failure: Option<io::Error> = None;
        loop {
            // No new requests once it failed or was cancelled: the answers in flight are drained.
            while failure.is_none() && inflight.len() < WINDOW && eof.is_none_or(|e| next < e) {
                let id = self.id();
                self.send(Msg::new(READ, id).bytes(handle).u64(next).u32(CHUNK))?;
                inflight.insert(id, (next, CHUNK));
                next += u64::from(CHUNK);
            }
            if inflight.is_empty() {
                break;
            }
            self.w.flush()?;
            let (kind, id, body) = self.recv()?;
            let (off, len) = inflight.remove(&id).ok_or_else(bad)?;
            let mut b = Body(&body);
            match kind {
                DATA => {
                    let data = b.bytes()?;
                    let got = data.len() as u32;
                    if got == 0 {
                        eof = Some(eof.map_or(off, |e| e.min(off)));
                    } else if got < len && failure.is_none() {
                        // A short read before the end: ask for the rest of that chunk.
                        let id = self.id();
                        let rest = off + u64::from(got);
                        self.send(Msg::new(READ, id).bytes(handle).u64(rest).u32(len - got))?;
                        inflight.insert(id, (rest, len - got));
                    }
                    if got > 0 {
                        ready.insert(off, data);
                    }
                }
                STATUS => match b.u32()? {
                    FX_EOF => eof = Some(eof.map_or(off, |e| e.min(off))),
                    code => {
                        failure.get_or_insert(status_error(code, &b.bytes().unwrap_or_default()));
                    }
                },
                _ => return Err(bad()),
            }
            while failure.is_none()
                && let Some(d) = ready.remove(&written)
            {
                out.write_all(&d)?;
                written += d.len() as u64;
                if !progress(written) {
                    failure = Some(cancelled());
                }
            }
        }
        match failure {
            Some(e) => Err(e),
            None => Ok(written),
        }
    }

    /// `input` into the remote file (created or truncated); `progress` false cancels.
    pub fn upload(
        &mut self,
        input: &mut impl Read,
        p: &Path,
        mut progress: impl FnMut(u64) -> bool,
    ) -> io::Result<u64> {
        let handle = self.open(p, WRITE_FLAG | CREAT | TRUNC)?;
        let r = self.push(&handle, input, &mut progress);
        let closed = self.close(&handle);
        let n = r?;
        closed?;
        Ok(n)
    }

    fn push(
        &mut self,
        handle: &[u8],
        input: &mut impl Read,
        progress: &mut impl FnMut(u64) -> bool,
    ) -> io::Result<u64> {
        let mut inflight: HashMap<u32, u64> = HashMap::new();
        let (mut sent, mut acked) = (0u64, 0u64);
        let mut done_reading = false;
        let mut failure: Option<io::Error> = None;
        let mut buf = vec![0; CHUNK as usize];
        loop {
            while failure.is_none() && !done_reading && inflight.len() < WINDOW {
                let n = read_full(input, &mut buf)?;
                if n == 0 {
                    done_reading = true;
                    break;
                }
                let id = self.id();
                self.send(Msg::new(WRITE, id).bytes(handle).u64(sent).bytes(&buf[..n]))?;
                inflight.insert(id, n as u64);
                sent += n as u64;
            }
            if inflight.is_empty() {
                break;
            }
            self.w.flush()?;
            let (kind, id, body) = self.recv()?;
            let n = inflight.remove(&id).ok_or_else(bad)?;
            let mut b = Body(&body);
            match (kind, b.u32()?) {
                (STATUS, FX_OK) => {
                    acked += n;
                    if failure.is_none() && !progress(acked) {
                        failure = Some(cancelled());
                    }
                }
                (STATUS, code) => {
                    failure.get_or_insert(status_error(code, &b.bytes().unwrap_or_default()));
                }
                _ => return Err(bad()),
            }
        }
        match failure {
            Some(e) => Err(e),
            None => Ok(acked),
        }
    }
}

/// Fill `buf` unless the input ends first (a pipe may give less per read).
fn read_full(input: &mut impl Read, buf: &mut [u8]) -> io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match input.read(&mut buf[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

    const SERVER: &str = "/usr/lib/openssh/sftp-server";

    /// A real sftp-server on pipes (what `ssh -s host sftp` gives), no ssh.
    fn server() -> Option<(Child, Client<ChildStdout, ChildStdin>)> {
        if !Path::new(SERVER).exists() {
            eprintln!("no {SERVER}: skipped");
            return None;
        }
        let mut child = Command::new(SERVER)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let (w, r) = (child.stdin.take().unwrap(), child.stdout.take().unwrap());
        Some((child, Client::new(r, w).unwrap()))
    }

    #[test]
    fn list_non_utf8_names() {
        let Some((_s, mut c)) = server() else { return };
        let d = tempfile::tempdir().unwrap();
        let bad = OsString::from_vec(b"bad\xff\xfename".to_vec());
        std::fs::write(d.path().join(&bad), "x").unwrap();
        std::fs::create_dir(d.path().join("sub")).unwrap();
        let mut names: Vec<_> = c
            .read_dir(d.path())
            .unwrap()
            .into_iter()
            .map(|e| e.name)
            .collect();
        names.sort();
        assert_eq!(names, [bad.clone(), OsString::from("sub")]);
        // Still usable after it: the connection survived.
        let mut out = Vec::new();
        c.download(&d.path().join(&bad), &mut out, |_| true)
            .unwrap();
        assert_eq!(out, b"x");
    }

    #[test]
    fn stat_dir_and_file() {
        let Some((_s, mut c)) = server() else { return };
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("f"), "12345").unwrap();
        let f = c.stat(&d.path().join("f")).unwrap();
        assert_eq!(f.size, Some(5));
        assert!(!f.is_dir() && f.mtime.is_some());
        assert!(c.stat(d.path()).unwrap().is_dir());
    }

    #[test]
    fn mkdir_rename_remove() {
        let Some((_s, mut c)) = server() else { return };
        let d = tempfile::tempdir().unwrap();
        let (a, b) = (d.path().join("a"), d.path().join("b"));
        c.mkdir(&a).unwrap();
        c.rename(&a, &b).unwrap();
        assert!(b.is_dir() && !a.exists());
        std::fs::write(b.join("f"), "").unwrap();
        c.remove(&b.join("f")).unwrap();
        c.rmdir(&b).unwrap();
        assert!(!b.exists());
    }

    #[test]
    fn round_trip_5mb() {
        let Some((_s, mut c)) = server() else { return };
        let d = tempfile::tempdir().unwrap();
        // Not a multiple of the chunk: the last read is short.
        let data: Vec<u8> = (0..5_000_123u32).map(|i| (i % 251) as u8).collect();
        let remote = d.path().join("up.bin");
        let mut seen = 0;
        let n = c
            .upload(&mut &data[..], &remote, |n| {
                seen = n;
                true
            })
            .unwrap();
        assert_eq!((n, seen), (data.len() as u64, data.len() as u64));
        assert_eq!(std::fs::read(&remote).unwrap(), data);
        let mut back = Vec::new();
        assert_eq!(
            c.download(&remote, &mut back, |_| true).unwrap(),
            data.len() as u64
        );
        assert_eq!(back, data);
        // An empty file both ways.
        let empty = d.path().join("empty");
        c.upload(&mut &b""[..], &empty, |_| true).unwrap();
        let mut out = Vec::new();
        assert_eq!(c.download(&empty, &mut out, |_| true).unwrap(), 0);
    }

    #[test]
    fn download_cancel() {
        let Some((_s, mut c)) = server() else { return };
        let d = tempfile::tempdir().unwrap();
        let remote = d.path().join("big");
        std::fs::write(&remote, vec![1u8; 3 << 20]).unwrap();
        let mut out = Vec::new();
        let e = c.download(&remote, &mut out, |n| n < 1 << 20).unwrap_err();
        assert_eq!(e.kind(), ErrorKind::Interrupted);
        assert!(out.len() < 3 << 20);
        // The answers in flight were drained: the next call works.
        assert!(c.stat(&remote).is_ok());
    }

    #[test]
    fn upload_cancel() {
        let Some((_s, mut c)) = server() else { return };
        let d = tempfile::tempdir().unwrap();
        let remote = d.path().join("up");
        let data = vec![2u8; 3 << 20];
        let e = c
            .upload(&mut &data[..], &remote, |n| n < 1 << 20)
            .unwrap_err();
        assert_eq!(e.kind(), ErrorKind::Interrupted);
        assert!(std::fs::metadata(&remote).unwrap().len() < 3 << 20);
        // The acks in flight were drained: the next call works.
        c.remove(&remote).unwrap();
    }

    #[test]
    fn missing_is_not_found() {
        let Some((_s, mut c)) = server() else { return };
        let d = tempfile::tempdir().unwrap();
        let none = d.path().join("none");
        assert_eq!(c.stat(&none).unwrap_err().kind(), ErrorKind::NotFound);
        assert_eq!(c.read_dir(&none).unwrap_err().kind(), ErrorKind::NotFound);
        let mut out = Vec::new();
        let e = c.download(&none, &mut out, |_| true).unwrap_err();
        assert_eq!(e.kind(), ErrorKind::NotFound);
    }

    #[test]
    fn realpath_dot() {
        let Some((_s, mut c)) = server() else { return };
        let here = std::env::current_dir().unwrap();
        assert_eq!(c.realpath(Path::new(".")).unwrap(), here);
    }
}
