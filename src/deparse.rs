//! R's `deparse` for the rlang AST — a port of GNU R's `src/main/deparse.c`
//! (`deparse2buff`, `args2buff`, `print2buff`, `writeline`, `linebreak`).
//!
//! This is what `print(f)`, `deparse(f)` and `format(f)` show for a closure.
//! `Rscript` runs with `keep.source = FALSE`, so R does *not* echo the original
//! text: it re-renders the parse tree under its own layout rules, which is why
//! the output is `function (x, y = 2) ` on its own line, four-space block
//! indentation, and an `if` inside `{ }` broken across three lines. Those rules
//! are reproduced here exactly rather than approximated, because a closure's
//! printed form is compared byte-for-byte against the reference in the parity
//! corpus.
//!
//! The layout state that drives it, straight from `LocalParseData`:
//!
//! - `indent` — printed lazily at the first text on a line, four spaces per
//!   level for the first four, then two, then one (R's `printtab2buff`).
//! - `incurly` — inside a `{ }`, an `if` puts its branches on their own lines;
//!   outside one it stays on a single line. This is the whole reason
//!   `function(a, b) if (a > b) a else b` deparses to one line while the same
//!   `if` wrapped in braces deparses to three.
//! - `len` / `cutoff` — the running line length against `width.cutoff = 60`.
//!   Binary operators and argument lists break there and indent the
//!   continuation one extra level.
//!
//! Parentheses are *preserved*, never re-derived: `Expr::Paren` keeps the `(`
//! the source wrote, exactly as R keeps a `(` call in its parse tree. So no
//! precedence table is needed and no parenthesis can be invented or lost.

use crate::ast::{Arg, BinOp, Expr, IndexKind, NaKind, Param, UnOp};
use crate::host::{fixed_decimals, render_fixed, render_sci, sci_decimals};

/// R's `width.cutoff` default for `deparse`.
const CUTOFF: usize = 60;

/// The deparse buffer: R's `LocalParseData`, minus the options rlang has no
/// use for.
struct Deparser {
    lines: Vec<String>,
    buf: String,
    /// Bytes on the current line, indentation included — R counts `strlen`.
    len: usize,
    indent: usize,
    startline: bool,
    incurly: usize,
}

impl Deparser {
    fn new() -> Self {
        Deparser {
            lines: Vec::new(),
            buf: String::new(),
            len: 0,
            indent: 0,
            startline: true,
            incurly: 0,
        }
    }

    /// `print2buff`: append text, tabbing over first if this is a line's start.
    fn print(&mut self, s: &str) {
        if self.startline {
            self.startline = false;
            self.tabs(self.indent);
        }
        self.buf.push_str(s);
        self.len += s.len();
    }

    /// `printtab2buff`: four spaces per level to depth 4, then two, then one.
    fn tabs(&mut self, n: usize) {
        for i in 1..=n {
            self.print(match i {
                1..=4 => "    ",
                5..=6 => "  ",
                _ => " ",
            });
        }
    }

    /// `writeline`: flush the current line (trailing spaces and all — R keeps
    /// them, which is why a deparsed header is `"function (x) "`).
    fn writeline(&mut self) {
        self.lines.push(std::mem::take(&mut self.buf));
        self.len = 0;
        self.startline = true;
    }

    /// `linebreak`: wrap past the cutoff, indenting the continuation once for
    /// the whole operator/argument run.
    fn linebreak(&mut self, lbreak: &mut bool) {
        if self.len > CUTOFF {
            if !*lbreak {
                *lbreak = true;
                self.indent += 1;
            }
            self.writeline();
        }
    }

    fn finish(mut self) -> Vec<String> {
        if !self.startline || !self.buf.is_empty() {
            self.writeline();
        }
        self.lines
    }

    /// `args2buff` with `formals = 1`: a formal prints its default only when it
    /// has one (an unsupplied formal is `R_MissingArg`, printed bare).
    fn formals(&mut self, params: &[Param]) {
        let mut lbreak = false;
        for (i, p) in params.iter().enumerate() {
            self.print(&quote_name(&p.name));
            if let Some(d) = &p.default {
                self.print(" = ");
                self.expr(d);
            }
            if i + 1 < params.len() {
                self.print(", ");
                self.linebreak(&mut lbreak);
            }
        }
        if lbreak {
            self.indent -= 1;
        }
    }

    /// `args2buff` with `formals = 0`: a call-site tag always prints its `=`,
    /// even when the value is empty (`x[i, ]` has neither tag nor value).
    fn args(&mut self, args: &[Arg]) {
        let mut lbreak = false;
        for (i, a) in args.iter().enumerate() {
            if let Some(n) = &a.name {
                self.print(&quote_name(n));
                self.print(" = ");
            }
            if let Some(v) = &a.value {
                self.expr(v);
            }
            if i + 1 < args.len() {
                self.print(", ");
                self.linebreak(&mut lbreak);
            }
        }
        if lbreak {
            self.indent -= 1;
        }
    }

    /// `PP_BINARY`: a space each side, with a wrap point after the operator.
    fn binary(&mut self, lhs: &Expr, op: &str, rhs: &Expr) {
        let mut lbreak = false;
        self.expr(lhs);
        self.print(" ");
        self.print(op);
        self.print(" ");
        self.linebreak(&mut lbreak);
        self.expr(rhs);
        if lbreak {
            self.indent -= 1;
        }
    }

    /// `PP_BINARY2`: no space, no wrap point (`x/2`, `x^2`, `1:10`, `x%%3`).
    fn binary2(&mut self, lhs: &Expr, op: &str, rhs: &Expr) {
        self.expr(lhs);
        self.print(op);
        self.expr(rhs);
    }

    fn expr(&mut self, e: &Expr) {
        match e {
            Expr::Num(n) => {
                let s = num_literal(*n);
                self.print(&s)
            }
            Expr::Int(i) => {
                let s = format!("{i}L");
                self.print(&s)
            }
            Expr::Str(s) => {
                let q = format!("\"{}\"", crate::builtins::encode_string(s));
                self.print(&q)
            }
            Expr::Bool(true) => self.print("TRUE"),
            Expr::Bool(false) => self.print("FALSE"),
            Expr::Null => self.print("NULL"),
            Expr::Na(k) => self.print(match k {
                NaKind::Logical => "NA",
                NaKind::Integer => "NA_integer_",
                NaKind::Real => "NA_real_",
                NaKind::Character => "NA_character_",
            }),
            Expr::Inf => self.print("Inf"),
            Expr::NaN => self.print("NaN"),
            Expr::Ident(s) => {
                let q = quote_name(s);
                self.print(&q)
            }
            Expr::Dots => self.print("..."),
            Expr::Break => self.print("break"),
            Expr::Next => self.print("next"),
            Expr::Paren(inner) => {
                self.print("(");
                self.expr(inner);
                self.print(")");
            }
            Expr::Call { fun, args } => {
                self.expr(fun);
                self.print("(");
                self.args(args);
                self.print(")");
            }
            // A nested `function` is language, not a closure: R writes it
            // `function(x) body` — no space after the keyword, and no line
            // break before the body (both of which a top-level closure gets).
            Expr::Function { params, body } => {
                self.print("function(");
                self.formals(params);
                self.print(") ");
                self.expr(body);
            }
            Expr::If { cond, then, els } => {
                self.print("if (");
                self.expr(cond);
                self.print(") ");
                // Inside `{ }` the branches go on their own lines — unless the
                // branch is itself a `{`, which stays on the `if` line (R's
                // `curlyahead`).
                let mut lookahead = false;
                if self.incurly > 0 {
                    lookahead = matches!(then.as_ref(), Expr::Block(_));
                    if !lookahead {
                        self.writeline();
                        self.indent += 1;
                    }
                }
                self.expr(then);
                match els {
                    Some(els) => {
                        if self.incurly > 0 {
                            self.writeline();
                            if !lookahead {
                                self.indent -= 1;
                            }
                        } else {
                            self.print(" ");
                        }
                        self.print("else ");
                        self.expr(els);
                    }
                    None => {
                        if self.incurly > 0 && !lookahead {
                            self.indent -= 1;
                        }
                    }
                }
            }
            Expr::For { var, seq, body } => {
                self.print("for (");
                let v = quote_name(var);
                self.print(&v);
                self.print(" in ");
                self.expr(seq);
                self.print(") ");
                self.expr(body);
            }
            Expr::While { cond, body } => {
                self.print("while (");
                self.expr(cond);
                self.print(") ");
                self.expr(body);
            }
            Expr::Repeat(body) => {
                self.print("repeat ");
                self.expr(body);
            }
            Expr::Block(stmts) => {
                self.print("{");
                self.incurly += 1;
                self.indent += 1;
                self.writeline();
                for s in stmts {
                    self.expr(s);
                    self.writeline();
                }
                self.indent -= 1;
                self.print("}");
                self.incurly -= 1;
            }
            Expr::Assign {
                target,
                value,
                super_assign,
            } => {
                // `PP_ASSIGN`: spaces, but no wrap point.
                self.expr(target);
                self.print(if *super_assign { " <<- " } else { " <- " });
                self.expr(value);
            }
            // `~` with one operand deparses as a prefix operator (`~x`), with
            // two as an infix one (`y ~ x`) — R downgrades `PP_BINARY` to
            // `PP_UNARY` when the call has a single argument.
            Expr::Formula { lhs, rhs } => match lhs {
                Some(l) => self.binary(l, "~", rhs),
                None => {
                    self.print("~");
                    self.expr(rhs);
                }
            },
            Expr::Binary { op, lhs, rhs } => match op {
                // The `PP_BINARY2` operators: `/`, `^` and `:` print tight.
                BinOp::Div => self.binary2(lhs, "/", rhs),
                BinOp::Pow => self.binary2(lhs, "^", rhs),
                BinOp::Colon => self.binary2(lhs, ":", rhs),
                _ => {
                    let name = match op {
                        BinOp::Add => "+",
                        BinOp::Sub => "-",
                        BinOp::Mul => "*",
                        BinOp::Lt => "<",
                        BinOp::Gt => ">",
                        BinOp::Le => "<=",
                        BinOp::Ge => ">=",
                        BinOp::Eq => "==",
                        BinOp::Ne => "!=",
                        BinOp::And => "&",
                        BinOp::Or => "|",
                        BinOp::And2 => "&&",
                        BinOp::Or2 => "||",
                        BinOp::Div | BinOp::Pow | BinOp::Colon => unreachable!(),
                    };
                    self.binary(lhs, name, rhs)
                }
            },
            // `%%` and `%/%` are primitives (`PP_BINARY2`, tight); every other
            // `%op%` is an R-level infix function, which R spaces.
            Expr::Special { name, lhs, rhs } => {
                let op = format!("%{name}%");
                if name.is_empty() || name == "/" {
                    self.binary2(lhs, &op, rhs)
                } else {
                    self.binary(lhs, &op, rhs)
                }
            }
            Expr::Unary { op, operand } => {
                self.print(match op {
                    UnOp::Neg => "-",
                    UnOp::Plus => "+",
                    UnOp::Not => "!",
                });
                self.expr(operand);
            }
            Expr::Index { kind, obj, args } => {
                self.expr(obj);
                match kind {
                    IndexKind::Single => {
                        self.print("[");
                        self.args(args);
                        self.print("]");
                    }
                    IndexKind::Double => {
                        self.print("[[");
                        self.args(args);
                        self.print("]]");
                    }
                    IndexKind::Dollar | IndexKind::At => {
                        self.print(if matches!(kind, IndexKind::Dollar) {
                            "$"
                        } else {
                            "@"
                        });
                        // `a$"b"` normalizes to `a$b` when the tag is a name.
                        for a in args {
                            match a.value.as_ref() {
                                Some(Expr::Str(s)) => {
                                    let q = quote_name(s);
                                    self.print(&q)
                                }
                                Some(v) => self.expr(v),
                                None => {}
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The lines `print`/`deparse`/`format` show for `function(params) body`, in
/// R's `keep.source = FALSE` layout: the header on its own line, then the body.
pub fn deparse_closure(params: &[Param], body: &Expr) -> Vec<String> {
    let mut d = Deparser::new();
    d.print("function (");
    d.formals(params);
    d.print(") ");
    d.writeline();
    d.expr(body);
    d.finish()
}

/// One expression's source text on a single logical line — for callers that
/// want a deparsed *argument* rather than a whole function (`rbind(x, x)`'s
/// seam labels, `table(z)`'s dimension name).
pub fn deparse_expr(e: &Expr) -> String {
    let mut d = Deparser::new();
    d.expr(e);
    d.finish().join("")
}

/// The first line of `e`'s deparse — R's `deparse1s(call)` followed by
/// `STRING_ELT(., 0)`, which is how a condition renders the call it carries.
/// A call whose deparse runs to several lines shows only its first, so
/// `withCallingHandlers({ … }, warning = …)` reports as `withCallingHandlers({`.
pub fn deparse_first_line(e: &Expr) -> String {
    let mut d = Deparser::new();
    d.expr(e);
    d.finish().into_iter().next().unwrap_or_default()
}

/// A numeric literal as R writes it: the same fixed-vs-scientific choice
/// `print` makes for a length-one double, so `100000` deparses to `1e+05`.
fn num_literal(x: f64) -> String {
    if !x.is_finite() {
        return render_fixed(x, 0);
    }
    let fixed = render_fixed(x, fixed_decimals(x));
    let sci = render_sci(x, sci_decimals(x));
    if crate::host::prefers_fixed(fixed.chars().count(), sci.chars().count()) {
        fixed
    } else {
        sci
    }
}

/// R's `isValidName`: a name that the parser would read back as this same
/// symbol prints bare; anything else gets backticks.
pub fn is_syntactic_name(s: &str) -> bool {
    if s == "..." {
        return true;
    }
    let mut cs = s.chars();
    let Some(first) = cs.next() else {
        return false;
    };
    if first != '.' && !first.is_ascii_alphabetic() {
        return false;
    }
    // `.5` and `.1x` are numbers, not names.
    if first == '.' && cs.clone().next().is_some_and(|c| c.is_ascii_digit()) {
        return false;
    }
    if !cs.all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_') {
        return false;
    }
    !matches!(
        s,
        "NULL"
            | "NA"
            | "TRUE"
            | "FALSE"
            | "Inf"
            | "NaN"
            | "NA_integer_"
            | "NA_real_"
            | "NA_character_"
            | "function"
            | "while"
            | "repeat"
            | "for"
            | "if"
            | "in"
            | "else"
            | "next"
            | "break"
    )
}

fn quote_name(s: &str) -> String {
    if is_syntactic_name(s) {
        s.to_string()
    } else {
        format!("`{s}`")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    /// Deparse the single `function` expression in `src`.
    fn dep(src: &str) -> Vec<String> {
        let exprs = parse(src).expect("parse");
        match &exprs[0] {
            Expr::Function { params, body } => deparse_closure(params, body),
            other => panic!("not a function: {other:?}"),
        }
    }

    #[test]
    fn header_is_its_own_line_with_a_trailing_space() {
        assert_eq!(dep("function(x) x + 1"), vec!["function (x) ", "x + 1"]);
    }

    #[test]
    fn if_breaks_only_inside_braces() {
        // Bare body: one line.
        assert_eq!(
            dep("function(a, b) if (a > b) a else b"),
            vec!["function (a, b) ", "if (a > b) a else b"]
        );
        // Braced body: R splits the branches and indents the consequent.
        assert_eq!(
            dep("function(y) { if (y > 3) y else 0 }"),
            vec![
                "function (y) ",
                "{",
                "    if (y > 3) ",
                "        y",
                "    else 0",
                "}"
            ]
        );
    }

    #[test]
    fn tight_operators_take_no_spaces() {
        assert_eq!(dep("function(x) x/2")[1], "x/2");
        assert_eq!(dep("function(x) x^2")[1], "x^2");
        assert_eq!(dep("function(x) x %% 3")[1], "x%%3");
        assert_eq!(dep("function(x) x %in% y")[1], "x %in% y");
        assert_eq!(dep("function(x) x * 2 + 1")[1], "x * 2 + 1");
    }

    #[test]
    fn parentheses_survive_verbatim() {
        assert_eq!(dep("function(x) (x + 1) * 2")[1], "(x + 1) * 2");
        assert_eq!(dep("function(x) -(x + 1)")[1], "-(x + 1)");
    }

    #[test]
    fn long_binary_run_wraps_at_the_cutoff() {
        assert_eq!(
            dep("function(a) { a + 100000 + 200000 + 300000 + 400000 + 500000 + 600000 + 700000 + 800000 }"),
            vec![
                "function (a) ",
                "{",
                "    a + 1e+05 + 2e+05 + 3e+05 + 4e+05 + 5e+05 + 6e+05 + 7e+05 + ",
                "        8e+05",
                "}",
            ]
        );
    }

    #[test]
    fn defaults_and_nested_functions() {
        assert_eq!(
            dep("function(x = c(1,2), y = \"a\") function(z = 2) z"),
            vec!["function (x = c(1, 2), y = \"a\") ", "function(z = 2) z"]
        );
    }

    #[test]
    fn non_syntactic_names_are_backticked() {
        assert!(!is_syntactic_name("my var"));
        assert!(!is_syntactic_name("if"));
        assert!(!is_syntactic_name(".5x"));
        assert!(is_syntactic_name("..."));
        assert!(is_syntactic_name(".x"));
    }
}
