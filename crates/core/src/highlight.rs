//! Keyword highlighting (SecureCRT's): words in the session's output shown in a colour. The rules
//! are in `~/.config/shagshell/highlights.toml`; finding the words in a line is here, painting
//! them is the terminal's.

use serde::{Deserialize, Serialize};
use std::io::{self, ErrorKind};
use std::ops::Range;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Colour {
    #[default]
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    Orange,
    White,
}

impl Colour {
    pub const ALL: [Colour; 8] = [
        Colour::Red,
        Colour::Green,
        Colour::Yellow,
        Colour::Blue,
        Colour::Magenta,
        Colour::Cyan,
        Colour::Orange,
        Colour::White,
    ];

    /// 0xRRGGBB, readable on a dark and on a light terminal alike.
    pub fn rgb(self) -> u32 {
        match self {
            Colour::Red => 0xE5484D,
            Colour::Green => 0x30A46C,
            Colour::Yellow => 0xF5D90A,
            Colour::Blue => 0x3E63DD,
            Colour::Magenta => 0xAB4ABA,
            Colour::Cyan => 0x05A2C2,
            Colour::Orange => 0xF76B15,
            Colour::White => 0xF0F0F0,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rule {
    /// A text to find (case does not matter).
    pub text: String,
    pub colour: Colour,
    /// Fill behind the text instead of colouring the letters.
    pub background: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rules {
    #[serde(rename = "rule")]
    pub rules: Vec<Rule>,
}

pub fn default_path() -> PathBuf {
    crate::session::store::default_root().parent().map_or_else(
        || PathBuf::from("highlights.toml"),
        |p| p.join("highlights.toml"),
    )
}

impl Rules {
    /// Missing: none yet. A broken file is an error, never an empty set to be written back.
    pub fn load(path: &Path) -> io::Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                toml::from_str(&text).map_err(|e| io::Error::new(ErrorKind::InvalidData, e))
            }
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e),
        }
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = toml::to_string(self).map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)
    }
}

/// Where `rules` find something in `text`: byte ranges (on character boundaries) with the index
/// of the rule. Where two overlap, the earlier rule (then the earlier place) wins.
pub fn find(rules: &[Rule], text: &str) -> Vec<(Range<usize>, usize)> {
    let mut taken: Vec<(Range<usize>, usize)> = Vec::new();
    // Lowercasing may change lengths for a few letters: then the position is not trusted and the
    // line is searched as it is (those letters do not match, which is harmless).
    let lower = text.to_lowercase();
    let same = lower.len() == text.len();
    let hay = if same { lower.as_str() } else { text };
    for (i, r) in rules.iter().enumerate() {
        let needle = r.text.to_lowercase();
        if needle.is_empty() {
            continue;
        }
        let mut from = 0;
        while let Some(at) = hay[from..].find(&needle) {
            let range = from + at..from + at + needle.len();
            from = range.end;
            let free = taken
                .iter()
                .all(|(t, _)| t.end <= range.start || range.end <= t.start);
            if free && text.is_char_boundary(range.start) && text.is_char_boundary(range.end) {
                taken.push((range, i));
            }
        }
    }
    taken.sort_by_key(|(r, _)| r.start);
    taken
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(text: &str, colour: Colour) -> Rule {
        Rule {
            text: text.into(),
            colour,
            background: false,
        }
    }

    #[test]
    fn finds_words_without_regard_to_case_and_first_rule_wins() {
        let rules = [
            rule("error", Colour::Red),
            rule("err", Colour::Blue),
            rule("up", Colour::Green),
        ];
        let hits = find(&rules, "Port Gi0/1 is UP, ERROR: err-disabled");
        let words: Vec<(&str, usize)> = hits
            .iter()
            .map(|(r, i)| (&"Port Gi0/1 is UP, ERROR: err-disabled"[r.clone()], *i))
            .collect();
        assert_eq!(words, [("UP", 2), ("ERROR", 0), ("err", 1)]);
    }

    #[test]
    fn utf8_and_empty_rules() {
        let rules = [rule("", Colour::Red), rule("żółć", Colour::Cyan)];
        let text = "a ŻÓŁĆ b";
        let hits = find(&rules, text);
        assert_eq!(hits.len(), 1);
        assert_eq!(&text[hits[0].0.clone()], "ŻÓŁĆ");
        assert!(find(&rules, "").is_empty());
    }

    #[test]
    fn rules_round_trip() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("x/highlights.toml");
        let r = Rules {
            rules: vec![
                Rule {
                    text: "down".into(),
                    colour: Colour::Orange,
                    background: true,
                },
                rule("up", Colour::Green),
            ],
        };
        r.save(&p).unwrap();
        assert_eq!(Rules::load(&p).unwrap(), r);
        assert_eq!(
            Rules::load(&d.path().join("none")).unwrap(),
            Rules::default()
        );
        std::fs::write(&p, "[[rule]\n").unwrap();
        assert!(Rules::load(&p).is_err());
    }
}
