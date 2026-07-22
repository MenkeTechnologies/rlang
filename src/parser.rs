//! Recursive-descent parser for R.
//!
//! The precedence ladder is R's own (`?Syntax`), from loosest to tightest:
//!
//! ```text
//! =                       (right)
//! <-  <<-                 (right)
//! ->  ->>                 (left, rewritten to <- with sides swapped)
//! |  ||
//! &  &&
//! !                       (unary)
//! ==  !=  <  >  <=  >=
//! +  -
//! *  /
//! %any%   |>
//! :
//! -  +                    (unary)
//! ^                       (right)
//! $  @  [  [[  (          (postfix)
//! ```
//!
//! Newlines end an expression only when it is already complete; the lexer has
//! already dropped the ones inside `(`/`[`, and the parser skips the rest after
//! any operator or comma it consumes.

use crate::ast::*;
use crate::lexer::{lex, Tok, Token};

/// Parse an R program into its top-level expressions.
pub fn parse(src: &str) -> Result<Vec<Expr>, String> {
    let mut p = Parser {
        toks: lex(src)?,
        pos: 0,
    };
    p.program()
}

struct Parser {
    toks: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> &Tok {
        &self.toks[self.pos.min(self.toks.len() - 1)].tok
    }
    fn line(&self) -> u32 {
        self.toks[self.pos.min(self.toks.len() - 1)].line
    }
    fn bump(&mut self) -> Tok {
        let t = self.toks[self.pos.min(self.toks.len() - 1)].tok.clone();
        self.pos += 1;
        t
    }
    fn eat(&mut self, t: &Tok) -> bool {
        if self.peek() == t {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, t: Tok) -> Result<(), String> {
        if self.eat(&t) {
            Ok(())
        } else {
            Err(format!(
                "line {}: expected {:?}, found {:?}",
                self.line(),
                t,
                self.peek()
            ))
        }
    }
    /// Skip newlines (used after operators/commas, where a line break cannot
    /// terminate the expression).
    fn nl(&mut self) {
        while matches!(self.peek(), Tok::Newline) {
            self.pos += 1;
        }
    }
    /// Skip newlines and semicolons (statement separators).
    fn seps(&mut self) {
        while matches!(self.peek(), Tok::Newline | Tok::Semi) {
            self.pos += 1;
        }
    }

    fn program(&mut self) -> Result<Vec<Expr>, String> {
        let mut out = Vec::new();
        self.seps();
        while !matches!(self.peek(), Tok::Eof) {
            out.push(self.expr()?);
            self.seps();
        }
        Ok(out)
    }

    // ── precedence ladder ──────────────────────────────────────────────

    /// Full expression: `=` assignment is the loosest binding form.
    pub fn expr(&mut self) -> Result<Expr, String> {
        let lhs = self.assign_arrow()?;
        if self.eat(&Tok::Eq) {
            self.nl();
            let value = self.expr()?;
            return Ok(Expr::Assign {
                target: Box::new(lhs),
                value: Box::new(value),
                super_assign: false,
            });
        }
        Ok(lhs)
    }

    /// `<-`, `<<-` (right-associative) and `->`, `->>` (rewritten).
    fn assign_arrow(&mut self) -> Result<Expr, String> {
        let lhs = self.or_expr()?;
        match self.peek().clone() {
            Tok::Assign | Tok::SuperAssign => {
                let sup = matches!(self.bump(), Tok::SuperAssign);
                self.nl();
                let value = self.assign_arrow()?;
                Ok(Expr::Assign {
                    target: Box::new(lhs),
                    value: Box::new(value),
                    super_assign: sup,
                })
            }
            Tok::RightAssign | Tok::RightSuper => {
                let sup = matches!(self.bump(), Tok::RightSuper);
                self.nl();
                let target = self.assign_arrow()?;
                Ok(Expr::Assign {
                    target: Box::new(target),
                    value: Box::new(lhs),
                    super_assign: sup,
                })
            }
            _ => Ok(lhs),
        }
    }

    fn or_expr(&mut self) -> Result<Expr, String> {
        let mut lhs = self.and_expr()?;
        loop {
            let op = match self.peek() {
                Tok::Pipe => BinOp::Or,
                Tok::PipePipe => BinOp::Or2,
                _ => return Ok(lhs),
            };
            self.pos += 1;
            self.nl();
            let rhs = self.and_expr()?;
            lhs = bin(op, lhs, rhs);
        }
    }

    fn and_expr(&mut self) -> Result<Expr, String> {
        let mut lhs = self.not_expr()?;
        loop {
            let op = match self.peek() {
                Tok::Amp => BinOp::And,
                Tok::AmpAmp => BinOp::And2,
                _ => return Ok(lhs),
            };
            self.pos += 1;
            self.nl();
            let rhs = self.not_expr()?;
            lhs = bin(op, lhs, rhs);
        }
    }

    fn not_expr(&mut self) -> Result<Expr, String> {
        if self.eat(&Tok::Bang) {
            self.nl();
            let operand = self.not_expr()?;
            return Ok(Expr::Unary {
                op: UnOp::Not,
                operand: Box::new(operand),
            });
        }
        self.comparison()
    }

    fn comparison(&mut self) -> Result<Expr, String> {
        let mut lhs = self.additive()?;
        loop {
            let op = match self.peek() {
                Tok::EqEq => BinOp::Eq,
                Tok::Ne => BinOp::Ne,
                Tok::Lt => BinOp::Lt,
                Tok::Gt => BinOp::Gt,
                Tok::Le => BinOp::Le,
                Tok::Ge => BinOp::Ge,
                _ => return Ok(lhs),
            };
            self.pos += 1;
            self.nl();
            let rhs = self.additive()?;
            lhs = bin(op, lhs, rhs);
        }
    }

    fn additive(&mut self) -> Result<Expr, String> {
        let mut lhs = self.multiplicative()?;
        loop {
            let op = match self.peek() {
                Tok::Plus => BinOp::Add,
                Tok::Minus => BinOp::Sub,
                _ => return Ok(lhs),
            };
            self.pos += 1;
            self.nl();
            let rhs = self.multiplicative()?;
            lhs = bin(op, lhs, rhs);
        }
    }

    fn multiplicative(&mut self) -> Result<Expr, String> {
        let mut lhs = self.special_expr()?;
        loop {
            let op = match self.peek() {
                Tok::Star => BinOp::Mul,
                Tok::Slash => BinOp::Div,
                _ => return Ok(lhs),
            };
            self.pos += 1;
            self.nl();
            let rhs = self.special_expr()?;
            lhs = bin(op, lhs, rhs);
        }
    }

    /// `%any%` infix operators and the native pipe `|>`, which share a level.
    fn special_expr(&mut self) -> Result<Expr, String> {
        let mut lhs = self.range_expr()?;
        loop {
            match self.peek().clone() {
                Tok::Special(name) => {
                    self.pos += 1;
                    self.nl();
                    let rhs = self.range_expr()?;
                    lhs = Expr::Special {
                        name,
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                    };
                }
                // `x |> f()` is pure syntax: it inserts `x` as the first
                // argument of the call on the right.
                Tok::PipeGt => {
                    self.pos += 1;
                    self.nl();
                    let rhs = self.range_expr()?;
                    lhs = pipe_into(lhs, rhs)?;
                }
                _ => return Ok(lhs),
            }
        }
    }

    fn range_expr(&mut self) -> Result<Expr, String> {
        let mut lhs = self.unary()?;
        while self.eat(&Tok::Colon) {
            self.nl();
            let rhs = self.unary()?;
            lhs = bin(BinOp::Colon, lhs, rhs);
        }
        Ok(lhs)
    }

    fn unary(&mut self) -> Result<Expr, String> {
        let op = match self.peek() {
            Tok::Minus => UnOp::Neg,
            Tok::Plus => UnOp::Plus,
            _ => return self.power(),
        };
        self.pos += 1;
        self.nl();
        let operand = self.unary()?;
        Ok(Expr::Unary {
            op,
            operand: Box::new(operand),
        })
    }

    /// `^` binds tighter than unary minus and is right-associative, so
    /// `-2^2` is `-(2^2)` and `2^3^2` is `2^(3^2)`.
    fn power(&mut self) -> Result<Expr, String> {
        let lhs = self.postfix()?;
        if self.eat(&Tok::Caret) {
            self.nl();
            let rhs = self.unary()?;
            return Ok(bin(BinOp::Pow, lhs, rhs));
        }
        Ok(lhs)
    }

    fn postfix(&mut self) -> Result<Expr, String> {
        let mut e = self.primary()?;
        loop {
            match self.peek() {
                Tok::LParen => {
                    self.pos += 1;
                    let args = self.args(&Tok::RParen)?;
                    self.expect(Tok::RParen)?;
                    e = Expr::Call {
                        fun: Box::new(e),
                        args,
                    };
                }
                Tok::LBracket => {
                    self.pos += 1;
                    let args = self.args(&Tok::RBracket)?;
                    self.expect(Tok::RBracket)?;
                    e = Expr::Index {
                        kind: IndexKind::Single,
                        obj: Box::new(e),
                        args,
                    };
                }
                Tok::LBracket2 => {
                    self.pos += 1;
                    let args = self.args(&Tok::RBracket)?;
                    // `[[` is closed by two separate `]` tokens (see lexer).
                    self.expect(Tok::RBracket)?;
                    self.expect(Tok::RBracket)?;
                    e = Expr::Index {
                        kind: IndexKind::Double,
                        obj: Box::new(e),
                        args,
                    };
                }
                Tok::Dollar | Tok::At => {
                    let kind = if matches!(self.bump(), Tok::Dollar) {
                        IndexKind::Dollar
                    } else {
                        IndexKind::At
                    };
                    self.nl();
                    let name = match self.bump() {
                        Tok::Ident(n) => n,
                        Tok::Str(s) => s,
                        other => {
                            return Err(format!(
                                "line {}: expected a name after $/@, found {other:?}",
                                self.line()
                            ))
                        }
                    };
                    e = Expr::Index {
                        kind,
                        obj: Box::new(e),
                        args: vec![Arg {
                            name: None,
                            value: Some(Expr::Str(name)),
                        }],
                    };
                }
                // `pkg::name` — rlang has one namespace, so the qualifier is
                // parsed and dropped rather than silently mis-resolving.
                Tok::ColonColon => {
                    self.pos += 1;
                    e = self.primary()?;
                }
                _ => return Ok(e),
            }
        }
    }

    /// A comma-separated argument list, allowing empty slots (`x[, 1]`).
    fn args(&mut self, close: &Tok) -> Result<Vec<Arg>, String> {
        let mut out = Vec::new();
        self.nl();
        if self.peek() == close {
            return Ok(out);
        }
        loop {
            self.nl();
            // A tagged argument: `name = value`, `"name" = value`, or a bare
            // `name =` (missing value).
            let tag = self.arg_tag();
            self.nl();
            let value = if self.peek() == close || matches!(self.peek(), Tok::Comma) {
                None
            } else {
                Some(self.expr()?)
            };
            out.push(Arg { name: tag, value });
            self.nl();
            if !self.eat(&Tok::Comma) {
                break;
            }
        }
        Ok(out)
    }

    /// Consume `name =` if that is what comes next; otherwise consume nothing.
    fn arg_tag(&mut self) -> Option<String> {
        let name = match self.peek().clone() {
            Tok::Ident(n) => n,
            Tok::Str(s) => s,
            Tok::Dots => "...".to_string(),
            _ => return None,
        };
        if matches!(self.toks.get(self.pos + 1).map(|t| &t.tok), Some(Tok::Eq)) {
            self.pos += 2;
            Some(name)
        } else {
            None
        }
    }

    fn primary(&mut self) -> Result<Expr, String> {
        let line = self.line();
        match self.bump() {
            Tok::Num(n) => Ok(Expr::Num(n)),
            Tok::Int(n) => Ok(Expr::Int(n)),
            Tok::Str(s) => Ok(Expr::Str(s)),
            Tok::True => Ok(Expr::Bool(true)),
            Tok::False => Ok(Expr::Bool(false)),
            Tok::Null => Ok(Expr::Null),
            Tok::Na(k) => Ok(Expr::Na(k)),
            Tok::Inf => Ok(Expr::Inf),
            Tok::NaN => Ok(Expr::NaN),
            Tok::Dots => Ok(Expr::Dots),
            Tok::Ident(n) => Ok(Expr::Ident(n)),
            Tok::Break => Ok(Expr::Break),
            Tok::Next => Ok(Expr::Next),
            Tok::LParen => {
                self.nl();
                let e = self.expr()?;
                self.nl();
                self.expect(Tok::RParen)?;
                Ok(e)
            }
            Tok::LBrace => {
                let mut body = Vec::new();
                self.seps();
                while !matches!(self.peek(), Tok::RBrace | Tok::Eof) {
                    body.push(self.expr()?);
                    self.seps();
                }
                self.expect(Tok::RBrace)?;
                Ok(Expr::Block(body))
            }
            Tok::If => {
                self.nl();
                self.expect(Tok::LParen)?;
                let cond = self.expr()?;
                self.expect(Tok::RParen)?;
                self.nl();
                let then = self.expr()?;
                // `else` may sit on the next line inside a `{ }` block; look
                // past newlines for it and rewind if it is not there.
                let save = self.pos;
                self.seps();
                let els = if self.eat(&Tok::Else) {
                    self.nl();
                    Some(Box::new(self.expr()?))
                } else {
                    self.pos = save;
                    None
                };
                Ok(Expr::If {
                    cond: Box::new(cond),
                    then: Box::new(then),
                    els,
                })
            }
            Tok::For => {
                self.nl();
                self.expect(Tok::LParen)?;
                let var = match self.bump() {
                    Tok::Ident(n) => n,
                    other => return Err(format!("line {line}: bad for-loop variable {other:?}")),
                };
                self.expect(Tok::In)?;
                let seq = self.expr()?;
                self.expect(Tok::RParen)?;
                self.nl();
                let body = self.expr()?;
                Ok(Expr::For {
                    var,
                    seq: Box::new(seq),
                    body: Box::new(body),
                })
            }
            Tok::While => {
                self.nl();
                self.expect(Tok::LParen)?;
                let cond = self.expr()?;
                self.expect(Tok::RParen)?;
                self.nl();
                let body = self.expr()?;
                Ok(Expr::While {
                    cond: Box::new(cond),
                    body: Box::new(body),
                })
            }
            Tok::Repeat => {
                self.nl();
                let body = self.expr()?;
                Ok(Expr::Repeat(Box::new(body)))
            }
            Tok::Function => {
                self.nl();
                self.expect(Tok::LParen)?;
                let params = self.params()?;
                self.expect(Tok::RParen)?;
                self.nl();
                let body = self.expr()?;
                Ok(Expr::Function {
                    params,
                    body: Box::new(body),
                })
            }
            // A leading `-`/`+`/`!` can reach here through `(`-wrapped forms.
            Tok::Minus => Ok(Expr::Unary {
                op: UnOp::Neg,
                operand: Box::new(self.unary()?),
            }),
            other => Err(format!("line {line}: unexpected {other:?}")),
        }
    }

    fn params(&mut self) -> Result<Vec<Param>, String> {
        let mut out = Vec::new();
        self.nl();
        if matches!(self.peek(), Tok::RParen) {
            return Ok(out);
        }
        loop {
            self.nl();
            let name = match self.bump() {
                Tok::Ident(n) => n,
                Tok::Dots => "...".to_string(),
                other => return Err(format!("line {}: bad parameter {other:?}", self.line())),
            };
            self.nl();
            let default = if self.eat(&Tok::Eq) {
                self.nl();
                Some(self.expr()?)
            } else {
                None
            };
            out.push(Param { name, default });
            self.nl();
            if !self.eat(&Tok::Comma) {
                break;
            }
        }
        Ok(out)
    }
}

fn bin(op: BinOp, lhs: Expr, rhs: Expr) -> Expr {
    Expr::Binary {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

/// `lhs |> f(a)` becomes `f(lhs, a)`. R requires the right side to be a call.
fn pipe_into(lhs: Expr, rhs: Expr) -> Result<Expr, String> {
    match rhs {
        Expr::Call { fun, mut args } => {
            args.insert(
                0,
                Arg {
                    name: None,
                    value: Some(lhs),
                },
            );
            Ok(Expr::Call { fun, args })
        }
        other => Err(format!(
            "the right-hand side of |> must be a call, found {other:?}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(src: &str) -> Expr {
        let mut v = parse(src).unwrap();
        assert_eq!(v.len(), 1, "expected exactly one top-level expression");
        v.pop().unwrap()
    }

    #[test]
    fn power_binds_tighter_than_unary_minus() {
        // R: -2^2 is -4, not 4.
        match one("-2^2") {
            Expr::Unary {
                op: UnOp::Neg,
                operand,
            } => assert!(matches!(*operand, Expr::Binary { op: BinOp::Pow, .. })),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn power_is_right_associative() {
        match one("2^3^2") {
            Expr::Binary {
                op: BinOp::Pow,
                rhs,
                ..
            } => assert!(matches!(*rhs, Expr::Binary { op: BinOp::Pow, .. })),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn colon_binds_tighter_than_arithmetic() {
        // R: 1:n-1 parses as (1:n)-1.
        match one("1:n-1") {
            Expr::Binary {
                op: BinOp::Sub,
                lhs,
                ..
            } => assert!(matches!(
                *lhs,
                Expr::Binary {
                    op: BinOp::Colon,
                    ..
                }
            )),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn right_arrow_assignment_swaps_sides() {
        match one("1 + 1 -> x") {
            Expr::Assign { target, value, .. } => {
                assert_eq!(*target, Expr::Ident("x".into()));
                assert!(matches!(*value, Expr::Binary { op: BinOp::Add, .. }));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn empty_index_arguments_are_preserved() {
        // `m[, 1]` — the empty first argument selects every row.
        match one("m[, 1]") {
            Expr::Index { args, .. } => {
                assert_eq!(args.len(), 2);
                assert!(args[0].value.is_none());
                assert_eq!(args[1].value, Some(Expr::Num(1.0)));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn pipe_inserts_lhs_as_first_argument() {
        match one("x |> f(2)") {
            Expr::Call { fun, args } => {
                assert_eq!(*fun, Expr::Ident("f".into()));
                assert_eq!(args[0].value, Some(Expr::Ident("x".into())));
                assert_eq!(args[1].value, Some(Expr::Num(2.0)));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn newline_ends_a_complete_expression_but_not_an_open_one() {
        assert_eq!(parse("1\n2").unwrap().len(), 2);
        assert_eq!(parse("1 +\n2").unwrap().len(), 1);
    }

    #[test]
    fn else_may_follow_on_a_new_line_inside_braces() {
        let e = one("{\nif (x) 1\nelse 2\n}");
        match e {
            Expr::Block(b) => assert!(matches!(b[0], Expr::If { els: Some(_), .. })),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn function_defaults_and_dots_parse() {
        match one("function(x, y = 2, ...) x") {
            Expr::Function { params, .. } => {
                assert_eq!(params[0].name, "x");
                assert_eq!(params[1].default, Some(Expr::Num(2.0)));
                assert_eq!(params[2].name, "...");
            }
            other => panic!("{other:?}"),
        }
    }
}
