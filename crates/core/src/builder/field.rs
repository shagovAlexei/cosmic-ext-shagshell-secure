//! A block's field: what it shows and what it takes. A value is Python text; a text field shows
//! a string literal as its plain text, anything else as `=` + the expression (`=cmd + "\r"`), and
//! takes `=…` back as an expression.

use super::catalog::{Entry, Kind, Ty};
use super::{Block, Handler};

/// A Python string literal (as `repr`): `'…'`, or `"…"` when the text has `'` and no `"`.
pub fn literal(text: &str) -> String {
    let q = if text.contains('\'') && !text.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::from(q);
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == q => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\x{:02x}", c as u32))
            }
            c => out.push(c),
        }
    }
    out.push(q);
    out
}

/// The text of a plain `'…'` / `"…"` literal (no prefix, no concatenation); `None` otherwise.
pub fn unliteral(value: &str) -> Option<String> {
    let mut chars = value.chars();
    let q = chars.next().filter(|c| *c == '\'' || *c == '"')?;
    let inner = value[1..].strip_suffix(q)?;
    if inner.starts_with(q) {
        return None; // '''…''' or an empty literal followed by another
    }
    let mut out = String::new();
    let mut it = inner.chars();
    while let Some(c) = it.next() {
        match c {
            '\\' => {
                let e = it.next()?;
                match e {
                    '\\' | '\'' | '"' => out.push(e),
                    'n' => out.push('\n'),
                    'r' => out.push('\r'),
                    't' => out.push('\t'),
                    '0' => out.push('\0'),
                    'x' | 'u' | 'U' => {
                        let n = match e {
                            'x' => 2,
                            'u' => 4,
                            _ => 8,
                        };
                        let hex: String = it.by_ref().take(n).collect();
                        let code =
                            (hex.len() == n).then(|| u32::from_str_radix(&hex, 16).ok())??;
                        out.push(char::from_u32(code)?);
                    }
                    _ => return None,
                }
            }
            c if c == q || c == '\n' => return None,
            c => out.push(c),
        }
    }
    Some(out)
}

fn is_number(v: &str) -> bool {
    !v.is_empty() && v.parse::<f64>().is_ok() && !v.contains(|c: char| c.is_alphabetic())
}

/// What the field of a parameter of type `ty` shows for `value`.
pub fn shown(ty: Ty, value: &str) -> String {
    let fits = match ty {
        Ty::Text | Ty::File | Ty::Session => {
            return unliteral(value).unwrap_or_else(|| format!("={value}"));
        }
        Ty::Expr => true,
        Ty::Number => is_number(value),
        Ty::Bool => value == "True" || value == "False",
        Ty::Choice(names) => names.contains(&value) || is_number(value),
    };
    match fits {
        true => value.to_string(),
        false => format!("={value}"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bad {
    Required,
    NotNumber,
}

/// The value a field typed `input` holds.
pub fn typed(ty: Ty, input: &str) -> Result<String, Bad> {
    if let Some(e) = input.strip_prefix('=') {
        return match e.trim() {
            "" => Err(Bad::Required),
            e => Ok(e.to_string()),
        };
    }
    match ty {
        Ty::Text | Ty::File | Ty::Session => Ok(literal(input)),
        Ty::Number => match input.trim() {
            "" => Err(Bad::Required),
            n if is_number(n) => Ok(n.to_string()),
            _ => Err(Bad::NotNumber),
        },
        Ty::Bool | Ty::Choice(_) | Ty::Expr => match input.trim() {
            "" => Err(Bad::Required),
            v => Ok(v.to_string()),
        },
    }
}

/// The Python text a call / property of `e` starts with: `crt.Screen.Send`, `tab.Activate`.
fn func_of(e: &Entry) -> String {
    match e.path.strip_prefix("Tab.") {
        Some(m) => format!("tab.{m}"),
        None => format!("crt.{}", e.path),
    }
}

/// A new block for a catalog entry: its required parameters filled with empty values.
pub fn new_block(e: &Entry) -> Block {
    if e.kind == Kind::Property {
        return Block::Assign {
            target: "value".into(),
            value: func_of(e),
        };
    }
    let args = (e.params.iter())
        .filter(|p| p.default.is_none() && !p.name.starts_with("**"))
        .map(|p| super::Arg {
            name: None,
            value: match p.ty {
                Ty::Text | Ty::File | Ty::Session => "''".into(),
                Ty::Number => "0".into(),
                Ty::Bool => "False".into(),
                Ty::Choice(names) => names.first().copied().unwrap_or("0").into(),
                Ty::Expr => "None".into(),
            },
        })
        .collect();
    Block::Call {
        func: func_of(e),
        args,
        assign: None,
    }
}

/// A new statement block of the palette's «Other»: `if`, `for`, `while`, `with`, `try`, `def`,
/// `assign`, `call`, `comment`, `raw`.
pub fn new_statement(kind: &str) -> Option<Block> {
    let s = |x: &str| x.to_string();
    Some(match kind {
        "if" => Block::If {
            branches: vec![(s("True"), vec![])],
            orelse: vec![],
        },
        "for" => Block::For {
            target: s("line"),
            iter: s("lines"),
            body: vec![],
        },
        "while" => Block::While {
            cond: s("True"),
            body: vec![],
        },
        "with" => Block::With {
            items: s("open('file.txt') as f"),
            body: vec![],
        },
        "try" => Block::Try {
            body: vec![],
            handlers: vec![Handler {
                ty: Some(s("Exception")),
                name: Some(s("e")),
                body: vec![],
            }],
            finally: None,
        },
        "def" => Block::Def {
            name: s("name"),
            params: s(""),
            body: vec![],
        },
        "assign" => Block::Assign {
            target: s("x"),
            value: s("0"),
        },
        "call" => Block::Call {
            func: s("name"),
            args: vec![],
            assign: None,
        },
        "comment" => Block::Comment { text: s("") },
        "raw" => Block::Raw { code: s("pass") },
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::super::catalog::lookup;
    use super::*;

    #[test]
    fn literal_round_trip() {
        for (t, lit) in [
            ("a", "'a'"),
            ("it's", "\"it's\""),
            ("\r\n", "'\\r\\n'"),
            ("тест", "'тест'"),
            ("\u{1}", "'\\x01'"),
            ("a\\b", "'a\\\\b'"),
            ("'\"", "'\\'\"'"),
        ] {
            assert_eq!(literal(t), lit, "{t:?}");
            assert_eq!(unliteral(lit).as_deref(), Some(t), "{lit}");
        }
        assert_eq!(unliteral("\"show\\tver\""), Some("show\tver".into()));
    }

    #[test]
    fn unliteral_rejects_prefixes_and_expressions() {
        for v in [
            "f'a'",
            "b'a'",
            "r'a'",
            "'a' + 'b'",
            "'a' 'b'",
            "x",
            "'''a'''",
            "'a",
            "",
            "'a\\q'",
        ] {
            assert_eq!(unliteral(v), None, "{v}");
        }
    }

    #[test]
    fn typed_text_and_expression() {
        assert_eq!(typed(Ty::Text, "show ver\\r"), Ok("'show ver\\\\r'".into()));
        assert_eq!(typed(Ty::Text, "=cmd + '\\r'"), Ok("cmd + '\\r'".into()));
        assert_eq!(typed(Ty::Text, ""), Ok("''".into()));
        assert_eq!(typed(Ty::Text, "="), Err(Bad::Required));
        assert_eq!(shown(Ty::Text, "'show ver'"), "show ver");
        assert_eq!(shown(Ty::Text, "cmd + '\\r'"), "=cmd + '\\r'");
        assert_eq!(shown(Ty::Session, "'/S \"r1\"'"), "/S \"r1\"");
    }

    #[test]
    fn typed_number() {
        assert_eq!(typed(Ty::Number, " 30 "), Ok("30".into()));
        assert_eq!(typed(Ty::Number, "-1.5"), Ok("-1.5".into()));
        assert_eq!(typed(Ty::Number, "abc"), Err(Bad::NotNumber));
        assert_eq!(typed(Ty::Number, ""), Err(Bad::Required));
        assert_eq!(typed(Ty::Number, "=t * 2"), Ok("t * 2".into()));
        assert_eq!(typed(Ty::Expr, ""), Err(Bad::Required));
        assert_eq!(typed(Ty::Expr, "['a', 'b']"), Ok("['a', 'b']".into()));
    }

    #[test]
    fn shown_number_expression() {
        assert_eq!(shown(Ty::Number, "30"), "30");
        assert_eq!(shown(Ty::Number, "t * 2"), "=t * 2");
        assert_eq!(shown(Ty::Bool, "True"), "True");
        assert_eq!(shown(Ty::Bool, "ok"), "=ok");
        assert_eq!(
            shown(Ty::Choice(&["BUTTON_YESNO"]), "BUTTON_YESNO"),
            "BUTTON_YESNO"
        );
        assert_eq!(shown(Ty::Choice(&["BUTTON_YESNO"]), "0"), "0");
        assert_eq!(shown(Ty::Expr, "['a']"), "['a']");
    }

    #[test]
    fn new_call_send() {
        let b = new_block(lookup("crt.Screen.Send").unwrap());
        assert_eq!(super::super::generate(&[b]), "crt.Screen.Send('')\n");
        let b = new_block(lookup("crt.Screen.WaitForString").unwrap());
        assert_eq!(
            super::super::generate(&[b]),
            "crt.Screen.WaitForString('')\n",
            "defaults left out"
        );
        let b = new_block(lookup("crt.Screen.WaitForStrings").unwrap());
        assert_eq!(
            super::super::generate(&[b]),
            "crt.Screen.WaitForStrings('')\n"
        );
        let b = new_block(lookup("crt.Screen.Get").unwrap());
        assert_eq!(super::super::generate(&[b]), "crt.Screen.Get(0, 0, 0, 0)\n");
    }

    #[test]
    fn new_call_tab_member_and_property() {
        let b = new_block(lookup("tab.Activate").unwrap());
        assert_eq!(super::super::generate(&[b]), "tab.Activate()\n");
        let b = new_block(lookup("crt.Screen.CurrentRow").unwrap());
        assert_eq!(
            super::super::generate(&[b]),
            "value = crt.Screen.CurrentRow\n"
        );
    }

    #[test]
    fn statements_compile() {
        for k in [
            "if", "for", "while", "with", "try", "def", "assign", "call", "comment", "raw",
        ] {
            let b = new_statement(k).unwrap_or_else(|| panic!("{k}"));
            let code = super::super::generate(&[b]);
            assert!(!code.trim().is_empty(), "{k}");
        }
        assert_eq!(new_statement("class"), None);
    }
}
