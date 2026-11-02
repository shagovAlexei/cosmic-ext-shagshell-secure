//! The tree as the rows on screen: what is open, what the filter keeps, Shift+click ranges,
//! the sessions a selection stands for.

use super::Node;
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub depth: u16,
    pub id: String,
    pub name: String,
    pub folder: bool,
    pub expanded: bool,
    /// A session's protocol (its icon); `None`: a folder.
    pub protocol: Option<super::Protocol>,
    /// The icon the session picked ("": by protocol).
    pub icon: String,
}

/// Rows for `tree`: folders in `expanded` show their children. A filter (substring of a
/// session's name or host, any case) keeps matching sessions and opens their folders.
pub fn visible(tree: &[Node], expanded: &BTreeSet<String>, filter: &str) -> Vec<Row> {
    let filter = filter.trim().to_lowercase();
    let mut out = Vec::new();
    walk(tree, "", 0, expanded, &filter, &mut out);
    out
}

/// The rows worth drawing of `len` rows `row_h` high, `offset` scrolled into a view `height`
/// high (a screen's worth more on each side, so a fast scroll does not show a gap). An unknown
/// height (0: not laid out yet) draws the first rows as if it were 2000 high. An offset past
/// the end (the list got shorter) counts as the end.
pub fn on_screen(len: usize, row_h: f32, offset: f32, height: f32) -> std::ops::Range<usize> {
    // A list that fits a screen is drawn whole: iced reports no viewport for it, so its height
    // may be stale (the window grew since).
    if len as f32 * row_h <= 4000.0 {
        return 0..len;
    }
    let height = if height > 0.0 { height } else { 2000.0 };
    // A filter may have shortened the list since the last scroll: the view is at its end now.
    let offset = offset.min((len as f32 * row_h - height).max(0.0));
    let first = ((offset - height) / row_h).max(0.0) as usize;
    let last = ((offset + 2.0 * height) / row_h).ceil() as usize;
    first.min(len)..last.min(len)
}

/// Rows of `nodes` under `prefix`; with a filter, only those leading to a match (`true`: any).
fn walk(
    nodes: &[Node],
    prefix: &str,
    depth: u16,
    expanded: &BTreeSet<String>,
    filter: &str,
    out: &mut Vec<Row>,
) -> bool {
    let mut any = false;
    for n in nodes {
        let id = match prefix {
            "" => n.name().to_string(),
            p => format!("{p}/{}", n.name()),
        };
        match n {
            Node::Session { name, session } => {
                let hit = filter.is_empty()
                    || name.to_lowercase().contains(filter)
                    || session.host.to_lowercase().contains(filter);
                if hit {
                    out.push(Row {
                        depth,
                        id,
                        name: name.clone(),
                        folder: false,
                        expanded: false,
                        protocol: Some(session.protocol),
                        icon: session.icon.clone(),
                    });
                    any = true;
                }
            }
            Node::Folder { name, children } => {
                let at = out.len();
                let open = !filter.is_empty() || expanded.contains(&id);
                out.push(Row {
                    depth,
                    id: id.clone(),
                    name: name.clone(),
                    folder: true,
                    expanded: open,
                    protocol: None,
                    icon: String::new(),
                });
                let inner = match open {
                    true => walk(children, &id, depth + 1, expanded, filter, out),
                    false => false,
                };
                if !filter.is_empty() && !inner {
                    out.truncate(at); // nothing in it matches
                } else {
                    any = true;
                }
            }
        }
    }
    any
}

/// Ids from `from` to `to` (either order) among `rows`; just `to` if `from` is not shown.
pub fn range(rows: &[Row], from: &str, to: &str) -> Vec<String> {
    let pos = |id: &str| rows.iter().position(|r| r.id == id);
    match (pos(from), pos(to)) {
        (Some(a), Some(b)) => rows[a.min(b)..=a.max(b)]
            .iter()
            .map(|r| r.id.clone())
            .collect(),
        _ => vec![to.to_string()],
    }
}

/// The sessions `ids` stand for (a folder: all sessions in it), in tree order, each once.
pub fn sessions_under(tree: &[Node], ids: &BTreeSet<String>) -> Vec<String> {
    fn go(
        nodes: &[Node],
        prefix: &str,
        picked: bool,
        ids: &BTreeSet<String>,
        out: &mut Vec<String>,
    ) {
        for n in nodes {
            let id = match prefix {
                "" => n.name().to_string(),
                p => format!("{p}/{}", n.name()),
            };
            let picked = picked || ids.contains(&id);
            match n {
                Node::Session { .. } if picked => out.push(id),
                Node::Session { .. } => {}
                Node::Folder { children, .. } => go(children, &id, picked, ids, out),
            }
        }
    }
    let mut out = Vec::new();
    go(tree, "", false, ids, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;

    #[test]
    fn on_screen_draws_the_view_and_a_screen_around_it() {
        assert_eq!(on_screen(600, 25.0, 0.0, 500.0), 0..40);
        assert_eq!(on_screen(600, 25.0, 5000.0, 500.0), 180..240);
        assert_eq!(on_screen(600, 25.0, 14500.0, 500.0), 560..600);
        assert_eq!(on_screen(10, 25.0, 0.0, 500.0), 0..10);
        assert_eq!(on_screen(160, 25.0, 0.0, 100.0), 0..160);
        assert_eq!(on_screen(600, 25.0, 0.0, 0.0), 0..160);
        assert_eq!(on_screen(40, 25.0, 10000.0, 500.0), 0..40);
    }

    fn sess(name: &str, host: &str) -> Node {
        Node::Session {
            name: name.into(),
            session: Session {
                host: host.into(),
                ..Default::default()
            },
        }
    }

    fn folder(name: &str, children: Vec<Node>) -> Node {
        Node::Folder {
            name: name.into(),
            children,
        }
    }

    fn tree() -> Vec<Node> {
        vec![
            folder(
                "lab",
                vec![
                    folder("cisco", vec![sess("sw-core", "10.1.1.1")]),
                    sess("r1", "10.0.0.1"),
                ],
            ),
            sess("web", "web.example"),
        ]
    }

    fn ids(rows: &[Row]) -> Vec<&str> {
        rows.iter().map(|r| r.id.as_str()).collect()
    }

    fn set(v: &[&str]) -> BTreeSet<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn rows_collapsed() {
        let r = visible(&tree(), &set(&[]), "");
        assert_eq!(ids(&r), ["lab", "web"]);
        assert!(r[0].folder && !r[0].expanded);
    }

    #[test]
    fn rows_expanded() {
        let r = visible(&tree(), &set(&["lab", "lab/cisco"]), "");
        assert_eq!(
            ids(&r),
            ["lab", "lab/cisco", "lab/cisco/sw-core", "lab/r1", "web"]
        );
        assert_eq!(r[2].depth, 2);
        assert_eq!(r[2].name, "sw-core");
    }

    #[test]
    fn rows_filter_opens_parents() {
        let r = visible(&tree(), &set(&[]), "10.1");
        assert_eq!(ids(&r), ["lab", "lab/cisco", "lab/cisco/sw-core"]);
        let r = visible(&tree(), &set(&[]), "WEB");
        assert_eq!(ids(&r), ["web"]);
    }

    #[test]
    fn rows_filter_none() {
        assert!(visible(&tree(), &set(&["lab"]), "nothing").is_empty());
    }

    #[test]
    fn range_either_direction() {
        let r = visible(&tree(), &set(&["lab", "lab/cisco"]), "");
        assert_eq!(
            range(&r, "lab/r1", "lab/cisco"),
            ["lab/cisco", "lab/cisco/sw-core", "lab/r1"]
        );
        assert_eq!(
            range(&r, "lab/cisco", "lab/r1"),
            ["lab/cisco", "lab/cisco/sw-core", "lab/r1"]
        );
        assert_eq!(range(&r, "gone", "web"), ["web"]);
    }

    #[test]
    fn sessions_under_folder_recursive_dedup() {
        let got = sessions_under(&tree(), &set(&["lab", "lab/r1", "web"]));
        assert_eq!(got, ["lab/cisco/sw-core", "lab/r1", "web"]);
    }
}
