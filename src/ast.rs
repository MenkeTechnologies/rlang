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
