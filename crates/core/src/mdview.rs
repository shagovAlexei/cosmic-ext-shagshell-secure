//! A markdown file as plain text plus the look of each stretch of it (the notes' formatted
//! view): the app shows the text in a read-only editor, so the mouse selects and copies it.
//! Headings, bold, italic, code, links and quotes keep their looks; sizes and pictures do not.

use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use std::ops::Range;

/// What one stretch of text looks like.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Look {
    /// 1–6 for a heading's text, 0 otherwise.
    pub heading: u8,
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    pub link: bool,
    pub quote: bool,
    /// A list marker or a rule: dimmed.
    pub mark: bool,
}

/// The looks of each line's byte ranges.
pub type Lines = Vec<Vec<(Range<usize>, Look)>>;

/// The text as shown and, per line, the looks of its byte ranges (in order, not overlapping).
#[derive(Debug, Default)]
pub struct View {
    pub text: String,
    pub lines: Lines,
}

#[derive(Default)]
struct State {
    out: String,
    segs: Vec<(Range<usize>, Look)>,
    heading: u8,
    strong: u32,
    em: u32,
    link: u32,
    quote: u32,
    code_block: bool,
    /// Per open list: the next number, `None` for a bullet list.
    lists: Vec<Option<u64>>,
    urls: Vec<String>,
}

impl State {
    fn look(&self) -> Look {
        Look {
            heading: self.heading,
            bold: self.strong > 0,
            italic: self.em > 0,
            code: self.code_block,
            link: self.link > 0,
            quote: self.quote > 0,
            mark: false,
        }
    }

    fn push(&mut self, s: &str, look: Look) {
        if s.is_empty() {
            return;
        }
        let start = self.out.len();
        self.out.push_str(s);
        self.segs.push((start..self.out.len(), look));
    }

    fn text(&mut self, s: &str) {
        let look = self.look();
        self.push(s, look);
    }

    /// The end of a block: the line ends, and outside a list a blank line follows.
    fn end_block(&mut self) {
        if !self.out.is_empty() && !self.out.ends_with('\n') {
            self.out.push('\n');
        }
        if self.lists.is_empty() && !self.out.ends_with("\n\n") {
            self.out.push('\n');
        }
    }
}

pub fn render(md: &str) -> View {
    let mut s = State::default();
    for ev in Parser::new(md) {
        match ev {
            Event::Start(Tag::Heading { level, .. }) => s.heading = level as u8,
            Event::End(TagEnd::Heading(_)) => {
                s.heading = 0;
                s.end_block();
            }
            Event::Start(Tag::Paragraph) => {
                if s.quote > 0 {
                    let look = Look {
                        quote: true,
                        mark: true,
                        ..Look::default()
                    };
                    s.push("▎ ", look);
                }
            }
            Event::End(TagEnd::Paragraph) => s.end_block(),
            Event::Start(Tag::BlockQuote(_)) => s.quote += 1,
            Event::End(TagEnd::BlockQuote(_)) => s.quote = s.quote.saturating_sub(1),
            Event::Start(Tag::CodeBlock(_)) => s.code_block = true,
            Event::End(TagEnd::CodeBlock) => {
                s.code_block = false;
                s.end_block();
            }
            Event::Start(Tag::List(first)) => s.lists.push(first),
            Event::End(TagEnd::List(_)) => {
                s.lists.pop();
                if s.lists.is_empty() {
                    s.end_block();
                }
            }
            Event::Start(Tag::Item) => {
                if !s.out.is_empty() && !s.out.ends_with('\n') {
                    s.out.push('\n');
                }
                let indent = "  ".repeat(s.lists.len().saturating_sub(1));
                let marker = match s.lists.last_mut() {
                    Some(Some(n)) => {
                        *n += 1;
                        format!("{indent}{}. ", *n - 1)
                    }
                    _ => format!("{indent}• "),
                };
                let look = Look {
                    mark: true,
                    ..Look::default()
                };
                s.push(&marker, look);
            }
            Event::End(TagEnd::Item) => {
                if !s.out.ends_with('\n') {
                    s.out.push('\n');
                }
            }
            Event::Start(Tag::Strong) => s.strong += 1,
            Event::End(TagEnd::Strong) => s.strong = s.strong.saturating_sub(1),
            Event::Start(Tag::Emphasis) => s.em += 1,
            Event::End(TagEnd::Emphasis) => s.em = s.em.saturating_sub(1),
            Event::Start(Tag::Link { dest_url, .. }) => {
                s.link += 1;
                s.urls.push(dest_url.to_string());
            }
            Event::End(TagEnd::Link) => {
                // The address stays visible (and copyable): the view cannot be clicked.
                if let Some(url) = s.urls.pop() {
                    s.text(&format!(" ({url})"));
                }
                s.link = s.link.saturating_sub(1);
            }
            Event::Text(t) | Event::Html(t) | Event::InlineHtml(t) => s.text(&t),
            Event::Code(t) => {
                let look = Look {
                    code: true,
                    ..s.look()
                };
                s.push(&t, look);
            }
            Event::SoftBreak => s.text(" "),
            Event::HardBreak => s.text("\n"),
            Event::Rule => {
                let look = Look {
                    mark: true,
                    ..Look::default()
                };
                s.push("────────────────\n\n", look);
            }
            _ => {}
        }
    }
    let kept = s.out.trim_end_matches('\n').len();
    s.out.truncate(kept);
    split_lines(s.out, s.segs)
}

/// The stretches cut at the line ends and given in each line's own bytes.
fn split_lines(text: String, segs: Vec<(Range<usize>, Look)>) -> View {
    let mut starts = vec![0];
    starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
    let mut lines = vec![Vec::new(); starts.len()];
    for (range, look) in segs {
        let end = range.end.min(text.len());
        let mut at = range.start;
        while at < end {
            let line = starts.partition_point(|&s| s <= at) - 1;
            let line_end = text[at..end].find('\n').map_or(end, |n| at + n);
            if line_end > at {
                lines[line].push((at - starts[line]..line_end - starts[line], look));
            }
            at = line_end + 1;
        }
    }
    View { text, lines }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lk(f: impl FnOnce(&mut Look)) -> Look {
        let mut l = Look::default();
        f(&mut l);
        l
    }

    #[test]
    fn a_heading_and_a_paragraph_with_bold() {
        let v = render("# Hi\n\ntext **b** end");
        assert_eq!(v.text, "Hi\n\ntext b end");
        assert_eq!(v.lines[0], vec![(0..2, lk(|l| l.heading = 1))]);
        assert!(v.lines[1].is_empty());
        assert_eq!(
            v.lines[2],
            vec![
                (0..5, Look::default()),
                (5..6, lk(|l| l.bold = true)),
                (6..10, Look::default())
            ]
        );
    }

    #[test]
    fn lists_are_marked_and_numbered() {
        let v = render("- a\n- b\n\n1. x\n2. y\n");
        assert_eq!(v.text, "• a\n• b\n\n1. x\n2. y");
        assert_eq!(v.lines[0][0], (0..4, lk(|l| l.mark = true)));
    }

    #[test]
    fn a_code_block_keeps_its_lines_and_a_link_its_address() {
        let v = render("```\nls -la\npwd\n```\n\n[site](https://x.org)");
        assert_eq!(v.text, "ls -la\npwd\n\nsite (https://x.org)");
        assert_eq!(v.lines[1], vec![(0..3, lk(|l| l.code = true))]);
        assert!(v.lines[3].iter().any(|(_, l)| l.link));
    }

    #[test]
    fn regression_ranges_stay_inside_their_lines() {
        let v = render("para one\nsecond *line*\n\n> quote\n\n---\n\ntail");
        for (i, line) in v.text.split('\n').enumerate() {
            for (r, _) in &v.lines[i] {
                assert!(
                    r.end <= line.len()
                        && line.is_char_boundary(r.start)
                        && line.is_char_boundary(r.end)
                );
            }
        }
        assert_eq!(v.lines.len(), v.text.split('\n').count());
    }
}
