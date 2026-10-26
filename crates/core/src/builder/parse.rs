//! `.py` → blocks: `res/python/shag_parse.py` reads the source with Python's own `ast` and
//! `tokenize` and prints the blocks as one JSON line.

use super::Block;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// Not Python 3 (a SecureCRT Python 2 script, a typo): the line and Python's words.
    Syntax { line: usize, msg: String },
    /// `python3` or the helper missing, or it failed.
    Python(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Error::Syntax { line, msg } => write!(f, "line {line}: {msg}"),
            Error::Python(e) => write!(f, "python3: {e}"),
        }
    }
}

/// The blocks of a script.
pub fn parse(src: &str) -> Result<Vec<Block>, Error> {
    parse_with(src, &crate::script::python_dir())
}

#[derive(serde::Deserialize)]
struct Bad {
    line: usize,
    msg: String,
}

#[derive(serde::Deserialize)]
struct Reply {
    ok: Option<Vec<Block>>,
    err: Option<Bad>,
}

fn parse_with(src: &str, dir: &Path) -> Result<Vec<Block>, Error> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let src = src.strip_prefix('\u{feff}').unwrap_or(src);
    let mut child = Command::new("python3")
        .arg("-I")
        .arg(dir.join("shag_parse.py"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Error::Python(e.to_string()))?;
    // A helper that died early closes its stdin: its stderr says why.
    let _ = child.stdin.take().map(|mut i| i.write_all(src.as_bytes()));
    let o = child
        .wait_with_output()
        .map_err(|e| Error::Python(e.to_string()))?;
    if !o.status.success() {
        return Err(Error::Python(
            String::from_utf8_lossy(&o.stderr).trim().to_string(),
        ));
    }
    match serde_json::from_slice::<Reply>(&o.stdout) {
        Ok(Reply { ok: Some(b), .. }) => Ok(b),
        Ok(Reply { err: Some(e), .. }) => Err(Error::Syntax {
            line: e.line,
            msg: e.msg,
        }),
        Ok(_) => Err(Error::Python("empty reply".into())),
        Err(e) => Err(Error::Python(e.to_string())),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::super::Arg;
    use super::*;

    /// The repository's helper: `script::python_dir` is the user's `scripts/.lib`, written at start.
    pub(crate) fn parse(src: &str) -> Result<Vec<Block>, Error> {
        parse_with(
            src,
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../res/python"),
        )
    }

    fn no_python() -> bool {
        std::process::Command::new("python3")
            .arg("-V")
            .output()
            .is_err()
    }

    fn s(x: &str) -> String {
        x.to_string()
    }

    fn assign(t: &str, v: &str) -> Block {
        Block::Assign {
            target: s(t),
            value: s(v),
        }
    }

    fn comment(t: &str) -> Block {
        Block::Comment { text: s(t) }
    }

    fn call0(f: &str) -> Block {
        Block::Call {
            func: s(f),
            args: vec![],
            assign: None,
        }
    }

    #[test]
    fn parse_call_and_assign() {
        if no_python() {
            return;
        }
        let b = parse("ok = crt.Screen.WaitForString('#', 10)\ncrt.Screen.Send('ls\\r')\nx = 5\n")
            .unwrap();
        assert_eq!(
            b,
            [
                Block::Call {
                    func: s("crt.Screen.WaitForString"),
                    args: vec![
                        Arg {
                            name: None,
                            value: s("'#'")
                        },
                        Arg {
                            name: None,
                            value: s("10")
                        }
                    ],
                    assign: Some(s("ok")),
                },
                Block::Call {
                    func: s("crt.Screen.Send"),
                    args: vec![Arg {
                        name: None,
                        value: s("'ls\\r'")
                    }],
                    assign: None,
                },
                assign("x", "5"),
            ]
        );
    }

    #[test]
    fn parse_keyword_args() {
        if no_python() {
            return;
        }
        let b = parse("n = crt.Screen.WaitForStrings('#', '>', timeout=10)\n").unwrap();
        let Block::Call { args, .. } = &b[0] else {
            panic!("{b:?}")
        };
        assert_eq!(
            args[2],
            Arg {
                name: Some(s("timeout")),
                value: s("10")
            }
        );
    }

    #[test]
    fn parse_elif_chain() {
        if no_python() {
            return;
        }
        let b = parse("if a:\n f()\nelif b:\n g()\nelse:\n h()\n").unwrap();
        assert_eq!(
            b,
            [Block::If {
                branches: vec![(s("a"), vec![call0("f")]), (s("b"), vec![call0("g")])],
                orelse: vec![call0("h")],
            }]
        );
    }

    #[test]
    fn parse_comments_and_blanks() {
        if no_python() {
            return;
        }
        let b = parse("# top\n\n\nx = 1  # tail\n\n# lead\ny = 2\n").unwrap();
        assert_eq!(
            b,
            [
                comment("top"),
                Block::Blank,
                comment("tail"),
                assign("x", "1"),
                Block::Blank,
                comment("lead"),
                assign("y", "2")
            ]
        );
    }

    #[test]
    fn parse_comments_in_bodies() {
        if no_python() {
            return;
        }
        let b =
            parse("for c in cmds:  # loop\n    # first\n    f(c)\n    # end of body\nelse_ = 1\n")
                .unwrap();
        assert_eq!(
            b,
            [
                comment("loop"),
                Block::For {
                    target: s("c"),
                    iter: s("cmds"),
                    body: vec![
                        comment("first"),
                        Block::Call {
                            func: s("f"),
                            args: vec![Arg {
                                name: None,
                                value: s("c")
                            }],
                            assign: None
                        }
                    ],
                },
                comment("end of body"),
                assign("else_", "1"),
            ]
        );
    }

    #[test]
    fn parse_pass_body_is_empty() {
        if no_python() {
            return;
        }
        assert_eq!(
            parse("while x:\n    pass\n").unwrap(),
            [Block::While {
                cond: s("x"),
                body: vec![]
            }]
        );
    }

    #[test]
    fn parse_try_with_def() {
        if no_python() {
            return;
        }
        let b = parse(
            "def f(a, b=1):\n    with open(a) as h:\n        h.read()\ntry:\n    f(1)\nexcept OSError as e:\n    g()\nexcept:\n    pass\nfinally:\n    k()\n",
        )
        .unwrap();
        assert_eq!(
            b,
            [
                Block::Def {
                    name: s("f"),
                    params: s("a, b=1"),
                    body: vec![Block::With {
                        items: s("open(a) as h"),
                        body: vec![call0("h.read")]
                    }],
                },
                Block::Try {
                    body: vec![Block::Call {
                        func: s("f"),
                        args: vec![Arg {
                            name: None,
                            value: s("1")
                        }],
                        assign: None
                    }],
                    handlers: vec![
                        super::super::Handler {
                            ty: Some(s("OSError")),
                            name: Some(s("e")),
                            body: vec![call0("g")]
                        },
                        super::super::Handler {
                            ty: None,
                            name: None,
                            body: vec![]
                        },
                    ],
                    finally: Some(vec![call0("k")]),
                },
            ]
        );
    }

    #[test]
    fn parse_raw() {
        if no_python() {
            return;
        }
        let b = parse("import os\nx += 1\nclass A:\n    pass\n").unwrap();
        assert_eq!(
            b,
            [
                Block::Raw {
                    code: s("import os")
                },
                Block::Raw { code: s("x += 1") },
                Block::Raw {
                    code: s("class A:\n    pass")
                },
            ]
        );
    }

    #[test]
    fn parse_python2_is_syntax_error() {
        if no_python() {
            return;
        }
        assert!(matches!(
            parse("x = 1\nprint \"hi\"\n"),
            Err(Error::Syntax { line: 2, .. })
        ));
    }

    #[test]
    fn parse_crlf_and_bom() {
        if no_python() {
            return;
        }
        assert_eq!(
            parse("\u{feff}x = 1\r\n# c\r\ny = 2\r\n").unwrap(),
            [assign("x", "1"), comment("c"), assign("y", "2")]
        );
    }

    #[test]
    fn parse_keeps_unicode() {
        if no_python() {
            return;
        }
        assert_eq!(
            parse("# привет\ns = 'мир'\n").unwrap(),
            [comment("привет"), assign("s", "'мир'")]
        );
    }

    #[test]
    fn parse_without_python() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            parse_with("x = 1\n", dir.path()),
            Err(Error::Python(_))
        ));
    }
}
