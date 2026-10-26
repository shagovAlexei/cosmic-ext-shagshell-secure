//! Script Builder (spec 36): the blocks of a `crt` script, `.py` <-> blocks.
//! `generate` writes the code; expressions are kept as Python text (as `ast.unparse` wrote them).

pub mod catalog;
mod parse;

pub use parse::{Error, parse};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Arg {
    /// `Some` for a keyword argument.
    pub name: Option<String>,
    /// A Python expression.
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Handler {
    /// The exception type; `None` = a bare `except:`.
    pub ty: Option<String>,
    pub name: Option<String>,
    pub body: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Block {
    /// `[assign =] func(args)`: a `crt` member (`catalog::lookup`) or any other function.
    Call {
        func: String,
        args: Vec<Arg>,
        assign: Option<String>,
    },
    Assign {
        target: String,
        value: String,
    },
    /// `if` and its `elif`s as branches (condition, body), then `else`.
    If {
        branches: Vec<(String, Vec<Block>)>,
        orelse: Vec<Block>,
    },
    For {
        target: String,
        iter: String,
        body: Vec<Block>,
    },
    While {
        cond: String,
        body: Vec<Block>,
    },
    /// `items`: `open(p) as f, …`.
    With {
        items: String,
        body: Vec<Block>,
    },
    Try {
        body: Vec<Block>,
        handlers: Vec<Handler>,
        /// `None`: no `finally:`.
        finally: Option<Vec<Block>>,
    },
    Def {
        name: String,
        params: String,
        body: Vec<Block>,
    },
    /// Without the `# `.
    Comment {
        text: String,
    },
    Blank,
    /// «Custom Python»: any statement, possibly several lines, written at level 0.
    Raw {
        code: String,
    },
}

/// The Python code of the blocks.
pub fn generate(blocks: &[Block]) -> String {
    let mut out = String::new();
    write(&mut out, blocks, 0);
    out
}

fn line(out: &mut String, level: usize, text: &str) {
    if !text.is_empty() {
        out.push_str(&"    ".repeat(level));
        out.push_str(text);
    }
    out.push('\n');
}

/// A compound statement's body: `pass` when it has no statement (Python needs one).
fn body(out: &mut String, blocks: &[Block], level: usize) {
    write(out, blocks, level);
    if blocks
        .iter()
        .all(|b| matches!(b, Block::Comment { .. } | Block::Blank))
    {
        line(out, level, "pass");
    }
}

fn write(out: &mut String, blocks: &[Block], level: usize) {
    for b in blocks {
        match b {
            Block::Call { func, args, assign } => {
                let args: Vec<String> = args
                    .iter()
                    .map(|a| match &a.name {
                        Some(n) => format!("{n}={}", a.value),
                        None => a.value.clone(),
                    })
                    .collect();
                let to = assign
                    .as_ref()
                    .map(|t| format!("{t} = "))
                    .unwrap_or_default();
                line(out, level, &format!("{to}{func}({})", args.join(", ")));
            }
            Block::Assign { target, value } => line(out, level, &format!("{target} = {value}")),
            Block::If { branches, orelse } => {
                for (i, (cond, b)) in branches.iter().enumerate() {
                    let kw = if i == 0 { "if" } else { "elif" };
                    line(out, level, &format!("{kw} {cond}:"));
                    body(out, b, level + 1);
                }
                if !orelse.is_empty() {
                    line(out, level, "else:");
                    write(out, orelse, level + 1);
                }
            }
            Block::For {
                target,
                iter,
                body: b,
            } => {
                line(out, level, &format!("for {target} in {iter}:"));
                body(out, b, level + 1);
            }
            Block::While { cond, body: b } => {
                line(out, level, &format!("while {cond}:"));
                body(out, b, level + 1);
            }
            Block::With { items, body: b } => {
                line(out, level, &format!("with {items}:"));
                body(out, b, level + 1);
            }
            Block::Try {
                body: b,
                handlers,
                finally,
            } => {
                line(out, level, "try:");
                body(out, b, level + 1);
                for h in handlers {
                    let head = match (&h.ty, &h.name) {
                        (Some(t), Some(n)) => format!("except {t} as {n}:"),
                        (Some(t), None) => format!("except {t}:"),
                        _ => "except:".to_string(),
                    };
                    line(out, level, &head);
                    body(out, &h.body, level + 1);
                }
                if finally.is_some() || handlers.is_empty() {
                    line(out, level, "finally:");
                    body(out, finally.as_deref().unwrap_or_default(), level + 1);
                }
            }
            Block::Def {
                name,
                params,
                body: b,
            } => {
                line(out, level, &format!("def {name}({params}):"));
                body(out, b, level + 1);
            }
            Block::Comment { text } if text.is_empty() => line(out, level, "#"),
            Block::Comment { text } => line(out, level, &format!("# {text}")),
            Block::Blank => line(out, level, ""),
            // ponytail: re-indents every line of Raw, a multi-line string literal inside a nested
            // Raw gets the indent too; keep Raw's own line offsets if that bites
            Block::Raw { code } => code.lines().for_each(|l| line(out, level, l)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(x: &str) -> String {
        x.to_string()
    }

    fn arg(v: &str) -> Arg {
        Arg {
            name: None,
            value: s(v),
        }
    }

    fn call(func: &str, args: &[&str]) -> Block {
        Block::Call {
            func: s(func),
            args: args.iter().map(|a| arg(a)).collect(),
            assign: None,
        }
    }

    #[test]
    fn gen_call() {
        let b = Block::Call {
            func: s("crt.Screen.WaitForString"),
            args: vec![
                arg("'#'"),
                arg("10"),
                Arg {
                    name: Some(s("bCaseInsensitive")),
                    value: s("True"),
                },
            ],
            assign: Some(s("ok")),
        };
        assert_eq!(
            generate(&[b]),
            "ok = crt.Screen.WaitForString('#', 10, bCaseInsensitive=True)\n"
        );
    }

    #[test]
    fn gen_nested() {
        let b = Block::For {
            target: s("c"),
            iter: s("cmds"),
            body: vec![Block::If {
                branches: vec![(s("c"), vec![call("crt.Screen.Send", &["c"])])],
                orelse: vec![Block::Raw { code: s("break") }],
            }],
        };
        assert_eq!(
            generate(&[b]),
            "for c in cmds:\n    if c:\n        crt.Screen.Send(c)\n    else:\n        break\n"
        );
    }

    #[test]
    fn gen_elif_try_def_with() {
        let b = Block::If {
            branches: vec![
                (s("a"), vec![call("f", &[])]),
                (s("b"), vec![call("g", &[])]),
            ],
            orelse: vec![],
        };
        assert_eq!(generate(&[b]), "if a:\n    f()\nelif b:\n    g()\n");
        let b = Block::Try {
            body: vec![call("f", &[])],
            handlers: vec![
                Handler {
                    ty: Some(s("OSError")),
                    name: Some(s("e")),
                    body: vec![call("g", &["e"])],
                },
                Handler {
                    ty: None,
                    name: None,
                    body: vec![call("h", &[])],
                },
            ],
            finally: Some(vec![call("k", &[])]),
        };
        assert_eq!(
            generate(&[b]),
            "try:\n    f()\nexcept OSError as e:\n    g(e)\nexcept:\n    h()\nfinally:\n    k()\n"
        );
        let b = Block::Def {
            name: s("f"),
            params: s("a, b=1"),
            body: vec![Block::Raw {
                code: s("return a"),
            }],
        };
        assert_eq!(generate(&[b]), "def f(a, b=1):\n    return a\n");
        let b = Block::With {
            items: s("open(p) as f"),
            body: vec![call("f.read", &[])],
        };
        assert_eq!(generate(&[b]), "with open(p) as f:\n    f.read()\n");
    }

    #[test]
    fn gen_empty_body_is_pass() {
        let b = Block::If {
            branches: vec![(s("x"), vec![])],
            orelse: vec![],
        };
        assert_eq!(generate(&[b]), "if x:\n    pass\n");
        let b = Block::Def {
            name: s("f"),
            params: s(""),
            body: vec![],
        };
        assert_eq!(generate(&[b]), "def f():\n    pass\n");
        let b = Block::Try {
            body: vec![],
            handlers: vec![],
            finally: None,
        };
        assert_eq!(generate(&[b]), "try:\n    pass\nfinally:\n    pass\n");
    }

    #[test]
    fn gen_comment_only_body_gets_pass() {
        let b = Block::If {
            branches: vec![(s("x"), vec![Block::Comment { text: s("c") }])],
            orelse: vec![],
        };
        assert_eq!(generate(&[b]), "if x:\n    # c\n    pass\n");
    }

    #[test]
    fn gen_comment_blank_raw() {
        let b = [
            Block::Comment { text: s("a") },
            Block::Comment { text: s("") },
            Block::Blank,
            Block::Def {
                name: s("f"),
                params: s(""),
                body: vec![Block::Raw {
                    code: s("class A:\n    x = 1"),
                }],
            },
        ];
        assert_eq!(
            generate(&b),
            "# a\n#\n\ndef f():\n    class A:\n        x = 1\n"
        );
        assert_eq!(generate(&[]), "");
    }

    /// `ast.dump` of a source: its meaning, without the layout and comments.
    fn ast_dump(src: &str) -> String {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut c = Command::new("python3")
            .args([
                "-I",
                "-c",
                "import ast,sys; print(ast.dump(ast.parse(sys.stdin.read())))",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        c.stdin
            .take()
            .unwrap()
            .write_all(src.trim_start_matches('\u{feff}').as_bytes())
            .unwrap();
        String::from_utf8(c.wait_with_output().unwrap().stdout).unwrap()
    }

    #[test]
    fn round_trip_examples() {
        use super::parse::tests::parse;
        if std::process::Command::new("python3")
            .arg("-V")
            .output()
            .is_err()
        {
            return;
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut files: Vec<(String, String)> = std::fs::read_dir(root.join("../../Example"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|x| x == "py"))
            .map(|p| {
                (
                    p.display().to_string(),
                    std::fs::read_to_string(&p).unwrap(),
                )
            })
            .collect();
        files.push((s("multiline.py"), s(include_str!("tests/multiline.py"))));
        let mut rec = crate::recorder::Recorder::new();
        rec.output(b"host$ ");
        rec.input(b"ls\r");
        files.push((s("recording"), rec.script("t")));
        assert!(files.len() > 10);
        for (name, src) in files {
            let b = parse(&src).unwrap_or_else(|e| panic!("{name}: {e}"));
            let g = generate(&b);
            assert_eq!(
                parse(&g).unwrap(),
                b,
                "{name}: blocks change on the second pass"
            );
            assert_eq!(
                generate(&parse(&g).unwrap()),
                g,
                "{name}: code changes on the second pass"
            );
            assert_eq!(ast_dump(&src), ast_dump(&g), "{name}: meaning changed");
            for c in src.lines().filter_map(|l| l.trim_start().strip_prefix('#')) {
                assert!(g.contains(c.trim()), "{name}: lost comment {c:?}");
            }
        }
    }
}
