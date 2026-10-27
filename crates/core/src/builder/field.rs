//! A block's field: what it shows and what it takes. A value is Python text; a text field shows
//! a string literal as its plain text, anything else as `=` + the expression (`=cmd + "\r"`), and
//! takes `=…` back as an expression.

use super::catalog::{Entry, Kind, Ty, lookup};
use super::{Arg, Block, Handler};

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

/// A text field shows what goes between the quotes: `\r`, `\n`, `\t`, `\\` as escapes, a quote
/// as it is.
fn escaped(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\x{:02x}", c as u32))
            }
            c => out.push(c),
        }
    }
    out
}

/// What a text field typed: its escapes kept (`\r` is Enter), its quotes escaped; a broken escape
/// (a lone `\` at the end) makes it plain text.
fn quoted(input: &str) -> String {
    let mut body = String::new();
    let mut esc = false;
    for c in input.chars() {
        if c == '\'' && !esc {
            body.push('\\');
        }
        esc = c == '\\' && !esc;
        body.push(c);
    }
    let lit = format!("'{body}'");
    match unliteral(&lit) {
        Some(_) => lit,
        None => literal(input),
    }
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
                    // Octal: one to three digits (`\\003` is Ctrl+C).
                    '0'..='7' => {
                        let mut code = e.to_digit(8)?;
                        for _ in 0..2 {
                            match it.clone().next().and_then(|d| d.to_digit(8)) {
                                Some(d) => {
                                    code = code * 8 + d;
                                    it.next();
                                }
                                None => break,
                            }
                        }
                        out.push(char::from_u32(code)?);
                    }
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
            return unliteral(value).map_or_else(|| format!("={value}"), |t| escaped(&t));
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
        Ty::Text | Ty::File | Ty::Session => Ok(quoted(input)),
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
        // Its function's name is made free by the tape (`builder::fresh_host_func`).
        "for_host" => Block::ForHost {
            hosts: s("hosts"),
            func: s("_host_1"),
            parallel: false,
            limit: s("10"),
            body: vec![],
        },
        _ => return None,
    })
}

/// A field of a block: the n-th parameter of a `crt` call, a statement's own parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum F {
    /// The catalog entry's n-th parameter (`*texts` / `**kw` hold several arguments).
    Arg(usize),
    /// «save to»: the variable a call's result goes to.
    Assign,
    /// A function that is not in the catalog, and its arguments as one text.
    Func,
    Args,
    Target,
    Value,
    /// `if` / `elif` branch n's condition; `while`'s is 0.
    Cond(usize),
    Iter,
    Items,
    Name,
    Params,
    HandlerTy(usize),
    HandlerName(usize),
    /// A comment's text, Custom Python's code.
    Text,
    /// «For each host»: the list, `True` / `False`, how many at a time.
    Hosts,
    Parallel,
    Limit,
}

/// `a, f(b, c), k='x, y'`: the arguments at the top level (brackets and quotes kept whole).
fn split_top(s: &str) -> Vec<String> {
    let (mut out, mut cur, mut depth, mut quote, mut esc) =
        (vec![], String::new(), 0i32, None, false);
    for c in s.chars() {
        match (quote, c) {
            (Some(_), _) if esc => esc = false,
            (Some(_), '\\') => esc = true,
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '\'' | '"') => quote = Some(c),
            (None, '(' | '[' | '{') => depth += 1,
            (None, ')' | ']' | '}') => depth -= 1,
            (None, ',') if depth == 0 => {
                out.push(std::mem::take(&mut cur).trim().to_string());
                continue;
            }
            _ => {}
        }
        cur.push(c);
    }
    out.push(cur.trim().to_string());
    out.retain(|a| !a.is_empty());
    out
}

/// `name=value` (not `==`): a keyword argument.
fn keyword(a: &str) -> Option<(String, String)> {
    let (k, v) = a.split_once('=')?;
    let k = k.trim();
    let ident = k
        .chars()
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '_')
        && k.chars().all(|c| c.is_alphanumeric() || c == '_');
    (ident && !v.starts_with('=')).then(|| (k.to_string(), v.trim().to_string()))
}

fn args_of(text: &str) -> Vec<Arg> {
    split_top(text)
        .into_iter()
        .map(|a| match keyword(&a) {
            Some((k, v)) => Arg {
                name: Some(k),
                value: v,
            },
            None => Arg {
                name: None,
                value: a,
            },
        })
        .collect()
}

fn join(args: &[&Arg]) -> Option<String> {
    let t: Vec<String> = args
        .iter()
        .map(|a| match &a.name {
            Some(n) => format!("{n}={}", a.value),
            None => a.value.clone(),
        })
        .collect();
    (!t.is_empty()).then(|| t.join(", "))
}

fn opt(v: &str) -> Option<String> {
    (!v.is_empty()).then(|| v.to_string())
}

/// A field's value (Python text); `None`: not set (an optional parameter left at its default).
pub fn get(b: &Block, f: F) -> Option<String> {
    match (b, f) {
        (Block::Call { func, args, .. }, F::Arg(i)) => {
            let e = lookup(func)?;
            let p = e.params.get(i)?;
            let np = args.iter().take_while(|a| a.name.is_none()).count();
            if p.name.starts_with("**") {
                let named: Vec<&str> = e.params.iter().map(|p| p.name).collect();
                join(
                    &args
                        .iter()
                        .filter(|a| a.name.as_deref().is_some_and(|n| !named.contains(&n)))
                        .collect::<Vec<_>>(),
                )
            } else if p.name.starts_with('*') {
                join(&args[i.min(np)..np].iter().collect::<Vec<_>>())
            } else if i < np {
                Some(args[i].value.clone())
            } else {
                args.iter()
                    .find(|a| a.name.as_deref() == Some(p.name))
                    .map(|a| a.value.clone())
            }
        }
        (Block::Call { assign, .. }, F::Assign) => assign.clone(),
        (Block::Call { func, .. }, F::Func) => Some(func.clone()),
        (Block::Call { args, .. }, F::Args) => join(&args.iter().collect::<Vec<_>>()),
        (Block::Assign { target, .. } | Block::For { target, .. }, F::Target) => {
            Some(target.clone())
        }
        (Block::Assign { value, .. }, F::Value) => Some(value.clone()),
        (Block::If { branches, .. }, F::Cond(i)) => branches.get(i).map(|(c, _)| c.clone()),
        (Block::While { cond, .. }, F::Cond(0)) => Some(cond.clone()),
        (Block::For { iter, .. }, F::Iter) => Some(iter.clone()),
        (Block::With { items, .. }, F::Items) => Some(items.clone()),
        (Block::Def { name, .. }, F::Name) => Some(name.clone()),
        (Block::Def { params, .. }, F::Params) => Some(params.clone()),
        (Block::Try { handlers, .. }, F::HandlerTy(i)) => handlers.get(i)?.ty.clone(),
        (Block::Try { handlers, .. }, F::HandlerName(i)) => handlers.get(i)?.name.clone(),
        (Block::Comment { text }, F::Text) | (Block::Raw { code: text }, F::Text) => {
            Some(text.clone())
        }
        (Block::ForHost { hosts, .. }, F::Hosts) => Some(hosts.clone()),
        (Block::ForHost { parallel, .. }, F::Parallel) => {
            Some(if *parallel { "True" } else { "False" }.into())
        }
        (Block::ForHost { limit, .. }, F::Limit) => Some(limit.clone()),
        _ => None,
    }
}

/// Sets a field; `""` takes an optional parameter / «save to» out. `false`: no such field.
pub fn set(b: &mut Block, f: F, value: &str) -> bool {
    let v = value.trim();
    match (b, f) {
        (Block::Call { func, args, .. }, F::Arg(i)) => {
            let Some(e) = lookup(func) else {
                return false;
            };
            let Some(p) = e.params.get(i) else {
                return false;
            };
            let np = args.iter().take_while(|a| a.name.is_none()).count();
            if p.name.starts_with("**") {
                let named: Vec<&str> = e.params.iter().map(|p| p.name).collect();
                args.retain(|a| a.name.as_deref().is_none_or(|n| named.contains(&n)));
                let new = args_of(v);
                if new.iter().any(|a| a.name.is_none()) {
                    return false;
                }
                args.extend(new);
            } else if p.name.starts_with('*') {
                let tail: Vec<Arg> = args.drain(i.min(np)..).collect();
                args.extend(args_of(v).into_iter().filter(|a| a.name.is_none()));
                args.extend(tail.into_iter().filter(|a| a.name.is_some()));
            } else if v.is_empty() {
                match (i < np, i + 1 == np) {
                    (true, true) => {
                        args.remove(i);
                    }
                    (true, false) => args[i].value = p.default.unwrap_or("None").to_string(),
                    _ => args.retain(|a| a.name.as_deref() != Some(p.name)),
                }
            } else if i < np {
                args[i].value = v.to_string();
            } else if i == np && !args.iter().any(|a| a.name.as_deref() == Some(p.name)) {
                args.insert(
                    np,
                    Arg {
                        name: None,
                        value: v.to_string(),
                    },
                );
            } else {
                match args.iter_mut().find(|a| a.name.as_deref() == Some(p.name)) {
                    Some(a) => a.value = v.to_string(),
                    None => args.push(Arg {
                        name: Some(p.name.to_string()),
                        value: v.to_string(),
                    }),
                }
            }
        }
        (Block::Call { assign, .. }, F::Assign) => *assign = opt(v),
        (Block::Call { func, .. }, F::Func) => *func = v.to_string(),
        (Block::Call { args, .. }, F::Args) => *args = args_of(v),
        (Block::Assign { target, .. } | Block::For { target, .. }, F::Target) => {
            *target = v.to_string()
        }
        (Block::Assign { value: x, .. }, F::Value) => *x = v.to_string(),
        (Block::If { branches, .. }, F::Cond(i)) => match branches.get_mut(i) {
            Some((c, _)) => *c = v.to_string(),
            None => return false,
        },
        (Block::While { cond, .. }, F::Cond(0)) => *cond = v.to_string(),
        (Block::For { iter, .. }, F::Iter) => *iter = v.to_string(),
        (Block::With { items, .. }, F::Items) => *items = v.to_string(),
        (Block::Def { name, .. }, F::Name) => *name = v.to_string(),
        (Block::Def { params, .. }, F::Params) => *params = v.to_string(),
        (Block::Try { handlers, .. }, F::HandlerTy(i)) => match handlers.get_mut(i) {
            Some(h) => h.ty = opt(v),
            None => return false,
        },
        (Block::Try { handlers, .. }, F::HandlerName(i)) => match handlers.get_mut(i) {
            Some(h) => h.name = opt(v),
            None => return false,
        },
        // A comment / Custom Python keep their text as typed (spaces, lines).
        (Block::Comment { text }, F::Text) | (Block::Raw { code: text }, F::Text) => {
            *text = value.to_string()
        }
        (Block::ForHost { hosts, .. }, F::Hosts) => *hosts = v.to_string(),
        (Block::ForHost { parallel, .. }, F::Parallel) => *parallel = v == "True",
        (Block::ForHost { limit, .. }, F::Limit) => *limit = v.to_string(),
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
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
        // As inside Python quotes: \r is Enter (as SecureCRT's buttons), a quote needs no escape.
        assert_eq!(typed(Ty::Text, "show ver\\r"), Ok("'show ver\\r'".into()));
        assert_eq!(typed(Ty::Text, "it's"), Ok("'it\\'s'".into()));
        assert_eq!(
            typed(Ty::Text, "C:\\"),
            Ok("'C:\\\\'".into()),
            "a lone backslash is kept"
        );
        assert_eq!(shown(Ty::Text, "'show ver\\r'"), "show ver\\r");
        assert_eq!(shown(Ty::Text, "\"it's\""), "it's");
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
            "for_host",
        ] {
            let b = new_statement(k).unwrap_or_else(|| panic!("{k}"));
            let code = super::super::generate(&[b]);
            assert!(!code.trim().is_empty(), "{k}");
        }
        assert_eq!(new_statement("class"), None);
    }

    fn send() -> Block {
        new_block(lookup("crt.Screen.WaitForString").unwrap())
    }

    #[test]
    fn field_positional_then_keyword() {
        let mut b = send();
        assert_eq!(get(&b, F::Arg(0)).as_deref(), Some("''"));
        assert_eq!(get(&b, F::Arg(1)), None);
        assert!(set(&mut b, F::Arg(1), "10"));
        assert!(set(&mut b, F::Arg(2), "True"));
        assert_eq!(
            super::super::generate(std::slice::from_ref(&b)),
            "crt.Screen.WaitForString('', 10, True)\n"
        );
        assert!(
            set(&mut b, F::Arg(1), ""),
            "the middle one back to its default"
        );
        assert_eq!(
            super::super::generate(std::slice::from_ref(&b)),
            "crt.Screen.WaitForString('', 0, True)\n"
        );
        assert!(set(&mut b, F::Arg(2), ""));
        assert_eq!(
            super::super::generate(std::slice::from_ref(&b)),
            "crt.Screen.WaitForString('', 0)\n"
        );
        assert!(set(&mut b, F::Assign, "ok"));
        assert_eq!(get(&b, F::Assign).as_deref(), Some("ok"));
        assert!(set(&mut b, F::Assign, ""));
        assert_eq!(get(&b, F::Assign), None);
    }

    #[test]
    fn field_keyword_when_a_gap() {
        let mut b = send();
        assert!(set(&mut b, F::Arg(2), "True"));
        assert_eq!(
            super::super::generate(std::slice::from_ref(&b)),
            "crt.Screen.WaitForString('', bCaseInsensitive=True)\n"
        );
        assert_eq!(get(&b, F::Arg(2)).as_deref(), Some("True"));
    }

    #[test]
    fn field_varargs_and_kw() {
        let mut b = new_block(lookup("crt.Screen.WaitForStrings").unwrap());
        assert!(set(&mut b, F::Arg(0), "'#', '>'"));
        assert!(set(&mut b, F::Arg(1), "timeout=10"));
        assert_eq!(
            super::super::generate(std::slice::from_ref(&b)),
            "crt.Screen.WaitForStrings('#', '>', timeout=10)\n"
        );
        assert_eq!(get(&b, F::Arg(0)).as_deref(), Some("'#', '>'"));
        assert_eq!(get(&b, F::Arg(1)).as_deref(), Some("timeout=10"));
    }

    #[test]
    fn field_statements() {
        let mut b = new_statement("for").unwrap();
        assert!(set(&mut b, F::Iter, "hosts"));
        assert_eq!(get(&b, F::Iter).as_deref(), Some("hosts"));
        assert!(!set(&mut b, F::Cond(0), "x"));
        let mut b = new_statement("call").unwrap();
        assert!(set(&mut b, F::Func, "f"));
        assert!(set(&mut b, F::Args, "1, b=2"));
        assert_eq!(
            super::super::generate(std::slice::from_ref(&b)),
            "f(1, b=2)\n"
        );
        let mut b = new_statement("try").unwrap();
        assert!(set(&mut b, F::HandlerTy(0), "OSError"));
        assert_eq!(get(&b, F::HandlerTy(0)).as_deref(), Some("OSError"));
        let mut b = new_statement("comment").unwrap();
        assert!(set(&mut b, F::Text, "hi"));
        assert_eq!(get(&b, F::Text).as_deref(), Some("hi"));
    }

    #[test]
    fn regression_octal_escapes() {
        // A SecureCRT line sending Ctrl+C: '\003' is one character.
        assert_eq!(unliteral("'\\003'"), Some("\u{3}".into()));
        assert_eq!(unliteral("'\\01x'"), Some("\u{1}x".into()));
        assert_eq!(unliteral("'\\0'"), Some("\0".into()));
        assert_eq!(shown(Ty::Text, "'\\003'"), "\\x03");
    }
}
