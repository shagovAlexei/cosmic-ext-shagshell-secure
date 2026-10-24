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
    let out = match Command::new("curl")
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
}
