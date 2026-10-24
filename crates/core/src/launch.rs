//! What runs in a session's tab: the system OpenSSH, with ControlMaster so SFTP and clones
//! reuse the connection.

use crate::session::{ForwardKind, Session};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LaunchError {
    /// «More ssh options» don't split (an unbalanced quote).
    BadExtra,
}

/// `ssh [-p] [-l] [-i] [-o ServerAliveInterval] -o ControlMaster… [extra…] host`.
pub fn ssh_argv(s: &Session, control_dir: &Path, home: &Path) -> Result<Vec<String>, LaunchError> {
    let mut a = vec!["ssh".to_string()];
    let mut opt = |flag: &str, v: String| a.extend([flag.to_string(), v]);
    if let Some(p) = s.port {
        opt("-p", p.to_string());
    }
    if let Some(u) = s.user.as_ref().filter(|u| !u.is_empty()) {
        opt("-l", u.clone());
    }
    if let Some(i) = s.identity.as_ref().filter(|i| !i.is_empty()) {
        let path = match i.strip_prefix("~/") {
            Some(rest) => home.join(rest).to_string_lossy().into_owned(),
            None => i.clone(),
        };
        opt("-i", path);
    }
    if let Some(k) = s.keepalive {
        opt("-o", format!("ServerAliveInterval={k}"));
    }
    if !s.jump.is_empty() {
        opt("-J", s.jump.clone());
    }
    for f in &s.forwards {
        match f.kind {
            ForwardKind::Local => opt("-L", forward_spec(f)),
            ForwardKind::Remote => opt("-R", forward_spec(f)),
            ForwardKind::Dynamic => opt("-D", f.listen.clone()),
        }
    }
    opt("-o", "ControlMaster=auto".into());
    opt("-o", format!("ControlPath={}/%C", control_dir.display()));
    opt("-o", "ControlPersist=60".into());
    a.extend(shlex::split(&s.extra).ok_or(LaunchError::BadExtra)?);
    // `--`: a host like `-oProxyCommand=…` is a host, never an option.
    a.extend(["--".to_string(), s.host.clone()]);
    Ok(a)
}

/// `listen:host:port`; an IPv6 host goes in brackets.
fn forward_spec(f: &crate::session::Forward) -> String {
    let host = match f.host.contains(':') && !f.host.starts_with('[') {
        true => format!("[{}]", f.host),
        false => f.host.clone(),
    };
    format!("{}:{host}:{}", f.listen, f.port)
}

/// A terminal tab's ssh arguments (after `ssh`) for its SFTP tab: the same options and
/// ControlPath (the open connection is reused), `-s -- host sftp`. `None`: not an ssh of ours.
pub fn sftp_args(ssh_args: &[String]) -> Option<Vec<String>> {
    let at = ssh_args.iter().rposition(|a| a == "--")?;
    let host = ssh_args.get(at + 1)?;
    if at + 2 != ssh_args.len() {
        return None; // a remote command after the host: not one we built
    }
    // Not what a session's own options may ask of its terminal: a tty would corrupt the SFTP
    // stream, -N / -f would never start it, its forwards are the terminal's.
    let mut a: Vec<String> = (ssh_args[..at].iter())
        .filter(|o| !matches!(o.as_str(), "-t" | "-tt" | "-N" | "-f"))
        .cloned()
        .collect();
    a.extend(["-T", "-o", "ClearAllForwardings=yes", "-s", "--"].map(String::from));
    a.extend([host.clone(), "sftp".into()]);
    Some(a)
}

/// ssh's environment: every prompt to our askpass (`exe`), which asks the app over `sock`.
/// `session`: the saved session (its keyring entries); the tab id is added by the tab.
pub fn askpass_env(
    exe: &Path,
    sock: &Path,
    session: Option<&str>,
    credential: Option<&str>,
) -> Vec<(String, String)> {
    let mut env = vec![
        ("SSH_ASKPASS".into(), exe.display().to_string()),
        ("SSH_ASKPASS_REQUIRE".into(), "force".into()),
        ("SHAGSHELL_ASKPASS_SOCK".into(), sock.display().to_string()),
    ];
    if let Some(s) = session {
        env.push(("SHAGSHELL_SESSION".into(), s.to_string()));
    }
    if let Some(c) = credential {
        env.push(("SHAGSHELL_CREDENTIAL".into(), c.to_string()));
    }
    env
}

/// `$XDG_RUNTIME_DIR/shagshell` (`/tmp/shagshell-<uid>` without it).
pub fn control_dir() -> PathBuf {
    use std::os::unix::fs::MetadataExt;
    match std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from) {
        Some(d) if d.is_absolute() => d.join("shagshell"),
        _ => {
            let uid = std::fs::metadata("/proc/self").map_or(0, |m| m.uid());
            PathBuf::from(format!("/tmp/shagshell-{uid}"))
        }
    }
}

/// The control dir, made private to us (0700) if new.
pub fn ensure_control_dir() -> std::io::Result<PathBuf> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    let dir = control_dir();
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&dir)?;
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sftp_from_terminal_args() {
        let ssh: Vec<String> = ["-p", "22", "-o", "ControlPath=/r/%C", "--", "-weird"]
            .map(String::from)
            .to_vec();
        let want = [
            "-p",
            "22",
            "-o",
            "ControlPath=/r/%C",
            "-T",
            "-o",
            "ClearAllForwardings=yes",
            "-s",
            "--",
            "-weird",
            "sftp",
        ];
        assert_eq!(sftp_args(&ssh).unwrap(), want);
        // A session's extra -t / -N / -L: not for the SFTP channel.
        let extra: Vec<String> = ["-t", "-N", "-L", "8080:h:80", "--", "h"]
            .map(String::from)
            .to_vec();
        let got = sftp_args(&extra).unwrap();
        assert!(!got.iter().any(|a| a == "-t" || a == "-N"), "{got:?}");
        assert!(
            got.windows(2)
                .any(|w| w == ["-o", "ClearAllForwardings=yes"])
        );
        assert_eq!(sftp_args(&["host".to_string()]), None);
        let cmd: Vec<String> = ["--", "h", "ls"].map(String::from).to_vec();
        assert_eq!(sftp_args(&cmd), None);
    }

    #[test]
    fn askpass_env_names() {
        let env = askpass_env(
            Path::new("/usr/bin/shagshell"),
            Path::new("/r/a.sock"),
            Some("lab/w"),
            Some("adm"),
        );
        let got: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        assert_eq!(
            got,
            [
                ("SSH_ASKPASS", "/usr/bin/shagshell"),
                ("SSH_ASKPASS_REQUIRE", "force"),
                ("SHAGSHELL_ASKPASS_SOCK", "/r/a.sock"),
                ("SHAGSHELL_SESSION", "lab/w"),
                ("SHAGSHELL_CREDENTIAL", "adm"),
            ]
        );
        assert_eq!(
            askpass_env(Path::new("x"), Path::new("y"), None, None).len(),
            3
        );
    }

    fn s(host: &str) -> Session {
        Session {
            host: host.into(),
            ..Default::default()
        }
    }

    const CTL: [&str; 6] = [
        "-o",
        "ControlMaster=auto",
        "-o",
        "ControlPath=/c/%C",
        "-o",
        "ControlPersist=60",
    ];

    fn argv(x: &Session) -> Vec<String> {
        ssh_argv(x, Path::new("/c"), Path::new("/home/u")).unwrap()
    }

    #[test]
    fn argv_minimal() {
        let mut want = vec!["ssh"];
        want.extend(CTL);
        want.extend(["--", "h"]);
        assert_eq!(argv(&s("h")), want);
    }

    #[test]
    fn argv_all() {
        let x = Session {
            host: "h".into(),
            port: Some(2222),
            user: Some("admin".into()),
            identity: Some("~/k".into()),
            keepalive: Some(30),
            extra: "-A -o Compression=yes".into(),
            ..Default::default()
        };
        let mut want = vec!["ssh", "-p", "2222", "-l", "admin", "-i", "/home/u/k"];
        want.extend(["-o", "ServerAliveInterval=30"]);
        want.extend(CTL);
        want.extend(["-A", "-o", "Compression=yes", "--", "h"]);
        assert_eq!(argv(&x), want);
    }

    #[test]
    fn argv_jump_and_forwards() {
        use crate::session::Forward;
        let fw = |kind, listen: &str, host: &str, port| Forward {
            kind,
            listen: listen.into(),
            host: host.into(),
            port,
        };
        let x = Session {
            host: "h".into(),
            jump: "u@bastion:2200,j2".into(),
            forwards: vec![
                fw(ForwardKind::Local, "8080", "db", 5432),
                fw(ForwardKind::Remote, "127.0.0.1:9000", "localhost", 3000),
                fw(ForwardKind::Dynamic, "1080", "", 0),
            ],
            ..Default::default()
        };
        let a = argv(&x).join(" ");
        assert!(a.contains("-J u@bastion:2200,j2"), "{a}");
        assert!(a.contains("-L 8080:db:5432"), "{a}");
        assert!(a.contains("-R 127.0.0.1:9000:localhost:3000"), "{a}");
        assert!(a.contains("-D 1080"), "{a}");
        let v6 = Session {
            forwards: vec![fw(ForwardKind::Local, "8080", "::1", 80)],
            ..x.clone()
        };
        assert!(argv(&v6).join(" ").contains("-L 8080:[::1]:80"));
        // the SFTP channel of that tab keeps the jump but not the forwards
        let args = argv(&x)[1..].to_vec();
        let s = sftp_args(&args).unwrap().join(" ");
        assert!(
            s.contains("-J u@bastion:2200,j2") && s.contains("ClearAllForwardings=yes"),
            "{s}"
        );
    }

    #[test]
    fn regression_extra_quotes() {
        let mut x = s("h");
        x.extra = r#"-o "ProxyCommand=ssh -W %h:%p j""#.into();
        let a = argv(&x);
        assert!(
            a.contains(&"ProxyCommand=ssh -W %h:%p j".to_string()),
            "{a:?}"
        );
    }

    #[test]
    fn regression_host_dash_is_not_an_option() {
        let a = argv(&s("-oProxyCommand=touch /tmp/x"));
        assert_eq!(a[a.len() - 2..], ["--", "-oProxyCommand=touch /tmp/x"]);
    }

    #[test]
    fn extra_unbalanced() {
        let mut x = s("h");
        x.extra = r#"-o "oops"#.into();
        assert_eq!(
            ssh_argv(&x, Path::new("/c"), Path::new("/h")),
            Err(LaunchError::BadExtra)
        );
    }
}
