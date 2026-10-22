//! Hosts from `~/.ssh/config` as sessions. Only the alias is kept: ssh applies the rest of its
//! config (HostName, User, ProxyJump…) itself when connecting, so a session never drifts from it.
// ponytail: `Include` files are not followed; add when someone keeps hosts there.

use crate::session::Store;

pub const FOLDER: &str = "ssh config";

/// Aliases of `Host` lines (any case, `Host=a`, several per line), without patterns
/// (`*`, `?`, `!`), each once, in order.
pub fn hosts(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let Some((key, rest)) = line.split_once(|c: char| c.is_whitespace() || c == '=') else {
            continue;
        };
        if !key.eq_ignore_ascii_case("host") {
            continue;
        }
        let names = rest.trim_start_matches(|c: char| c.is_whitespace() || c == '=');
        for n in names.split_whitespace() {
            if !n.contains(['*', '?', '!']) && !out.iter().any(|o| o == n) {
                out.push(n.to_string());
            }
        }
    }
    out
}

/// Each host as a session in the «ssh config» folder: (imported, skipped as already there).
pub fn import(store: &Store, text: &str) -> (usize, usize) {
    let (mut done, mut skipped) = (0, 0);
    for h in hosts(text) {
        match store.create(FOLDER, &h, &h) {
            Ok(_) => done += 1,
            Err(_) => skipped += 1,
        }
    }
    (done, skipped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_basic() {
        let t = "Host web\n  HostName 10.0.0.1\nHost db  db2\n  User x\n";
        assert_eq!(hosts(t), ["web", "db", "db2"]);
    }

    #[test]
    fn hosts_skips_patterns() {
        let t = "Host *\n  ServerAliveInterval 5\nHost *.lan !bad a?c ok\n";
        assert_eq!(hosts(t), ["ok"]);
    }

    #[test]
    fn hosts_case_and_equals() {
        let t = "host a\nHOST=b\n\tHost  c\n# Host d\nMatch host e\nHost a\n";
        assert_eq!(hosts(t), ["a", "b", "c"]);
    }

    #[test]
    fn import_skips_existing() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::new(d.path().join("s"));
        assert_eq!(import(&store, "Host a b\n"), (2, 0));
        assert_eq!(import(&store, "Host a b c\n"), (1, 2));
        assert_eq!(store.get("ssh config/c").unwrap().host, "c");
    }
}
