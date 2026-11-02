//! The Active Sessions list: the open tabs, in «Sessions» and «Editors», narrowed by a filter.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A terminal or an SFTP tab.
    Session,
    /// A note, the Script Builder, the console.
    Editor,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub id: u64,
    pub kind: Kind,
    pub title: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Section {
    pub kind: Kind,
    pub rows: Vec<Row>,
}

/// The rows whose title contains `filter` (any case), by kind, tab order kept; a kind with
/// no row is left out.
pub fn group(rows: &[Row], filter: &str) -> Vec<Section> {
    let f = filter.trim().to_lowercase();
    [Kind::Session, Kind::Editor]
        .into_iter()
        .filter_map(|kind| {
            let rows: Vec<Row> = (rows.iter())
                .filter(|r| r.kind == kind && r.title.to_lowercase().contains(&f))
                .cloned()
                .collect();
            (!rows.is_empty()).then_some(Section { kind, rows })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(id: u64, kind: Kind, title: &str) -> Row {
        Row {
            id,
            kind,
            title: title.into(),
        }
    }

    #[test]
    fn splits_by_kind_and_keeps_order() {
        let rows = [
            r(1, Kind::Editor, "notes.md"),
            r(2, Kind::Session, "web-01"),
            r(3, Kind::Session, "db"),
        ];
        let s = group(&rows, "");
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].kind, Kind::Session);
        assert_eq!(s[0].rows.iter().map(|r| r.id).collect::<Vec<_>>(), [2, 3]);
        assert_eq!(s[1].rows[0].id, 1);
    }

    #[test]
    fn filter_is_case_insensitive_and_drops_empty_sections() {
        let rows = [
            r(1, Kind::Editor, "notes.md"),
            r(2, Kind::Session, "Web-01"),
        ];
        let s = group(&rows, " WEB");
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].rows[0].id, 2);
        assert!(group(&rows, "zzz").is_empty());
    }
}
