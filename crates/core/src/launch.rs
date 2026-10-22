//! What runs in a session's tab: the system OpenSSH, with ControlMaster so SFTP and clones
//! reuse the connection.

use crate::session::Session;
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
    opt("-o", "ControlMaster=auto".into());
    opt("-o", format!("ControlPath={}/%C", control_dir.display()));
    opt("-o", "ControlPersist=60".into());
    a.extend(shlex::split(&s.extra).ok_or(LaunchError::BadExtra)?);
    a.push(s.host.clone());
    Ok(a)
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
        want.push("h");
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
        };
        let mut want = vec!["ssh", "-p", "2222", "-l", "admin", "-i", "/home/u/k"];
        want.extend(["-o", "ServerAliveInterval=30"]);
        want.extend(CTL);
        want.extend(["-A", "-o", "Compression=yes", "h"]);
        assert_eq!(argv(&x), want);
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
    fn extra_unbalanced() {
        let mut x = s("h");
        x.extra = r#"-o "oops"#.into();
        assert_eq!(
            ssh_argv(&x, Path::new("/c"), Path::new("/h")),
            Err(LaunchError::BadExtra)
        );
    }
}
