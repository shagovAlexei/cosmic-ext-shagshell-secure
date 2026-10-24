//! Creating SSH keys with the system `ssh-keygen` (Tools → Create Public Key…) and the arguments
//! of `ssh-copy-id`. No key material is handled here: `ssh-keygen` writes the files. A
//! passphrase never goes into an argument (`/proc/<pid>/cmdline` is world-readable): it is given
//! through `SSH_ASKPASS`, which is our own binary printing it from the environment.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Ed25519,
    Ecdsa,
    Rsa,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Ed25519, Kind::Ecdsa, Kind::Rsa];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Ed25519 => "ed25519",
            Kind::Ecdsa => "ecdsa",
            Kind::Rsa => "rsa",
        }
    }

    /// The key sizes `ssh-keygen -b` takes (the first is the default); none: a fixed size.
    pub fn sizes(self) -> &'static [u32] {
        match self {
            Kind::Ed25519 => &[],
            Kind::Ecdsa => &[256, 384, 521],
            Kind::Rsa => &[3072, 4096],
        }
    }
}

pub struct Spec {
    pub kind: Kind,
    /// Index into `kind.sizes()`.
    pub size: usize,
    pub file: PathBuf,
    pub comment: String,
    /// Empty: no passphrase.
    pub passphrase: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The private or the public file is there already (never overwritten).
    Exists(PathBuf),
    NoFile,
    /// `ssh-keygen` could not run, or said no.
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Generated {
    pub fingerprint: String,
    pub public: String,
}

/// `~/.ssh/id_ed25519` and so on.
pub fn default_file(kind: Kind, ssh_dir: &Path) -> PathBuf {
    ssh_dir.join(format!("id_{}", kind.name()))
}

/// `user@host` for the comment of a new key.
pub fn default_comment() -> String {
    let user = std::env::var("USER").unwrap_or_default();
    let host = std::fs::read_to_string("/proc/sys/kernel/hostname").unwrap_or_default();
    format!("{user}@{}", host.trim())
}

/// `ssh-keygen`'s arguments. With a passphrase `-N` is left out: it asks (through askpass).
pub fn args(s: &Spec) -> Vec<String> {
    let mut a = vec!["-q".to_string(), "-t".into(), s.kind.name().into()];
    if let Some(bits) = s.kind.sizes().get(s.size) {
        a.extend(["-b".to_string(), bits.to_string()]);
    }
    a.extend(["-f".to_string(), s.file.display().to_string()]);
    a.extend(["-C".to_string(), s.comment.clone()]);
    if s.passphrase.is_empty() {
        a.extend(["-N".to_string(), String::new()]);
    }
    a
}

fn public_of(file: &Path) -> PathBuf {
    let mut p = file.as_os_str().to_owned();
    p.push(".pub");
    p.into()
}

/// Makes the key. `askpass`: the program `SSH_ASKPASS` runs to get the passphrase (our binary).
pub fn generate(s: &Spec, askpass: &Path) -> Result<Generated, Error> {
    use std::os::unix::fs::DirBuilderExt;
    if !s.file.is_absolute() || s.file.is_dir() {
        return Err(Error::NoFile);
    }
    let public = public_of(&s.file);
    for f in [&s.file, &public] {
        if f.symlink_metadata().is_ok() {
            return Err(Error::Exists(f.clone()));
        }
    }
    if let Some(dir) = s.file.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)
            .map_err(|e| Error::Failed(format!("{}: {e}", dir.display())))?;
    }
    let mut cmd = Command::new("ssh-keygen");
    cmd.args(args(s))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    if !s.passphrase.is_empty() {
        cmd.env("SSH_ASKPASS", askpass)
            .env("SSH_ASKPASS_REQUIRE", "force")
            .env("SHAGSHELL_ASKPASS_FIXED", &s.passphrase);
    }
    let out = cmd
        .output()
        .map_err(|e| Error::Failed(format!("ssh-keygen: {e}")))?;
    if !out.status.success() {
        let text = String::from_utf8_lossy(&out.stderr);
        let line = text.lines().rev().find(|l| !l.trim().is_empty());
        return Err(Error::Failed(
            line.unwrap_or("ssh-keygen failed").to_string(),
        ));
    }
    Ok(Generated {
        // Only for show: the key exists whatever this says.
        fingerprint: fingerprint(&public).unwrap_or_default(),
        public: std::fs::read_to_string(&public)
            .map_err(|e| Error::Failed(format!("{}: {e}", public.display())))?
            .trim()
            .to_string(),
    })
}

/// `256 SHA256:… comment (ED25519)` of a public key file.
pub fn fingerprint(public: &Path) -> Result<String, Error> {
    let out = Command::new("ssh-keygen")
        .arg("-lf")
        .arg(public)
        .output()
        .map_err(|e| Error::Failed(format!("ssh-keygen: {e}")))?;
    match out.status.success() {
        true => Ok(String::from_utf8_lossy(&out.stdout).trim().to_string()),
        false => Err(Error::Failed(format!(
            "{}: not a public key",
            public.display()
        ))),
    }
}

/// The `*.pub` files of `ssh_dir`, by name.
pub fn public_keys(ssh_dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(ssh_dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "pub") && p.is_file())
        .collect();
    v.sort();
    v
}

/// `ssh-copy-id` for a session: its port, jump hosts and the shared ControlMaster connection.
/// `user`: empty for the one ssh picks.
#[allow(clippy::too_many_arguments)]
pub fn copy_id_args(
    public: &Path,
    host: &str,
    user: &str,
    port: Option<u16>,
    jump: &str,
    identity: &str,
    control_dir: &Path,
) -> Vec<String> {
    let mut a = vec!["-i".to_string(), public.display().to_string()];
    if let Some(p) = port {
        a.extend(["-p".to_string(), p.to_string()]);
    }
    let mut opt = |o: String| a.extend(["-o".to_string(), o]);
    if !jump.trim().is_empty() {
        opt(format!("ProxyJump={}", jump.trim()));
    }
    if !identity.trim().is_empty() {
        opt(format!("IdentityFile={}", identity.trim()));
    }
    opt("ControlMaster=auto".into());
    opt(format!("ControlPath={}/%C", control_dir.display()));
    opt("ControlPersist=60".into());
    a.push(match user.is_empty() {
        true => host.to_string(),
        false => format!("{user}@{host}"),
    });
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(dir: &Path, kind: Kind, pass: &str) -> Spec {
        Spec {
            kind,
            size: 0,
            file: dir.join("sub/id_test"),
            comment: "me@box".into(),
            passphrase: pass.into(),
        }
    }

    #[test]
    fn arguments() {
        let d = Path::new("/k");
        assert_eq!(
            args(&spec(d, Kind::Ed25519, "")),
            [
                "-q",
                "-t",
                "ed25519",
                "-f",
                "/k/sub/id_test",
                "-C",
                "me@box",
                "-N",
                ""
            ]
        );
        let s = Spec {
            size: 1,
            ..spec(d, Kind::Rsa, "x")
        };
        assert_eq!(
            args(&s),
            [
                "-q",
                "-t",
                "rsa",
                "-b",
                "4096",
                "-f",
                "/k/sub/id_test",
                "-C",
                "me@box"
            ]
        ); // the passphrase is nowhere in the arguments
    }

    #[test]
    fn copy_id() {
        let a = copy_id_args(
            Path::new("/k.pub"),
            "h",
            "u",
            Some(2222),
            "j@x",
            "~/.ssh/k",
            Path::new("/c"),
        );
        assert_eq!(
            a,
            [
                "-i",
                "/k.pub",
                "-p",
                "2222",
                "-o",
                "ProxyJump=j@x",
                "-o",
                "IdentityFile=~/.ssh/k",
                "-o",
                "ControlMaster=auto",
                "-o",
                "ControlPath=/c/%C",
                "-o",
                "ControlPersist=60",
                "u@h"
            ]
        );
        assert_eq!(
            copy_id_args(Path::new("/k.pub"), "h", "", None, "", "", Path::new("/c"))
                .last()
                .unwrap(),
            "h"
        );
    }

    #[test]
    fn makes_keys_and_never_overwrites() {
        if Command::new("ssh-keygen").arg("-?").output().is_err() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let g = generate(
            &spec(dir.path(), Kind::Ed25519, ""),
            Path::new("/nonexistent"),
        )
        .unwrap();
        assert!(g.public.starts_with("ssh-ed25519 ") && g.fingerprint.contains("SHA256:"));
        assert_eq!(
            generate(&spec(dir.path(), Kind::Ed25519, ""), Path::new("/x")),
            Err(Error::Exists(dir.path().join("sub/id_test")))
        );
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(dir.path().join("sub/id_test"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o077, 0);
        assert_eq!(public_keys(&dir.path().join("sub")).len(), 1);
    }

    /// The passphrase comes through the askpass program (a script here), not the arguments.
    #[test]
    fn passphrase_through_askpass() {
        if Command::new("ssh-keygen").arg("-?").output().is_err() {
            return;
        }
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let ap = dir.path().join("ap.sh");
        std::fs::write(&ap, "#!/bin/sh\necho \"$SHAGSHELL_ASKPASS_FIXED\"\n").unwrap();
        std::fs::set_permissions(&ap, std::fs::Permissions::from_mode(0o755)).unwrap();
        let s = spec(dir.path(), Kind::Ed25519, "s3cret phrase");
        generate(&s, &ap).unwrap();
        let ok = Command::new("ssh-keygen")
            .args(["-y", "-P", "s3cret phrase", "-f"])
            .arg(&s.file)
            .output()
            .unwrap();
        assert!(ok.status.success());
    }
}
