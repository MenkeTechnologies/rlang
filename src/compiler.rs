//! Lower the R AST to `fusevm::Chunk`.
//!
//! Control flow (`if`, `for`, `while`, `repeat`, `&&`, `||`) lowers to native
//! fusevm jumps with native `Int` loop counters, so the tracing JIT can compile
//! loops. Everything else — every value, every operator, every call — lowers to
//! a `CallBuiltin` that lands in `builtins.rs`, because in R even `1 + 1` is a
//! vectorized operation over attribute-carrying objects and not something the
//! VM's native `Add` could do.
//!
//! Assignment is compiled the way R defines it: `f(x) <- v` is
//! `x <- \`f<-\`(x, v)`, and `x\[i\] <- v` rebuilds `x` and re-binds it, so nested
//! targets like `x$a\[\[2\]\] <- v` unwind outward-in through the same two rules.

use crate::ast::*;
use crate::host::{ops, ClosureDef};
use fusevm::{Chunk, ChunkBuilder, Op, Value};

/// A compiled program: the top-level chunk plus every closure body it defines.
pub struct Program {
    pub main: Chunk,
    pub closures: Vec<ClosureDef>,
}

/// Jump fixups for one enclosing loop.
struct LoopCtx {
    /// Where `next` jumps to (the increment/condition edge).
    continues: Vec<usize>,
    /// Where `break` jumps to (past the loop).
    breaks: Vec<usize>,
}

#[derive(Default)]
struct Compiler {
    closures: Vec<ClosureDef>,
    loops: Vec<LoopCtx>,
    /// Counter for unique loop temporaries in the VM's own variable space.
    tmp: usize,
}

/// Compile a parsed program.
pub fn compile(exprs: &[Expr]) -> Result<Program, String> {
    let mut c = Compiler::default();
    let mut b = ChunkBuilder::new();
    if exprs.is_empty() {
        b.emit(Op::CallBuiltin(ops::CONST_NULL, 0), 0);
    }
    for (i, e) in exprs.iter().enumerate() {
        c.expr(&mut b, e)?;
        // Top level echoes each visible value, exactly like `Rscript` does.
        b.emit(Op::CallBuiltin(ops::AUTOPRINT, 1), 0);
        if i + 1 < exprs.len() {
            b.emit(Op::Pop, 0);
        }
    }
    Ok(Program {
        main: b.build(),
        closures: c.closures,
    })
}

/// Renumber a program's closure ids by `base`.
///
/// The REPL keeps one host across prompts, so each new program's closure bodies
/// are appended to the ones already loaded; the `LoadInt(id)` that feeds every
/// `MKCLOSURE` has to move with them.
pub fn shift_closure_ids(prog: Program, base: usize) -> Program {
    if base == 0 {
        return prog;
    }
    let shift = |chunk: &mut Chunk| {
        for i in 0..chunk.ops.len() {
            let is_mkclosure = matches!(
                chunk.ops.get(i + 1),
                Some(Op::CallBuiltin(id, 1)) if *id == ops::MKCLOSURE
            );
            if !is_mkclosure {
                continue;
            }
            if let Some(Op::LoadInt(n)) = chunk.ops.get_mut(i) {
                *n += base as i64;
            }
        }
        rehash(chunk);
    };
    let mut main = prog.main;
    shift(&mut main);
    let closures = prog
        .closures
        .into_iter()
        .map(|mut c| {
            shift(&mut c.chunk);
            c
        })
        .collect();
    Program { main, closures }
}

/// Recompute a chunk's cached op hash after rewriting its ops, so the JIT's
/// code cache keys off what the chunk now contains.
fn rehash(chunk: &mut Chunk) {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    chunk.ops.hash(&mut h);
    chunk.constants.hash(&mut h);
    chunk.op_hash = h.finish();
}

impl Compiler {
    fn kstr(&mut self, b: &mut ChunkBuilder, s: &str) {
        let idx = b.add_constant(Value::str(s));
        b.emit(Op::LoadConst(idx), 0);
    }

    fn tmp_name(&mut self, b: &mut ChunkBuilder, tag: &str) -> u16 {
        self.tmp += 1;
        b.add_name(&format!("__{tag}{}", self.tmp))
    }

    /// Compile a body sequence: every value but the last is discarded.
    fn seq(&mut self, b: &mut ChunkBuilder, body: &[Expr]) -> Result<(), String> {
        if body.is_empty() {
            b.emit(Op::CallBuiltin(ops::CONST_NULL, 0), 0);
            return Ok(());
        }
        for (i, e) in body.iter().enumerate() {
            self.expr(b, e)?;
            if i + 1 < body.len() {
                b.emit(Op::Pop, 0);
            }
        }
        Ok(())
    }

    fn expr(&mut self, b: &mut ChunkBuilder, e: &Expr) -> Result<(), String> {
        match e {
            Expr::Num(n) => {
                b.emit(Op::LoadFloat(*n), 0);
                b.emit(Op::CallBuiltin(ops::CONST_DBL, 1), 0);
            }
            Expr::Int(n) => {
                b.emit(Op::LoadInt(*n), 0);
                b.emit(Op::CallBuiltin(ops::CONST_INT, 1), 0);
            }
            Expr::Str(s) => {
                self.kstr(b, s);
                b.emit(Op::CallBuiltin(ops::CONST_STR, 1), 0);
            }
            Expr::Bool(v) => {
                b.emit(if *v { Op::LoadTrue } else { Op::LoadFalse }, 0);
                b.emit(Op::CallBuiltin(ops::CONST_LGL, 1), 0);
            }
            Expr::Null => {
                b.emit(Op::CallBuiltin(ops::CONST_NULL, 0), 0);
            }
            Expr::Na(kind) => {
                let tag = match kind {
                    NaKind::Logical => 0,
                    NaKind::Integer => 1,
                    NaKind::Real => 2,
                    NaKind::Character => 3,
                };
                b.emit(Op::LoadInt(tag), 0);
                b.emit(Op::CallBuiltin(ops::CONST_NA, 1), 0);
            }
            Expr::Inf => {
                b.emit(Op::LoadFloat(f64::INFINITY), 0);
                b.emit(Op::CallBuiltin(ops::CONST_DBL, 1), 0);
            }
            Expr::NaN => {
                b.emit(Op::LoadFloat(f64::NAN), 0);
                b.emit(Op::CallBuiltin(ops::CONST_DBL, 1), 0);
            }
            Expr::Ident(name) => {
                self.kstr(b, name);
                b.emit(Op::CallBuiltin(ops::GETVAR, 1), 0);
            }
            Expr::Dots => {
                b.emit(Op::CallBuiltin(ops::DOTS, 0), 0);
            }
            Expr::Function { params, body } => {
                let id = self.closure(params, body)?;
                b.emit(Op::LoadInt(id as i64), 0);
                b.emit(Op::CallBuiltin(ops::MKCLOSURE, 1), 0);
            }
            Expr::Block(body) => self.seq(b, body)?,
            Expr::Call { fun, args } => {
                // A named callee resolves in function position, skipping
                // non-function bindings (`c <- 1; c(1, 2)` still concatenates).
                match fun.as_ref() {
                    Expr::Ident(name) => {
                        self.kstr(b, name);
                        b.emit(Op::CallBuiltin(ops::GETFUN, 1), 0);
                    }
                    other => self.expr(b, other)?,
                }
                self.args(b, args)?;
                b.emit(Op::CallBuiltin(ops::CALL, 2), 0);
            }
            Expr::Binary { op, lhs, rhs } => match op {
                BinOp::And2 | BinOp::Or2 => self.short_circuit(b, op, lhs, rhs)?,
                _ => {
                    self.expr(b, lhs)?;
                    self.expr(b, rhs)?;
                    self.kstr(b, binop_name(op));
                    b.emit(Op::CallBuiltin(ops::BINOP, 3), 0);
                }
            },
            Expr::Special { name, lhs, rhs } => {
                self.expr(b, lhs)?;
                self.expr(b, rhs)?;
                self.kstr(b, name);
                b.emit(Op::CallBuiltin(ops::SPECIAL, 3), 0);
            }
            Expr::Unary { op, operand } => {
                self.expr(b, operand)?;
                self.kstr(
                    b,
                    match op {
                        UnOp::Neg => "-",
                        UnOp::Plus => "+",
                        UnOp::Not => "!",
                    },
                );
                b.emit(Op::CallBuiltin(ops::UNOP, 2), 0);
            }
            Expr::Index { kind, obj, args } => {
                self.expr(b, obj)?;
                match kind {
                    IndexKind::Dollar | IndexKind::At => {
                        // The name is a literal, not an evaluated expression.
                        match args.first().and_then(|a| a.value.clone()) {
                            Some(Expr::Str(n)) => self.kstr(b, &n),
                            _ => return Err("malformed $ access".into()),
                        }
                        b.emit(Op::CallBuiltin(ops::DOLLAR, 2), 0);
                    }
                    IndexKind::Single => {
                        self.args(b, args)?;
                        b.emit(Op::CallBuiltin(ops::INDEX, 2), 0);
                    }
                    IndexKind::Double => {
                        self.args(b, args)?;
                        b.emit(Op::CallBuiltin(ops::INDEX2, 2), 0);
                    }
                }
            }
            Expr::If { cond, then, els } => {
                self.expr(b, cond)?;
                b.emit(Op::CallBuiltin(ops::TRUTHY, 1), 0);
                let jf = b.emit(Op::JumpIfFalse(0), 0);
                self.expr(b, then)?;
                let jend = b.emit(Op::Jump(0), 0);
                let else_at = b.current_pos();
                b.patch_jump(jf, else_at);
                match els {
                    Some(e) => self.expr(b, e)?,
                    // `if` without `else` yields invisible NULL.
                    None => {
                        b.emit(Op::CallBuiltin(ops::NULL_INVISIBLE, 0), 0);
                    }
                }
                let end = b.current_pos();
                b.patch_jump(jend, end);
            }
            Expr::While { cond, body } => {
                self.loops.push(LoopCtx {
                    continues: Vec::new(),
                    breaks: Vec::new(),
                });
                let start = b.current_pos();
                self.expr(b, cond)?;
                b.emit(Op::CallBuiltin(ops::TRUTHY, 1), 0);
                let jf = b.emit(Op::JumpIfFalse(0), 0);
                self.expr(b, body)?;
                b.emit(Op::Pop, 0);
                b.emit(Op::Jump(start), 0);
                let end = b.current_pos();
                b.patch_jump(jf, end);
                self.close_loop(b, start, end);
                // A loop evaluates to invisible NULL.
                b.emit(Op::CallBuiltin(ops::NULL_INVISIBLE, 0), 0);
            }
            Expr::Repeat(body) => {
                self.loops.push(LoopCtx {
                    continues: Vec::new(),
                    breaks: Vec::new(),
                });
                let start = b.current_pos();
                self.expr(b, body)?;
                b.emit(Op::Pop, 0);
                b.emit(Op::Jump(start), 0);
                let end = b.current_pos();
                self.close_loop(b, start, end);
                // A loop evaluates to invisible NULL.
                b.emit(Op::CallBuiltin(ops::NULL_INVISIBLE, 0), 0);
            }
            Expr::For { var, seq, body } => self.for_loop(b, var, seq, body)?,
            Expr::Break => {
                if self.loops.is_empty() {
                    return Err("no loop for break/next, jumping to top level".into());
                }
                let j = b.emit(Op::Jump(0), 0);
                self.loops.last_mut().unwrap().breaks.push(j);
                // Keep the stack shape uniform: the jump is taken, so this is
                // only reached by the verifier, never at runtime.
                b.emit(Op::CallBuiltin(ops::CONST_NULL, 0), 0);
            }
            Expr::Next => {
                if self.loops.is_empty() {
                    return Err("no loop for break/next, jumping to top level".into());
                }
                let j = b.emit(Op::Jump(0), 0);
                self.loops.last_mut().unwrap().continues.push(j);
                b.emit(Op::CallBuiltin(ops::CONST_NULL, 0), 0);
            }
            Expr::Assign {
                target,
                value,
                super_assign,
            } => self.assign(b, target, value, *super_assign)?,
        }
        Ok(())
    }

    /// Patch every `break`/`next` recorded for the innermost loop.
    fn close_loop(&mut self, b: &mut ChunkBuilder, cont: usize, end: usize) {
        let ctx = self.loops.pop().expect("close_loop without a loop");
        for j in ctx.breaks {
            b.patch_jump(j, end);
        }
        for j in ctx.continues {
            b.patch_jump(j, cont);
        }
    }

    /// `for (v in seq) body` — a native counter loop over the sequence's
    /// elements, so fusevm's JIT sees an ordinary integer loop.
    fn for_loop(
        &mut self,
        b: &mut ChunkBuilder,
        var: &str,
        seq: &Expr,
        body: &Expr,
    ) -> Result<(), String> {
        let v_seq = self.tmp_name(b, "seq");
        let v_len = self.tmp_name(b, "len");
        let v_i = self.tmp_name(b, "i");

        self.expr(b, seq)?;
        b.emit(Op::SetVar(v_seq), 0);
        b.emit(Op::GetVar(v_seq), 0);
        b.emit(Op::CallBuiltin(ops::SEQ_LEN, 1), 0);
        b.emit(Op::SetVar(v_len), 0);
        b.emit(Op::LoadInt(0), 0);
        b.emit(Op::SetVar(v_i), 0);

        self.loops.push(LoopCtx {
            continues: Vec::new(),
            breaks: Vec::new(),
        });
        let start = b.current_pos();
        b.emit(Op::GetVar(v_i), 0);
        b.emit(Op::GetVar(v_len), 0);
        b.emit(Op::NumLt, 0);
        let jf = b.emit(Op::JumpIfFalse(0), 0);

        // var <- seq[[i]]
        self.kstr(b, var);
        b.emit(Op::GetVar(v_seq), 0);
        b.emit(Op::GetVar(v_i), 0);
        b.emit(Op::CallBuiltin(ops::SEQ_ELEM, 2), 0);
        b.emit(Op::CallBuiltin(ops::SETVAR, 2), 0);
        b.emit(Op::Pop, 0);

        self.expr(b, body)?;
        b.emit(Op::Pop, 0);

        let cont = b.current_pos();
        b.emit(Op::GetVar(v_i), 0);
        b.emit(Op::LoadInt(1), 0);
        b.emit(Op::Add, 0);
        b.emit(Op::SetVar(v_i), 0);
        b.emit(Op::Jump(start), 0);
        let end = b.current_pos();
        b.patch_jump(jf, end);
        self.close_loop(b, cont, end);
        // `for` itself evaluates to invisible NULL.
        b.emit(Op::CallBuiltin(ops::NULL_INVISIBLE, 0), 0);
        Ok(())
    }

    /// `&&` / `||`: evaluate the right side only when the left cannot already
    /// decide the answer. A definite FALSE short-circuits `&&`, a definite TRUE
    /// short-circuits `||`; an `NA` left operand falls through to the
    /// vectorized rule, which is how R gets `NA && FALSE` to be FALSE.
    fn short_circuit(
        &mut self,
        b: &mut ChunkBuilder,
        op: &BinOp,
        lhs: &Expr,
        rhs: &Expr,
    ) -> Result<(), String> {
        self.expr(b, lhs)?;
        b.emit(Op::Dup, 0);
        let probe = if matches!(op, BinOp::And2) {
            ops::IS_FALSE
        } else {
            ops::IS_TRUE
        };
        b.emit(Op::CallBuiltin(probe, 1), 0);
        let done = b.emit(Op::JumpIfTrue(0), 0);
        self.expr(b, rhs)?;
        self.kstr(b, binop_name(op));
        b.emit(Op::CallBuiltin(ops::BINOP, 3), 0);
        let end = b.current_pos();
        b.patch_jump(done, end);
        Ok(())
    }

    /// Build the argument list for a call or an index.
    fn args(&mut self, b: &mut ChunkBuilder, args: &[Arg]) -> Result<(), String> {
        for a in args {
            match &a.name {
                Some(n) => self.kstr(b, n),
                None => {
                    b.emit(Op::LoadUndef, 0);
                }
            }
            match &a.value {
                Some(v) => self.expr(b, v)?,
                // An empty argument (`x[, 1]`) is R's "missing" marker.
                None => {
                    b.emit(Op::LoadUndef, 0);
                }
            }
        }
        let argc = u8::try_from(args.len() * 2)
            .map_err(|_| "too many arguments in one call (limit 127)".to_string())?;
        b.emit(Op::CallBuiltin(ops::MKARGS, argc), 0);
        Ok(())
    }

    /// Compile a `function(...)` body into its own chunk, with a prologue that
    /// fills in each defaulted formal the caller omitted.
    fn closure(&mut self, params: &[Param], body: &Expr) -> Result<usize, String> {
        let saved = std::mem::take(&mut self.loops);
        let mut fb = ChunkBuilder::new();
        for p in params {
            let Some(default) = &p.default else { continue };
            // if (missing(p)) p <- <default>
            self.kstr(&mut fb, &p.name);
            fb.emit(Op::CallBuiltin(ops::MISSING, 1), 0);
            let skip = fb.emit(Op::JumpIfFalse(0), 0);
            self.kstr(&mut fb, &p.name);
            self.expr(&mut fb, default)?;
            fb.emit(Op::CallBuiltin(ops::SETVAR, 2), 0);
            fb.emit(Op::Pop, 0);
            let here = fb.current_pos();
            fb.patch_jump(skip, here);
        }
        self.expr(&mut fb, body)?;
        self.loops = saved;
        self.closures.push(ClosureDef {
            params: params.iter().map(|p| p.name.clone()).collect(),
            chunk: fb.build(),
        });
        Ok(self.closures.len() - 1)
    }

    // ── assignment ─────────────────────────────────────────────────────

    fn assign(
        &mut self,
        b: &mut ChunkBuilder,
        target: &Expr,
        value: &Expr,
        sup: bool,
    ) -> Result<(), String> {
        match target {
            Expr::Ident(name) | Expr::Str(name) => {
                self.kstr(b, name);
                self.expr(b, value)?;
                let op = if sup { ops::SETSUPER } else { ops::SETVAR };
                b.emit(Op::CallBuiltin(op, 2), 0);
            }
            _ => {
                self.rebuild(b, target, value, sup)?;
            }
        }
        Ok(())
    }

    /// Compile a complex assignment target: build the modified container, then
    /// assign *that* back to the inner target, recursing outward-in.
    fn rebuild(
        &mut self,
        b: &mut ChunkBuilder,
        target: &Expr,
        value: &Expr,
        sup: bool,
    ) -> Result<(), String> {
        match target {
            Expr::Index { kind, obj, args } => {
                self.expr(b, obj)?;
                match kind {
                    IndexKind::Dollar | IndexKind::At => {
                        match args.first().and_then(|a| a.value.clone()) {
                            Some(Expr::Str(n)) => self.kstr(b, &n),
                            _ => return Err("malformed $ assignment target".into()),
                        }
                        self.expr(b, value)?;
                        b.emit(Op::CallBuiltin(ops::DOLLAR_SET, 3), 0);
                    }
                    IndexKind::Single => {
                        self.args(b, args)?;
                        self.expr(b, value)?;
                        b.emit(Op::CallBuiltin(ops::INDEX_SET, 3), 0);
                    }
                    IndexKind::Double => {
                        self.args(b, args)?;
                        self.expr(b, value)?;
                        b.emit(Op::CallBuiltin(ops::INDEX2_SET, 3), 0);
                    }
                }
                self.assign_stack(b, obj, sup)
            }
            // `f(x, ...) <- v` is `x <- \`f<-\`(x, ..., value = v)`.
            Expr::Call { fun, args } => {
                let Expr::Ident(fname) = fun.as_ref() else {
                    return Err("invalid function in complex assignment".into());
                };
                let Some(inner) = args.first().and_then(|a| a.value.clone()) else {
                    return Err(format!("invalid target for {fname}<-"));
                };
                self.kstr(b, fname);
                self.expr(b, &inner)?;
                self.args(b, &args[1..])?;
                self.expr(b, value)?;
                b.emit(Op::CallBuiltin(ops::REPLACE, 4), 0);
                self.assign_stack(b, &inner, sup)
            }
            other => Err(format!("invalid assignment target: {other:?}")),
        }
    }

    /// Assign the value already on top of the stack to `target`.
    fn assign_stack(
        &mut self,
        b: &mut ChunkBuilder,
        target: &Expr,
        sup: bool,
    ) -> Result<(), String> {
        match target {
            Expr::Ident(name) | Expr::Str(name) => {
                // SETVAR wants [name, value]; the value is already on top.
                self.kstr(b, name);
                b.emit(Op::Swap, 0);
                let op = if sup { ops::SETSUPER } else { ops::SETVAR };
                b.emit(Op::CallBuiltin(op, 2), 0);
                Ok(())
            }
            Expr::Index { kind, obj, args } => {
                // Stack: [value]. Build [obj, args, value] with a rotate.
                self.expr(b, obj)?;
                match kind {
                    IndexKind::Dollar | IndexKind::At => {
                        match args.first().and_then(|a| a.value.clone()) {
                            Some(Expr::Str(n)) => self.kstr(b, &n),
                            _ => return Err("malformed $ assignment target".into()),
                        }
                        b.emit(Op::Rot, 0);
                        b.emit(Op::CallBuiltin(ops::DOLLAR_SET, 3), 0);
                    }
                    IndexKind::Single => {
                        self.args(b, args)?;
                        b.emit(Op::Rot, 0);
                        b.emit(Op::CallBuiltin(ops::INDEX_SET, 3), 0);
                    }
                    IndexKind::Double => {
                        self.args(b, args)?;
                        b.emit(Op::Rot, 0);
                        b.emit(Op::CallBuiltin(ops::INDEX2_SET, 3), 0);
                    }
                }
                self.assign_stack(b, obj, sup)
            }
            other => Err(format!("invalid nested assignment target: {other:?}")),
        }
    }
}

fn binop_name(op: &BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Pow => "^",
        BinOp::Lt => "<",
        BinOp::Gt => ">",
        BinOp::Le => "<=",
        BinOp::Ge => ">=",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::And | BinOp::And2 => "&",
        BinOp::Or | BinOp::Or2 => "|",
        BinOp::Colon => ":",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    fn ops_of(src: &str) -> Vec<Op> {
        compile(&parse(src).unwrap()).unwrap().main.ops
    }

    #[test]
    fn loops_lower_to_native_jumps_not_builtins() {
        let ops = ops_of("for (i in 1:3) i");
        assert!(ops.iter().any(|o| matches!(o, Op::NumLt)));
        assert!(ops.iter().any(|o| matches!(o, Op::JumpIfFalse(_))));
        assert!(ops.iter().any(|o| matches!(o, Op::Add)));
    }

    #[test]
    fn short_circuit_and_emits_a_guard_jump() {
        let ops = ops_of("a && b");
        assert!(ops
            .iter()
            .any(|o| matches!(o, Op::CallBuiltin(id, 1) if *id == ops::IS_FALSE)));
        assert!(ops.iter().any(|o| matches!(o, Op::JumpIfTrue(_))));
    }

    #[test]
    fn nested_index_assignment_rebuilds_outward_in() {
        // x$a[1] <- 9  ->  INDEX_SET on the inner vector, then DOLLAR_SET on x,
        // then SETVAR of x.
        let ops = ops_of("x$a[1] <- 9");
        let ids: Vec<u16> = ops
            .iter()
            .filter_map(|o| match o {
                Op::CallBuiltin(id, _) => Some(*id),
                _ => None,
            })
            .collect();
        let pos = |id: u16| ids.iter().position(|x| *x == id);
        assert!(pos(ops::INDEX_SET) < pos(ops::DOLLAR_SET));
        assert!(pos(ops::DOLLAR_SET) < pos(ops::SETVAR));
    }

    #[test]
    fn replacement_functions_lower_to_replace() {
        let ops = ops_of("names(x) <- c(\"a\")");
        assert!(ops
            .iter()
            .any(|o| matches!(o, Op::CallBuiltin(id, 4) if *id == ops::REPLACE)));
    }

    #[test]
    fn break_outside_a_loop_is_a_compile_error() {
        assert!(compile(&parse("break").unwrap()).is_err());
    }
}
