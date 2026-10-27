//! Script Builder (spec 36): the blocks of a `crt` script, `.py` <-> blocks.
//! `generate` writes the code; expressions are kept as Python text (as `ast.unparse` wrote them).

pub mod catalog;
pub mod field;
mod parse;
pub mod tape;

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
    /// «For each host»: the body as a function of (tab, host), called by `shag_blocks.each_host`
    /// for every host (in threads when `parallel`, `limit` at a time).
    ForHost {
        hosts: String,
        func: String,
        parallel: bool,
        limit: String,
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

/// The code and every block's first line (1-based), in the order written.
pub fn generate_map(blocks: &[Block]) -> (String, Vec<(tape::Pos, usize)>) {
    let mut w = Writer::default();
    w.list(blocks, 0, &[], 0);
    (w.out, w.map)
}

/// The block written last at or before `line` (where a syntax error of the code is).
pub fn at_line(map: &[(tape::Pos, usize)], line: usize) -> Option<&tape::Pos> {
    map.iter()
        .take_while(|(_, l)| *l <= line)
        .last()
        .map(|(p, _)| p)
}

/// A «for each host» function name no block uses yet: `_host_1`, `_host_2`…
pub fn fresh_host_func(blocks: &[Block]) -> String {
    fn walk(l: &[Block], used: &mut Vec<String>) {
        for b in l {
            if let Block::ForHost { func, .. } = b {
                used.push(func.clone());
            }
            for body in tape::bodies(b) {
                walk(body, used);
            }
        }
    }
    let mut used = vec![];
    walk(blocks, &mut used);
    (1..)
        .map(|n| format!("_host_{n}"))
        .find(|n| !used.contains(n))
        .unwrap_or_default()
}

/// The Python code of the blocks.
pub fn generate(blocks: &[Block]) -> String {
    generate_map(blocks).0
}

#[derive(Default)]
struct Writer {
    out: String,
    map: Vec<(tape::Pos, usize)>,
    lines: usize,
}

impl Writer {
    fn line(&mut self, level: usize, text: &str) {
        if !text.is_empty() {
            self.out.push_str(&"    ".repeat(level));
            self.out.push_str(text);
        }
        self.out.push('\n');
        self.lines += 1;
    }

    /// A compound statement's body (`body`-th of the block at `at`): `pass` when it has no
    /// statement (Python needs one).
    fn body(&mut self, blocks: &[Block], level: usize, at: &[(usize, usize)], body: usize) {
        self.list(blocks, level, at, body);
        if blocks
            .iter()
            .all(|b| matches!(b, Block::Comment { .. } | Block::Blank))
        {
            self.line(level, "pass");
        }
    }

    /// The blocks of the `body`-th list of the block at `at` (the top list: `at` empty, body 0).
    fn list(&mut self, blocks: &[Block], level: usize, at: &[(usize, usize)], body: usize) {
        for (i, b) in blocks.iter().enumerate() {
            let mut pos = at.to_vec();
            pos.push((body, i));
            self.map.push((pos.clone(), self.lines + 1));
            self.block(b, level, &pos);
        }
    }

    fn block(&mut self, b: &Block, level: usize, pos: &[(usize, usize)]) {
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
                self.line(level, &format!("{to}{func}({})", args.join(", ")));
            }
            Block::Assign { target, value } => self.line(level, &format!("{target} = {value}")),
            Block::If { branches, orelse } => {
                for (i, (cond, b)) in branches.iter().enumerate() {
                    let kw = if i == 0 { "if" } else { "elif" };
                    self.line(level, &format!("{kw} {cond}:"));
                    self.body(b, level + 1, pos, i);
                }
                if !orelse.is_empty() {
                    self.line(level, "else:");
                    self.body(orelse, level + 1, pos, branches.len());
                }
            }
            Block::For { target, iter, body } => {
                self.line(level, &format!("for {target} in {iter}:"));
                self.body(body, level + 1, pos, 0);
            }
            Block::While { cond, body } => {
                self.line(level, &format!("while {cond}:"));
                self.body(body, level + 1, pos, 0);
            }
            Block::With { items, body } => {
                self.line(level, &format!("with {items}:"));
                self.body(body, level + 1, pos, 0);
            }
            Block::Try {
                body,
                handlers,
                finally,
            } => {
                self.line(level, "try:");
                self.body(body, level + 1, pos, 0);
                for (i, h) in handlers.iter().enumerate() {
                    let head = match (&h.ty, &h.name) {
                        (Some(t), Some(n)) => format!("except {t} as {n}:"),
                        (Some(t), None) => format!("except {t}:"),
                        _ => "except:".to_string(),
                    };
                    self.line(level, &head);
                    self.body(&h.body, level + 1, pos, i + 1);
                }
                if finally.is_some() || handlers.is_empty() {
                    self.line(level, "finally:");
                    let f = finally.as_deref().unwrap_or_default();
                    self.body(f, level + 1, pos, handlers.len() + 1);
                }
            }
            Block::Def { name, params, body } => {
                self.line(level, &format!("def {name}({params}):"));
                self.body(body, level + 1, pos, 0);
            }
            Block::ForHost {
                hosts,
                func,
                parallel,
                limit,
                body,
            } => {
                self.line(level, &format!("def {func}(tab, host):"));
                self.body(body, level + 1, pos, 0);
                let p = if *parallel { "True" } else { "False" };
                self.line(
                    level,
                    &format!("each_host({hosts}, {func}, parallel={p}, limit={limit})"),
                );
            }
            Block::Comment { text } if text.is_empty() => self.line(level, "#"),
            Block::Comment { text } => self.line(level, &format!("# {text}")),
            Block::Blank => self.line(level, ""),
            // ponytail: re-indents every line of Raw, a multi-line string literal inside a nested
            // Raw gets the indent too; keep Raw's own line offsets if that bites
            Block::Raw { code } => code.lines().for_each(|l| self.line(level, l)),
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
    fn map_nested_lines() {
        let b = [
            Block::Comment { text: s("a") },
            Block::For {
                target: s("x"),
                iter: s("y"),
                body: vec![
                    call("f", &[]),
                    Block::If {
                        branches: vec![(s("c"), vec![call("g", &[])])],
                        orelse: vec![call("h", &[])],
                    },
                ],
            },
        ];
        let (code, map) = generate_map(&b);
        assert_eq!(code, generate(&b));
        assert_eq!(
            map,
            [
                (vec![(0, 0)], 1),
                (vec![(0, 1)], 2),
                (vec![(0, 1), (0, 0)], 3),
                (vec![(0, 1), (0, 1)], 4),
                (vec![(0, 1), (0, 1), (0, 0)], 5),
                (vec![(0, 1), (0, 1), (1, 0)], 7),
            ]
        );
    }

    #[test]
    fn at_line_picks_last_started() {
        let map = vec![
            (vec![(0, 0)], 1),
            (vec![(0, 1)], 2),
            (vec![(0, 1), (0, 0)], 3),
        ];
        assert_eq!(at_line(&map, 3), Some(&vec![(0, 1), (0, 0)]));
        assert_eq!(at_line(&map, 2), Some(&vec![(0, 1)]));
        assert_eq!(at_line(&map, 9), Some(&vec![(0, 1), (0, 0)]));
        assert_eq!(at_line(&map, 0), None);
    }

    #[test]
    fn gen_for_host() {
        let b = Block::ForHost {
            hosts: s("hosts"),
            func: s("_host_1"),
            parallel: true,
            limit: s("10"),
            body: vec![call("run", &["tab", "'uptime'"])],
        };
        assert_eq!(
            generate(&[b]),
            "def _host_1(tab, host):\n    run(tab, 'uptime')\neach_host(hosts, _host_1, parallel=True, limit=10)\n"
        );
    }

    #[test]
    fn host_names_are_fresh() {
        let used = |n: &str| Block::ForHost {
            hosts: s("h"),
            func: s(n),
            parallel: false,
            limit: s("10"),
            body: vec![],
        };
        assert_eq!(fresh_host_func(&[]), "_host_1");
        let inner = Block::If {
            branches: vec![(s("x"), vec![used("_host_2")])],
            orelse: vec![],
        };
        assert_eq!(fresh_host_func(&[used("_host_1"), inner]), "_host_3");
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
        for (n, t) in crate::bundled::TEMPLATES {
            files.push((s(n), s(t)));
        }
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

    fn no_python() -> bool {
        std::process::Command::new("python3")
            .arg("-V")
            .output()
            .is_err()
    }

    #[test]
    fn regression_else_pass_comment() {
        if no_python() {
            return;
        }
        let src = "if a:\n    f()\nelse:\n    pass  # nothing\n";
        let g = generate(&super::parse::tests::parse(src).unwrap());
        assert_eq!(ast_dump(src), ast_dump(&g), "{g}");
    }

    #[test]
    fn regression_call_on_expression() {
        if no_python() {
            return;
        }
        for src in [
            "(lambda: 1)()\n",
            "(a or b)()\n",
            "x = (a if c else b)(1)\n",
            "(-a)()\n",
            "x = (a < b)()\n",
        ] {
            let g = generate(&super::parse::tests::parse(src).unwrap());
            assert_eq!(ast_dump(src), ast_dump(&g), "{src} -> {g}");
        }
    }

    #[test]
    fn regression_shared_line_comment() {
        if no_python() {
            return;
        }
        for src in ["x = 1; y = 2  # c\n", "if a: f()  # c\n"] {
            let g = generate(&super::parse::tests::parse(src).unwrap());
            assert_eq!(g.matches("# c").count(), 1, "{src} -> {g}");
        }
    }
}
