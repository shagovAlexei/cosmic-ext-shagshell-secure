//! ssh's askpass: what a prompt asks for, and the helper ↔ app protocol over the app's unix socket
//! (one JSON line each way). ssh runs `shagshell "<prompt>"` with `SSH_ASKPASS_REQUIRE=force`.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Password,
    Passphrase,
    /// «Are you sure you want to continue connecting (yes/no/[fingerprint])?»
    HostKey,
    /// Keyboard-interactive questions (OTP codes…): asked, never saved.
    Other,
}

pub fn classify(prompt: &str) -> Kind {
    let p = prompt.to_lowercase();
    if p.contains("(yes/no") {
        Kind::HostKey
    } else if p.contains("passphrase") {
        Kind::Passphrase
    } else if p.trim_end().ends_with("password:") {
        Kind::Password
    } else {
        Kind::Other
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    pub prompt: String,
    /// `SHAGSHELL_SESSION`: the saved session the ssh runs for.
    pub session: Option<String>,
    /// `SHAGSHELL_TAB`.
    pub tab: Option<u64>,
    /// The ssh process (the helper's parent): one connection's prompts.
    pub ppid: u32,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reply {
    Answer(String),
    Cancel,
}

/// Never the secret, whatever prints it.
impl std::fmt::Debug for Reply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Reply::Answer(_) => f.write_str("Answer(…)"),
            Reply::Cancel => f.write_str("Cancel"),
        }
    }
}

/// The app's socket: one per running instance, next to the ControlMaster sockets.
pub fn socket_path(control_dir: &Path, pid: u32) -> PathBuf {
    control_dir.join(format!("askpass-{pid}.sock"))
}

fn line<T: Serialize>(w: &mut impl Write, v: &T) -> io::Result<()> {
    let mut s = serde_json::to_string(v)?;
    s.push('\n');
    w.write_all(s.as_bytes())?;
    w.flush()
}

fn read<T: for<'de> Deserialize<'de>>(r: &mut impl BufRead) -> io::Result<T> {
    let mut s = String::new();
    if r.read_line(&mut s)? == 0 {
        return Err(io::ErrorKind::UnexpectedEof.into());
    }
    Ok(serde_json::from_str(&s)?)
}

pub fn read_request(r: &mut impl BufRead) -> io::Result<Request> {
    read(r)
}

pub fn write_reply(w: &mut impl Write, reply: &Reply) -> io::Result<()> {
    line(w, reply)
}

/// The helper's side: ask the app, wait for its answer (as long as the user takes).
pub fn ask(sock: &Path, req: &Request) -> io::Result<Reply> {
    let mut s = std::os::unix::net::UnixStream::connect(sock)?;
    line(&mut s, req)?;
    read(&mut BufReader::new(s))
}

/// `shagshell "<prompt>"` run by ssh: ask the app at `sock`, print the answer. The exit code ssh
/// reads: 0 answered, 1 cancelled or no app.
pub fn run_helper(sock: &Path, prompt: &str) -> i32 {
    let req = Request {
        prompt: prompt.to_string(),
        session: std::env::var("SHAGSHELL_SESSION").ok(),
        tab: std::env::var("SHAGSHELL_TAB")
            .ok()
            .and_then(|t| t.parse().ok()),
        ppid: std::os::unix::process::parent_id(),
    };
    match ask(sock, &req) {
        Ok(Reply::Answer(a)) => {
            println!("{a}");
            0
        }
        Ok(Reply::Cancel) => 1,
        Err(e) => {
            eprintln!("shagshell askpass: {}: {e}", sock.display());
            1
        }
    }
}

/// Cancel: end that ssh (else it sends empty passwords for its other tries — a lockout on network
/// gear). Only if it is ours: its environment names our socket. True: signalled.
pub fn abort(ppid: u32, sock: &Path) -> bool {
    let want = format!("SHAGSHELL_ASKPASS_SOCK={}", sock.display());
    let ours = std::fs::read(format!("/proc/{ppid}/environ"))
        .is_ok_and(|env| env.split(|&b| b == 0).any(|v| v == want.as_bytes()));
    ours && std::process::Command::new("kill")
        .args(["-TERM", &ppid.to_string()])
        .status()
        .is_ok_and(|s| s.success())
}

/// Prompts already answered from the keyring, per ssh process: the second one means the saved
/// password was wrong (ask the user, or ssh gets the same wrong one three times).
/// Also the ssh processes the user cancelled: ssh takes a failed askpass for an empty answer and
/// asks again (three times), Cancel must end the connection as SecureCRT's does.
// ponytail: never forgotten; a reused pid of a later ssh would be refused — prune when it bites.
#[derive(Default)]
pub struct Tries {
    seen: HashSet<(u32, Kind)>,
    cancelled: HashSet<u32>,
}

impl Tries {
    /// True the first time for this ssh and kind.
    pub fn first(&mut self, ppid: u32, kind: Kind) -> bool {
        self.seen.insert((ppid, kind))
    }

    pub fn cancel(&mut self, ppid: u32) {
        self.cancelled.insert(ppid);
    }

    /// Its prompts are answered Cancel at once.
    pub fn cancelled(&self, ppid: u32) -> bool {
        self.cancelled.contains(&ppid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOST_KEY: &str = "The authenticity of host '[127.0.0.1]:2222 ([127.0.0.1]:2222)' can't \
        be established.\nED25519 key fingerprint is SHA256:QMliS7/ye2Z8w+uxbUUp2cV3NnhiLNjHzGQuGM6xADg.\n\
        This key is not known by any other names.\n\
        Are you sure you want to continue connecting (yes/no/[fingerprint])? ";

    #[test]
    fn classify_real_prompts() {
        assert_eq!(classify("home@192.168.0.20's password: "), Kind::Password);
        assert_eq!(classify("(admin@sw1) Password: "), Kind::Password);
        assert_eq!(
            classify("Enter passphrase for key '/home/u/.ssh/id_ed25519': "),
            Kind::Passphrase
        );
        assert_eq!(classify(HOST_KEY), Kind::HostKey);
        assert_eq!(classify("Verification code: "), Kind::Other);
        assert_eq!(
            classify("Password change required. New password? "),
            Kind::Other
        );
    }

    #[test]
    fn request_round_trip() {
        let req = Request {
            prompt: HOST_KEY.into(),
            session: Some("lab/web".into()),
            tab: Some(3),
            ppid: 42,
        };
        let mut buf = Vec::new();
        line(&mut buf, &req).unwrap();
        assert_eq!(buf.iter().filter(|&&b| b == b'\n').count(), 1);
        assert_eq!(read_request(&mut &buf[..]).unwrap(), req);
    }

    #[test]
    fn reply_debug_hides_secret() {
        let s = format!("{:?}", Reply::Answer("hunter2".into()));
        assert!(!s.contains("hunter2"), "{s}");
    }

    #[test]
    fn ask_over_a_socket() {
        let d = tempfile::tempdir().unwrap();
        let sock = socket_path(d.path(), 7);
        let l = std::os::unix::net::UnixListener::bind(&sock).unwrap();
        let app = std::thread::spawn(move || {
            let (s, _) = l.accept().unwrap();
            let req = read_request(&mut BufReader::new(&s)).unwrap();
            write_reply(&mut &s, &Reply::Answer(format!("for {}", req.ppid))).unwrap();
        });
        let req = Request {
            prompt: "p".into(),
            session: None,
            tab: None,
            ppid: 9,
        };
        assert_eq!(ask(&sock, &req).unwrap(), Reply::Answer("for 9".into()));
        app.join().unwrap();
        // The app gone: an error, not a hang.
        assert!(ask(&d.path().join("none.sock"), &req).is_err());
    }

    #[test]
    fn tries_second_is_retry() {
        let mut t = Tries::default();
        assert!(t.first(1, Kind::Password));
        assert!(!t.first(1, Kind::Password));
        assert!(t.first(2, Kind::Password)); // another tab's ssh
        assert!(t.first(1, Kind::Passphrase));
    }

    #[test]
    fn abort_only_our_ssh() {
        let sock = Path::new("/run/test/askpass-1.sock");
        let spawn = |s: &str| {
            std::process::Command::new("sleep")
                .arg("30")
                .env("SHAGSHELL_ASKPASS_SOCK", s)
                .spawn()
                .unwrap()
        };
        // Just spawned, it may not have exec'd yet (its /proc environ still ours).
        let settle = || std::thread::sleep(std::time::Duration::from_millis(200));
        let mut other = spawn("/run/test/askpass-2.sock");
        settle();
        assert!(!abort(other.id(), sock));
        let mut ours = spawn("/run/test/askpass-1.sock");
        settle();
        assert!(abort(ours.id(), sock));
        assert!(!ours.wait().unwrap().success());
        other.kill().unwrap();
        let _ = other.wait();
    }

    #[test]
    fn regression_cancel_ends_the_connection() {
        let mut t = Tries::default();
        assert!(!t.cancelled(5));
        t.cancel(5);
        assert!(t.cancelled(5));
        assert!(!t.cancelled(6));
    }
}
