//! Update check at launch / Help → Check for Updates: the latest release tag from GitHub, compared with ours.

use std::process::Command;

pub const REPO: &str = "https://github.com/shagovAlexei/cosmic-ext-shagshell-secure";
pub const LATEST_API: &str =
    "https://api.github.com/repos/shagovAlexei/cosmic-ext-shagshell-secure/releases/latest";

/// What a check found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Latest,
    /// The newer release's tag (`v0.18.0`).
    Available(String),
    Failed(String),
}

/// A string field (`"tag_name": "v0.17.0"`) at the top level of the release JSON.
pub fn field(json: &str, name: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    v.get(name)?.as_str().map(String::from)
}

/// `tag` (`v0.18.0`) is a later version than `current` (`0.17.0`); anything but N.N.N → false.
pub fn newer(current: &str, tag: &str) -> bool {
    let nums = |s: &str| -> Option<Vec<u64>> {
        let v: Vec<u64> = s
            .trim_start_matches('v')
            .split('.')
            .map(|n| n.parse().ok())
            .collect::<Option<_>>()?;
        (v.len() == 3).then_some(v)
    };
    matches!((nums(current), nums(tag)), (Some(c), Some(t)) if t > c)
}

/// What GitHub's answer (`json`) means for `current`.
pub fn status(current: &str, json: &str) -> Status {
    match field(json, "tag_name") {
        Some(t) if newer(current, &t) => Status::Available(t),
        Some(_) => Status::Latest,
        // An API error (rate limit…) says why in "message".
        None => Status::Failed(field(json, "message").unwrap_or_else(|| "no tag_name".into())),
    }
}

/// Ask GitHub through `curl` (10 s limit). Blocking. `SHAGSHELL_UPDATE_URL` replaces the address
/// (a `file://` one is how the window is tried out).
pub fn check(current: &str) -> Status {
    let url = std::env::var("SHAGSHELL_UPDATE_URL").unwrap_or_else(|_| LATEST_API.into());
    let out = match Command::new("/usr/bin/curl")
        .args(["-sSL", "-m", "10", "--", &url])
        .output()
    {
        Ok(o) => o,
        Err(e) => return Status::Failed(format!("curl: {e}")),
    };
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Status::Failed(if err.is_empty() {
            format!("curl: {}", out.status)
        } else {
            err
        });
    }
    status(current, &String::from_utf8_lossy(&out.stdout))
}

/// `v0.18.0`: the only shape of a tag we put in a download address.
pub fn valid_tag(tag: &str) -> bool {
    newer("0.0.0", tag) || tag == "v0.0.0"
}

/// The package named by the first line of `SHA256SUMS` (`<hash>  shagshell_0.18.0_amd64.deb`);
/// anything with a path or another shape is refused: the name goes into a download address.
pub fn deb_name(sums: &str) -> Option<String> {
    let name = sums
        .lines()
        .next()?
        .split_whitespace()
        .nth(1)?
        .trim_start_matches('*');
    let plain = name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-' | b'+'));
    (plain && name.starts_with("shagshell_") && name.ends_with(".deb")).then(|| name.to_string())
}

/// `sha256sum -c SHA256SUMS` in `dir`: Ok only when every listed file matches.
pub fn verify(dir: &std::path::Path) -> Result<(), String> {
    let out = Command::new("/usr/bin/sha256sum")
        .args(["-c", "--strict", "SHA256SUMS"])
        .current_dir(dir)
        .output()
        .map_err(|e| format!("sha256sum: {e}"))?;
    match out.status.success() {
        true => Ok(()),
        false => Err(String::from_utf8_lossy(&out.stdout)
            .lines()
            .chain(String::from_utf8_lossy(&out.stderr).lines())
            .find(|l| !l.ends_with(": OK"))
            .unwrap_or("checksum mismatch")
            .to_string()),
    }
}

/// Download release `tag`'s package and `SHA256SUMS` into `dir` (curl, from this repository only)
/// and check the hash. A package that does not match is deleted. Blocking.
pub fn download(tag: &str, dir: &std::path::Path) -> Result<std::path::PathBuf, String> {
    if !valid_tag(tag) {
        return Err(format!("bad tag {tag}"));
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let get = |name: &str| -> Result<(), String> {
        let url = format!("{REPO}/releases/download/{tag}/{name}");
        let out = Command::new("/usr/bin/curl")
            .args(["-fsSL", "-m", "300", "-o"])
            .arg(dir.join(name))
            .args(["--", &url])
            .output()
            .map_err(|e| format!("curl: {e}"))?;
        match out.status.success() {
            true => Ok(()),
            false => Err(String::from_utf8_lossy(&out.stderr).trim().to_string()),
        }
    };
    get("SHA256SUMS")?;
    let sums = std::fs::read_to_string(dir.join("SHA256SUMS")).map_err(|e| e.to_string())?;
    let name = deb_name(&sums).ok_or("SHA256SUMS: no package")?;
    get(&name)?;
    let deb = dir.join(&name);
    if let Err(e) = verify(dir) {
        let _ = std::fs::remove_file(&deb);
        return Err(e);
    }
    Ok(deb)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_from_release_json() {
        let j = r#"{"url":"x","tag_name" : "v0.18.0","name":"v0.18.0"}"#;
        assert_eq!(status("0.17.0", j), Status::Available("v0.18.0".into()));
        assert_eq!(status("0.18.0", j), Status::Latest);
        let limit = r#"{"message":"API rate limit exceeded for 1.2.3.4.","documentation_url":"x"}"#;
        assert_eq!(
            status("0.18.0", limit),
            Status::Failed("API rate limit exceeded for 1.2.3.4.".into())
        );
    }

    #[test]
    fn a_nested_tag_name_is_not_ours() {
        let j = r#"{"assets":[{"tag_name":"v9.9.9"}],"name":"x"}"#;
        assert_eq!(status("0.1.0", j), Status::Failed("no tag_name".into()));
        assert_eq!(
            status("0.1.0", "<html>"),
            Status::Failed("no tag_name".into())
        );
    }

    #[test]
    fn newer_compares_numbers() {
        assert!(newer("0.17.0", "v0.18.0"));
        assert!(newer("0.9.0", "v0.10.0")); // not by text
        assert!(!newer("0.17.0", "v0.17.0"));
        assert!(!newer("0.17.1", "v0.17.0"));
        assert!(!newer("0.17.0", "nightly"));
        assert!(!newer("0.18.0", "v0.18.0.0"));
    }

    #[test]
    fn deb_name_is_a_plain_package_name() {
        let h = "a".repeat(64);
        assert_eq!(
            deb_name(&format!("{h}  shagshell_0.46.0_amd64.deb\n")),
            Some("shagshell_0.46.0_amd64.deb".into())
        );
        assert_eq!(deb_name(&format!("{h}  ../evil.deb")), None);
        assert_eq!(deb_name(&format!("{h}  shagshell_/../x.deb")), None);
        assert_eq!(deb_name(&format!("{h}  other_1.deb")), None);
        assert_eq!(deb_name(""), None);
        assert!(valid_tag("v0.46.0") && !valid_tag("v0.46.0/../x") && !valid_tag("latest"));
    }

    #[test]
    fn regression_update_refuses_a_package_that_does_not_match_its_hash() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("shagshell_0.1.0_amd64.deb"), b"package").unwrap();
        // sha256 of "package"
        let ok = "bc4a71180870f7945155fbb02f4b0a2e3faa2a62d6d31b7039013055ed19869a";
        let line = |h: &str| format!("{h}  shagshell_0.1.0_amd64.deb\n");
        std::fs::write(d.path().join("SHA256SUMS"), line(ok)).unwrap();
        assert_eq!(verify(d.path()), Ok(()));
        std::fs::write(d.path().join("SHA256SUMS"), line(&"0".repeat(64))).unwrap();
        assert!(verify(d.path()).is_err());
    }
}
