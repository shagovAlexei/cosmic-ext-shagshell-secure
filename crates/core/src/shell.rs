//! Which program a local shell tab runs.

use std::ffi::OsStr;
use std::path::PathBuf;

const FALLBACK: &str = "/bin/sh";

/// `$SHELL` if it names a program that can run, else `/bin/sh`.
pub fn pick(env_shell: Option<&OsStr>) -> PathBuf {
    env_shell
        .map(PathBuf::from)
        .filter(|p| crate::check::is_executable(p))
        .unwrap_or_else(|| FALLBACK.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn file(mode: u32) -> (tempfile::TempDir, PathBuf) {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("myshell");
        std::fs::write(&p, "").unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(mode)).unwrap();
        (d, p)
    }

    #[test]
    fn pick_env_executable() {
        let (_d, p) = file(0o755);
        assert_eq!(pick(Some(p.as_os_str())), p);
    }

    #[test]
    fn pick_unset() {
        assert_eq!(pick(None), PathBuf::from(FALLBACK));
    }

    #[test]
    fn pick_empty() {
        assert_eq!(pick(Some(OsStr::new(""))), PathBuf::from(FALLBACK));
    }

    #[test]
    fn pick_missing() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("nope");
        assert_eq!(pick(Some(p.as_os_str())), PathBuf::from(FALLBACK));
    }

    #[test]
    fn regression_pick_not_executable() {
        let (_d, p) = file(0o644);
        assert_eq!(pick(Some(p.as_os_str())), PathBuf::from(FALLBACK));
    }
}
