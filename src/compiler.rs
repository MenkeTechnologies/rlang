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
    /// Names bound to native frame slots (fusevm `GetVar`/`SetVar`, JIT-visible)
    /// instead of the string-keyed environment. Empty unless the whole top level
    /// is slot-safe (see [`slot_locals`]).
    locals: std::collections::HashSet<String>,
}

/// Builtins that reach into an environment by name; their presence makes it
/// unsafe to keep any local in a native slot instead of the string environment.
const DYNAMIC_ENV_FNS: &[&str] = &[
    "get", "get0", "mget", "assign", "exists", "environment", "environmentName",
    "eval", "evalq", "local", "with", "within", "do.call", "Recall", "sys.call",
    "sys.function", "sys.frame", "match.call", "match.arg", "parent.frame",
    "parent.env", "ls", "rm", "remove", "attach", "detach", "substitute", "quote",
    "bquote", "missing", "on.exit", "delayedAssign", "makeActiveBinding", "new.env",
    "globalenv", "as.environment", "list2env", "eapply", "Reduce", "Filter", "Map",
    "do.call", "apply", "sapply", "lapply", "vapply", "mapply", "tapply", "outer",
];

/// The names in a whole-program top level that are safe to bind to native frame
/// slots: the unit must contain no nested `function` (a closure captures the
/// enclosing frame by name), no `<<-`, no formula, and no call to an
/// environment-reaching builtin. A name assigned through a complex target
/// (`x[i] <-`, `x$f <-`) is excluded — `rebuild` reads/writes it by name — so
/// its every access stays on the environment path. If the unit is not slot-safe
/// the set is empty and nothing changes from the string-keyed default.
fn slot_locals(exprs: &[Expr]) -> std::collections::HashSet<String> {
    let mut targets = std::collections::HashSet::new();
    let mut blocked = std::collections::HashSet::new();
    let mut safe = true;
    for e in exprs {
        scan_slots(e, &mut safe, &mut targets, &mut blocked);
    }
    if !safe {
        return std::collections::HashSet::new();
    }
    targets.retain(|n| !blocked.contains(n));
    targets
}

/// The root variable name of a (possibly complex) assignment target, e.g. `x`
/// for `x[[1]]$y[2]`.
fn root_ident(e: &Expr) -> Option<&str> {
    match e {
        Expr::Ident(n) | Expr::Str(n) => Some(n),
        Expr::Index { obj, .. } => root_ident(obj),
        Expr::Call { args, .. } => args.first().and_then(|a| a.value.as_ref()).and_then(root_ident),
        _ => None,
    }
}

fn scan_slots(
    e: &Expr,
    safe: &mut bool,
    targets: &mut std::collections::HashSet<String>,
    blocked: &mut std::collections::HashSet<String>,
) {
    if !*safe {
        return;
    }
    let mut go = |x: &Expr, s: &mut bool| scan_slots(x, s, targets, blocked);
    match e {
        Expr::Function { .. } | Expr::Formula { .. } => *safe = false,
        Expr::Assign { target, value, super_assign } => {
            if *super_assign {
                *safe = false;
                return;
            }
            match target.as_ref() {
                Expr::Ident(n) | Expr::Str(n) => {
                    targets.insert(n.clone());
                }
                other => {
                    if let Some(r) = root_ident(other) {
                        blocked.insert(r.to_string());
                    }
                    scan_slots(other, safe, targets, blocked);
                }
            }
            scan_slots(value, safe, targets, blocked);
        }
        Expr::For { var, seq, body } => {
            targets.insert(var.clone());
            scan_slots(seq, safe, targets, blocked);
            scan_slots(body, safe, targets, blocked);
        }
        Expr::Call { fun, args } => {
            if let Expr::Ident(name) = fun.as_ref() {
                if DYNAMIC_ENV_FNS.contains(&name.as_str()) {
                    *safe = false;
                    return;
                }
            }
            scan_slots(fun, safe, targets, blocked);
            for a in args {
                if let Some(v) = &a.value {
                    scan_slots(v, safe, targets, blocked);
                }
            }
        }
        Expr::If { cond, then, els } => {
            go(cond, safe);
            go(then, safe);
            if let Some(x) = els {
                go(x, safe);
            }
        }
        Expr::While { cond, body } => {
            go(cond, safe);
            go(body, safe);
        }
        Expr::Repeat(b) => go(b, safe),
        Expr::Block(xs) => {
            for x in xs {
                scan_slots(x, safe, targets, blocked);
            }
        }
        Expr::Binary { lhs, rhs, .. } | Expr::Special { lhs, rhs, .. } => {
            go(lhs, safe);
            go(rhs, safe);
        }
        Expr::Unary { operand, .. } => go(operand, safe),
        Expr::Index { obj, args, .. } => {
            go(obj, safe);
            for a in args {
                if let Some(v) = &a.value {
                    scan_slots(v, safe, targets, blocked);
                }
            }
        }
        _ => {}
    }
}

/// Compile a whole program — top-level locals may be bound to native frame
/// slots (JIT-visible) when the unit is slot-safe.
pub fn compile(exprs: &[Expr]) -> Result<Program, String> {
    compile_inner(exprs, true)
}

/// Compile without slot binding. The REPL keeps one host across prompts and
/// persists variables through the shared environment, so its top-level names
/// must live there, not in a per-chunk slot vector.
pub fn compile_no_slots(exprs: &[Expr]) -> Result<Program, String> {
    compile_inner(exprs, false)
}

fn compile_inner(exprs: &[Expr], use_slots: bool) -> Result<Program, String> {
    let mut c = Compiler::default();
    if use_slots {
        c.locals = slot_locals(exprs);
    }
    let mut b = ChunkBuilder::new();
    if exprs.is_empty() {
        b.emit(Op::CallBuiltin(ops::CONST_NULL, 0), 0);
    }
    for (i, e) in exprs.iter().enumerate() {
        // A slot assignment's native `SetVar` does not clear the visibility flag
        // the way the `SETVAR` builtin does, so it can't reach `AUTOPRINT` — but
        // an assignment is invisible and never echoed anyway, so `stmt` stores it
        // directly (no Dup/Pop). All other top-level values are echoed like
        // `Rscript`.
        let slot_assign = matches!(e, Expr::Assign { target, super_assign: false, .. }
            if matches!(target.as_ref(), Expr::Ident(n) | Expr::Str(n) if c.locals.contains(n)));
        if slot_assign {
            c.stmt(&mut b, e)?;
        } else {
            c.expr(&mut b, e)?;
            b.emit(Op::CallBuiltin(ops::AUTOPRINT, 1), 0);
            if i + 1 < exprs.len() {
                b.emit(Op::Pop, 0);
            }
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
            if i + 1 < body.len() {
                self.stmt(b, e)?;
            } else {
                self.expr(b, e)?;
            }
        }
        Ok(())
    }

    /// Compile an expression in statement position — its value is discarded. A
    /// slot assignment stores directly (native `SetVar` consumes the value) with
    /// no `Dup`/`Pop`, saving two ops per statement: this is the hot path of a
    /// `for`/`while` body like `s <- s + i`.
    fn stmt(&mut self, b: &mut ChunkBuilder, e: &Expr) -> Result<(), String> {
        if let Expr::Assign { target, value, super_assign: false } = e {
            if let Expr::Ident(n) | Expr::Str(n) = target.as_ref() {
                if self.locals.contains(n) {
                    self.expr(b, value)?;
                    let slot = b.add_name(n);
                    b.emit(Op::SetVar(slot), 0);
                    return Ok(());
                }
            }
        }
        self.expr(b, e)?;
        b.emit(Op::Pop, 0);
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
                if self.locals.contains(name) {
                    // A slot-bound local: native read, JIT-visible, no env hash.
                    let slot = b.add_name(name);
                    b.emit(Op::GetVar(slot), 0);
                } else {
                    self.kstr(b, name);
                    b.emit(Op::CallBuiltin(ops::GETVAR, 1), 0);
                }
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
                // `switch` is a lazy special form (only the selected branch may
                // run), so it compiles to a jump table rather than an eager call.
                if let Expr::Ident(name) = fun.as_ref() {
                    if name == "switch" && !args.is_empty() {
                        return self.switch_expr(b, args);
                    }
                }
                // A named callee resolves in function position, skipping
                // non-function bindings (`c <- 1; c(1, 2)` still concatenates).
                match fun.as_ref() {
                    Expr::Ident(name) => {
                        self.kstr(b, name);
                        b.emit(Op::CallBuiltin(ops::GETFUN, 1), 0);
                    }
                    other => self.expr(b, other)?,
                }
                // `library(pkg)`/`require(pkg)` take the package name unevaluated
                // (NSE); a bare symbol is compiled as its string name so the
                // loader receives it instead of failing to find a variable.
                let pkg_nse = matches!(
                    fun.as_ref(),
                    Expr::Ident(n) if matches!(n.as_str(),
                        "library" | "require" | "requireNamespace" | "loadNamespace")
                );
                if pkg_nse {
                    if let Some(Arg { name, value: Some(Expr::Ident(sym)) }) = args.first() {
                        let mut rewritten = args.clone();
                        rewritten[0] = Arg {
                            name: name.clone(),
                            value: Some(Expr::Str(sym.clone())),
                        };
                        self.args(b, &rewritten)?;
                        b.emit(Op::CallBuiltin(ops::CALL, 2), 0);
                        return Ok(());
                    }
                }
                self.args(b, args)?;
                b.emit(Op::CallBuiltin(ops::CALL, 2), 0);
            }
            Expr::Formula { lhs, rhs } => {
                // A formula is unevaluated language: deparse it back to R source
                // and build the formula object in the embedded R.
                let src = match lhs {
                    Some(l) => format!("{} ~ {}", deparse_ast(l), deparse_ast(rhs)),
                    None => format!("~ {}", deparse_ast(rhs)),
                };
                let call = Expr::Call {
                    fun: Box::new(Expr::Ident(".rlang_formula".into())),
                    args: vec![Arg {
                        name: None,
                        value: Some(Expr::Str(src)),
                    }],
                };
                self.expr(b, &call)?;
            }
            Expr::Binary { op, lhs, rhs } => match op {
                BinOp::And2 | BinOp::Or2 => self.short_circuit(b, op, lhs, rhs)?,
                _ => {
                    self.expr(b, lhs)?;
                    self.expr(b, rhs)?;
                    // `+ - * /` lower to native fusevm ops: two unboxed scalars
                    // compute directly in the dispatch loop (and are JIT-visible),
                    // while a boxed vector/NA operand delegates to R's `arith`
                    // through the VM's numeric hook (installed in builtins). Ops
                    // whose native semantics differ from R (`%%` sign, `^` NA
                    // rules, comparison NaN→NA) keep the builtin path.
                    match native_binop(op) {
                        Some(nop) => {
                            b.emit(nop, 0);
                        }
                        None => {
                            self.kstr(b, binop_name(op));
                            b.emit(Op::CallBuiltin(ops::BINOP, 3), 0);
                        }
                    }
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
                self.stmt(b, body)?;
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
                self.stmt(b, body)?;
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
        // `for (i in a:b)` with a slot loop variable iterates a native integer
        // counter and computes `i = from + c*step` with native ops — no
        // per-element builtin call, so `--aot` can lower the whole loop.
        if self.locals.contains(var) {
            if let Expr::Binary { op: BinOp::Colon, lhs, rhs } = seq {
                return self.for_range(b, var, lhs, rhs, body);
            }
        }
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
        if self.locals.contains(var) {
            // Native store into the loop variable's slot; `SetVar` consumes the
            // fetched element, leaving nothing (no trailing `Pop`).
            b.emit(Op::GetVar(v_seq), 0);
            b.emit(Op::GetVar(v_i), 0);
            b.emit(Op::CallBuiltin(ops::SEQ_ELEM, 2), 0);
            let slot = b.add_name(var);
            b.emit(Op::SetVar(slot), 0);
        } else {
            self.kstr(b, var);
            b.emit(Op::GetVar(v_seq), 0);
            b.emit(Op::GetVar(v_i), 0);
            b.emit(Op::CallBuiltin(ops::SEQ_ELEM, 2), 0);
            b.emit(Op::CallBuiltin(ops::SETVAR, 2), 0);
            b.emit(Op::Pop, 0);
        }

        self.stmt(b, body)?;

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

    /// `for (i in from:to)` as a native counter loop: `RANGE_FROM/STEP/LEN` (one
    /// call each, at setup) reproduce `:`'s typed start, ±1 step, and count; the
    /// body computes `i = from + c*step` with native `Mul`/`Add`, `NumLt` for the
    /// bound — no `SEQ_ELEM`, no `1:N` materialization, and `--aot`-lowerable.
    fn for_range(
        &mut self,
        b: &mut ChunkBuilder,
        var: &str,
        lhs: &Expr,
        rhs: &Expr,
        body: &Expr,
    ) -> Result<(), String> {
        let v_a = self.tmp_name(b, "ra");
        let v_b = self.tmp_name(b, "rb");
        let v_from = self.tmp_name(b, "rfrom");
        let v_step = self.tmp_name(b, "rstep");
        let v_len = self.tmp_name(b, "rlen");
        let v_c = self.tmp_name(b, "rc");
        let islot = b.add_name(var);

        self.expr(b, lhs)?;
        b.emit(Op::SetVar(v_a), 0);
        self.expr(b, rhs)?;
        b.emit(Op::SetVar(v_b), 0);
        for (op, dst) in [
            (ops::RANGE_FROM, v_from),
            (ops::RANGE_STEP, v_step),
            (ops::RANGE_LEN, v_len),
        ] {
            b.emit(Op::GetVar(v_a), 0);
            b.emit(Op::GetVar(v_b), 0);
            b.emit(Op::CallBuiltin(op, 2), 0);
            b.emit(Op::SetVar(dst), 0);
        }
        b.emit(Op::LoadInt(0), 0);
        b.emit(Op::SetVar(v_c), 0);

        self.loops.push(LoopCtx {
            continues: Vec::new(),
            breaks: Vec::new(),
        });
        let start = b.current_pos();
        b.emit(Op::GetVar(v_c), 0);
        b.emit(Op::GetVar(v_len), 0);
        b.emit(Op::NumLt, 0);
        let jf = b.emit(Op::JumpIfFalse(0), 0);

        b.emit(Op::GetVar(v_from), 0);
        b.emit(Op::GetVar(v_c), 0);
        b.emit(Op::GetVar(v_step), 0);
        b.emit(Op::Mul, 0);
        b.emit(Op::Add, 0);
        b.emit(Op::SetVar(islot), 0);

        self.stmt(b, body)?;

        let cont = b.current_pos();
        b.emit(Op::GetVar(v_c), 0);
        b.emit(Op::LoadInt(1), 0);
        b.emit(Op::Add, 0);
        b.emit(Op::SetVar(v_c), 0);
        b.emit(Op::Jump(start), 0);
        let end = b.current_pos();
        b.patch_jump(jf, end);
        self.close_loop(b, cont, end);
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

    /// Compile `switch(EXPR, name = value, …, default)` as a lazy jump table:
    /// evaluate `EXPR` once, ask `SWITCH_INDEX` which branch to run, then jump
    /// straight to that branch's code — no other branch is evaluated. An empty
    /// branch (`a =`) falls through to the next branch's value.
    fn switch_expr(&mut self, b: &mut ChunkBuilder, args: &[Arg]) -> Result<(), String> {
        let expr = args[0]
            .value
            .as_ref()
            .ok_or("switch: missing EXPR argument")?;
        let branches = &args[1..];
        let m = branches.len();
        self.expr(b, expr)?;
        for br in branches {
            self.kstr(b, br.name.as_deref().unwrap_or(""));
        }
        let argc = u8::try_from(1 + m).map_err(|_| "switch: too many branches".to_string())?;
        b.emit(Op::CallBuiltin(ops::SWITCH_INDEX, argc), 0);

        // Dispatch: the index `i` stays on the stack; for each branch `k`,
        // `Dup; LoadInt k; Eq; JumpIfTrue body_k`.
        let mut jt = Vec::with_capacity(m);
        for k in 0..m {
            b.emit(Op::Dup, 0);
            b.emit(Op::LoadInt(k as i64), 0);
            b.emit(Op::NumEq, 0);
            jt.push(b.emit(Op::JumpIfTrue(0), 0));
        }
        // No branch matched: drop `i`, yield invisible NULL.
        b.emit(Op::Pop, 0);
        b.emit(Op::CallBuiltin(ops::NULL_INVISIBLE, 0), 0);
        let mut end_jumps = vec![b.emit(Op::Jump(0), 0)];

        // Bodies. `value_start[k]` is the point after `k`'s `Pop`, where a
        // fall-through from an earlier empty branch lands.
        let mut value_start = vec![0usize; m];
        let mut fallthrough: Vec<(usize, usize)> = Vec::new();
        for k in 0..m {
            let body_start = b.current_pos();
            b.patch_jump(jt[k], body_start);
            b.emit(Op::Pop, 0); // drop `i`
            value_start[k] = b.current_pos();
            match branches[k].value.as_ref() {
                Some(v) => {
                    self.expr(b, v)?;
                    end_jumps.push(b.emit(Op::Jump(0), 0));
                }
                None => {
                    if k + 1 < m {
                        // Empty branch: run the next branch's value instead.
                        fallthrough.push((k, b.emit(Op::Jump(0), 0)));
                    } else {
                        b.emit(Op::CallBuiltin(ops::NULL_INVISIBLE, 0), 0);
                        end_jumps.push(b.emit(Op::Jump(0), 0));
                    }
                }
            }
        }
        for (k, j) in fallthrough {
            b.patch_jump(j, value_start[k + 1]);
        }
        let end = b.current_pos();
        for j in end_jumps {
            b.patch_jump(j, end);
        }
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
            Expr::Ident(name) | Expr::Str(name) if !sup && self.locals.contains(name) => {
                // A slot-bound local: native store, JIT-visible. `SetVar` pops
                // the value, so `Dup` keeps the copy an assignment expression
                // yields (statements pop it, `y <- x <- 1` and `f(x <- 1)` use it).
                self.expr(b, value)?;
                b.emit(Op::Dup, 0);
                let slot = b.add_name(name);
                b.emit(Op::SetVar(slot), 0);
            }
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

/// Deparse an expression back to R source — enough of the grammar to reconstruct
/// what appears inside a model formula (`y ~ x + log(z)`, `v ~ g`, `~ .`).
pub(crate) fn deparse_ast(e: &Expr) -> String {
    match e {
        Expr::Num(n) => {
            if *n == n.trunc() && n.abs() < 1e15 {
                format!("{}", *n as i64)
            } else {
                format!("{n}")
            }
        }
        Expr::Int(i) => format!("{i}L"),
        Expr::Str(s) => format!("{s:?}"),
        Expr::Bool(true) => "TRUE".into(),
        Expr::Bool(false) => "FALSE".into(),
        Expr::Null => "NULL".into(),
        Expr::Na(_) => "NA".into(),
        Expr::Inf => "Inf".into(),
        Expr::NaN => "NaN".into(),
        Expr::Ident(s) => s.clone(),
        Expr::Dots => "...".into(),
        Expr::Formula { lhs, rhs } => match lhs {
            Some(l) => format!("{} ~ {}", deparse_ast(l), deparse_ast(rhs)),
            None => format!("~ {}", deparse_ast(rhs)),
        },
        Expr::Binary { op, lhs, rhs } => {
            format!("{} {} {}", deparse_ast(lhs), binop_name(op), deparse_ast(rhs))
        }
        Expr::Special { name, lhs, rhs } => {
            format!("{} %{}% {}", deparse_ast(lhs), name, deparse_ast(rhs))
        }
        Expr::Unary { op, operand } => {
            let s = match op {
                crate::ast::UnOp::Neg => "-",
                crate::ast::UnOp::Plus => "+",
                crate::ast::UnOp::Not => "!",
            };
            format!("{s}{}", deparse_ast(operand))
        }
        Expr::Call { fun, args } => {
            let parts: Vec<String> = args
                .iter()
                .map(|a| {
                    let v = a.value.as_ref().map(deparse_ast).unwrap_or_default();
                    match &a.name {
                        Some(n) => format!("{n} = {v}"),
                        None => v,
                    }
                })
                .collect();
            format!("{}({})", deparse_ast(fun), parts.join(", "))
        }
        Expr::Index { kind, obj, args } => {
            let inner: Vec<String> = args
                .iter()
                .map(|a| a.value.as_ref().map(deparse_ast).unwrap_or_default())
                .collect();
            match kind {
                crate::ast::IndexKind::Single => format!("{}[{}]", deparse_ast(obj), inner.join(", ")),
                crate::ast::IndexKind::Double => format!("{}[[{}]]", deparse_ast(obj), inner.join(", ")),
                crate::ast::IndexKind::Dollar => format!("{}${}", deparse_ast(obj), inner.join("")),
                crate::ast::IndexKind::At => format!("{}@{}", deparse_ast(obj), inner.join("")),
            }
        }
        // Anything else (blocks, closures, assignments) is not expected inside a
        // formula; fall back to a placeholder rather than mis-rendering.
        _ => ".".into(),
    }
}

/// The native fusevm op for a binary operator whose scalar semantics match R
/// exactly. `Op::Div` is always-float (so `5L/2L` is `2.5`, as R). `%%`/`%/%`
/// arrive as `Expr::Special`, not here; `^`, comparisons, and `& |` keep the
/// builtin path because their native forms diverge from R on NA/NaN or sign.
fn native_binop(op: &BinOp) -> Option<Op> {
    match op {
        BinOp::Add => Some(Op::Add),
        BinOp::Sub => Some(Op::Sub),
        BinOp::Mul => Some(Op::Mul),
        BinOp::Div => Some(Op::Div),
        _ => None,
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
