//! SecureCRT's Connect Bar: one line in the toolbar that names a saved session or a host.

use crate::session::form::valid_host;

#[derive(Debug, PartialEq, Eq)]
pub enum Target {
    /// A saved session, by id.
    Session(String),
    /// `[ssh2://][user@]host[:port]`: connect without saving.
    Host {
        user: Option<String>,
        host: String,
        port: Option<u16>,
    },
}

/// What `text` asks for, given the saved sessions' `ids` (`folder/…/name`):
/// - `/S name` or a session's exact name (any case): that session;
/// - `ssh2://…`, or text with `@` or `:`: a host;
/// - else the one session whose name contains it, or a host when none / several do.
pub fn parse(text: &str, ids: &[String]) -> Option<Target> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    let name = |id: &str| id.rsplit('/').next().unwrap_or(id).to_lowercase();
    let find = |want: &str| {
        let want = want.to_lowercase();
        (ids.iter())
            .find(|id| name(id) == want || id.to_lowercase() == want)
            .cloned()
    };
    if let Some(n) = t.strip_prefix("/S ").or_else(|| t.strip_prefix("/s ")) {
        return find(n.trim()).map(Target::Session);
    }
    if let Some(id) = find(t) {
        return Some(Target::Session(id));
    }
    let url = t
        .strip_prefix("ssh2://")
        .or_else(|| t.strip_prefix("ssh://"));
    if url.is_none() && !t.contains(['@', ':']) {
        let low = t.to_lowercase();
        let hits: Vec<&String> = ids.iter().filter(|id| name(id).contains(&low)).collect();
        if let [one] = hits[..] {
            return Some(Target::Session(one.clone()));
        }
    }
    host(url.unwrap_or(t))
}

fn host(t: &str) -> Option<Target> {
    let (user, rest) = match t.rsplit_once('@') {
        Some((u, r)) => (Some(u.to_string()).filter(|u| !u.is_empty()), r),
        None => (None, t),
    };
    let (host, port) = match rest.rsplit_once(':') {
        Some((h, p)) => (h, Some(p.parse::<u16>().ok().filter(|&p| p > 0)?)),
        None => (rest, None),
    };
    valid_host(host).then(|| Target::Host {
        user,
        host: host.to_string(),
        port,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids() -> Vec<String> {
        ["Lab/router", "Lab/server1", "Lab/server2", "web"]
            .map(String::from)
            .to_vec()
    }

    #[test]
    fn a_name_picks_a_saved_session() {
        let s = |x: &str| Some(Target::Session(x.into()));
        assert_eq!(parse("router", &ids()), s("Lab/router"));
        assert_eq!(parse("ROUT", &ids()), s("Lab/router")); // the only one containing it
        assert_eq!(parse("/S server2", &ids()), s("Lab/server2"));
        assert_eq!(parse("Lab/server1", &ids()), s("Lab/server1"));
        assert_eq!(parse("/S nope", &ids()), None);
        assert_eq!(parse("  ", &ids()), None);
    }

    #[test]
    fn anything_else_is_a_host() {
        let h = |u: Option<&str>, host: &str, port| {
            Some(Target::Host {
                user: u.map(String::from),
                host: host.into(),
                port,
            })
        };
        assert_eq!(parse("server", &ids()), h(None, "server", None)); // two match: a host
        assert_eq!(
            parse("admin@10.0.0.1:2222", &ids()),
            h(Some("admin"), "10.0.0.1", Some(2222))
        );
        assert_eq!(parse("ssh2://web", &ids()), h(None, "web", None)); // ssh2://: a host, even a session's name
        assert_eq!(parse("10.0.0.1:x", &ids()), None); // a bad port
        assert_eq!(parse("-oProxy=x", &ids()), None); // never an ssh option
    }
}
