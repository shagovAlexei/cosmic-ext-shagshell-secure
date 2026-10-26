"""ShagShell Secure Script Builder: a Python source on stdin -> its blocks, one JSON line on stdout.

Python does the syntax (spec 36): `ast` for the statements, `tokenize` for the comments. The
blocks are those of `shagshell_core::builder::Block`; expressions are kept as `ast.unparse` text.
Out: {"ok": [block, ...]} or {"err": {"line": N, "msg": "..."}}.
"""
import ast
import io
import json
import sys
import tokenize


def main():
    src = sys.stdin.buffer.read().decode("utf-8").replace("\r\n", "\n").replace("\r", "\n")
    try:
        tree = ast.parse(src)
    except SyntaxError as e:
        print(json.dumps({"err": {"line": e.lineno or 0, "msg": e.msg}}))
        return
    print(json.dumps({"ok": Reader(src).module(tree)}))


class Reader:
    def __init__(self, src):
        self.lines = src.split("\n")
        self.comments = {}
        for tok in tokenize.generate_tokens(io.StringIO(src).readline):
            if tok.type == tokenize.COMMENT:
                t = tok.string[1:].rstrip()
                self.comments[tok.start[0]] = t[1:] if t.startswith(" ") else t

    def module(self, tree):
        out, line = self.body(tree.body, 1)
        self.gap(line, len(self.lines), out, flush=False)
        return out

    def body(self, stmts, line):
        """The blocks of a body whose gap starts at `line`, and the line after it."""
        out = []
        only_pass = len(stmts) == 1 and isinstance(stmts[0], ast.Pass)
        for st in stmts:
            deco = getattr(st, "decorator_list", None)
            first = deco[0].lineno if deco else st.lineno
            self.gap(line, first - 1, out)
            line = st.end_lineno + 1
            if only_pass:  # an empty body: the generator writes the `pass` back
                self.own(first, st.end_lineno, out)
            else:
                self.stmt(st, first, out)
        return out, line

    def gap(self, a, b, out, flush=True):
        """Lines between statements: comments, and one Blank per run of empty lines."""
        pending = False
        for n in range(a, b + 1):
            if n in self.comments:
                if pending and out:
                    out.append({"kind": "blank"})
                pending = False
                out.append({"kind": "comment", "text": self.comments[n]})
            elif not self.lines[n - 1].strip():
                pending = True
        if flush and pending and out:
            out.append({"kind": "blank"})

    def own(self, a, b, out):
        """The comments on a statement's own lines go just before its block."""
        for n in range(a, b + 1):
            if n in self.comments:
                out.append({"kind": "comment", "text": self.comments[n]})

    def stmt(self, st, first, out):
        u = ast.unparse
        b = None
        if isinstance(st, ast.Expr) and plain_call(st.value):
            b = call(st.value, None)
        elif isinstance(st, ast.Assign) and len(st.targets) == 1 and not st.type_comment:
            t = st.targets[0]
            if plain_call(st.value) and isinstance(t, (ast.Name, ast.Attribute)):
                b = call(st.value, u(t))
            else:
                b = {"kind": "assign", "target": u(t), "value": u(st.value)}
        if b is not None:
            self.own(first, st.end_lineno, out)
            out.append(b)
            return
        # compound statements: only the first line is the header
        nxt = st.lineno + 1
        if isinstance(st, ast.If):
            self.own(first, st.lineno, out)
            branches, node = [], st
            while True:
                body, nxt = self.body(node.body, nxt)
                branches.append([u(node.test), body])
                if len(node.orelse) == 1 and isinstance(node.orelse[0], ast.If):
                    node = node.orelse[0]
                else:
                    break
            orelse, _ = self.body(node.orelse, nxt)
            out.append({"kind": "if", "branches": branches, "orelse": orelse})
        elif isinstance(st, (ast.For, ast.While)) and not st.orelse and not getattr(st, "type_comment", None):
            self.own(first, st.lineno, out)
            body, _ = self.body(st.body, nxt)
            if isinstance(st, ast.For):
                out.append({"kind": "for", "target": u(st.target), "iter": u(st.iter), "body": body})
            else:
                out.append({"kind": "while", "cond": u(st.test), "body": body})
        elif isinstance(st, ast.With) and not st.type_comment:
            self.own(first, st.lineno, out)
            items = ", ".join(
                u(i.context_expr) + (" as " + u(i.optional_vars) if i.optional_vars else "") for i in st.items
            )
            body, _ = self.body(st.body, nxt)
            out.append({"kind": "with", "items": items, "body": body})
        elif isinstance(st, ast.Try) and not st.orelse:
            self.own(first, st.lineno, out)
            body, nxt = self.body(st.body, nxt)
            handlers = []
            for h in st.handlers:
                hb, nxt = self.body(h.body, nxt)
                handlers.append({"ty": u(h.type) if h.type else None, "name": h.name, "body": hb})
            fin = self.body(st.finalbody, nxt)[0] if st.finalbody else None
            out.append({"kind": "try", "body": body, "handlers": handlers, "finally": fin})
        elif (isinstance(st, ast.FunctionDef) and not st.decorator_list and not st.returns
              and not getattr(st, "type_params", None) and not st.type_comment):
            self.own(first, st.lineno, out)
            body, _ = self.body(st.body, nxt)
            out.append({"kind": "def", "name": st.name, "params": u(st.args), "body": body})
        else:
            self.own(first, st.end_lineno, out)
            out.append({"kind": "raw", "code": u(st)})


def plain_call(e):
    """A call the block's fields can hold: no `*args` / `**kwargs`."""
    return (isinstance(e, ast.Call) and not any(isinstance(a, ast.Starred) for a in e.args)
            and all(k.arg for k in e.keywords))


def call(c, assign):
    args = [{"name": None, "value": ast.unparse(a)} for a in c.args]
    args += [{"name": k.arg, "value": ast.unparse(k.value)} for k in c.keywords]
    return {"kind": "call", "func": ast.unparse(c.func), "args": args, "assign": assign}


if __name__ == "__main__":
    main()
