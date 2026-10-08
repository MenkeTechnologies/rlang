//! The R abstract syntax tree.
//!
//! R has no statements — a program is a sequence of expressions, and every form
//! (`if`, `for`, `{`, assignment) is itself an expression with a value. The tree
//! mirrors that: there is only `Expr`.

/// A binary operator. `Special` carries the `%name%` form (including `%%`,
/// `%/%`, `%in%` and user-defined infix operators).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Lt,
    Gt,
    Le,
    Ge,
    Eq,
    Ne,
    /// Vectorized `&`
    And,
    /// Vectorized `|`
    Or,
    /// Scalar short-circuit `&&`
    And2,
    /// Scalar short-circuit `||`
    Or2,
    /// `:` — the integer sequence operator
    Colon,
}

/// A prefix operator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Plus,
    Not,
}

/// How an index expression was written; each is a distinct R operator with
/// distinct semantics (`[` keeps attributes and can select many elements, `[[`
/// extracts exactly one, `$` matches a name literally).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexKind {
    /// `x[...]`
    Single,
    /// `x[[...]]`
    Double,
    /// `x$name`
    Dollar,
    /// `x@name`
    At,
}

/// One argument at a call site. `name` is the tag in `f(n = 1)`; `value` is
/// `None` for an empty argument (`x[, 1]`), which R passes as "missing".
#[derive(Debug, Clone, PartialEq)]
pub struct Arg {
    pub name: Option<String>,
    pub value: Option<Expr>,
}

/// One formal parameter of a `function(...)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub default: Option<Expr>,
}

/// An R expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A double literal (`1.5`, `1e3`, and bare `1` — unsuffixed numbers are
    /// doubles in R).
    Num(f64),
    /// An integer literal written with the `L` suffix (`1L`).
    Int(i64),
    Str(String),
    Bool(bool),
    Null,
    /// `NA` (logical), or a typed `NA_integer_` / `NA_real_` / `NA_character_`.
    Na(NaKind),
    Inf,
    NaN,
    /// A bare name, including backtick-quoted ones.
    Ident(String),
    /// `...` — the variadic forwarding parameter.
    Dots,
    Call {
        fun: Box<Expr>,
        args: Vec<Arg>,
    },
    Function {
        params: Vec<Param>,
        body: Box<Expr>,
    },
    If {
        cond: Box<Expr>,
        then: Box<Expr>,
        els: Option<Box<Expr>>,
    },
    For {
        var: String,
        seq: Box<Expr>,
        body: Box<Expr>,
    },
    While {
        cond: Box<Expr>,
        body: Box<Expr>,
    },
    Repeat(Box<Expr>),
    /// `{ ... }` — a braced sequence; its value is the last expression's.
    Block(Vec<Expr>),
    /// `( ... )`. Kept in the tree because R keeps it: `(` is a function that
    /// returns its argument *visibly*, so `(x <- 5)` echoes where `x <- 5` does
    /// not, and `deparse` reproduces the parentheses the source wrote instead
    /// of re-deriving them from precedence.
    Paren(Box<Expr>),
    /// `<-`, `=`, `->` (normalized to `<-`), and `<<-`/`->>` (`super = true`).
    Assign {
        target: Box<Expr>,
        value: Box<Expr>,
        super_assign: bool,
    },
    /// A model formula `lhs ~ rhs` (or one-sided `~ rhs`). Unevaluated: it is
    /// deparsed back to R source and built as a formula object in the CRAN
    /// bridge, since formulas are non-standard-evaluation language objects.
    Formula {
        lhs: Option<Box<Expr>>,
        rhs: Box<Expr>,
    },
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    /// `%name%` — the special/user-defined infix form, dispatched by name.
    Special {
        name: String,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Unary {
        op: UnOp,
        operand: Box<Expr>,
    },
    Index {
        kind: IndexKind,
        obj: Box<Expr>,
        args: Vec<Arg>,
    },
    Break,
    Next,
}

/// Which typed `NA` was written. R distinguishes them because the type of an
/// `NA` decides the type of the vector it lands in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NaKind {
    Logical,
    Integer,
    Real,
    Character,
}

/// The tree for a call to `op` with `args`, in the syntactic form the parser
/// builds for the same call written out.
///
/// In R the two are one object: `if (a) b` *is* the call `` `if`(a, b) ``, and
/// `x + 1` is `` `+`(x, 1) ``. rlang's tree keeps the syntactic forms as their own
/// nodes, so a call written with a backtick-quoted keyword or operator head —
/// or assembled by `as.call` — is folded into that node here, and then
/// evaluates and deparses exactly as the syntax does. A call with no syntactic
/// spelling (a tagged argument, the wrong arity, a non-symbol `for` variable)
/// stays an ordinary call.
pub fn call_syntax(fun: Expr, args: Vec<Arg>) -> Expr {
    let Expr::Ident(op) = &fun else {
        return Expr::Call {
            fun: Box::new(fun),
            args,
        };
    };
    // Only an untagged, fully supplied argument list has a syntactic spelling.
    let plain: Option<Vec<Expr>> = match args.iter().all(|a| a.name.is_none()) {
        true => args.iter().map(|a| a.value.clone()).collect(),
        false => None,
    };
    let b = Box::new;
    let binop = match op.as_str() {
        "+" => Some(BinOp::Add),
        "-" => Some(BinOp::Sub),
        "*" => Some(BinOp::Mul),
        "/" => Some(BinOp::Div),
        "^" => Some(BinOp::Pow),
        "<" => Some(BinOp::Lt),
        ">" => Some(BinOp::Gt),
        "<=" => Some(BinOp::Le),
        ">=" => Some(BinOp::Ge),
        "==" => Some(BinOp::Eq),
        "!=" => Some(BinOp::Ne),
        "&" => Some(BinOp::And),
        "|" => Some(BinOp::Or),
        "&&" => Some(BinOp::And2),
        "||" => Some(BinOp::Or2),
        ":" => Some(BinOp::Colon),
        _ => None,
    };
    let built = match (op.as_str(), plain) {
        (_, Some(v)) if binop.is_some() && v.len() == 2 => binop.map(|op| Expr::Binary {
            op,
            lhs: b(v[0].clone()),
            rhs: b(v[1].clone()),
        }),
        ("-" | "+" | "!", Some(v)) if v.len() == 1 => {
            let op = match op.as_str() {
                "-" => UnOp::Neg,
                "+" => UnOp::Plus,
                _ => UnOp::Not,
            };
            Some(Expr::Unary {
                op,
                operand: b(v[0].clone()),
            })
        }
        (s, Some(v)) if s.len() >= 2 && s.starts_with('%') && s.ends_with('%') && v.len() == 2 => {
            // The lexer strips the `%`s; `%%` and `%/%` lex to "" and "/".
            let name = match s {
                "%%" => String::new(),
                "%/%" => "/".to_string(),
                other => other[1..other.len() - 1].to_string(),
            };
            Some(Expr::Special {
                name,
                lhs: b(v[0].clone()),
                rhs: b(v[1].clone()),
            })
        }
        ("(", Some(v)) if v.len() == 1 => Some(Expr::Paren(b(v[0].clone()))),
        ("{", Some(v)) => Some(Expr::Block(v)),
        ("if", Some(v)) if v.len() == 2 || v.len() == 3 => Some(Expr::If {
            cond: b(v[0].clone()),
            then: b(v[1].clone()),
            els: v.get(2).cloned().map(b),
        }),
        ("while", Some(v)) if v.len() == 2 => Some(Expr::While {
            cond: b(v[0].clone()),
            body: b(v[1].clone()),
        }),
        ("repeat", Some(v)) if v.len() == 1 => Some(Expr::Repeat(b(v[0].clone()))),
        ("for", Some(v)) if v.len() == 3 => match &v[0] {
            Expr::Ident(var) => Some(Expr::For {
                var: var.clone(),
                seq: b(v[1].clone()),
                body: b(v[2].clone()),
            }),
            _ => None,
        },
        ("<-" | "<<-", Some(v)) if v.len() == 2 => Some(Expr::Assign {
            target: b(v[0].clone()),
            value: b(v[1].clone()),
            super_assign: op == "<<-",
        }),
        ("$" | "@", Some(v)) if v.len() == 2 => match &v[1] {
            Expr::Ident(n) | Expr::Str(n) => Some(Expr::Index {
                kind: if op == "$" {
                    IndexKind::Dollar
                } else {
                    IndexKind::At
                },
                obj: b(v[0].clone()),
                args: vec![Arg {
                    name: None,
                    value: Some(Expr::Str(n.clone())),
                }],
            }),
            _ => None,
        },
        _ => None,
    };
    if let Some(e) = built {
        return e;
    }
    // `[` / `[[` keep tags and empty subscripts (`x[, 1]`, `x[i, drop = FALSE]`).
    if matches!(op.as_str(), "[" | "[[") && args.first().is_some_and(|a| a.name.is_none()) {
        if let Some(obj) = args[0].value.clone() {
            return Expr::Index {
                kind: if op == "[" {
                    IndexKind::Single
                } else {
                    IndexKind::Double
                },
                obj: b(obj),
                args: args[1..].to_vec(),
            };
        }
    }
    Expr::Call { fun: b(fun), args }
}
