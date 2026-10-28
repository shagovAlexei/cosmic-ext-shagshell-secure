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
    /// `SHAGSHELL_CREDENTIAL`: the session's credential set (its password is the set's).
    #[serde(default)]
    pub credential: Option<String>,
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
        credential: std::env::var("SHAGSHELL_CREDENTIAL").ok(),
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

/// OpenSSH asking for a key's passphrase in its own words, for the session's key (`identity`,
/// absolute) or, without one, for an existing `~/.ssh/id_*`. A server's keyboard-interactive
/// question reaches askpass too, worded as it likes: it never gets the saved passphrase.
pub fn passphrase_asked_by_ssh(prompt: &str, identity: Option<&Path>, home: &Path) -> bool {
    let Some(key) = (prompt.trim_end())
        .strip_prefix("Enter passphrase for key '")
        .and_then(|r| r.strip_suffix("':"))
        .map(Path::new)
    else {
        return false;
    };
    match identity {
        Some(i) => key == i,
        None => {
            key.parent() == Some(&home.join(".ssh"))
                && (key.file_name().and_then(|n| n.to_str())).is_some_and(|n| n.starts_with("id_"))
                && key.is_file()
        }
    }
}

/// `name`'s value in a `/proc/<pid>/environ` (NUL-separated `NAME=value`).
pub fn env_var(env: &[u8], name: &str) -> Option<String> {
    env.split(|&b| b == 0).find_map(|v| {
        let rest = v.strip_prefix(name.as_bytes())?.strip_prefix(b"=")?;
        Some(String::from_utf8_lossy(rest).into_owned())
    })
}

/// The pid of the process at the other end of a local socket (`SO_PEERCRED`).
pub fn peer_pid(s: &std::os::unix::net::UnixStream) -> Option<u32> {
    use std::os::fd::AsRawFd;
    let mut c = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: `c` and `len` are ours, `len` is `c`'s size; the fd is open while `s` lives.
    let r = unsafe {
        libc::getsockopt(
            s.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&raw mut c).cast(),
            &mut len,
        )
    };
    (r == 0 && c.pid > 0).then(|| c.pid.unsigned_abs())
}

/// The key an ssh was started with (`-i path` in its `/proc/<pid>/cmdline`, before `--`).
pub fn identity_in(cmdline: &[u8]) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    let args: Vec<&[u8]> = cmdline.split(|&b| b == 0).collect();
    let end = args.iter().position(|a| *a == b"--").unwrap_or(args.len());
    let at = args[..end].iter().position(|a| *a == b"-i")?;
    let p = args.get(at + 1).filter(|_| at + 1 < end)?;
    Some(PathBuf::from(std::ffi::OsStr::from_bytes(p)))
}

/// The request with what the helper said about itself replaced by what the processes say:
/// `peer` is the helper (from the socket), its parent the ssh, whose environment the tab set.
/// None: not a helper started by an ssh with our environment.
pub fn checked(req: Request, peer: u32, sock: &Path) -> Option<Request> {
    let ssh = parent_of(peer)?;
    let env = std::fs::read(format!("/proc/{ssh}/environ")).ok()?;
    let ours = env_var(&env, "SHAGSHELL_ASKPASS_SOCK").is_some_and(|s| Path::new(&s) == sock);
    ours.then(|| Request {
        prompt: req.prompt,
        session: env_var(&env, "SHAGSHELL_SESSION"),
        credential: env_var(&env, "SHAGSHELL_CREDENTIAL"),
        tab: env_var(&env, "SHAGSHELL_TAB").and_then(|t| t.parse().ok()),
        ppid: ssh,
    })
}

/// Whose saved secret answers this prompt, if any: a password of a session with a credential set
/// is the set's; others the session's. `direct`: asked by the ssh a tab started (see `direct`) —
/// a ProxyJump ssh talks to another host and gets none. `ssh_key`: a passphrase prompt is
/// OpenSSH's own for the session's key (`passphrase_asked_by_ssh`).
pub fn owner(
    req: &Request,
    kind: Kind,
    direct: bool,
    ssh_key: bool,
) -> Option<crate::keyring::Owner> {
    use crate::keyring::Owner;
    if !direct || !matches!(kind, Kind::Password | Kind::Passphrase) {
        return None;
    }
    if kind == Kind::Passphrase && !ssh_key {
        return None;
    }
    match (&req.credential, &req.session) {
        (Some(c), _) if kind == Kind::Password => Some(Owner::Credential(c.clone())),
        (_, Some(s)) => Some(Owner::Session(s.clone())),
        _ => None,
    }
}

/// Prompts already answered from the keyring, per ssh process: the second one means the saved
/// password was wrong (ask the user, or ssh gets the same wrong one three times).
/// Also the ssh processes the user cancelled: ssh takes a failed askpass for an empty answer and
/// asks again (three times), Cancel must end the connection as SecureCRT's does.
/// And the answers the user saved: if ssh asks again, the saved one was wrong.
#[derive(Default)]
pub struct Tries {
    seen: HashSet<(u32, Kind)>,
    cancelled: HashSet<u32>,
    saved: HashSet<(u32, Kind)>,
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

    /// The answer to this prompt was saved to the keyring.
    pub fn saved(&mut self, ppid: u32, kind: Kind) {
        self.saved.insert((ppid, kind));
    }

    /// ssh asks again after a saved answer: it was wrong (the retry starts with «Save» on, and
    /// unticking it removes the wrong one).
    pub fn was_saved(&self, ppid: u32, kind: Kind) -> bool {
        self.saved.contains(&(ppid, kind))
    }

    /// Ended ssh processes forgotten: a later one with the same pid starts afresh.
    pub fn forget_dead(&mut self, alive: impl Fn(u32) -> bool) {
        self.seen.retain(|(p, _)| alive(*p));
        self.saved.retain(|(p, _)| alive(*p));
        self.cancelled.retain(|p| alive(*p));
    }
}

/// A process runs (`/proc/<pid>`).
pub fn alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

/// Its parent process (`/proc/<pid>/stat`, field 4).
pub fn parent_of(pid: u32) -> Option<u32> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // `pid (comm) state ppid …`: comm may hold spaces and parens, so after the last ')'.
    let rest = &stat[stat.rfind(')')? + 1..];
    rest.split_whitespace().nth(1)?.parse().ok()
}

/// The ssh a tab started (a child of the app), not one it runs itself (`-J` / ProxyJump: another
/// host, whose prompts must never get this session's saved password).
pub fn direct(ppid: u32) -> bool {
    parent_of(ppid) == Some(std::process::id())
}

/// Sockets left by instances that are gone (closed by the window's ×, or crashed).
pub fn clean_stale(control_dir: &Path) {
    let Ok(dir) = std::fs::read_dir(control_dir) else {
        return;
    };
    for e in dir.flatten() {
        let name = e.file_name();
        let pid = (name.to_str()).and_then(|n| {
            n.strip_prefix("askpass-")?
                .strip_suffix(".sock")?
                .parse()
                .ok()
        });
        if let Some(pid) = pid
            && !alive(pid)
        {
            let _ = std::fs::remove_file(e.path());
        }
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
            credential: None,
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
            credential: None,
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
    fn regression_server_question_gets_no_passphrase() {
        // A server's keyboard-interactive question reaches askpass too: «Enter passphrase:»
        // from a hacked router got the session's saved key passphrase.
        let home = Path::new("/home/u");
        let key = Path::new("/home/u/keys/r1");
        let ok = |p: &str, id: Option<&Path>| passphrase_asked_by_ssh(p, id, home);
        assert!(ok(
            "Enter passphrase for key '/home/u/keys/r1': ",
            Some(key)
        ));
        assert!(!ok("Enter passphrase: ", Some(key)));
        assert!(!ok(
            "Enter passphrase for key '/home/u/.ssh/id_rsa': ",
            Some(key)
        ));
        assert!(!ok("Your passphrase please: ", None));
        // No key of the session's own: one of ~/.ssh/id_*, that exists.
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir(d.path().join(".ssh")).unwrap();
        std::fs::write(d.path().join(".ssh/id_ed25519"), "k").unwrap();
        let p = format!(
            "Enter passphrase for key '{}/.ssh/id_ed25519': ",
            d.path().display()
        );
        assert!(passphrase_asked_by_ssh(&p, None, d.path()));
        let p = format!(
            "Enter passphrase for key '{}/.ssh/id_rsa': ",
            d.path().display()
        );
        assert!(!passphrase_asked_by_ssh(&p, None, d.path()), "no such file");
    }

    #[test]
    fn regression_request_facts_come_from_the_processes() {
        // The helper said who it was (ppid, session): any program could ask for any session's
        // password. Now they are read from the ssh's environment.
        let env = b"A=1\0SHAGSHELL_SESSION=lab/r1\0SHAGSHELL_TAB=7\0SHAGSHELL_ASKPASS_SOCK=/s\0";
        assert_eq!(env_var(env, "SHAGSHELL_SESSION").as_deref(), Some("lab/r1"));
        assert_eq!(env_var(env, "SHAGSHELL_TAB").as_deref(), Some("7"));
        assert_eq!(env_var(env, "SHAGSHELL_CREDENTIAL"), None);
        assert_eq!(env_var(env, "SHAGSHELL"), None, "a prefix is not a name");
        let cmd = b"ssh\0-p\x0022\0-i\0/home/u/k\0--\0h\0";
        assert_eq!(identity_in(cmd), Some(PathBuf::from("/home/u/k")));
        assert_eq!(
            identity_in(b"ssh\0--\0-i\0x\0"),
            None,
            "after -- is the host"
        );
        // A socket pair: the peer is this process.
        let (a, _b) = std::os::unix::net::UnixStream::pair().unwrap();
        assert_eq!(peer_pid(&a), Some(std::process::id()));
    }

    #[test]
    fn owner_of_a_prompt() {
        use crate::keyring::Owner;
        let mut req = Request {
            prompt: String::new(),
            session: Some("lab/r1".into()),
            credential: None,
            tab: None,
            ppid: 1,
        };
        let s = Some(Owner::Session("lab/r1".into()));
        assert_eq!(owner(&req, Kind::Password, true, false), s);
        assert_eq!(owner(&req, Kind::Password, false, false), None); // ProxyJump
        assert_eq!(owner(&req, Kind::HostKey, true, false), None);
        req.credential = Some("adm".into());
        assert_eq!(
            owner(&req, Kind::Password, true, false),
            Some(Owner::Credential("adm".into()))
        );
        assert_eq!(owner(&req, Kind::Passphrase, true, true), s); // the key is the session's
        assert_eq!(owner(&req, Kind::Passphrase, true, false), None); // a server's question
        req.session = None;
        assert_eq!(owner(&req, Kind::Passphrase, true, true), None);
    }

    #[test]
    fn saved_then_asked_again() {
        let mut t = Tries::default();
        assert!(!t.was_saved(1, Kind::Password));
        t.saved(1, Kind::Password);
        assert!(t.was_saved(1, Kind::Password));
        assert!(!t.was_saved(1, Kind::Passphrase));
        assert!(!t.was_saved(2, Kind::Password));
    }

    #[test]
    fn regression_reused_pid_starts_afresh() {
        let mut t = Tries::default();
        t.first(1, Kind::Password);
        t.cancel(1);
        t.saved(1, Kind::Password);
        t.forget_dead(|p| p != 1);
        assert!(!t.cancelled(1));
        assert!(!t.was_saved(1, Kind::Password));
        assert!(t.first(1, Kind::Password));
    }

    #[test]
    fn regression_jump_ssh_is_not_direct() {
        let mut child = std::process::Command::new("sh")
            .args(["-c", "sleep 30 & wait"])
            .spawn()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert_eq!(parent_of(child.id()), Some(std::process::id()));
        assert!(direct(child.id()));
        // Its own child (as ssh's ProxyJump ssh): not ours directly.
        let grandchild: u32 =
            std::fs::read_to_string(format!("/proc/{0}/task/{0}/children", child.id()))
                .unwrap()
                .split_whitespace()
                .next()
                .unwrap()
                .parse()
                .unwrap();
        assert!(!direct(grandchild));
        child.kill().unwrap();
        let _ = child.wait();
        let _ = std::process::Command::new("kill")
            .arg(grandchild.to_string())
            .status();
    }

    #[test]
    fn clean_stale_keeps_live_ones() {
        let d = tempfile::tempdir().unwrap();
        let mine = socket_path(d.path(), std::process::id());
        let dead = socket_path(d.path(), 99_999_999);
        let other = d.path().join("ctl-socket");
        for f in [&mine, &dead, &other] {
            std::fs::write(f, "").unwrap();
        }
        clean_stale(d.path());
        assert!(mine.exists() && other.exists() && !dead.exists());
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
