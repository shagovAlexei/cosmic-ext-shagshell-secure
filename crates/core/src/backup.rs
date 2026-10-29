//! File → Export / Import Settings (spec 38): `~/.config/shagshell` and the Global Options as one
//! `.tar.gz` (the system `tar`). Passwords are not in it: they live in the keyring.

use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

/// The Global Options' folder under `~/.config/cosmic`.
pub const APP_ID: &str = "io.github.shagovalexei.ShagShellSecure";

fn tar(args: &mut std::process::Command) -> io::Result<()> {
    let out = args.output()?;
    match out.status.success() {
        true => Ok(()),
        false => Err(io::Error::other(format!(
            "tar: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))),
    }
}

/// The settings under `config_home` (`~/.config`) into `out`.
pub fn export(config_home: &Path, out: &Path) -> io::Result<()> {
    let mut c = std::process::Command::new("/usr/bin/tar");
    // The program writes scripts/.lib itself at start.
    c.args(["-czf"])
        .arg(out)
        .arg("--exclude=shagshell/scripts/.lib");
    c.arg("-C").arg(config_home).arg("shagshell");
    let cosmic = config_home.join("cosmic");
    if cosmic.join(APP_ID).is_dir() {
        c.arg("-C").arg(&cosmic).arg(APP_ID);
    }
    tar(&mut c)
}

/// `archive`'s files over those under `config_home`; how many. Refused whole (nothing written)
/// when it holds anything but files and folders under the two tops.
pub fn import(archive: &Path, config_home: &Path) -> io::Result<usize> {
    let tmp = config_home.join(format!(".shagshell-import-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&tmp)?;
    }
    let r = (|| {
        let mut c = std::process::Command::new("/usr/bin/tar");
        c.args(["-xzf"]).arg(archive).arg("-C").arg(&tmp);
        c.args(["--no-same-owner", "--no-same-permissions"]);
        tar(&mut c)?;
        let mut files = Vec::new();
        for top in std::fs::read_dir(&tmp)? {
            let top = top?;
            let name = top.file_name();
            let Some(to) = (match name.to_str() {
                Some("shagshell") => Some(config_home.join("shagshell")),
                Some(APP_ID) => Some(config_home.join("cosmic").join(APP_ID)),
                _ => None,
            }) else {
                return Err(io::Error::new(
                    ErrorKind::InvalidData,
                    format!("not a settings archive: {}", name.to_string_lossy()),
                ));
            };
            walk(&top.path(), &to, &mut files)?;
        }
        for (from, to) in &files {
            if let Some(d) = to.parent() {
                std::fs::create_dir_all(d)?;
            }
            let part = to.with_extension("shagshell-import");
            std::fs::copy(from, &part)?;
            std::fs::rename(&part, to)?;
        }
        Ok(files.len())
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    r
}

/// Every file under `from` with where it goes under `to`; a link or a device: refused.
fn walk(from: &Path, to: &Path, out: &mut Vec<(PathBuf, PathBuf)>) -> io::Result<()> {
    let m = std::fs::symlink_metadata(from)?;
    if m.is_file() {
        out.push((from.to_path_buf(), to.to_path_buf()));
        return Ok(());
    }
    if !m.is_dir() {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            format!("not a plain file: {}", from.display()),
        ));
    }
    for e in std::fs::read_dir(from)? {
        let e = e?;
        walk(&e.path(), &to.join(e.file_name()), out)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(p: &Path, text: &str) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    #[test]
    fn export_then_import_on_another_computer() {
        let a = tempfile::tempdir().unwrap();
        write(
            &a.path().join("shagshell/sessions/lab/r1.toml"),
            "host = \"r1\"\n",
        );
        write(&a.path().join("shagshell/keymap.toml"), "[[key]]\n");
        write(&a.path().join("shagshell/scripts/.lib/crt.py"), "# ours");
        write(
            &a.path().join(format!("cosmic/{APP_ID}/v1/language")),
            "\"ru\"",
        );
        let out = a.path().join("s.tar.gz");
        export(a.path(), &out).unwrap();

        let b = tempfile::tempdir().unwrap();
        write(&b.path().join("shagshell/keymap.toml"), "old");
        write(&b.path().join("shagshell/commands.toml"), "kept");
        let n = import(&out, b.path()).unwrap();
        assert_eq!(n, 3, "sessions, keymap, language; not scripts/.lib");
        let read = |p: &str| std::fs::read_to_string(b.path().join(p)).unwrap();
        assert_eq!(read("shagshell/sessions/lab/r1.toml"), "host = \"r1\"\n");
        assert_eq!(read("shagshell/keymap.toml"), "[[key]]\n");
        assert_eq!(
            read("shagshell/commands.toml"),
            "kept",
            "nothing else deleted"
        );
        assert_eq!(read(&format!("cosmic/{APP_ID}/v1/language")), "\"ru\"");
        assert!(!b.path().join("shagshell/scripts/.lib").exists());
        let left: Vec<_> = std::fs::read_dir(b.path())
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert_eq!(left.len(), 2, "no temp folder left: {left:?}");
    }

    #[test]
    fn regression_archive_with_a_link_writes_nothing() {
        // A link in an archive from someone else could aim a later file anywhere.
        let a = tempfile::tempdir().unwrap();
        let src = a.path().join("src");
        write(&src.join("shagshell/keymap.toml"), "x");
        std::os::unix::fs::symlink("/etc", src.join("shagshell/evil")).unwrap();
        let out = a.path().join("bad.tar.gz");
        let ok = std::process::Command::new("tar")
            .args(["-czf"])
            .arg(&out)
            .arg("-C")
            .arg(&src)
            .arg("shagshell")
            .status()
            .unwrap();
        assert!(ok.success());
        let b = tempfile::tempdir().unwrap();
        assert!(import(&out, b.path()).is_err());
        assert!(!b.path().join("shagshell").exists(), "nothing written");
        // Another top folder: refused too.
        let src2 = a.path().join("src2");
        write(&src2.join(".bashrc"), "rm -rf ~");
        let out2 = a.path().join("bad2.tar.gz");
        std::process::Command::new("tar")
            .arg("-czf")
            .arg(&out2)
            .arg("-C")
            .arg(&src2)
            .arg(".bashrc")
            .status()
            .unwrap();
        assert!(import(&out2, b.path()).is_err());
        assert!(!b.path().join(".bashrc").exists());
    }
}
