//! Running as a flatpak. ssh, ssh-keygen, sftp and python3 come with the runtime and run in the
//! sandbox (askpass, ControlMaster and the script sockets stay as they are); telnet, picocom,
//! lrzsz, the local shell and «Launch App» programs are the host's: `shagshell-host`
//! (`flatpak-spawn --host`, see `flatpak/shagshell-host`) runs them.

use std::ffi::OsStr;

/// The program in the flatpak that runs its arguments on the host.
pub const HOST: &str = "shagshell-host";

/// Inside a flatpak sandbox.
pub fn inside() -> bool {
    std::path::Path::new("/.flatpak-info").exists()
}

/// `argv` run on the host when `inside` a flatpak, as is outside.
pub fn host(mut argv: Vec<String>, inside: bool) -> Vec<String> {
    if inside {
        argv.insert(0, HOST.into());
    }
    argv
}

/// A local shell tab's program and arguments: `$SHELL` (else `/bin/sh`); from a flatpak the
/// host's, which the sandbox can't check, so it is taken as said.
pub fn local_shell(env_shell: Option<&OsStr>, inside: bool) -> Vec<String> {
    match (inside, env_shell) {
        (true, Some(s)) if !s.is_empty() => host(vec![s.to_string_lossy().into_owned()], true),
        (true, _) => host(vec!["/bin/sh".into()], true),
        (false, s) => vec![crate::shell::pick(s).to_string_lossy().into_owned()],
    }
}

/// ssh's `-F ~/.ssh/config` from a flatpak, if there is one. The runtime's ssh is upstream
/// OpenSSH: it refuses a group-writable `~/.ssh/config` (Pop!_OS's umask 002 makes them so) that
/// Debian's ssh takes, but checks no permissions of a file named with -F. Empty outside.
pub fn ssh_config(home: &std::path::Path, inside: bool) -> Vec<String> {
    let config = home.join(".ssh/config");
    match inside && config.is_file() {
        true => vec!["-F".into(), config.display().to_string()],
        false => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn host_prefix_only_inside() {
        assert_eq!(host(v(&["winbox", "h"]), false), v(&["winbox", "h"]));
        assert_eq!(
            host(v(&["winbox", "h"]), true),
            v(&["shagshell-host", "winbox", "h"])
        );
    }

    #[test]
    fn regression_ssh_config_group_writable() {
        // Pop!_OS's umask 002 leaves ~/.ssh/config 664: Debian's ssh takes it, the runtime's
        // upstream ssh refuses it («Bad owner or permissions»), but not one named with -F.
        let d = tempfile::tempdir().unwrap();
        assert!(ssh_config(d.path(), true).is_empty());
        std::fs::create_dir(d.path().join(".ssh")).unwrap();
        std::fs::write(d.path().join(".ssh/config"), "").unwrap();
        let want = d.path().join(".ssh/config").display().to_string();
        assert_eq!(ssh_config(d.path(), true), v(&["-F", &want]));
        assert!(ssh_config(d.path(), false).is_empty());
    }

    #[test]
    fn local_shell_inside_is_the_hosts() {
        let zsh = Some(OsStr::new("/usr/bin/zsh"));
        assert_eq!(
            local_shell(zsh, true),
            v(&["shagshell-host", "/usr/bin/zsh"])
        );
        assert_eq!(local_shell(None, true), v(&["shagshell-host", "/bin/sh"]));
        assert_eq!(
            local_shell(Some(OsStr::new("")), true),
            v(&["shagshell-host", "/bin/sh"])
        );
        // Outside: checked as before (a missing program falls back to /bin/sh).
        assert_eq!(
            local_shell(Some(OsStr::new("/no/such/shell")), false),
            v(&["/bin/sh"])
        );
    }
}
