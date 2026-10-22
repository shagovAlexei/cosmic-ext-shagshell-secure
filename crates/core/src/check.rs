//! `shagshell --check`: which external programs the app drives are installed.

use std::ffi::OsStr;

/// Programs probed by `--check`: ssh is required, the rest enable Telnet, Serial and scripts.
pub const TOOLS: [&str; 4] = ["ssh", "telnet", "picocom", "python3"];

/// For each name in order: is it an executable file in a `path` (PATH-style) dir?
/// An empty component (`a::b`) is skipped: the shell would read it as the current dir.
pub fn probe(names: &[&str], path: Option<&OsStr>) -> Vec<(String, bool)> {
    use std::os::unix::fs::PermissionsExt;
    let dirs: Vec<_> = path
        .map(|p| {
            std::env::split_paths(p)
                .filter(|d| !d.as_os_str().is_empty())
                .collect()
        })
        .unwrap_or_default();
    let found = |name: &str| {
        dirs.iter().any(|d| {
            std::fs::metadata(d.join(name))
                .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
    };
    names.iter().map(|n| (n.to_string(), found(n))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    fn exe(dir: &Path, name: &str, mode: u32) {
        let p = dir.join(name);
        std::fs::write(&p, "").unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(mode)).unwrap();
    }

    #[test]
    fn found_executable() {
        let d = tempfile::tempdir().unwrap();
        exe(d.path(), "ssh", 0o755);
        assert_eq!(
            probe(&["ssh"], Some(d.path().as_os_str())),
            [("ssh".into(), true)]
        );
    }

    #[test]
    fn regression_not_executable_is_missing() {
        let d = tempfile::tempdir().unwrap();
        exe(d.path(), "ssh", 0o644);
        assert_eq!(
            probe(&["ssh"], Some(d.path().as_os_str())),
            [("ssh".into(), false)]
        );
    }

    #[test]
    fn directory_is_missing() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir(d.path().join("ssh")).unwrap();
        assert_eq!(
            probe(&["ssh"], Some(d.path().as_os_str())),
            [("ssh".into(), false)]
        );
    }

    #[test]
    fn no_path_all_missing() {
        let got = probe(&TOOLS, None);
        let names: Vec<_> = got.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, TOOLS);
        assert!(got.iter().all(|(_, ok)| !ok));
    }

    #[test]
    fn empty_components_skipped() {
        let d = tempfile::tempdir().unwrap();
        exe(d.path(), "ssh", 0o755);
        let path = format!("::{}", d.path().display());
        assert_eq!(
            probe(&["ssh"], Some(OsStr::new(&path))),
            [("ssh".into(), true)]
        );
    }
}
