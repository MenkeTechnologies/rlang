//! Registered fusevm builtins, R's vectorized operators, and the primitive
//! function library.
//!
//! Every `Op::CallBuiltin(id, argc)` the compiler emits lands in a `b_*`
//! function here: it marshals values off the VM stack, calls into the
//! thread-local `RHost`, and pushes the result. Host borrows are taken in small
//! scopes — never across a call back into R — so `lapply` can run a closure body
//! on a nested VM while the outer builtin is still on the stack.

use crate::host::NameMap;
use crate::host::{
    call_value, fixed_decimals, ops, render_fixed, render_sci, sci_decimals, with_host,
    CombinatorKind, RData, RKind, Signal,
};
use crate::ast::{Arg, Expr};
use fusevm::{Value, VM};
use std::cell::RefCell;
use std::rc::Rc;

/// Register every rlang builtin on `vm`.
pub fn install(vm: &mut VM) {
    // Native `Op::Add/Sub/Mul/Div` (emitted for scalar `+ - * /`) compute two
    // unboxed numbers directly; when either operand is a boxed R value (vector,
    // NA, attributed), the VM delegates here — the same vector arithmetic the
    // `BINOP` builtin runs. This is also what puts the VM in strict numeric mode.
    vm.set_numeric_hook(std::sync::Arc::new(|op, a, b| {
        let name = match op {
            fusevm::NumOp::Add => "+",
            fusevm::NumOp::Sub => "-",
            fusevm::NumOp::Mul => "*",
            fusevm::NumOp::Div => "/",
            fusevm::NumOp::Mod => "%%",
            fusevm::NumOp::Pow => "^",
            fusevm::NumOp::Neg => "-",
            fusevm::NumOp::Lt => "<",
            fusevm::NumOp::Gt => ">",
            fusevm::NumOp::Le => "<=",
            fusevm::NumOp::Ge => ">=",
            fusevm::NumOp::Eq => "==",
            fusevm::NumOp::Ne => "!=",
        };
        binop(name, a, b)
    }));
    // R's `integer` is a C `int`, and `INT_MIN` is reserved as `NA_integer_`, so
    // the representable range is ±`INT_MAX`. rlang's native integers are fusevm
    // `Value::Int` (i64), so without this bound `2147483647L + 1L` computed
    // 2147483648 in the VM's own dispatch loop and never reached rlang, while R
    // answers `NA` with a warning. Narrowing the fixnum range makes a native
    // `+ - *` whose result leaves 32 bits delegate to the numeric hook above,
    // where `arith` applies R's overflow rule. The bounds check is two ALU ops
    // folded into fusevm's overflow accumulator (and is carried into JIT-compiled
    // code), so the in-range hot path keeps its native speed.
    vm.set_fixnum_range(-(i32::MAX as i64), i32::MAX as i64);
    vm.register_builtin(ops::GETVAR, b_getvar);
    vm.register_builtin(ops::GETFUN, b_getfun);
    vm.register_builtin(ops::SETVAR, b_setvar);
    vm.register_builtin(ops::SETSUPER, b_setsuper);
    vm.register_builtin(ops::MKARGS, b_mkargs);
    vm.register_builtin(ops::CALL, b_call);
    vm.register_builtin(ops::MKCLOSURE, b_mkclosure);
    vm.register_builtin(ops::CONST_DBL, b_const_dbl);
    vm.register_builtin(ops::CONST_INT, b_const_int);
    vm.register_builtin(ops::CONST_STR, b_const_str);
    vm.register_builtin(ops::CONST_LGL, b_const_lgl);
    vm.register_builtin(ops::CONST_NULL, b_const_null);
    vm.register_builtin(ops::CONST_NA, b_const_na);
    vm.register_builtin(ops::DOTS, b_dots);
    vm.register_builtin(ops::BINOP, b_binop);
    vm.register_builtin(ops::UNOP, b_unop);
    vm.register_builtin(ops::SPECIAL, b_special);
    vm.register_builtin(ops::INDEX, b_index);
    vm.register_builtin(ops::INDEX2, b_index2);
    vm.register_builtin(ops::DOLLAR, b_dollar);
    vm.register_builtin(ops::INDEX_SET, b_index_set);
    vm.register_builtin(ops::INDEX2_SET, b_index2_set);
    vm.register_builtin(ops::DOLLAR_SET, b_dollar_set);
    vm.register_builtin(ops::REPLACE, b_replace);
    vm.register_builtin(ops::TRUTHY, b_truthy);
    vm.register_builtin(ops::SEQ_LEN, b_seq_len);
    vm.register_builtin(ops::SEQ_ELEM, b_seq_elem);
    vm.register_builtin(ops::RANGE_FROM, b_range_from);
    vm.register_builtin(ops::RANGE_STEP, b_range_step);
    vm.register_builtin(ops::RANGE_LEN, b_range_len);
    vm.register_builtin(ops::AUTOPRINT, b_autoprint);
    vm.register_builtin(ops::IS_FALSE, b_is_false);
    vm.register_builtin(ops::IS_TRUE, b_is_true);
    vm.register_builtin(ops::MISSING, b_missing);
    vm.register_builtin(ops::NULL_INVISIBLE, b_null_invisible);
    vm.register_builtin(ops::SET_VISIBLE, b_set_visible);
    vm.register_builtin(ops::SWITCH_INDEX, b_switch_index);
    vm.register_builtin(ops::WARN_FLUSH, b_warn_flush);
    vm.register_builtin(ops::MARK_SHARED, b_mark_shared);
    vm.register_builtin(ops::OPEN_CALL, b_open_call);
    vm.register_builtin(ops::CALL_OPENED, b_call_opened);
}

// ── small host wrappers (each takes and releases the borrow) ────────────

fn as_dbl(v: &Value) -> Vec<Option<f64>> {
    with_host(|h| h.as_dbl(v))
}
fn as_int(v: &Value) -> Vec<Option<i64>> {
    with_host(|h| h.as_int(v))
}
fn as_lgl(v: &Value) -> Vec<Option<bool>> {
    with_host(|h| h.as_lgl(v))
}
fn as_str(v: &Value) -> Vec<Option<String>> {
    with_host(|h| h.as_str(v))
}
fn str1(v: &Value) -> Option<String> {
    with_host(|h| h.str1(v))
}
fn num1(v: &Value) -> Option<f64> {
    with_host(|h| h.num1(v))
}
fn lgl1(v: &Value) -> Option<bool> {
    with_host(|h| h.lgl1(v))
}
fn len(v: &Value) -> usize {
    with_host(|h| h.length(v))
}
fn is_null(v: &Value) -> bool {
    with_host(|h| h.is_null(v))
}
fn data(v: &Value) -> RData {
    with_host(|h| h.data_of(v))
}
/// Which `RData` variant `v` holds. Unlike [`data`], this does not copy the
/// payload, so a type test costs the same whatever the vector's length.
fn kind(v: &Value) -> RKind {
    with_host(|h| h.kind_of(v))
}
pub(crate) fn mk_dbl(xs: Vec<Option<f64>>) -> Value {
    with_host(|h| h.dbl(xs))
}
pub(crate) fn mk_int(xs: Vec<Option<i64>>) -> Value {
    with_host(|h| h.int(xs))
}
pub(crate) fn mk_lgl(xs: Vec<Option<bool>>) -> Value {
    with_host(|h| h.lgl(xs))
}
pub(crate) fn mk_str(xs: Vec<Option<String>>) -> Value {
    with_host(|h| h.str_vec(xs))
}
pub(crate) fn mk_list(xs: Vec<Value>) -> Value {
    with_host(|h| h.list(xs))
}
/// The R object an unevaluated expression *is*. R does not wrap everything in a
/// language object: `quote(1)` is the number, `quote(x)` is a name, and only a
/// compound expression is a call. `quote((x))` keeps its parentheses, because
/// R's parse tree keeps the `(` call.
pub(crate) fn mk_lang(e: Expr) -> Value {
    match e {
        Expr::Ident(n) => with_host(|h| h.alloc(RData::Sym(n))),
        Expr::Num(x) => scalar_dbl(x),
        Expr::Int(n) => scalar_int(n),
        Expr::Str(t) => scalar_str(t),
        Expr::Bool(b) => scalar_lgl(b),
        Expr::Null => null(),
        other => with_host(|h| h.alloc(RData::Lang(other))),
    }
}

/// The deparsed source of each argument in a call — what a builtin that names
/// the expression a caller wrote (`stopifnot`) reads out of its own call.
/// `None` when the source is not a call.
fn call_arg_sources(src: &str) -> Option<Vec<String>> {
    let mut exprs = crate::parser::parse(src).ok()?;
    if exprs.len() != 1 {
        return None;
    }
    let Expr::Call { args, .. } = exprs.remove(0) else {
        return None;
    };
    Some(
        args.iter()
            .map(|a| {
                a.value
                    .as_ref()
                    .map(crate::deparse::deparse_first_line)
                    .unwrap_or_default()
            })
            .collect(),
    )
}

/// Run `body` under the context R's own dispatch would have made: the same
/// call with its head replaced by the method that handles it. `seq(7, 5, by =
/// 3)` fails inside `seq.default(7, 5, by = 3)` in R, and names it.
fn in_method<T>(method: &str, body: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    let dispatched = with_host(|h| h.current_call_source()).and_then(|src| {
        let (_, rest) = src.split_once('(')?;
        Some(format!("{method}({rest}"))
    });
    let Some(call) = dispatched else {
        return body();
    };
    push_context(&call);
    let out = body();
    with_host(|h| {
        // The method's context comes off on the next line, so an error raised
        // under it has to claim the call here — as `call_op` does for a
        // primitive that failed by returning `Err`.
        if out.is_err() && h.error.is_none() {
            let c = h.current_call_source();
            h.set_error_call(c);
        }
        h.calls.pop();
    });
    out
}

/// The call text R uses when a function is invoked without a name to call it
/// by: the function's own deparse, parenthesised — `(function (a, b) `. Only
/// the first line, which is all a diagnostic shows and all `<Anonymous>` needs.
fn inline_call_text(f: &Value) -> String {
    match function_src(f).and_then(|s| s.into_iter().next()) {
        Some(head) => format!("({head}"),
        None => "FUN()".to_string(),
    }
}

/// The environment a value holds, for the builtins that take an `envir`.
fn env_of(v: &Value) -> Option<crate::host::Env> {
    match data(v) {
        RData::Environment(e) => Some(e),
        _ => None,
    }
}

/// Run `body` with `env` as the environment names resolve through — a frame
/// with no closure behind it, which is what R's `eval(expr, envir)` evaluates
/// in. The frame comes off however `body` ends.
fn in_env<T>(env: crate::host::Env, body: impl FnOnce() -> T) -> T {
    with_host(|h| {
        h.frames.push(crate::host::Frame {
            env,
            args: Vec::new(),
            fun_name: None,
            fun: None,
            dispatch: None,
            on_exit: Vec::new(),
        })
    });
    let out = body();
    with_host(|h| {
        h.frames.pop();
    });
    out
}

/// Run an expression in the environment the caller is standing in.
///
/// It is compiled the way any other source is — deparsed back out and put
/// through the front end — and run on its own VM, which resolves names through
/// the current frame because that is where `run_chunk` leaves them. The
/// closures it defines are *appended* to the table rather than replacing it:
/// the program that called `eval` is still running out of the same table.
fn eval_expr(e: &Expr) -> Result<Value, String> {
    let src = crate::deparse::deparse_lines(e);
    // Without slots: a top-level binding must land in the *environment*, since
    // that is what the caller reads afterwards and what an `envir` argument
    // pointed at. The REPL compiles this way for the same reason.
    let prog = crate::compile_no_slots(&src)?;
    let base = with_host(|h| h.closures.len());
    let shifted = crate::compiler::shift_closure_ids(prog, base);
    // A whole-program chunk echoes each statement's value; an `eval` is one
    // expression inside a running program, and its value belongs to whatever
    // asked for it. R's `eval` returns visibly but prints nothing of its own.
    let echo = with_host(|h| std::mem::replace(&mut h.echo, false));
    with_host(|h| h.closures.extend(shifted.closures));
    let out = crate::host::run_chunk(shifted.main);
    with_host(|h| h.echo = echo);
    out
}

/// What each name in the current *function* frame stands for, as R's
/// `substitute` sees it: a formal the caller supplied stands for the expression
/// they wrote, and anything else bound in the frame stands for its value.
///
/// `None` outside a closure — at top level R substitutes nothing, which is why
/// this is an absence rather than an empty table.
fn frame_substitutions() -> Option<Vec<(String, Expr)>> {
    let params = with_host(|h| h.current_formals())?;
    let src = with_host(|h| h.enclosing_source())?;
    let mut exprs = crate::parser::parse(&src).ok()?;
    if exprs.len() != 1 {
        return None;
    }
    let Expr::Call { args, .. } = exprs.remove(0) else {
        return None;
    };
    let tags: Vec<Option<String>> = args.iter().map(|a| a.name.clone()).collect();
    let to = crate::host::match_positions(&params, &tags).ok()?;
    let mut map: Vec<(String, Expr)> = Vec::new();
    for (ai, slot) in to.iter().enumerate() {
        let Some(pi) = slot else { continue };
        if let (Some(p), Some(v)) = (params.get(*pi), args[ai].value.clone()) {
            map.push((p.clone(), v));
        }
    }
    // What fell through to `...` stands for itself, in order, so
    // `substitute(list(...))` reads back as the arguments that were written.
    let dots: Vec<Arg> = args
        .iter()
        .zip(&to)
        .filter(|(_, slot)| slot.is_none())
        .map(|(a, _)| a.clone())
        .collect();
    if !dots.is_empty() {
        map.push((
            "...".to_string(),
            Expr::Call {
                fun: Box::new(Expr::Ident("...".into())),
                args: dots,
            },
        ));
    }
    Some(map)
}

/// A value written back as the expression that would produce it, for the
/// symbols `substitute` replaces by value rather than by expression.
fn value_as_expr(v: &Value) -> Option<Expr> {
    let mut exprs = crate::parser::parse(&deparse_value(v)).ok()?;
    match exprs.len() {
        1 => Some(exprs.remove(0)),
        _ => None,
    }
}

/// Walk `e` replacing every symbol the table accounts for. `by_value` also
/// resolves a symbol the table misses but the current frame binds — R's rule
/// for a local, which stands for its value. A symbol neither accounts for is
/// left as it was written.
fn substitute_in(e: &Expr, map: &[(String, Expr)], by_value: bool) -> Expr {
    let sub = |x: &Expr| substitute_in(x, map, by_value);
    let boxed = |x: &Expr| Box::new(substitute_in(x, map, by_value));
    match e {
        Expr::Ident(n) => {
            if let Some((_, r)) = map.iter().find(|(k, _)| k == n) {
                return r.clone();
            }
            if by_value {
                if let Some(v) = with_host(|h| h.lookup(n)) {
                    if let Some(x) = value_as_expr(&v) {
                        return x;
                    }
                }
            }
            e.clone()
        }
        Expr::Call { fun, args } => Expr::Call {
            fun: boxed(fun),
            args: substitute_args(args, map, by_value),
        },
        Expr::Binary { op, lhs, rhs } => Expr::Binary {
            op: op.clone(),
            lhs: boxed(lhs),
            rhs: boxed(rhs),
        },
        Expr::Special { name, lhs, rhs } => Expr::Special {
            name: name.clone(),
            lhs: boxed(lhs),
            rhs: boxed(rhs),
        },
        Expr::Unary { op, operand } => Expr::Unary {
            op: op.clone(),
            operand: boxed(operand),
        },
        Expr::Paren(x) => Expr::Paren(boxed(x)),
        Expr::Block(xs) => Expr::Block(xs.iter().map(sub).collect()),
        Expr::If { cond, then, els } => Expr::If {
            cond: boxed(cond),
            then: boxed(then),
            els: els.as_ref().map(|x| boxed(x)),
        },
        Expr::Index { kind, obj, args } => Expr::Index {
            kind: *kind,
            obj: boxed(obj),
            args: substitute_args(args, map, by_value),
        },
        other => other.clone(),
    }
}

/// An argument list with `...` spliced back into the arguments it stands for.
fn substitute_args(args: &[Arg], map: &[(String, Expr)], by_value: bool) -> Vec<Arg> {
    let mut out = Vec::with_capacity(args.len());
    for a in args {
        match &a.value {
            Some(Expr::Dots) => match map.iter().find(|(k, _)| k == "...") {
                Some((_, Expr::Call { args: dots, .. })) => out.extend(dots.iter().cloned()),
                _ => out.push(a.clone()),
            },
            Some(v) => out.push(Arg {
                name: a.name.clone(),
                value: Some(substitute_in(v, map, by_value)),
            }),
            None => out.push(a.clone()),
        }
    }
    out
}

/// Parse one R expression and hand it back as a language object.
fn parse_one(src: &str) -> Result<Value, String> {
    let mut exprs = crate::parser::parse(src)?;
    match exprs.len() {
        1 => Ok(mk_lang(exprs.remove(0))),
        _ => Err(format!("not a single expression: {src}")),
    }
}

/// `src` re-written the way `match.call()` shows it: every argument that binds
/// to a named formal carries that formal's name, and the arguments come back in
/// formal order. Arguments that went to `...` keep their own tag (or none) and
/// stay in the order written, after the named ones — which is where R puts
/// them.
///
/// `None` when the source is not a call at all, which leaves the caller to hand
/// back the expression unchanged.
fn matched_call(src: &str, params: &[String]) -> Result<Option<Expr>, String> {
    let mut exprs = crate::parser::parse(src)?;
    if exprs.len() != 1 {
        return Ok(None);
    }
    let Expr::Call { fun, args } = exprs.remove(0) else {
        return Ok(None);
    };
    let tags: Vec<Option<String>> = args.iter().map(|a| a.name.clone()).collect();
    let to = crate::host::match_positions(params, &tags)?;
    // Formal order first: walk the formals and emit whichever argument matched.
    let mut out: Vec<Arg> = Vec::with_capacity(args.len());
    for (pi, p) in params.iter().enumerate() {
        if p == "..." {
            continue;
        }
        if let Some(ai) = to.iter().position(|slot| *slot == Some(pi)) {
            out.push(Arg {
                name: Some(p.clone()),
                value: args[ai].value.clone(),
            });
        }
    }
    // …then the ones that fell through to `...`, in the order written.
    for (ai, arg) in args.iter().enumerate() {
        if to[ai].is_none() {
            out.push(arg.clone());
        }
    }
    Ok(Some(Expr::Call { fun, args: out }))
}

/// How many parts [`lang_parts`] would produce, counted without building any of
/// them. `length()` runs with the host already borrowed, so it cannot allocate
/// the parts to count them.
pub(crate) fn lang_len(e: &Expr) -> usize {
    match e {
        Expr::Call { args, .. } => 1 + args.len(),
        Expr::Binary { .. } | Expr::Special { .. } | Expr::Assign { .. } => 3,
        Expr::Unary { .. } | Expr::Paren(_) | Expr::Repeat(_) => 2,
        Expr::Block(xs) => 1 + xs.len(),
        Expr::If { els, .. } => 3 + usize::from(els.is_some()),
        Expr::While { .. } => 3,
        Expr::For { .. } => 4,
        Expr::Index { args, .. } => 2 + args.len(),
        _ => 1,
    }
}

/// A call taken apart the way R's `as.list` does: the function or operator
/// first, then one element per argument, keeping the tag a named argument was
/// written with. Every part is itself a language object, a name or a constant.
///
/// R decomposes every compound form this way, control flow included, because in
/// R they *are* calls — `quote(if (a) b)` is a call to `if`. The forms rlang's
/// tree keeps as their own nodes are unfolded back into that shape here.
pub(crate) fn lang_parts(e: &Expr) -> Vec<(Option<String>, Value)> {
    use crate::ast::{IndexKind, UnOp};
    let sym = |n: &str| with_host(|h| h.alloc(RData::Sym(n.to_string())));
    let part = |x: &Expr| (None, mk_lang(x.clone()));
    match e {
        Expr::Call { fun, args } => {
            let mut out = vec![(None, mk_lang((**fun).clone()))];
            for arg in args {
                let v = match &arg.value {
                    Some(x) => mk_lang(x.clone()),
                    None => Value::Undef,
                };
                out.push((arg.name.clone(), v));
            }
            out
        }
        Expr::Binary { op, lhs, rhs } => vec![
            (None, sym(crate::deparse::binop_symbol(op))),
            part(lhs),
            part(rhs),
        ],
        Expr::Special { name, lhs, rhs } => {
            // The lexer strips the `%`s; `%%` and `%/%` lex to "" and "/".
            let spelled = match name.as_str() {
                "" => "%%".to_string(),
                "/" => "%/%".to_string(),
                other => format!("%{other}%"),
            };
            vec![(None, sym(&spelled)), part(lhs), part(rhs)]
        }
        Expr::Unary { op, operand } => {
            let n = match op {
                UnOp::Neg => "-",
                UnOp::Plus => "+",
                UnOp::Not => "!",
            };
            vec![(None, sym(n)), part(operand)]
        }
        Expr::Paren(x) => vec![(None, sym("(")), part(x)],
        Expr::Block(xs) => {
            let mut out = vec![(None, sym("{"))];
            out.extend(xs.iter().map(part));
            out
        }
        Expr::If { cond, then, els } => {
            let mut out = vec![(None, sym("if")), part(cond), part(then)];
            if let Some(e) = els {
                out.push(part(e));
            }
            out
        }
        Expr::While { cond, body } => vec![(None, sym("while")), part(cond), part(body)],
        Expr::Repeat(b) => vec![(None, sym("repeat")), part(b)],
        Expr::For { var, seq, body } => vec![
            (None, sym("for")),
            (None, sym(var)),
            part(seq),
            part(body),
        ],
        Expr::Assign {
            target,
            value,
            super_assign,
        } => vec![
            (None, sym(if *super_assign { "<<-" } else { "<-" })),
            part(target),
            part(value),
        ],
        Expr::Index { kind, obj, args } => {
            let head = match kind {
                IndexKind::Single => "[",
                IndexKind::Double => "[[",
                IndexKind::Dollar => "$",
                IndexKind::At => "@",
            };
            let mut out = vec![(None, sym(head)), part(obj)];
            for arg in args {
                let v = match &arg.value {
                    // `x$name` holds the name as a string in the tree; R keeps
                    // it as a symbol.
                    Some(Expr::Str(n))
                        if matches!(kind, IndexKind::Dollar | IndexKind::At) =>
                    {
                        sym(n)
                    }
                    Some(x) => mk_lang(x.clone()),
                    None => Value::Undef,
                };
                out.push((arg.name.clone(), v));
            }
            out
        }
        // Anything left is a leaf as far as taking it apart goes.
        other => vec![(None, mk_lang(other.clone()))],
    }
}

/// A language object as the character vector `as.character` gives: a name is
/// its own text, and a call is one element per part, each deparsed.
fn lang_text(e: &Expr) -> Vec<String> {
    match e {
        Expr::Ident(n) => vec![n.clone()],
        other => lang_parts(other)
            .into_iter()
            .map(|(_, v)| deparse_value(&v))
            .collect(),
    }
}

/// The expression behind a language object, for a caller that needs the tree
/// rather than the value: `Some` for a call or a name, and for a constant the
/// literal it came from, so `as.list(quote(f(1)))` can take its elements apart.
pub(crate) fn lang_of(v: &Value) -> Option<Expr> {
    match data(v) {
        RData::Lang(e) => Some(e),
        RData::Sym(n) => Some(Expr::Ident(n)),
        _ => None,
    }
}
fn scalar_dbl(x: f64) -> Value {
    with_host(|h| h.scalar_dbl(x))
}
fn scalar_int(x: i64) -> Value {
    with_host(|h| h.scalar_int(x))
}
fn scalar_lgl(x: bool) -> Value {
    with_host(|h| h.scalar_lgl(x))
}
fn scalar_str(x: impl Into<String>) -> Value {
    with_host(|h| h.scalar_str(x))
}
pub(crate) fn null() -> Value {
    with_host(|h| h.null())
}
pub(crate) fn names_of(v: &Value) -> Vec<Option<String>> {
    with_host(|h| h.names(v))
}
fn class_of(v: &Value) -> Vec<String> {
    with_host(|h| h.class_of(v))
}
fn elements(v: &Value) -> Vec<Value> {
    with_host(|h| h.elements(v))
}
/// Whether `v` carries the `factor` class — the one integer vector R's type
/// predicates refuse to call numeric.
fn is_factor(v: &Value) -> bool {
    class_of(v).iter().any(|c| c == "factor")
}
/// Whether `v` is an *ordered* factor, whose levels carry a `<` order. R keeps
/// the two apart with separate group generics: `Ops.ordered` gives `<`/`>`
/// meaning, `Ops.factor` refuses them.
fn is_ordered(v: &Value) -> bool {
    class_of(v).iter().any(|c| c == "ordered")
}
/// A factor's level labels, in code order.
fn levels_of(v: &Value) -> Vec<String> {
    with_host(|h| h.attr(v, "levels"))
        .map(|l| as_str(&l).into_iter().flatten().collect())
        .unwrap_or_default()
}
/// A factor's elements as their labels — `as.character(f)`. An `NA` or
/// out-of-range code stays `NA`.
fn factor_labels(v: &Value) -> Vec<Option<String>> {
    let levels = levels_of(v);
    as_int(v)
        .iter()
        .map(|c| c.and_then(|i| levels.get((i - 1) as usize).cloned()))
        .collect()
}
/// Character coercion the way R's factor methods do it: a factor contributes
/// its *labels*, anything else its ordinary `as.character`. This is the
/// coercion behind `paste`, `toString`, `match`, `%in%`, `split`, `tapply` and
/// `as.vector` — every one of which would otherwise see the integer codes.
fn as_str_labels(v: &Value) -> Vec<Option<String>> {
    if is_factor(v) {
        factor_labels(v)
    } else {
        as_str(v)
    }
}
/// Build a factor from 1-based `codes` into `levels`. The one constructor
/// `factor`, `droplevels`, `cut` and the factor-preserving primitives share.
fn mk_factor(codes: Vec<Option<i64>>, levels: Vec<String>, ordered: bool) -> Value {
    let out = mk_int(codes);
    let lv = mk_str(levels.into_iter().map(Some).collect());
    // An ordered factor carries `c("ordered", "factor")`.
    let cls = if ordered {
        mk_str(vec![Some("ordered".into()), Some("factor".into())])
    } else {
        scalar_str("factor")
    };
    with_host(|h| {
        h.set_attr(&out, "levels", lv);
        h.set_attr(&out, "class", cls);
    });
    out
}
/// Restore `levels` and `class` from the factor `src` onto `out`, which holds
/// the selected codes.
///
/// This is R's `[.factor`: `NextMethod("[")` subsets the codes like any integer
/// vector, then the level table and the class are put back. `rep.factor` does
/// the same via `structure(y, class = class(x), levels = levels(x))`, and
/// `sort.default`/`rev.default`/`head`/`tail` inherit it by going through `[`.
fn carry_factor(out: &Value, src: &Value) {
    if !is_factor(src) {
        return;
    }
    with_host(|h| {
        if let Some(l) = h.attr(src, "levels") {
            h.set_attr(out, "levels", l);
        }
        if let Some(c) = h.attr(src, "class") {
            h.set_attr(out, "class", c);
        }
    });
}
/// `factor(f, exclude = NA)` — rebuild a factor keeping only the levels that
/// are actually used, in their original order. This is `droplevels`, and what
/// `f[i, drop = TRUE]` applies to its result.
fn drop_unused_levels(v: &Value) -> Value {
    let levels = levels_of(v);
    let labels = factor_labels(v);
    let kept: Vec<String> = levels
        .into_iter()
        .filter(|l| labels.iter().any(|s| s.as_deref() == Some(l.as_str())))
        .collect();
    let codes = labels
        .iter()
        .map(|s| {
            s.as_ref()
                .and_then(|s| kept.iter().position(|l| l == s))
                .map(|p| p as i64 + 1)
        })
        .collect();
    mk_factor(codes, kept, is_ordered(v))
}
/// The element of a named list/vector bound to `name`, if present (`x$name`).
fn element_field(v: &Value, name: &str) -> Option<Value> {
    let names = names_of(v);
    let i = names.iter().position(|n| n.as_deref() == Some(name))?;
    elements(v).into_iter().nth(i)
}
fn element_at(v: &Value, i: usize) -> Value {
    with_host(|h| h.element_at(v, i))
}
pub(crate) fn set_names(v: &Value, names: Vec<Option<String>>) {
    if names.iter().all(|n| n.is_none()) {
        with_host(|h| {
            let nl = h.null();
            h.set_attr(v, "names", nl)
        });
        return;
    }
    let nv = mk_str(names);
    with_host(|h| h.set_attr(v, "names", nv));
}

/// Marshal a length-1 R vector to a fusevm scalar for a `.Call` FFI invocation.
/// fusevm's v1 FFI ABI takes `i64` / `f64` / string scalars, so integer and
/// logical vectors map to `Int`, doubles to `Float`, and character to `Str`.
fn r_to_fusevm(v: &Value) -> Result<Value, String> {
    match data(v) {
        RData::Str(_) => str1(v)
            .map(Value::str)
            .ok_or_else(|| "`.Call`: NA string argument".to_string()),
        RData::Int(_) | RData::Lgl(_) => as_int(v)
            .first()
            .copied()
            .flatten()
            .map(Value::Int)
            .ok_or_else(|| "`.Call`: NA integer argument".to_string()),
        RData::Dbl(_) => num1(v)
            .map(Value::Float)
            .ok_or_else(|| "`.Call`: NA numeric argument".to_string()),
        _ => Err("`.Call` arguments must be length-1 numeric, integer, or character".to_string()),
    }
}

/// Convert the fusevm scalar an FFI export returned back into a length-1 R
/// vector (the inverse of [`r_to_fusevm`]).
fn fusevm_to_r(v: Value) -> Value {
    match v {
        Value::Int(n) => scalar_int(n),
        Value::Float(f) => scalar_dbl(f),
        Value::Bool(b) => scalar_lgl(b),
        Value::Str(s) => scalar_str(s.to_string()),
        _ => null(),
    }
}

/// The string payload of a compiler-emitted constant.
fn name_of(v: &Value) -> String {
    match v {
        Value::Str(s) => s.to_string(),
        other => with_host(|h| h.str1(other)).unwrap_or_default(),
    }
}

fn pop_n(vm: &mut VM, n: usize) -> Vec<Value> {
    let mut out = vec![Value::Undef; n];
    for slot in out.iter_mut().rev() {
        *slot = vm.pop();
    }
    out
}

/// Record an R error and stop this chunk.
fn abort(vm: &mut VM, msg: String) -> Value {
    with_host(|h| {
        if h.error.is_none() {
            let call = h.current_call_source();
            h.set_error_call(call);
            h.error = Some(msg);
        }
    });
    vm.ip = vm.chunk.ops.len();
    Value::Undef
}

/// Stop this chunk if a control signal (`break`/`next`/`return`) is pending.
fn propagate(vm: &mut VM, v: Value) -> Value {
    let pending = with_host(|h| h.signal.is_some() || h.error.is_some());
    if pending {
        vm.ip = vm.chunk.ops.len();
    }
    v
}

// ── variables, calls, constants ─────────────────────────────────────────

fn b_getvar(vm: &mut VM, _: u8) -> Value {
    let name = name_of(&vm.pop());
    // Evaluating a symbol makes its value visible (R sets `R_Visible = TRUE`).
    // Without this a function whose body is a bare symbol stayed invisible after
    // a default-argument prologue ran `p <- <default>` (an invisible assignment),
    // so `function(x = 3) x` printed nothing.
    with_host(|h| h.visible = true);
    match with_host(|h| h.lookup(&name)) {
        Some(v) => v,
        None => match primitive_value(&name) {
            Some(v) => v,
            // A bare name that is a function in a loaded CRAN package (used as a
            // value, e.g. `sapply(x, digest)`) resolves to a delegating builtin;
            // a genuine unknown is still "object not found".
            None if cran_has_function(&name) => with_host(|h| h.alloc(RData::Builtin(name))),
            None => abort(vm, format!("object '{name}' not found")),
        },
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn cran_has_function(name: &str) -> bool {
    crate::rembed::has_function(name)
}
#[cfg(target_arch = "wasm32")]
fn cran_has_function(_: &str) -> bool {
    false
}

fn b_getfun(vm: &mut VM, _: u8) -> Value {
    let name = name_of(&vm.pop());
    match with_host(|h| h.lookup_function(&name)) {
        Some(v) => v,
        // An unknown *function* resolves to a builtin regardless: at call time
        // the CRAN bridge delegates it to embedded R (a base function rlang
        // doesn't implement, or a loaded package's routine), reproducing the
        // "could not find function" error if R lacks it too. A bare *variable*
        // (`b_getvar`) is unaffected — it still errors as "object not found".
        None => primitive_value(&name)
            .unwrap_or_else(|| with_host(|h| h.alloc(RData::Builtin(name.clone())))),
    }
}

/// A primitive as a first-class value, so `sapply(x, sqrt)` works.
fn primitive_value(name: &str) -> Option<Value> {
    if let Some(v) = base_constant(name) {
        return Some(v);
    }
    is_primitive(name).then(|| with_host(|h| h.alloc(RData::Builtin(name.to_string()))))
}

/// R's built-in constants, bound in the base environment: `pi`, the letter and
/// month name vectors. `T`/`F` are handled as literals by the lexer.
fn base_constant(name: &str) -> Option<Value> {
    let letters = |upper: bool| {
        mk_str(
            (b'a'..=b'z')
                .map(|c| Some(((if upper { c - 32 } else { c }) as char).to_string()))
                .collect(),
        )
    };
    match name {
        "pi" => Some(scalar_dbl(std::f64::consts::PI)),
        "T" => Some(scalar_lgl(true)),
        "F" => Some(scalar_lgl(false)),
        ".Machine" => {
            // The handful of `.Machine` fields R programs actually read.
            let vals = vec![
                scalar_int(i32::MAX as i64),
                scalar_dbl(f64::EPSILON),
                scalar_dbl(f64::MIN_POSITIVE),
                scalar_dbl(f64::MAX),
            ];
            let out = mk_list(vals);
            set_names(
                &out,
                ["integer.max", "double.eps", "double.xmin", "double.xmax"]
                    .into_iter()
                    .map(|s| Some(s.to_string()))
                    .collect(),
            );
            Some(out)
        }
        "LETTERS" => Some(letters(true)),
        "letters" => Some(letters(false)),
        "month.name" => Some(mk_str(
            [
                "January",
                "February",
                "March",
                "April",
                "May",
                "June",
                "July",
                "August",
                "September",
                "October",
                "November",
                "December",
            ]
            .into_iter()
            .map(|m| Some(m.to_string()))
            .collect(),
        )),
        "month.abb" => Some(mk_str(
            [
                "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
            ]
            .into_iter()
            .map(|m| Some(m.to_string()))
            .collect(),
        )),
        _ => None,
    }
}

fn b_setvar(vm: &mut VM, _: u8) -> Value {
    let val = vm.pop();
    let name = name_of(&vm.pop());
    with_host(|h| {
        h.set_var(&name, val.clone());
        // Assignment returns its value invisibly.
        h.visible = false;
    });
    val
}

fn b_setsuper(vm: &mut VM, _: u8) -> Value {
    let val = vm.pop();
    let name = name_of(&vm.pop());
    with_host(|h| {
        h.set_super(&name, val.clone());
        h.visible = false;
    });
    val
}

/// Build an argument list; an untagged `...` argument splices in place.
fn b_mkargs(vm: &mut VM, argc: u8) -> Value {
    let flat = pop_n(vm, argc as usize);
    let mut out: Vec<(Option<String>, Value)> = Vec::with_capacity(flat.len() / 2);
    for pair in flat.chunks(2) {
        let tag = match &pair[0] {
            Value::Undef => None,
            other => Some(name_of(other)),
        };
        let val = pair[1].clone();
        match (&tag, data(&val)) {
            (None, RData::Args(inner)) => out.extend(inner),
            _ => out.push((tag, val)),
        }
    }
    with_host(|h| h.alloc(RData::Args(out)))
}

/// Open a call context before the arguments are evaluated, for a callee R
/// implements as a closure. The deparsed call is returned unchanged so it stays
/// where the compiler put it — under the callee, as `CALL_OPENED`'s operand.
fn b_open_call(vm: &mut VM, _: u8) -> Value {
    // Emitted only where R makes a function context, so the flag is settled
    // here rather than waiting for the callee to resolve.
    let text = vm.peek().clone();
    with_host(|h| {
        h.calls.push(crate::host::CallCtx {
            text,
            closure: true,
            // Open, not entered: what runs next is the argument list.
            entered: false,
        })
    });
    vm.pop()
}

fn b_call(vm: &mut VM, _: u8) -> Value {
    call_op(vm, false)
}

/// `CALL` for a context [`b_open_call`] already opened.
fn b_call_opened(vm: &mut VM, _: u8) -> Value {
    call_op(vm, true)
}

fn call_op(vm: &mut VM, opened: bool) -> Value {
    let argv = vm.pop();
    let f = vm.pop();
    // The deparsed call, pushed under the callee by the compiler. It stands on
    // the host's context stack for the duration of the call so a condition
    // raised inside can report it, and comes off however the call ends.
    let call = vm.pop();
    let args = match data(&argv) {
        RData::Args(a) => a,
        _ => Vec::new(),
    };
    let name = match data(&f) {
        RData::Builtin(n) => Some(n),
        _ => None,
    };
    let closure = matches!(data(&f), RData::Closure { .. });
    // Most calls are visible by default (R sets `R_Visible = TRUE` on entry).
    // The `suppress*` wrappers are visibility-transparent: they return their
    // argument with its visibility intact. Because rlang evaluates arguments
    // eagerly, that visibility is already in `h.visible` here, so skipping the
    // reset lets an invisible argument (`suppressMessages(library(x))`) stay
    // invisible instead of auto-printing NULL.
    let transparent = matches!(
        name.as_deref(),
        Some("suppressMessages" | "suppressWarnings" | "suppressPackageStartupMessages")
    );
    if !transparent {
        with_host(|h| h.visible = true);
    }
    // With `OPEN_CALL` the context is already open, and already flagged: the
    // compiler emits it exactly where R makes one. Without it the callee is one
    // R implements as a primitive and makes no context for — unless the name
    // has been rebound to a closure, which does. Either way the call is being
    // *entered* now: its arguments are evaluated.
    with_host(|h| match opened {
        true => {
            if let Some(c) = h.calls.last_mut() {
                c.entered = true;
            }
        }
        false => h.calls.push(crate::host::CallCtx {
            text: call,
            closure,
            entered: true,
        }),
    });
    let out = call_value(&f, args, name);
    with_host(|h| {
        // A primitive that failed by returning `Err` has recorded no call, and
        // the one R would name is this call — which comes off the stack on the
        // next line, before `abort` below could read it.
        if out.is_err() && h.error.is_none() {
            let call = h.current_call_source();
            h.set_error_call(call);
        }
        h.calls.pop();
    });
    match out {
        Ok(v) => propagate(vm, v),
        Err(e) => abort(vm, e),
    }
}

fn b_mkclosure(vm: &mut VM, _: u8) -> Value {
    let id = match vm.pop() {
        Value::Int(i) => i as usize,
        _ => 0,
    };
    with_host(|h| {
        let env = h.env();
        h.alloc(RData::Closure { id, env })
    })
}

fn b_const_dbl(vm: &mut VM, _: u8) -> Value {
    let x = match vm.pop() {
        Value::Float(f) => f,
        Value::Int(i) => i as f64,
        _ => f64::NAN,
    };
    scalar_dbl(x)
}

fn b_const_int(vm: &mut VM, _: u8) -> Value {
    let x = match vm.pop() {
        Value::Int(i) => i,
        Value::Float(f) => f as i64,
        _ => 0,
    };
    scalar_int(x)
}

fn b_const_str(vm: &mut VM, _: u8) -> Value {
    let s = name_of(&vm.pop());
    scalar_str(s)
}

fn b_const_lgl(vm: &mut VM, _: u8) -> Value {
    let b = matches!(vm.pop(), Value::Bool(true));
    scalar_lgl(b)
}

fn b_const_null(_: &mut VM, _: u8) -> Value {
    null()
}

fn b_null_invisible(_: &mut VM, _: u8) -> Value {
    with_host(|h| h.visible = false);
    null()
}

/// `(x)` — the value unchanged, but visible. Native scalars pass through
/// untouched so a parenthesized expression stays on the unboxed path.
fn b_set_visible(vm: &mut VM, _: u8) -> Value {
    let v = vm.pop();
    with_host(|h| h.visible = true);
    v
}

/// `switch` dispatch: the stack holds `EXPR` then each branch name (as string
/// constants). Returns the 0-based index of the branch to run, or -1. A
/// character `EXPR` matches a branch name, else falls to the first unnamed
/// (default) branch; a numeric `EXPR` selects by 1-based position. Fall-through
/// for empty branches is resolved by the compiled jump table, not here.
fn b_switch_index(vm: &mut VM, argc: u8) -> Value {
    let all = pop_n(vm, argc as usize);
    let expr = all.first().cloned().unwrap_or(Value::Undef);
    let names: Vec<String> = all.iter().skip(1).map(name_of).collect();
    let expr_v = expr; // already an R value
    let idx = if kind(&expr_v) == RKind::Str {
        match str1(&expr_v) {
            Some(s) => names
                .iter()
                .position(|n| *n == s)
                .or_else(|| names.iter().position(|n| n.is_empty()))
                .map(|p| p as i64)
                .unwrap_or(-1),
            None => -1,
        }
    } else {
        match num1(&expr_v) {
            Some(n) => {
                let n = n as i64;
                if n >= 1 && n <= names.len() as i64 {
                    n - 1
                } else {
                    -1
                }
            }
            None => -1,
        }
    };
    Value::Int(idx)
}

fn b_const_na(vm: &mut VM, _: u8) -> Value {
    match vm.pop() {
        Value::Int(1) => mk_int(vec![None]),
        Value::Int(2) => mk_dbl(vec![None]),
        Value::Int(3) => mk_str(vec![None]),
        _ => mk_lgl(vec![None]),
    }
}

fn b_dots(_: &mut VM, _: u8) -> Value {
    let d = with_host(|h| h.dots());
    with_host(|h| h.alloc(RData::Args(d)))
}

fn b_missing(vm: &mut VM, _: u8) -> Value {
    let name = name_of(&vm.pop());
    let bound = with_host(|h| h.env().borrow().vars.contains_key(&name));
    Value::Bool(!bound)
}

// ── operators ───────────────────────────────────────────────────────────

fn b_binop(vm: &mut VM, _: u8) -> Value {
    let op = name_of(&vm.pop());
    let rhs = vm.pop();
    let lhs = vm.pop();
    match binop(&op, &lhs, &rhs) {
        Ok(v) => v,
        Err(e) => abort(vm, e),
    }
}

fn b_unop(vm: &mut VM, _: u8) -> Value {
    let op = name_of(&vm.pop());
    let x = vm.pop();
    // `-f` / `!f` reach `Ops.factor` with one argument, so they warn and give NA
    // rather than negating the codes.
    if let Some(r) = ops_factor(&op, &x, None) {
        return match r {
            Ok(v) => propagate(vm, v),
            Err(e) => abort(vm, e),
        };
    }
    // `-x` and `!x` are element-wise, so they keep the operand's names/dim/
    // dimnames the same way the binary operators do via `carry_attrs`.
    let out = match op.as_str() {
        "-" => match data(&x) {
            RData::Int(v) => mk_int(v.iter().map(|e| e.map(|n| -n)).collect()),
            _ => mk_dbl(as_dbl(&x).iter().map(|e| e.map(|n| -n)).collect()),
        },
        "+" => return x,
        "!" => mk_lgl(as_lgl(&x).iter().map(|e| e.map(|b| !b)).collect()),
        other => return abort(vm, format!("invalid unary operator '{other}'")),
    };
    carry_attrs(&out, &x, &x);
    out
}

fn b_special(vm: &mut VM, _: u8) -> Value {
    let name = name_of(&vm.pop());
    let rhs = vm.pop();
    let lhs = vm.pop();
    let out = match name.as_str() {
        // `%%` and `%/%` lex to the empty and "/" names.
        "" => binop("%%", &lhs, &rhs),
        "/" => binop("%/%", &lhs, &rhs),
        "in" => Ok(value_in(&lhs, &rhs)),
        other => {
            // A user-defined infix operator is an ordinary function named
            // `%other%`.
            let fname = format!("%{other}%");
            match with_host(|h| h.lookup_function(&fname)).or_else(|| primitive_value(&fname)) {
                Some(f) => call_value(&f, vec![(None, lhs), (None, rhs)], Some(fname)),
                None => Err(format!("could not find function \"{fname}\"")),
            }
        }
    };
    match out {
        Ok(v) => propagate(vm, v),
        Err(e) => abort(vm, e),
    }
}

/// `x %in% table`.
fn value_in(x: &Value, table: &Value) -> Value {
    let hay: Vec<Option<String>> = as_str_labels(table);
    let out = as_str_labels(x)
        .into_iter()
        .map(|e| Some(hay.contains(&e)))
        .collect();
    mk_lgl(out)
}

/// R's binary operators, vectorized with recycling and NA propagation.
/// Emit a warning the way R's top-level handler does: the banner, the message,
/// and — for a warning carrying no call — a trailing space before the newline.
fn r_warning(msg: &str, call: Option<String>) {
    crate::host::queue_warning(msg, call);
}

/// `warning(msg)`'s own default action. R's `do_warning` calls `findCall`,
/// which walks past `warning()`'s own context — a builtin makes none — to the
/// enclosing closure call, so `f <- function(x) warning("w"); f(1)` reports
/// `In f(1) : w` and a top-level `warning("w")` reports no call at all.
fn user_warning(msg: &str) {
    let call = with_host(|h| h.enclosing_call());
    crate::host::queue_warning(msg, call);
}

/// R's `CoercionWarning`, which raises its warning with plain `warning()`
/// rather than `warningcall`: with no call of its own it lands on the innermost
/// R *context*, and a builtin makes none, so the call reported is the caller's.
fn coercion_warning(msg: &str) -> Result<(), String> {
    signal_warning_in(msg, with_host(|h| h.context_call()))
}

/// A warning raised from an arithmetic or comparison operator. R names that
/// call — `In 1:3 + 1:2 : longer object length …` — which rlang cannot: `+ - *
/// /` lower to native fusevm ops carrying no call text, and pushing one on
/// every arithmetic op would cost the hot path the whole design exists to keep
/// native. Reported with no call, which is the shape R uses when there is none,
/// rather than with the enclosing call, which would name the wrong one.
fn binop_warning(msg: &str) -> Result<(), String> {
    signal_warning_in(msg, None)
}

/// Call `f` from inside an apply-family primitive under the context R's own
/// implementation makes. R builds that call itself — `do_lapply` evaluates
/// `FUN(X[[i]], ...)`, `apply` evaluates `FUN(newX[, i], ...)`, `Reduce`
/// evaluates `f(init, x[[i]])` — so a condition raised inside reports it rather
/// than the `lapply(…)` the user wrote. A primitive `f` makes no context in R
/// either, and then the apply function's own call stands, so nothing is pushed.
fn call_fun(f: &Value, args: Vec<(Option<String>, Value)>, call: &str) -> Result<Value, String> {
    if !matches!(data(f), RData::Closure { .. }) {
        return call_value(f, args, None);
    }
    push_context(call);
    let out = call_value(f, args, None);
    with_host(|h| {
        h.calls.pop();
    });
    out
}

/// Open a call context for R code rlang implements in Rust — the calls R's own
/// definition would have made, which is what a condition raised inside reports.
fn push_context(call: &str) {
    let text = Value::str(call);
    with_host(|h| {
        h.calls.push(crate::host::CallCtx {
            text,
            closure: true,
            entered: true,
        })
    });
}

/// The context R's `lapply`-family C code evaluates `FUN` under.
const FUN_CALL: &str = "FUN(X[[i]], ...)";

/// End of a top-level statement: print whatever it queued. R holds warnings
/// under the default `options(warn = 0)` until the statement finishes, so
/// `print(1:3 + 1:2)` shows its value first and the recycling warning after.
fn b_warn_flush(_: &mut VM, _: u8) -> Value {
    crate::host::flush_warnings(false);
    null()
}

/// R's "NAs introduced by coercion": warn when converting a *character* vector
/// turned a non-NA element into NA. Only text can fail to parse, so a numeric or
/// logical source never warns however many NAs it already held.
fn warn_coerced_na(x: &Value, produced_na: impl Iterator<Item = bool>) -> Result<(), String> {
    if kind(x) != RKind::Str {
        return Ok(());
    }
    let src = as_str(x);
    if produced_na
        .enumerate()
        .any(|(i, na)| na && src.get(i).is_some_and(|e| e.is_some()))
    {
        coercion_warning("NAs introduced by coercion")?;
    }
    Ok(())
}

/// Narrow an R value to something the options store can hold. Options are
/// scalars in every case rlang acts on, so a longer vector keeps only its first
/// element — the same truncation R's own option setters apply.
fn r_to_option_value(v: &Value) -> Result<crate::host::OptionValue, String> {
    use crate::host::OptionValue as O;
    match data(v) {
        RData::Int(xs) => xs
            .first()
            .copied()
            .flatten()
            .map(O::Int)
            .ok_or_else(|| "invalid value in 'options'".to_string()),
        RData::Dbl(xs) => xs
            .first()
            .copied()
            .flatten()
            .map(O::Dbl)
            .ok_or_else(|| "invalid value in 'options'".to_string()),
        RData::Lgl(xs) => xs
            .first()
            .copied()
            .flatten()
            .map(O::Lgl)
            .ok_or_else(|| "invalid value in 'options'".to_string()),
        RData::Str(xs) => xs
            .first()
            .cloned()
            .flatten()
            .map(O::Str)
            .ok_or_else(|| "invalid value in 'options'".to_string()),
        _ => Err("invalid value in 'options'".into()),
    }
}

/// The inverse: an option's stored value as the R value `options`/`getOption`
/// hand back. An option that was never set reads as `NULL`, which is what puts
/// a `NULL` element in the list `options("nosuchthing")` returns.
fn option_value_to_r(v: Option<crate::host::OptionValue>) -> Value {
    use crate::host::OptionValue as O;
    match v {
        Some(O::Int(n)) => scalar_int(n),
        Some(O::Dbl(x)) => scalar_dbl(x),
        Some(O::Lgl(b)) => scalar_lgl(b),
        Some(O::Str(s)) => scalar_str(s),
        None => null(),
    }
}

/// R's `Ops.factor` / `Ops.ordered` group generics, or `None` when no operand is
/// a factor.
///
/// A factor's integer codes are an implementation detail that an operator must
/// never expose: `f == "a"` compares *labels*, and every operator with no
/// meaning on a label set answers `NA` with a warning rather than silently
/// comparing codes. `Ops.factor` allows only `==` and `!=`; `Ops.ordered`
/// additionally allows `< > <= >=`, which compare level positions, and delegates
/// `==`/`!=` back here via `NextMethod`.
///
/// `rhs` is `None` for a unary operator (`!f`).
fn ops_factor(op: &str, lhs: &Value, rhs: Option<&Value>) -> Option<Result<Value, String>> {
    let lf = is_factor(lhs);
    let rf = rhs.is_some_and(is_factor);
    // `:` is not a member of R's Ops group — on factors it builds an
    // interaction, which is a separate primitive, so leave it alone.
    if (!lf && !rf) || op == ":" {
        return None;
    }
    let ordered = (lf && is_ordered(lhs)) || (rf && is_ordered(rhs.unwrap()));
    let ok = match op {
        "==" | "!=" => true,
        "<" | ">" | "<=" | ">=" => ordered,
        _ => false,
    };
    if !ok {
        // R: `warning(gettextf("%s not meaningful for factors", sQuote(.Generic)))`
        // — `sQuote` yields directional quotes — and `Ops.ordered`'s plainer
        // `sprintf("'%s' is not meaningful for ordered factors", .Generic)`.
        let msg = if ordered {
            format!("'{op}' is not meaningful for ordered factors")
        } else {
            format!("\u{2018}{op}\u{2019} not meaningful for factors")
        };
        // A real condition, not just a printed line: `tryCatch(warning =)` can
        // catch it, `suppressWarnings` can muffle it, and a calling handler sees
        // it before the NA vector comes back.
        if let Err(e) = binop_warning(&msg) {
            return Some(Err(e));
        }
        let n = len(lhs).max(rhs.map_or(0, len));
        return Some(Ok(mk_lgl(vec![None; n])));
    }
    let rhs = rhs.expect("a binary operator reached the comparison path");
    // Two factors must describe the same level set, whichever comparison it is.
    if lf && rf {
        let (l1, l2) = (levels_of(lhs), levels_of(rhs));
        let mismatch = if op == "==" || op == "!=" {
            // `Ops.factor` compares the level sets as sets.
            let (mut a, mut b) = (l1.clone(), l2.clone());
            a.sort();
            b.sort();
            a != b
        } else {
            // `Ops.ordered` compares them positionally — the order is the point.
            l1 != l2
        };
        if mismatch {
            return Some(Err("level sets of factors are different".into()));
        }
    }
    if op == "==" || op == "!=" {
        // Both sides become labels, then it is an ordinary string comparison.
        return Some(compare(
            op,
            &mk_str(as_str_labels(lhs)),
            &mk_str(as_str_labels(rhs)),
        ));
    }
    // Ordering compares level *positions*. A side that is already ordered
    // contributes its codes; a bare value is matched into the other's levels
    // (`NA` when it is not a level at all, which makes the comparison NA).
    let levels = if lf { levels_of(lhs) } else { levels_of(rhs) };
    let rank = |v: &Value| -> Value {
        if is_factor(v) {
            mk_int(as_int(v))
        } else {
            mk_int(
                as_str(v)
                    .iter()
                    .map(|s| {
                        s.as_ref()
                            .and_then(|s| levels.iter().position(|l| l == s))
                            .map(|p| p as i64 + 1)
                    })
                    .collect(),
            )
        }
    };
    Some(compare(op, &rank(lhs), &rank(rhs)))
}

pub fn binop(op: &str, lhs: &Value, rhs: &Value) -> Result<Value, String> {
    // An operator on a foreign R object (`date + months(3)`, `sparse %*% x`)
    // delegates to embedded R, which knows the S3/S4 method.
    if kind(lhs) == RKind::RForeign || kind(rhs) == RKind::RForeign {
        return cran_call(op, &[(None, lhs.clone()), (None, rhs.clone())]);
    }
    // A factor operand dispatches to R's group generic before any of the
    // ordinary numeric/string paths can see its codes.
    if let Some(r) = ops_factor(op, lhs, Some(rhs)) {
        return r;
    }
    match op {
        "+" | "-" | "*" | "/" | "^" | "%%" | "%/%" => arith(op, lhs, rhs),
        "==" | "!=" | "<" | ">" | "<=" | ">=" => compare(op, lhs, rhs),
        "&" | "|" => logic(op, lhs, rhs),
        ":" => Ok(colon(lhs, rhs)),
        other => Err(format!("invalid operator '{other}'")),
    }
}

/// The recycled length of a binary operation, and whether it is empty.
///
/// R warns when the shorter operand does not tile the longer one exactly — the
/// result is still produced, from a partial final cycle.
fn recycle_len(a: usize, b: usize) -> Result<usize, String> {
    if a == 0 || b == 0 {
        return Ok(0);
    }
    let (lo, hi) = (a.min(b), a.max(b));
    if hi % lo != 0 {
        // A real condition, so `tryCatch(warning = )` and `suppressWarnings`
        // reach it the way they reach `warning()`.
        binop_warning("longer object length is not a multiple of shorter object length")?;
    }
    Ok(hi)
}

/// Copy `names`/`dim`/`dimnames` from the operand that shaped the result.
///
/// `dimnames` travels with `dim` — a result that keeps a matrix's shape keeps
/// its row/column labels too, so `dimnames(m + 1)` is `dimnames(m)`. Dropping it
/// printed every derived matrix with bare `[,1]` headers instead of the names.
fn carry_attrs(out: &Value, lhs: &Value, rhs: &Value) {
    let src = if len(lhs) >= len(rhs) { lhs } else { rhs };
    for key in ["names", "dim", "dimnames"] {
        if let Some(a) = with_host(|h| h.attr(src, key)) {
            with_host(|h| h.set_attr(out, key, a));
        }
    }
}

/// Return `out` carrying `x`'s shape — `names`, `dim` and `dimnames`.
///
/// R's element-wise builtins (`round`, `log`, `is.na`, `nchar`, `toupper`,
/// `gsub`, `!`, unary `-`, …) produce a result that lines up 1:1 with their
/// input, and every one of them keeps that input's labels and shape:
/// `nchar(c(a = "xx"))` is named `a` and `is.na(m)` is a matrix. Without this
/// they all returned bare vectors, which additionally broke `m[lower.tri(m)]`
/// because the mask lost the `dim` that made it a mask.
fn shaped_like(out: Value, x: &Value) -> Result<Value, String> {
    carry_attrs(&out, x, x);
    Ok(out)
}

/// R's warning when `+ - *` on two integers leaves the 32-bit range.
const INT_OVERFLOW: &str = "NAs produced by integer overflow";

/// Can `x` be stored as an R `integer`? R's integers are C `int`, and `INT_MIN`
/// is reserved as `NA_integer_`, so the representable range is
/// `[-2147483647, 2147483647]` — `-2147483647L - 1L` is `NA`, not `-2147483648`.
/// rlang holds integers in `i64`, so without this check integer arithmetic would
/// silently widen past what R can represent and return a number where R returns
/// `NA` (`2147483647L + 1L` printed `2147483648`).
fn int_in_range(x: f64) -> bool {
    x.is_finite() && x.abs() <= i32::MAX as f64
}

/// R's warning when an explicit coercion lands outside the 32-bit integer range.
const INT_COERCE_RANGE: &str = "NAs introduced by coercion to integer range";

/// Apply R's integer range to the result of an explicit `as.integer`-style
/// coercion: a value C's `int` cannot hold becomes `NA_integer_`, with one
/// warning for the whole vector.
///
/// This deliberately lives at the *coercion builtins*, not inside the shared
/// `as_int` helper, because `as_int` also serves internal index and dimension
/// arithmetic where a wide value is a position to bounds-check rather than a
/// number to narrow. Without it `as.integer(2^31)` printed 2147483648 — a value
/// R has no way to represent — for the same reason `2147483647L + 1L` used to.
fn int_range_na(xs: Vec<Option<i64>>) -> Vec<Option<i64>> {
    let mut lost = false;
    let out = xs
        .into_iter()
        .map(|e| {
            e.and_then(|n| match int_in_range(n as f64) {
                true => Some(n),
                false => {
                    lost = true;
                    None
                }
            })
        })
        .collect();
    if lost {
        // Another `CoercionWarning` message, reported the same way.
        r_warning(INT_COERCE_RANGE, with_host(|h| h.context_call()));
    }
    out
}

fn arith(op: &str, lhs: &Value, rhs: &Value) -> Result<Value, String> {
    // Scalar fast path: two length-1 unattributed numerics compute directly,
    // skipping the vector engine's data_of clones, as_dbl allocations, output
    // Vec, and carry_attrs — a single result allocation remains. This is the
    // common case in tight loops (`s <- s + i`).
    if let (Some((x, xi)), Some((y, yi))) = (
        with_host(|h| h.scalar_real(lhs)),
        with_host(|h| h.scalar_real(rhs)),
    ) {
        let r = match op {
            "+" => x + y,
            "-" => x - y,
            "*" => x * y,
            "/" => x / y,
            "^" => r_pow(x, y),
            "%%" => r_mod(x, y),
            _ => r_idiv(x, y),
        };
        // Same integer-vs-double rule as the vector path: `+ - * %% %/%` on two
        // integer/logical operands stays integer (NA when the result is
        // non-finite); `/` and `^` are always double.
        let int_result = matches!(op, "+" | "-" | "*" | "%%" | "%/%") && xi && yi;
        return Ok(if int_result {
            // Integer stays unboxed unless the result is non-finite or leaves
            // the 32-bit range, both of which R reports as NA_integer_ — and an
            // unboxed `Value::Int` can't be NA.
            match int_in_range(r) {
                true => Value::Int(r as i64),
                false => {
                    if r.is_finite() {
                        r_warning(INT_OVERFLOW, None);
                    }
                    mk_int(vec![None])
                }
            }
        } else {
            // Every double (incl. NaN/Inf, which are values, not NA) is unboxed.
            Value::Float(r)
        });
    }
    if kind(lhs) == RKind::Str || kind(rhs) == RKind::Str {
        return Err("non-numeric argument to binary operator".into());
    }
    let n = recycle_len(len(lhs), len(rhs))?;
    // Integer arithmetic stays integer for `+ - * %% %/%`; `/` and `^` always
    // produce doubles, exactly as R does.
    let int_result = matches!(op, "+" | "-" | "*" | "%%" | "%/%")
        && matches!(kind(lhs), RKind::Int | RKind::Lgl)
        && matches!(kind(rhs), RKind::Int | RKind::Lgl);
    let (a, b) = (as_dbl(lhs), as_dbl(rhs));
    let mut out: Vec<Option<f64>> = Vec::with_capacity(n);
    for i in 0..n {
        let (x, y) = (a[i % a.len()], b[i % b.len()]);
        out.push(match (x, y) {
            // R resolves `x ^ 0` and `1 ^ y` to 1 before it ever inspects the
            // operands for NA, so `NA ^ 0` and `1 ^ NA` are 1 rather than NA.
            // These arms come first because the `_ => None` NA rule below would
            // otherwise swallow them.
            _ if op == "^" && (y == Some(0.0) || x == Some(1.0)) => Some(1.0),
            (Some(x), Some(y)) => Some(match op {
                "+" => x + y,
                "-" => x - y,
                "*" => x * y,
                "/" => x / y,
                "^" => r_pow(x, y),
                // R's `%%` is C `fmod` (an *exact* remainder against the stored
                // divisor) re-signed to follow the divisor, not the dividend.
                // The old `x - y*floor(x/y)` rounded the quotient first, so
                // `10 %% 0.04` collapsed to 0 instead of R's 0.04.
                "%%" => r_mod(x, y),
                _ => r_idiv(x, y),
            }),
            _ => None,
        });
    }
    let v = if int_result {
        let mut overflowed = false;
        let ints = out
            .into_iter()
            .map(|e| {
                e.and_then(|x| {
                    let ok = int_in_range(x);
                    overflowed |= !ok && x.is_finite();
                    ok.then_some(x as i64)
                })
            })
            .collect();
        if overflowed {
            r_warning(INT_OVERFLOW, None);
        }
        mk_int(ints)
    } else {
        mk_dbl(out)
    };
    carry_attrs(&v, lhs, rhs);
    Ok(v)
}

fn compare(op: &str, lhs: &Value, rhs: &Value) -> Result<Value, String> {
    // Scalar fast path (see `arith`): two length-1 unattributed numerics compare
    // directly. NaN on either side yields NA, matching the vector path.
    if let (Some((x, _)), Some((y, _))) = (
        with_host(|h| h.scalar_real(lhs)),
        with_host(|h| h.scalar_real(rhs)),
    ) {
        return Ok(
            match (!x.is_nan() && !y.is_nan()).then(|| cmp_result(op, x.partial_cmp(&y).unwrap())) {
                Some(b) => Value::Bool(b),
                // NaN on either side is NA, which an unboxed `Value::Bool` can't hold.
                None => mk_lgl(vec![None]),
            },
        );
    }
    let n = recycle_len(len(lhs), len(rhs))?;
    let as_text = kind(lhs) == RKind::Str || kind(rhs) == RKind::Str;
    let mut out: Vec<Option<bool>> = Vec::with_capacity(n);
    if as_text {
        let (a, b) = (as_str(lhs), as_str(rhs));
        for i in 0..n {
            let (x, y) = (&a[i % a.len()], &b[i % b.len()]);
            out.push(match (x, y) {
                (Some(x), Some(y)) => Some(cmp_result(op, crate::collate::str_cmp(x, y))),
                _ => None,
            });
        }
    } else {
        let (a, b) = (as_dbl(lhs), as_dbl(rhs));
        for i in 0..n {
            let (x, y) = (a[i % a.len()], b[i % b.len()]);
            out.push(match (x, y) {
                (Some(x), Some(y)) if !x.is_nan() && !y.is_nan() => {
                    Some(cmp_result(op, x.partial_cmp(&y).unwrap()))
                }
                _ => None,
            });
        }
    }
    let v = mk_lgl(out);
    carry_attrs(&v, lhs, rhs);
    Ok(v)
}

fn cmp_result(op: &str, ord: std::cmp::Ordering) -> bool {
    use std::cmp::Ordering::*;
    match op {
        "==" => ord == Equal,
        "!=" => ord != Equal,
        "<" => ord == Less,
        ">" => ord == Greater,
        "<=" => ord != Greater,
        _ => ord != Less,
    }
}

/// `&` and `|`, with R's three-valued logic: `NA & FALSE` is FALSE and
/// `NA | TRUE` is TRUE, because the answer is decided regardless of the NA.
fn logic(op: &str, lhs: &Value, rhs: &Value) -> Result<Value, String> {
    let n = recycle_len(len(lhs), len(rhs))?;
    let (a, b) = (as_lgl(lhs), as_lgl(rhs));
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let (x, y) = (a[i % a.len()], b[i % b.len()]);
        out.push(match op {
            "&" => match (x, y) {
                (Some(false), _) | (_, Some(false)) => Some(false),
                (Some(true), Some(true)) => Some(true),
                _ => None,
            },
            _ => match (x, y) {
                (Some(true), _) | (_, Some(true)) => Some(true),
                (Some(false), Some(false)) => Some(false),
                _ => None,
            },
        });
    }
    Ok(mk_lgl(out))
}

/// `from:to` — an integer sequence when both ends are whole numbers.
fn colon(lhs: &Value, rhs: &Value) -> Value {
    let from = num1(lhs).unwrap_or(f64::NAN);
    let to = num1(rhs).unwrap_or(f64::NAN);
    if from.is_nan() || to.is_nan() {
        return mk_int(vec![None]);
    }
    let whole = from == from.trunc() && to == to.trunc();
    let mut out = Vec::new();
    if from <= to {
        let mut x = from;
        while x <= to + 1e-10 {
            out.push(Some(x));
            x += 1.0;
        }
    } else {
        let mut x = from;
        while x >= to - 1e-10 {
            out.push(Some(x));
            x -= 1.0;
        }
    }
    if whole {
        mk_int(out.into_iter().map(|e| e.map(|x| x as i64)).collect())
    } else {
        mk_dbl(out)
    }
}

// ── conditions and loop support ─────────────────────────────────────────

fn b_truthy(vm: &mut VM, _: u8) -> Value {
    let v = vm.pop();
    match as_lgl(&v).first().copied() {
        Some(Some(b)) => Value::Bool(b),
        Some(None) => abort(vm, "missing value where TRUE/FALSE needed".into()),
        None => abort(vm, "argument is of length zero".into()),
    }
}

fn b_is_false(vm: &mut VM, _: u8) -> Value {
    let v = vm.pop();
    Value::Bool(matches!(as_lgl(&v).first(), Some(Some(false))))
}

fn b_is_true(vm: &mut VM, _: u8) -> Value {
    let v = vm.pop();
    Value::Bool(matches!(as_lgl(&v).first(), Some(Some(true))))
}

fn b_seq_len(vm: &mut VM, _: u8) -> Value {
    let v = vm.pop();
    Value::Int(len(&v) as i64)
}

fn range_ends(vm: &mut VM) -> (f64, f64, bool) {
    let to = num1(&vm.pop()).unwrap_or(f64::NAN);
    let from = num1(&vm.pop()).unwrap_or(f64::NAN);
    let whole = from == from.trunc() && to == to.trunc();
    (from, to, whole)
}

/// Typed start of `from:to` (integer when both ends whole, else double; `NA`
/// when an end is `NA`/`NaN`) — the loop-setup helper for a native range `for`.
fn b_range_from(vm: &mut VM, _: u8) -> Value {
    let (from, to, whole) = range_ends(vm);
    if from.is_nan() || to.is_nan() {
        return mk_int(vec![None]);
    }
    if whole {
        Value::Int(from as i64)
    } else {
        Value::Float(from)
    }
}

/// Typed ±1 step of `from:to`, matching the element type.
fn b_range_step(vm: &mut VM, _: u8) -> Value {
    let (from, to, whole) = range_ends(vm);
    // NaN-preserving: `!(from > to)` is true when either end is NaN, which
    // `from <= to` is not — keep the original ordering semantics.
    let up = !matches!(from.partial_cmp(&to), Some(std::cmp::Ordering::Greater));
    if whole {
        Value::Int(if up { 1 } else { -1 })
    } else {
        Value::Float(if up { 1.0 } else { -1.0 })
    }
}

/// Element count of `from:to` — `floor(|to-from| + 1e-10) + 1`, matching
/// `colon`; `1` for an `NA`/`NaN` end.
fn b_range_len(vm: &mut VM, _: u8) -> Value {
    let (from, to, _) = range_ends(vm);
    if from.is_nan() || to.is_nan() {
        return Value::Int(1);
    }
    Value::Int(((to - from).abs() + 1e-10).floor() as i64 + 1)
}

fn b_seq_elem(vm: &mut VM, _: u8) -> Value {
    let i = match vm.pop() {
        Value::Int(i) => i as usize,
        Value::Float(f) => f as usize,
        _ => 0,
    };
    let v = vm.pop();
    element_at(&v, i)
}

fn b_autoprint(vm: &mut VM, _: u8) -> Value {
    let v = vm.pop();
    let show = with_host(|h| {
        let s = h.echo && h.visible && h.error.is_none() && h.signal.is_none();
        // R sets `R_Visible` before each top-level statement, and this is where
        // rlang arms the next one. A chunk that is not echoing has no next
        // top-level statement — it is an `eval` inside a running program — and
        // its last statement's visibility is the value's, so it stands:
        // `eval(quote(cat("hi")))` returns NULL invisibly, as `cat` does.
        if h.echo {
            h.visible = true;
        }
        s
    });
    if show {
        // R autoprints by calling `print`, so a user's `print.myclass` runs for
        // a bare `obj` at top level exactly as it does for `print(obj)`.
        match s3_primitive_method("print", std::slice::from_ref(&(None, v.clone()))) {
            Some(Err(e)) => return abort(vm, e),
            Some(Ok(_)) => {}
            None => print_value(&v),
        }
    }
    propagate(vm, v)
}

// ── indexing ────────────────────────────────────────────────────────────

fn args_of(v: &Value) -> Vec<(Option<String>, Value)> {
    match data(v) {
        RData::Args(a) => a,
        _ => Vec::new(),
    }
}

/// Delegate `op(x, args…)` on a foreign R handle to embedded R.
fn foreign_index(vm: &mut VM, op: &str, x: Value, mut args: Vec<(Option<String>, Value)>) -> Value {
    args.insert(0, (None, x));
    match cran_call(op, &args) {
        Ok(v) => v,
        Err(e) => abort(vm, e),
    }
}

/// `x[i] <- v` / `x[[i]] <- v` / `x$n <- v` on a foreign handle. R turns each
/// into the replacement call `x <- `op`(x, ...index..., value)`, so we delegate
/// the same shape to embedded R and hand back the modified object.
fn foreign_index_set(
    vm: &mut VM,
    op: &str,
    x: Value,
    mut args: Vec<(Option<String>, Value)>,
    value: Value,
) -> Value {
    args.insert(0, (None, x));
    args.push((None, value));
    match cran_call(op, &args) {
        Ok(v) => v,
        Err(e) => abort(vm, e),
    }
}

fn b_index(vm: &mut VM, _: u8) -> Value {
    let argv = vm.pop();
    let x = vm.pop();
    if kind(&x) == RKind::RForeign {
        return foreign_index(vm, "[", x, args_of(&argv));
    }
    match index_single(&x, &args_of(&argv)) {
        Ok(v) => v,
        Err(e) => abort(vm, e),
    }
}

fn b_index2(vm: &mut VM, _: u8) -> Value {
    let argv = vm.pop();
    let x = vm.pop();
    if kind(&x) == RKind::RForeign {
        return foreign_index(vm, "[[", x, args_of(&argv));
    }
    match index_double(&x, &args_of(&argv)) {
        Ok(v) => v,
        Err(e) => abort(vm, e),
    }
}

fn b_dollar(vm: &mut VM, _: u8) -> Value {
    let name = name_of(&vm.pop());
    let x = vm.pop();
    if kind(&x) == RKind::RForeign {
        // `df$col` on a foreign object is `df[["col"]]` in R.
        return foreign_index(vm, "[[", x, vec![(None, scalar_str(name))]);
    }
    match data(&x) {
        RData::Environment(e) => e.borrow().vars.get(&name).cloned().unwrap_or_else(null),
        _ => {
            let names = names_of(&x);
            match names
                .iter()
                .position(|n| n.as_deref() == Some(name.as_str()))
            {
                Some(i) => match data(&x) {
                    RData::List(xs) => xs.get(i).cloned().unwrap_or_else(null),
                    _ => element_at(&x, i),
                },
                None => null(),
            }
        }
    }
}

/// Resolve one index argument to zero-based positions over a length-`n` vector.
/// Handles R's four index forms: positive, negative (exclusion), logical
/// (recycled), and character (by name). `None` positions select NA.
fn resolve_index(
    idx: &Value,
    n: usize,
    names: &[Option<String>],
) -> Result<Vec<Option<usize>>, String> {
    match data(idx) {
        RData::Str(keys) => Ok(keys
            .iter()
            .map(|k| {
                k.as_ref().and_then(|k| {
                    names
                        .iter()
                        .position(|nm| nm.as_deref() == Some(k.as_str()))
                })
            })
            .collect()),
        RData::Lgl(mask) if !mask.is_empty() => {
            let width = n.max(mask.len());
            let mut out = Vec::new();
            for i in 0..width {
                match mask[i % mask.len()] {
                    Some(true) => out.push((i < n).then_some(i)),
                    Some(false) => {}
                    None => out.push(None),
                }
            }
            Ok(out)
        }
        _ => {
            let nums = as_dbl(idx);
            if nums.iter().flatten().any(|x| *x < 0.0) {
                if nums.iter().flatten().any(|x| *x > 0.0) {
                    return Err("can't mix positive and negative subscripts".into());
                }
                let drop: Vec<usize> = nums
                    .iter()
                    .flatten()
                    .map(|x| (-x) as usize)
                    .filter(|x| *x >= 1)
                    .collect();
                return Ok((0..n)
                    .filter(|i| !drop.contains(&(i + 1)))
                    .map(Some)
                    .collect());
            }
            Ok(nums
                .iter()
                .filter(|x| **x != Some(0.0))
                .map(|x| match x {
                    Some(v) => {
                        let i = *v as usize;
                        (i >= 1 && i <= n).then_some(i - 1)
                    }
                    None => None,
                })
                .collect())
        }
    }
}

/// `x[...]` — subsetting, which keeps the container type and the names.
fn index_single(x: &Value, args: &[(Option<String>, Value)]) -> Result<Value, String> {
    let supplied: Vec<&Value> = args
        .iter()
        .filter(|(_, v)| !matches!(v, Value::Undef))
        .map(|(_, v)| v)
        .collect();
    // N-dimensional indexing `a[i, j, …]` when the subscript count matches the
    // array's `dim` rank (covers 2-D matrices and 3-D+ arrays alike).
    if args.len() >= 2 {
        if let Some(dim) = with_host(|h| h.attr(x, "dim")) {
            let d: Vec<usize> = as_int(&dim)
                .iter()
                .map(|e| e.unwrap_or(0) as usize)
                .collect();
            // `drop =` is an option, not a subscript: `m["r1", , drop = FALSE]`
            // still indexes a 2-D matrix with exactly two subscripts.
            let subs: Vec<(Option<String>, Value)> = args
                .iter()
                .filter(|(t, _)| t.as_deref() != Some("drop"))
                .cloned()
                .collect();
            let drop = args
                .iter()
                .find(|(t, _)| t.as_deref() == Some("drop"))
                .and_then(|(_, v)| as_lgl(v).first().copied().flatten())
                .unwrap_or(true);
            if d.len() == subs.len() {
                return array_index(x, &subs, &d, drop);
            }
        }
    }
    if supplied.is_empty() {
        return Ok(x.clone());
    }
    // A lone matrix subscript is a coordinate table (`m[cbind(i, j)]`), which
    // resolves to one linear position per row rather than to the flat index
    // vector the general path below would read out of it.
    if supplied.len() == 1 {
        if let Some(lin) = matrix_subscript(x, supplied[0])? {
            return index_single(x, &[(None, mk_int(lin))]);
        }
    }
    let n = len(x);
    let names = names_of(x);
    let pos = resolve_index(supplied[0], n, &names)?;
    let out = take_positions(x, &pos);
    if !names.is_empty() {
        let sel: Vec<Option<String>> = pos
            .iter()
            .map(|p| p.and_then(|i| names.get(i).cloned().flatten()))
            .collect();
        // Subsetting a *named* vector always yields names, even when every
        // selected element is out of bounds or an unmatched label: R fills those
        // slots with `NA_character_`, which prints as `<NA>`. `set_names` nulls
        // an all-NA vector (the right call for `names(x) <- NULL`-style writes),
        // so the attribute is set directly here — `x[3]` on `c(a=1,b=2)` is
        // `<NA>\n  NA`, not an unnamed `[1] NA`.
        let nv = mk_str(sel);
        with_host(|h| h.set_attr(&out, "names", nv));
    }
    // `[.factor` puts the level table and class back on the subset codes, and
    // `drop = TRUE` then re-levels to only the labels that survived.
    carry_factor(&out, x);
    let drop = args
        .iter()
        .find(|(t, _)| t.as_deref() == Some("drop"))
        .and_then(|(_, v)| as_lgl(v).first().copied().flatten())
        .unwrap_or(false);
    if drop && is_factor(x) {
        return Ok(drop_unused_levels(&out));
    }
    Ok(out)
}

/// R's *matrix subscript*: when an array is indexed with a single argument that
/// is itself a matrix of `rank(x)` columns, each ROW of that matrix is one full
/// coordinate tuple, so `m[cbind(i, j)]` picks `length(i)` individual cells
/// rather than flattening the index to a vector of linear positions.
///
/// Returns the selected cells as 1-based linear positions so both the read
/// (`index_single`) and write (`assign_index`) paths can hand them to their
/// existing 1-D machinery, exactly as they already do for `a[i, j, …]`.
/// `Ok(None)` means this was not a matrix subscript, which leaves the ordinary
/// flat-vector indexing in place.
///
/// R's per-coordinate rules, each verified against the reference `Rscript`:
/// an NA coordinate selects an NA element; a `0` coordinate drops that row from
/// the result entirely (so the answer is shorter than the index has rows); a
/// negative coordinate, an out-of-range coordinate, and an unmatched dimname are
/// all errors rather than NA.
fn matrix_subscript(x: &Value, idx: &Value) -> Result<Option<Vec<Option<i64>>>, String> {
    let to_dims =
        |v: &Value| -> Vec<usize> { as_int(v).iter().map(|e| e.unwrap_or(0) as usize).collect() };
    let (Some(xd), Some(idd)) = (
        with_host(|h| h.attr(x, "dim")),
        with_host(|h| h.attr(idx, "dim")),
    ) else {
        return Ok(None);
    };
    let (d, id) = (to_dims(&xd), to_dims(&idd));
    // Only a 2-D index whose column count equals the array's rank is a
    // coordinate table; anything else (including a matrix index on a matrix of a
    // different rank) keeps R's plain linear-index meaning.
    if d.len() < 2 || id.len() != 2 || id[1] != d.len() {
        return Ok(None);
    }
    let (rows, cols) = (id[0], id[1]);
    let oob = || "subscript out of bounds".to_string();
    let dn = dimnames_of(x);
    // `Some(None)` is an NA coordinate, `None` a dropped (`0`) one. The index
    // matrix is column-major, so its element `(r, c)` sits at `c * rows + r`.
    let mut coords: Vec<Vec<Option<Option<usize>>>> = Vec::with_capacity(cols);
    // `cols == d.len()` (checked above), so the take() walks every dimension.
    for (c, dim) in d.iter().enumerate().take(cols) {
        let mut col = Vec::with_capacity(rows);
        for r in 0..rows {
            col.push(match data(idx) {
                RData::Str(keys) => {
                    let labels = dn.get(c).cloned().flatten().unwrap_or_default();
                    match keys.get(c * rows + r).cloned().flatten() {
                        None => Some(None),
                        Some(k) => Some(Some(
                            labels
                                .iter()
                                .position(|l| l.as_deref() == Some(k.as_str()))
                                .ok_or_else(oob)?,
                        )),
                    }
                }
                _ => match as_int(idx).get(c * rows + r).copied().flatten() {
                    None => Some(None),
                    Some(0) => None,
                    Some(n) if n < 0 => {
                        return Err("negative values are not allowed in a matrix subscript".into())
                    }
                    Some(n) if n as usize > *dim => return Err(oob()),
                    Some(n) => Some(Some(n as usize - 1)),
                },
            });
        }
        coords.push(col);
    }
    Ok(Some(
        (0..rows)
            .filter(|&r| coords.iter().all(|col| col[r].is_some()))
            .map(|r| {
                // Column-major linearisation: i0 + d0*i1 + d0*d1*i2 + …
                let mut lin = 0usize;
                let mut stride = 1usize;
                for (c, dc) in d.iter().enumerate() {
                    lin += coords[c][r].flatten()? * stride;
                    stride *= dc;
                }
                Some(lin as i64 + 1)
            })
            .collect(),
    ))
}

/// Build a new vector/list from zero-based positions (`None` → NA element).
fn take_positions(x: &Value, pos: &[Option<usize>]) -> Value {
    // The selection is read *through* a borrow of the heap object rather than
    // from a copy of it. `data(x)` clones the whole vector, which made reading
    // one element — `x[i]` in a loop — cost O(length(x)) and the loop itself
    // quadratic. The picked elements are collected inside the borrow and
    // allocated after it, because allocating re-enters the host.
    enum Picked {
        Lgl(Vec<Option<bool>>),
        Int(Vec<Option<i64>>),
        Dbl(Vec<Option<f64>>),
        Str(Vec<Option<String>>),
        List(Vec<Option<Value>>),
        Null,
    }
    let picked = with_host(|h| match h.get(x).map(|o| &o.data) {
        Some(RData::Lgl(v)) => Picked::Lgl(
            pos.iter()
                .map(|p| p.and_then(|i| v.get(i).copied().flatten()))
                .collect(),
        ),
        Some(RData::Int(v)) => Picked::Int(
            pos.iter()
                .map(|p| p.and_then(|i| v.get(i).copied().flatten()))
                .collect(),
        ),
        Some(RData::Dbl(v)) => Picked::Dbl(
            pos.iter()
                .map(|p| p.and_then(|i| v.get(i).copied().flatten()))
                .collect(),
        ),
        Some(RData::Str(v)) => Picked::Str(
            pos.iter()
                .map(|p| p.and_then(|i| v.get(i).cloned().flatten()))
                .collect(),
        ),
        Some(RData::List(v)) => Picked::List(
            pos.iter()
                .map(|p| p.and_then(|i| v.get(i).cloned()))
                .collect(),
        ),
        // An unboxed scalar has no heap object to borrow; it is one element, so
        // materialising it costs nothing. `h.data_of` and not `data`, which
        // would re-enter the host borrow this closure already holds.
        None => match h.data_of(x) {
            RData::Lgl(v) => Picked::Lgl(
                pos.iter()
                    .map(|p| p.and_then(|i| v.get(i).copied().flatten()))
                    .collect(),
            ),
            RData::Int(v) => Picked::Int(
                pos.iter()
                    .map(|p| p.and_then(|i| v.get(i).copied().flatten()))
                    .collect(),
            ),
            RData::Dbl(v) => Picked::Dbl(
                pos.iter()
                    .map(|p| p.and_then(|i| v.get(i).copied().flatten()))
                    .collect(),
            ),
            _ => Picked::Null,
        },
        _ => Picked::Null,
    });
    match picked {
        Picked::Lgl(v) => mk_lgl(v),
        Picked::Int(v) => mk_int(v),
        Picked::Dbl(v) => mk_dbl(v),
        Picked::Str(v) => mk_str(v),
        // A position that selected nothing is R's `NULL` element; filling it
        // needs the host, so it happens after the borrow rather than inside it.
        Picked::List(v) => mk_list(v.into_iter().map(|e| e.unwrap_or_else(null)).collect()),
        Picked::Null => null(),
    }
}

/// The column-major linear positions an N-D subscript selects, paired with the
/// indices each margin selected (which is what carries `dimnames` through).
type ArraySelection = (Vec<Option<usize>>, Vec<Vec<usize>>);

/// The column-major linear positions an N-D subscript `a[i, j, …]` selects, plus
/// the indices each margin selected. An empty subscript takes the whole margin;
/// a character subscript resolves against that margin's `dimnames`. Shared by
/// array read and array assignment.
fn array_positions(
    args: &[(Option<String>, Value)],
    dims: &[usize],
    dimnames: &[Option<Vec<Option<String>>>],
) -> Result<ArraySelection, String> {
    let k = dims.len();
    let sel: Vec<Vec<usize>> = (0..k)
        .map(|d| match &args[d].1 {
            Value::Undef => Ok((0..dims[d]).collect()),
            v => {
                let labels = dimnames.get(d).cloned().flatten().unwrap_or_default();
                resolve_index(v, dims[d], &labels).map(|p| p.into_iter().flatten().collect())
            }
        })
        .collect::<Result<_, String>>()?;
    // Column-major strides: the first subscript varies fastest.
    let mut stride = vec![1usize; k];
    for d in 1..k {
        stride[d] = stride[d - 1] * dims[d - 1];
    }
    let shape: Vec<usize> = sel.iter().map(|s| s.len()).collect();
    let total: usize = shape.iter().product();
    let mut pos = Vec::with_capacity(total);
    let mut idx = vec![0usize; k];
    for _ in 0..total {
        let lin: usize = (0..k).map(|d| sel[d][idx[d]] * stride[d]).sum();
        pos.push(Some(lin));
        for d in 0..k {
            idx[d] += 1;
            if idx[d] < shape[d] {
                break;
            }
            idx[d] = 0;
        }
    }
    Ok((pos, sel))
}

/// `a[i, j, …]` read over an N-D `dim`: gather the selected slice, then (with
/// `drop = TRUE`) drop the length-1 dimensions. A rank ≥ 2 remainder keeps its
/// `dim` and `dimnames`; a remainder dropped to a vector takes the surviving
/// margin's labels as its `names`.
fn array_index(
    x: &Value,
    args: &[(Option<String>, Value)],
    dims: &[usize],
    drop: bool,
) -> Result<Value, String> {
    let dn = dimnames_of(x);
    let (pos, sel) = array_positions(args, dims, &dn)?;
    let out = take_positions(x, &pos);
    let shape: Vec<usize> = sel.iter().map(|s| s.len()).collect();

    // The labels a margin keeps are its own, taken in selection order.
    let labels = |d: usize| -> Option<Vec<Option<String>>> {
        let all = dn.get(d).cloned().flatten()?;
        Some(
            sel[d]
                .iter()
                .map(|&i| all.get(i).cloned().flatten())
                .collect(),
        )
    };
    let kept: Vec<usize> = (0..shape.len())
        .filter(|&d| !drop || shape[d] != 1)
        .collect();

    if kept.len() >= 2 {
        let dim = mk_int(kept.iter().map(|&d| Some(shape[d] as i64)).collect());
        with_host(|h| h.set_attr(&out, "dim", dim));
        if kept.iter().any(|&d| labels(d).is_some()) {
            let dnv = mk_list(
                kept.iter()
                    .map(|&d| labels(d).map(mk_str).unwrap_or_else(null))
                    .collect(),
            );
            with_host(|h| h.set_attr(&out, "dimnames", dnv));
        }
    } else if let Some(&d) = kept.first() {
        if let Some(l) = labels(d) {
            if l.iter().any(|n| n.is_some()) {
                set_names(&out, l);
            }
        }
    }
    Ok(out)
}

/// `x[[...]]` — extraction of exactly one element.
fn index_double(x: &Value, args: &[(Option<String>, Value)]) -> Result<Value, String> {
    let Some((_, idx)) = args.first() else {
        return Err("subscript out of bounds".into());
    };
    if let RData::Environment(e) = data(x) {
        let key = str1(idx).unwrap_or_default();
        return Ok(e.borrow().vars.get(&key).cloned().unwrap_or_else(null));
    }
    // A call indexes into its own parts: `quote(f(1))[[1]]` is the name `f`.
    if let RData::Lang(e) = data(x) {
        let parts = lang_parts(&e);
        let i = num1(idx).unwrap_or(0.0) as usize;
        return match i.checked_sub(1).and_then(|i| parts.get(i)) {
            Some((_, v)) => Ok(v.clone()),
            None => Err("subscript out of bounds".into()),
        };
    }
    let names = names_of(x);
    let i = match data(idx) {
        RData::Str(k) => {
            let key = k.first().cloned().flatten().unwrap_or_default();
            match names
                .iter()
                .position(|n| n.as_deref() == Some(key.as_str()))
            {
                Some(i) => i,
                None => return Ok(null()),
            }
        }
        _ => match num1(idx) {
            Some(v) if v >= 1.0 && (v as usize) <= len(x) => v as usize - 1,
            _ => return Err("subscript out of bounds".into()),
        },
    };
    match data(x) {
        RData::List(v) => Ok(v.get(i).cloned().unwrap_or_else(null)),
        // `[[.factor` restores the level table and class just as `[.factor`
        // does, so `f[[2]]` is a length-1 factor, not a bare code. It has to be
        // rebuilt rather than taken from `element_at`, whose unboxed scalar
        // cannot carry attributes.
        _ if is_factor(x) => {
            let out = mk_int(vec![as_int(x).get(i).copied().flatten()]);
            carry_factor(&out, x);
            Ok(out)
        }
        _ => Ok(element_at(x, i)),
    }
}

/// `MARK_SHARED`: pin the value on top of the stack as permanently shared.
fn b_mark_shared(vm: &mut VM, _: u8) -> Value {
    let v = vm.peek().clone();
    with_host(|h| h.mark_shared(&v));
    vm.pop()
}

/// Whether `x` may be written into rather than rebuilt — R's `!MAYBE_SHARED`,
/// plus the references R does not need to count and rlang does.
///
/// R counts bindings, containers and the argument promise, and relies on the
/// promise to cover an argument that is mid-evaluation. rlang evaluates
/// arguments eagerly, so a value that has already been computed for the current
/// call sits on the fusevm operand stack instead of in a promise, and a
/// slot-bound top-level local (see `compiler::slot_locals`) sits in the VM's
/// `globals` rather than in an environment. Neither is reachable from the
/// reference count, so both are scanned here. Both are small — the operand
/// stack is a handful of pending operands, `globals` is one entry per name in
/// the chunk — and the scan only runs once the count has already agreed.
fn may_mutate(vm: &VM, x: &Value) -> bool {
    let Value::Obj(i) = x else { return false };
    if !with_host(|h| h.may_mutate(x)) {
        return false;
    }
    let same = |o: &Value| matches!(o, Value::Obj(j) if j == i);
    !vm.stack.iter().any(same)
        && !vm.globals.iter().any(same)
        && !vm.frames.iter().any(|f| f.slots.iter().any(same))
}

fn b_index_set(vm: &mut VM, _: u8) -> Value {
    let value = vm.pop();
    let argv = vm.pop();
    let x = vm.pop();
    if kind(&x) == RKind::RForeign {
        return foreign_index_set(vm, "[<-", x, args_of(&argv), value);
    }
    let inplace = may_mutate(vm, &x);
    match assign_index(&x, &args_of(&argv), &value, false, inplace) {
        Ok(v) => v,
        Err(e) => abort(vm, e),
    }
}

fn b_index2_set(vm: &mut VM, _: u8) -> Value {
    let value = vm.pop();
    let argv = vm.pop();
    let x = vm.pop();
    if kind(&x) == RKind::RForeign {
        return foreign_index_set(vm, "[[<-", x, args_of(&argv), value);
    }
    let inplace = may_mutate(vm, &x);
    match assign_index(&x, &args_of(&argv), &value, true, inplace) {
        Ok(v) => v,
        Err(e) => abort(vm, e),
    }
}

fn b_dollar_set(vm: &mut VM, _: u8) -> Value {
    let value = vm.pop();
    let name = name_of(&vm.pop());
    let x = vm.pop();
    if let RData::Environment(e) = data(&x) {
        with_host(|h| h.bind(&e, &name, value));
        return x;
    }
    if kind(&x) == RKind::RForeign {
        // `df$n <- v` is `df[["n"]] <- v` in R.
        return foreign_index_set(vm, "[[<-", x, vec![(None, scalar_str(name))], value);
    }
    let key = scalar_str(name);
    let args = vec![(None, key)];
    let inplace = may_mutate(vm, &x);
    match assign_index(&x, &args, &value, true, inplace) {
        Ok(v) => v,
        Err(e) => abort(vm, e),
    }
}

/// `x[i] <- v` and `x[[i]] <- v`. Both promote `x` to a type that can hold `v`,
/// grow it when the index is past the end, and (for lists) drop the element
/// when the value is NULL.
fn assign_index(
    x: &Value,
    args: &[(Option<String>, Value)],
    value: &Value,
    single_slot: bool,
    inplace: bool,
) -> Result<Value, String> {
    // `a[i, j, …] <- v`: turn the N-D selection into linear column-major
    // positions and reuse the 1-D path (which promotes type and preserves the
    // `dim` attribute through `copy_of`). Mirrors `array_index` for reads.
    if args.len() >= 2 {
        if let Some(dim) = with_host(|h| h.attr(x, "dim")) {
            let d: Vec<usize> = as_int(&dim)
                .iter()
                .map(|e| e.unwrap_or(0) as usize)
                .collect();
            if d.len() == args.len() {
                let (pos, _) = array_positions(args, &d, &dimnames_of(x))?;
                let lin: Vec<Option<i64>> =
                    pos.into_iter().map(|p| p.map(|i| i as i64 + 1)).collect();
                return assign_index(x, &[(None, mk_int(lin))], value, false, inplace);
            }
        }
    }
    let Some((_, idx)) = args.iter().find(|(_, v)| !matches!(v, Value::Undef)) else {
        return Ok(x.clone());
    };
    // `m[cbind(i, j)] <- v` writes one cell per coordinate row, mirroring the
    // read path. `[[` never takes a matrix subscript, so `single_slot` is out.
    if !single_slot && args.len() == 1 {
        if let Some(lin) = matrix_subscript(x, idx)? {
            return assign_index(x, &[(None, mk_int(lin))], value, false, inplace);
        }
    }
    let is_list = kind(x) == RKind::List
        || (single_slot && !is_null(value) && len(value) > 1)
        || matches!(kind(value), RKind::List | RKind::Closure | RKind::Builtin);
    let n = len(x);

    // The names are needed to resolve a character subscript, and again to
    // rebuild the vector — and nowhere else. Reading them up front cloned every
    // name on every `x[i] <- v`, which left an assignment loop over a *named*
    // vector quadratic even once the write itself was in place (65s at
    // n = 40000 against R's 0.16s), so each user takes them when it needs them.
    let mut names: Option<Vec<Option<String>>> = None;

    // Character index that names a new element appends it.
    let mut positions: Vec<usize> = Vec::new();
    let mut new_names: Vec<(usize, String)> = Vec::new();
    match data(idx) {
        RData::Str(keys) => {
            let known = names.insert(names_of(x));
            let mut next = n;
            for k in keys.iter().flatten() {
                match known
                    .iter()
                    .position(|nm| nm.as_deref() == Some(k.as_str()))
                {
                    Some(i) => positions.push(i),
                    None => {
                        positions.push(next);
                        new_names.push((next, k.clone()));
                        next += 1;
                    }
                }
            }
        }
        _ => {
            // Assigning past the end grows the vector, so resolve against the
            // larger of the current length and the highest index named.
            let highest = as_dbl(idx).iter().flatten().fold(0.0f64, |a, b| a.max(*b)) as usize;
            // A non-character subscript never consults the names, so this is
            // the branch that must not read them.
            for p in resolve_index(idx, n.max(highest), &[])? {
                match p {
                    Some(i) => positions.push(i),
                    None => return Err("NAs are not allowed in subscripted assignments".into()),
                }
            }
        }
    }

    // R's reference-count rule (`REFCNT`, real counting since R 4.0): a value
    // nothing else holds is written into, and only a shared one is copied
    // first. Everything below this point rebuilds the vector, which is what
    // made `for (i in 1:n) x[i] <- v` — R's most idiomatic loop — quadratic.
    //
    // The write is only equivalent to the rebuild when nothing about the
    // object's shape changes: no growth, no new name, and no type promotion.
    // Anything else falls through to the general path.
    //
    // R's `EnlargeVector` also grows an unshared vector in place, over-
    // committing its length by 5% so that appending one element at a time is
    // amortised rather than quadratic; a Rust `Vec` over-commits on push
    // already, so growing here is just letting `splice` extend it. It is
    // allowed only for a vector carrying no attributes, which is the shape the
    // un-preallocated loop has: `EnlargeVector` pads `names` and clears
    // `dim`/`dimnames` as it grows, and maintaining those in place needs the
    // same reference-count discipline for each attribute object that `x` itself
    // just went through. An attributed vector keeps the rebuild path.
    let grows = positions.iter().any(|p| *p >= n);
    let bare = with_host(|h| h.get(x).is_some_and(|o| o.attrs.is_empty()));
    if inplace && new_names.is_empty() && !positions.is_empty() && (!grows || bare) {
        let xk = kind(x);
        let vk = kind(value);
        if is_list
            && xk == RKind::List
            && single_slot
            && !is_null(value)
            && with_host(|h| h.set_list_elements(x, &positions, value)).is_some()
        {
            return Ok(x.clone());
        }
        if !is_list && vk.rank() <= xk.rank() {
            // The four atomic arms differ only in the vector type they coerce
            // the replacement to before splicing it in; `None` (a type the
            // in-place path does not handle) and `false` (the handle turned out
            // to hold something else) both leave the rebuild path to run.
            macro_rules! write_into {
                ($variant:ident, $coerce:expr) => {{
                    let s = $coerce;
                    with_host(|h| {
                        h.with_data_mut(x, |d| match d {
                            RData::$variant(v) => {
                                splice(v, &positions, &s, n);
                                true
                            }
                            _ => false,
                        })
                    })
                }};
            }
            let wrote = match xk {
                RKind::Lgl => write_into!(Lgl, as_lgl(value)),
                RKind::Int => write_into!(Int, as_int(value)),
                RKind::Dbl => write_into!(Dbl, as_dbl(value)),
                RKind::Str => write_into!(Str, as_str(value)),
                _ => None,
            };
            if wrote == Some(true) {
                return Ok(x.clone());
            }
        }
    }

    // Everything from here rebuilds the vector, and every rebuild carries the
    // names across, so this is where they are read if the subscript did not
    // already need them.
    let mut names = names.unwrap_or_else(|| names_of(x));

    if is_list {
        let mut items: Vec<Value> = match data(x) {
            RData::List(v) => v,
            RData::Null => Vec::new(),
            _ => elements(x),
        };
        // Assigning NULL into a list removes those elements.
        if is_null(value) && single_slot {
            let mut sorted = positions.clone();
            sorted.sort_unstable();
            for p in sorted.into_iter().rev() {
                if p < items.len() {
                    items.remove(p);
                    if p < names.len() {
                        names.remove(p);
                    }
                }
            }
            let out = mk_list(items);
            if !names.is_empty() {
                set_names(&out, names);
            }
            return Ok(out);
        }
        let vals: Vec<Value> = if single_slot {
            vec![value.clone()]
        } else {
            elements(value)
        };
        for (k, p) in positions.iter().enumerate() {
            while items.len() <= *p {
                items.push(null());
                names.push(blank());
            }
            items[*p] = vals[k % vals.len().max(1)].clone();
        }
        for (i, nm) in new_names {
            while names.len() <= i {
                names.push(blank());
            }
            names[i] = Some(nm);
        }
        let grew = items.len() > n;
        let out = mk_list(items);
        if !names.is_empty() {
            set_names(&out, names.clone());
        }
        copy_attrs(x, &out, grew);
        return Ok(out);
    }

    // Atomic assignment: promote to the wider of the two types.
    let rank = with_host(|h| h.kind_of(x).rank().max(h.kind_of(value).rank()));
    let grow = positions
        .iter()
        .copied()
        .max()
        .map(|m| m + 1)
        .unwrap_or(n)
        .max(n);
    let out = match rank {
        1 => {
            let mut v = as_lgl(x);
            let s = as_lgl(value);
            splice(&mut v, &positions, &s, grow);
            mk_lgl(v)
        }
        2 => {
            let mut v = as_int(x);
            let s = as_int(value);
            splice(&mut v, &positions, &s, grow);
            mk_int(v)
        }
        4 => {
            let mut v = as_str(x);
            let s = as_str(value);
            splice(&mut v, &positions, &s, grow);
            mk_str(v)
        }
        _ => {
            let mut v = as_dbl(x);
            let s = as_dbl(value);
            splice(&mut v, &positions, &s, grow);
            mk_dbl(v)
        }
    };
    for (i, nm) in new_names {
        while names.len() <= i {
            names.push(blank());
        }
        names[i] = Some(nm);
    }
    if !names.is_empty() {
        while names.len() < grow {
            names.push(blank());
        }
        set_names(&out, names);
    }
    copy_attrs(x, &out, grow > n);
    Ok(out)
}

/// The name R's `EnlargeNames` gives a slot the assignment skipped past:
/// `R_BlankString`, not `NA`. `x <- c(a = 1); x[3] <- 9` names the gap `""`.
fn blank() -> Option<String> {
    Some(String::new())
}

/// Carry `x`'s attributes onto the vector an assignment rebuilt. `names` is
/// always set separately by the caller, and `dim`/`dimnames` are dropped when
/// the assignment grew the vector — R's `EnlargeVector` clears both, because a
/// vector one element longer than its own `dim` says is no longer that array.
fn copy_attrs(x: &Value, out: &Value, grew: bool) {
    for (k, v) in with_host(|h| h.attrs_of(x)) {
        let dropped = k == "names" || (grew && (k == "dim" || k == "dimnames"));
        if !dropped {
            with_host(|h| h.set_attr(out, &k, v));
        }
    }
}

/// Write `src` (recycled) into `dst` at `positions`, growing `dst` to `grow`.
fn splice<T: Clone>(dst: &mut Vec<Option<T>>, positions: &[usize], src: &[Option<T>], grow: usize) {
    while dst.len() < grow {
        dst.push(None);
    }
    if src.is_empty() {
        return;
    }
    for (k, p) in positions.iter().enumerate() {
        while dst.len() <= *p {
            dst.push(None);
        }
        dst[*p] = src[k % src.len()].clone();
    }
}

/// `f(x, extra) <- value` — the replacement functions.
fn b_replace(vm: &mut VM, _: u8) -> Value {
    let value = vm.pop();
    let argv = vm.pop();
    let x = vm.pop();
    let fname = name_of(&vm.pop());
    let extra = args_of(&argv);
    match replacement(&fname, &x, &extra, &value) {
        Ok(v) => v,
        Err(e) => abort(vm, e),
    }
}

fn replacement(
    fname: &str,
    x: &Value,
    extra: &[(Option<String>, Value)],
    value: &Value,
) -> Result<Value, String> {
    let out = copy_of(x);
    match fname {
        "names" => {
            let nm = if is_null(value) {
                null()
            } else {
                mk_str(as_str(value))
            };
            with_host(|h| h.set_attr(&out, "names", nm));
            Ok(out)
        }
        "class" => {
            with_host(|h| h.set_attr(&out, "class", value.clone()));
            Ok(out)
        }
        "dim" => {
            let d = mk_int(as_int(value));
            with_host(|h| h.set_attr(&out, "dim", d));
            Ok(out)
        }
        "attr" => {
            let key = extra
                .iter()
                .find(|(n, _)| n.as_deref() == Some("which"))
                .or_else(|| extra.first())
                .map(|(_, v)| str1(v).unwrap_or_default())
                .unwrap_or_default();
            with_host(|h| h.set_attr(&out, &key, value.clone()));
            Ok(out)
        }
        "length" => {
            let want = num1(value).unwrap_or(0.0) as usize;
            let pos: Vec<Option<usize>> = (0..want).map(|i| (i < len(x)).then_some(i)).collect();
            Ok(take_positions(x, &pos))
        }
        "levels" => {
            with_host(|h| h.set_attr(&out, "levels", mk_str(as_str(value))));
            Ok(out)
        }
        "dimnames" => {
            // Each element is a character vector or `NULL` (that margin has no
            // labels); a `NULL` list drops the attribute entirely.
            let dn = if is_null(value) {
                null()
            } else {
                mk_list(
                    elements(value)
                        .iter()
                        .map(|e| {
                            if is_null(e) {
                                null()
                            } else {
                                mk_str(as_str(e))
                            }
                        })
                        .collect(),
                )
            };
            with_host(|h| h.set_attr(&out, "dimnames", dn));
            Ok(out)
        }
        // `rownames(x) <- v` / `colnames(x) <- v` rewrite one margin of
        // `dimnames` and leave the others as they were.
        "rownames" | "colnames" => {
            let idx = usize::from(fname == "colnames");
            let ndim = with_host(|h| h.attr(&out, "dim"))
                .map(|d| len(&d))
                .unwrap_or(2)
                .max(idx + 1);
            let old = dimnames_of(&out);
            let mut margins: Vec<Value> = (0..ndim)
                .map(|k| match old.get(k).cloned().flatten() {
                    Some(names) => mk_str(names),
                    None => null(),
                })
                .collect();
            margins[idx] = if is_null(value) {
                null()
            } else {
                mk_str(as_str(value))
            };
            let dn = if margins.iter().all(is_null) {
                null()
            } else {
                mk_list(margins)
            };
            with_host(|h| h.set_attr(&out, "dimnames", dn));
            Ok(out)
        }
        "substr" => {
            // `substr(x, start, stop) <- value`: overwrite chars start..=stop in
            // place, taking at most `stop-start+1` characters from `value` and
            // never changing the string's length.
            let start = extra.first().and_then(|(_, v)| num1(v)).unwrap_or(1.0) as usize;
            let stop = extra.get(1).and_then(|(_, v)| num1(v)).unwrap_or(1e6) as usize;
            let vals = as_str(value);
            Ok(mk_str(
                as_str(x)
                    .iter()
                    .enumerate()
                    .map(|(i, s)| {
                        s.as_ref().map(|s| {
                            let repl: Vec<char> = vals
                                .get(i % vals.len().max(1))
                                .cloned()
                                .flatten()
                                .unwrap_or_default()
                                .chars()
                                .collect();
                            let mut chars: Vec<char> = s.chars().collect();
                            let span = stop.saturating_sub(start.saturating_sub(1));
                            for (j, rc) in repl.iter().take(span).enumerate() {
                                let pos = start.saturating_sub(1) + j;
                                if pos < chars.len() {
                                    chars[pos] = *rc;
                                }
                            }
                            chars.into_iter().collect()
                        })
                    })
                    .collect(),
            ))
        }
        // A user-defined replacement function: `\`f<-\`(x, ..., value)`.
        other => {
            let fq = format!("{other}<-");
            let f = with_host(|h| h.lookup_function(&fq))
                .ok_or_else(|| format!("could not find function \"{fq}\""))?;
            let mut args: Vec<(Option<String>, Value)> = vec![(None, x.clone())];
            args.extend(extra.iter().cloned());
            args.push((Some("value".into()), value.clone()));
            call_value(&f, args, Some(fq))
        }
    }
}

/// A fresh heap object with the same data and attributes — R's copy-on-modify.
fn copy_of(x: &Value) -> Value {
    with_host(|h| {
        let d = h.data_of(x);
        let a = h.attrs_of(x);
        h.alloc_with(d, a)
    })
}

// ===========================================================================
// The primitive function library.
// ===========================================================================

/// Whether `name` is one of the primitives implemented in Rust. Operators
/// count: in R they are ordinary functions, which is what lets
/// ``Reduce(`+`, 1:4)`` and ``sapply(xs, `[`, 1)`` work.
pub fn is_primitive(name: &str) -> bool {
    PRIMITIVE_SET.with(|set| set.contains(name))
}

thread_local! {
    /// `PRIMITIVES` + `OPERATORS` as a set, built once.
    ///
    /// Every call to a function the user has not bound reaches here — `nchar(x)`
    /// in a loop asks whether `nchar` is a primitive on each iteration — and
    /// scanning the two slices compared the name against up to 278 strings each
    /// time. The tables stay slices because they are also the LSP's completion
    /// corpus and the docs' source of truth, which want the order.
    static PRIMITIVE_SET: std::collections::HashSet<&'static str, rustc_hash::FxBuildHasher> =
        PRIMITIVES.iter().chain(OPERATORS).copied().collect();
}

/// The operators reachable as functions through their backtick names.
pub const OPERATORS: &[&str] = &[
    "+", "-", "*", "/", "^", "%%", "%/%", "==", "!=", "<", ">", "<=", ">=", "&", "|", "!", ":",
    "[", "[[", "$",
];

/// Every primitive rlang implements; also the corpus the LSP completes from.
pub const PRIMITIVES: &[&str] = &[
    "c",
    "length",
    "lengths",
    "names",
    "attr",
    "attributes",
    "class",
    "inherits",
    "unclass",
    "structure",
    "print",
    "cat",
    "paste",
    "paste0",
    "format",
    "formatC",
    "prettyNum",
    "sprintf",
    "message",
    "warning",
    "stop",
    "invisible",
    "identity",
    "seq",
    "seq.int",
    "rep_len",
    "unname",
    "all.equal",
    "seq_len",
    "seq_along",
    "rep",
    "rev",
    "sort",
    "order",
    "unique",
    "which",
    "which.max",
    "which.min",
    "match",
    "is.element",
    "duplicated",
    "rank",
    "sort.list",
    "xtfrm",
    "any",
    "all",
    "xor",
    "sum",
    "prod",
    "mean",
    "median",
    "quantile",
    "cor",
    "rle",
    "inverse.rle",
    "var",
    "sd",
    "min",
    "max",
    "range",
    "abs",
    "sqrt",
    "exp",
    "log",
    "log2",
    "log10",
    "floor",
    "ceiling",
    "round",
    "signif",
    "trunc",
    "sign",
    "sin",
    "cos",
    "tan",
    "asin",
    "acos",
    "atan",
    "atan2",
    "sinh",
    "cosh",
    "tanh",
    "expm1",
    "log1p",
    "gamma",
    "lgamma",
    "factorial",
    "lfactorial",
    "choose",
    "beta",
    "lbeta",
    "cumsum",
    "cumprod",
    "cummax",
    "cummin",
    "diff",
    "pmax",
    "pmin",
    "tabulate",
    "findInterval",
    "is.null",
    "is.na",
    "is.nan",
    "is.finite",
    "is.infinite",
    "anyNA",
    "complete.cases",
    "is.double",
    "is.integer",
    "is.numeric",
    "is.character",
    "is.logical",
    "is.function",
    "is.list",
    "is.vector",
    "as.numeric",
    "as.double",
    "as.integer",
    "as.character",
    "as.logical",
    "as.vector",
    "as.list",
    "list",
    "unlist",
    "lapply",
    "sapply",
    "vapply",
    "Map",
    "mapply",
    "Reduce",
    "Filter",
    "Find",
    "Position",
    "split",
    "tapply",
    "modifyList",
    "rapply",
    "do.call",
    "nchar",
    "substr",
    "substring",
    "strtrim",
    "utf8ToInt",
    "intToUtf8",
    "toupper",
    "tolower",
    "casefold",
    "chartr",
    "strtoi",
    "strrep",
    "encodeString",
    "strsplit",
    "sub",
    "gsub",
    "grepl",
    "grep",
    "regexpr",
    "gregexpr",
    "regmatches",
    "trimws",
    "startsWith",
    "endsWith",
    "matrix",
    "array",
    "aperm",
    "dim",
    "nrow",
    "ncol",
    "t",
    "rowSums",
    "colSums",
    "rowMeans",
    "colMeans",
    "apply",
    "diag",
    "%*%",
    "%o%",
    "outer",
    "crossprod",
    "tcrossprod",
    "cbind",
    "rbind",
    "head",
    "tail",
    "append",
    "setdiff",
    "union",
    "intersect",
    "identical",
    "isTRUE",
    "isFALSE",
    "ifelse",
    "stopifnot",
    "numeric",
    "character",
    "logical",
    "integer",
    "vector",
    "setNames",
    "exists",
    "get",
    "assign",
    "environment",
    "new.env",
    "missing",
    "return",
    "UseMethod",
    "NextMethod",
    "tryCatch",
    "withCallingHandlers",
    "try",
    "on.exit",
    "conditionMessage",
    "conditionCall",
    "simpleError",
    "simpleWarning",
    "simpleMessage",
    "simpleCondition",
    "signalCondition",
    "withRestarts",
    "invokeRestart",
    "computeRestarts",
    "restartDescription",
    "isRestart",
    "factor",
    "levels",
    "nlevels",
    "droplevels",
    "cut",
    "table",
    "typeof",
    "mode",
    "storage.mode",
    "bitwAnd",
    "bitwOr",
    "bitwXor",
    "bitwNot",
    "bitwShiftL",
    "bitwShiftR",
    "Recall",
    "Negate",
    "Vectorize",
    "toString",
    "deparse",
    "rownames",
    "colnames",
    "dimnames",
    "options",
    "getOption",
    // Inline-Rust FFI bridge (src/ffi.rs): register a `rust {}` block, then call
    // its exports through R's own native-call verb.
    ".rust",
    ".Call",
    // CRAN bridge: package loaders delegate to an embedded GNU R (src/rembed.rs).
    "library",
    "require",
    "requireNamespace",
    "loadNamespace",
    "suppressMessages",
    "suppressWarnings",
    "suppressPackageStartupMessages",
    ".rlang_formula",
    ".rlang_quote",
    "quote",
    "bquote",
    "as.name",
    "as.symbol",
    "sys.call",
    "match.call",
    "sys.function",
    "eval",
    "evalq",
    ".rlang_substitute",
    "substitute",
    "is.call",
    "is.name",
    "is.symbol",
];

/// Invoke a runtime-constructed function ([`RData::Combinator`]): `Negate`
/// negates the wrapped function's logical result; `Vectorize` applies it
/// elementwise over the recycled arguments and simplifies.
pub fn call_combinator(
    kind: CombinatorKind,
    inner: &Value,
    args: Vec<(Option<String>, Value)>,
) -> Result<Value, String> {
    match kind {
        CombinatorKind::Negate => {
            let r = call_value(inner, args, None)?;
            Ok(mk_lgl(as_lgl(&r).iter().map(|e| e.map(|b| !b)).collect()))
        }
        CombinatorKind::Vectorize => {
            let lists: Vec<Vec<Value>> = args.iter().map(|(_, v)| elements(v)).collect();
            let n = lists.iter().map(|l| l.len()).max().unwrap_or(0);
            let mut out = Vec::with_capacity(n);
            for i in 0..n {
                let call_args: Vec<(Option<String>, Value)> = lists
                    .iter()
                    .map(|l| (None, l[i % l.len().max(1)].clone()))
                    .collect();
                out.push(call_value(inner, call_args, None)?);
            }
            Ok(simplify(&mk_list(out)))
        }
    }
}

/// Call a primitive by name with evaluated arguments.
pub fn call_primitive(name: &str, args: Vec<(Option<String>, Value)>) -> Result<Value, String> {
    if OPERATORS.contains(&name) {
        return call_operator(name, &args);
    }
    // A foreign R object (data frame, S4, raw, …) "infects" the call: rlang's
    // native builtins can't operate on an opaque handle, so the whole call is
    // delegated to embedded R, which does understand it.
    if args.iter().any(|(_, v)| kind(v) == RKind::RForeign) {
        return cran_call(name, &args);
    }
    // A primitive R treats as generic hands off to a user's S3 method before
    // running its own implementation, so `print.myclass` wins over the default
    // layout the way it does in R.
    if !with_host(|h| std::mem::take(&mut h.suppress_s3)) {
        if let Some(res) = s3_primitive_method(name, &args) {
            return res;
        }
    }
    let a = Args::new(args);
    match name {
        // ── construction and coercion ───────────────────────────────────
        "c" => Ok(concat(&a)),
        "list" => {
            let out = mk_list(a.values());
            let nm = a.tags();
            if nm.iter().any(|n| n.is_some()) {
                // R pads the untagged slots of a partially named list with the
                // *empty string*, not `NA` — `names(list(a = 1, 2))` is
                // `c("a", "")`. The distinction is visible when printing, where
                // an empty name numbers its element `[[2]]` but an `NA` name
                // heads it `$<NA>`.
                set_names(
                    &out,
                    nm.into_iter()
                        .map(|n| Some(n.unwrap_or_default()))
                        .collect(),
                );
            }
            Ok(out)
        }
        "vector" => {
            let mode = a
                .get(0, "mode")
                .and_then(|v| str1(&v))
                .unwrap_or_else(|| "logical".into());
            let n = a.get(1, "length").and_then(|v| num1(&v)).unwrap_or(0.0) as usize;
            Ok(empty_vector(&mode, n))
        }
        "numeric" | "double" => Ok(mk_dbl(vec![Some(0.0); a.n(0, 0.0) as usize])),
        "integer" => Ok(mk_int(vec![Some(0); a.n(0, 0.0) as usize])),
        "character" => Ok(mk_str(vec![Some(String::new()); a.n(0, 0.0) as usize])),
        "logical" => Ok(mk_lgl(vec![Some(false); a.n(0, 0.0) as usize])),
        "as.numeric" | "as.double" => {
            let x = a.req(0, "x")?;
            let out = as_dbl(&x);
            warn_coerced_na(&x, out.iter().map(|e| e.is_none()))?;
            Ok(mk_dbl(out))
        }
        "as.integer" => {
            let x = a.req(0, "x")?;
            let out = int_range_na(as_int(&x));
            warn_coerced_na(&x, out.iter().map(|e| e.is_none()))?;
            Ok(mk_int(out))
        }
        "as.character" => {
            let x = a.req(0, "x")?;
            // A name is its own text; a call is its parts, each deparsed —
            // `as.character(quote(f(1)))` is `c("f", "1")`.
            if let Some(e) = lang_of(&x) {
                return Ok(mk_str(lang_text(&e).into_iter().map(Some).collect()));
            }
            // `as.character(factor)` yields the level labels, not the codes.
            if class_of(&x).iter().any(|c| c == "factor") {
                let levels = with_host(|h| h.attr(&x, "levels"))
                    .map(|l| as_str(&l))
                    .unwrap_or_default();
                return Ok(mk_str(
                    as_int(&x)
                        .iter()
                        .map(|c| c.and_then(|i| levels.get((i - 1) as usize).cloned().flatten()))
                        .collect(),
                ));
            }
            Ok(mk_str(crate::host::with_print_digits(
                crate::host::AS_CHARACTER_DIGITS,
                || as_str(&x),
            )))
        }
        "as.logical" => Ok(mk_lgl(as_lgl(&a.req(0, "x")?))),
        "as.vector" => {
            let x = a.req(0, "x")?;
            // `as.vector.factor` is the exception to the rule below: dropping a
            // factor's attributes would leave the codes, so R hands back the
            // labels instead.
            if is_factor(&x) {
                return Ok(mk_str(factor_labels(&x)));
            }
            // `mode =` is a coercion, not a hint: `as.vector(2^31, "integer")` is
            // the same coercion as `as.integer(2^31)` and must narrow to NA the
            // same way. Ignoring it returned the argument untouched.
            match a.get(1, "mode").and_then(|v| str1(&v)).as_deref() {
                Some("integer") => return Ok(mk_int(int_range_na(as_int(&x)))),
                Some("double") | Some("numeric") => return Ok(mk_dbl(as_dbl(&x))),
                Some("character") => return Ok(mk_str(as_str(&x))),
                Some("logical") => return Ok(mk_lgl(as_lgl(&x))),
                Some("list") => return Ok(mk_list(elements(&x))),
                _ => {}
            }
            // `as.vector` drops attributes (names, dim, class, levels), so a
            // `table` collapses to its plain integer counts.
            let out = copy_of(&x);
            with_host(|h| {
                let nl = h.null();
                for attr in ["names", "dim", "class", "levels", "dimnames"] {
                    h.set_attr(&out, attr, nl.clone());
                }
            });
            Ok(out)
        }
        "as.list" => {
            let x = a.req(0, "x")?;
            // A call becomes the list of its parts, tagged where an argument
            // was named — `as.list(quote(f(a = 1)))` keeps the `a`.
            if let RData::Lang(e) = data(&x) {
                let parts = lang_parts(&e);
                let out = mk_list(parts.iter().map(|(_, v)| v.clone()).collect());
                // An untagged argument becomes an *empty* name, not a missing
                // one: R heads it `[[i]]`, where an `NA` name heads `$<NA>`.
                if parts.iter().any(|(t, _)| t.is_some()) {
                    let nm = parts
                        .iter()
                        .map(|(t, _)| Some(t.clone().unwrap_or_default()))
                        .collect();
                    set_names(&out, nm);
                }
                return Ok(out);
            }
            let out = mk_list(elements(&x));
            let nm = names_of(&x);
            if !nm.is_empty() {
                set_names(&out, nm);
            }
            Ok(out)
        }
        "unlist" => Ok(unlist(&a.req(0, "x")?)),

        // ── attributes and metadata ─────────────────────────────────────
        "length" => Ok(scalar_int(len(&a.req(0, "x")?) as i64)),
        "lengths" => {
            let x = a.req(0, "x")?;
            let nm = names_of(&x);
            let out = mk_int(elements(&x).iter().map(|e| Some(len(e) as i64)).collect());
            if !nm.is_empty() {
                set_names(&out, nm);
            }
            Ok(out)
        }
        "names" => {
            let x = a.req(0, "x")?;
            let nm = names_of(&x);
            // A zero-length vector can still *carry* a names attribute, and R
            // reports it as `character(0)` rather than `NULL`; `names_of`
            // flattens that case into the same empty vec as "no names at all",
            // so the attribute itself decides between the two.
            if nm.is_empty() {
                let has = with_host(|h| h.attr(&x, "names")).is_some_and(|n| !is_null(&n));
                return Ok(if has { mk_str(vec![]) } else { null() });
            }
            Ok(mk_str(nm))
        }
        "setNames" => {
            let x = copy_of(&a.req(0, "object")?);
            let nm = a.req(1, "nm")?;
            set_names(&x, as_str(&nm));
            Ok(x)
        }
        "attr" => {
            let x = a.req(0, "x")?;
            let which = a.get(1, "which").and_then(|v| str1(&v)).unwrap_or_default();
            Ok(with_host(|h| h.attr(&x, &which)).unwrap_or_else(null))
        }
        "attributes" => {
            let x = a.req(0, "x")?;
            let attrs = with_host(|h| h.attrs_of(&x));
            if attrs.is_empty() {
                return Ok(null());
            }
            let out = mk_list(attrs.values().cloned().collect());
            set_names(&out, attrs.keys().map(|k| Some(k.clone())).collect());
            Ok(out)
        }
        "class" => Ok(mk_str(
            class_of(&a.req(0, "x")?).into_iter().map(Some).collect(),
        )),
        "inherits" => {
            let x = a.req(0, "x")?;
            let what: Vec<String> = as_str(&a.req(1, "what")?).into_iter().flatten().collect();
            let cls = class_of(&x);
            Ok(scalar_lgl(what.iter().any(|w| cls.contains(w))))
        }
        "unclass" => {
            let out = copy_of(&a.req(0, "x")?);
            let nl = null();
            with_host(|h| h.set_attr(&out, "class", nl));
            Ok(out)
        }
        "structure" => {
            let out = copy_of(&a.req(0, ".Data")?);
            for (tag, v) in a.rest(1) {
                if let Some(t) = tag {
                    let key = if t == ".Names" {
                        "names".to_string()
                    } else {
                        t
                    };
                    with_host(|h| h.set_attr(&out, &key, v));
                }
            }
            Ok(out)
        }
        "typeof" => {
            let x = a.req(0, "x")?;
            Ok(scalar_str(with_host(|h| h.type_of(&x))))
        }
        "mode" => {
            let x = a.req(0, "x")?;
            let t = with_host(|h| h.type_of(&x));
            Ok(scalar_str(match t {
                "integer" | "double" => "numeric",
                "closure" | "builtin" => "function",
                // R's `mode` names an unevaluated expression by what it is
                // rather than by its internal type.
                "language" => "call",
                "symbol" => "name",
                other => other,
            }))
        }
        "storage.mode" => {
            let x = a.req(0, "x")?;
            Ok(scalar_str(with_host(|h| h.type_of(&x))))
        }
        "dim" => {
            let x = a.req(0, "x")?;
            Ok(with_host(|h| h.attr(&x, "dim")).unwrap_or_else(null))
        }
        "nrow" | "ncol" => {
            let x = a.req(0, "x")?;
            let d = with_host(|h| h.attr(&x, "dim"))
                .map(|d| as_int(&d))
                .unwrap_or_default();
            let i = usize::from(name == "ncol");
            Ok(match d.get(i) {
                Some(Some(n)) => scalar_int(*n),
                _ => null(),
            })
        }
        "rownames" | "colnames" => {
            let x = a.req(0, "x")?;
            let idx = if name == "rownames" { 0 } else { 1 };
            match dimnames_of(&x).get(idx) {
                Some(Some(names)) => Ok(mk_str(names.clone())),
                _ => Ok(null()),
            }
        }
        "dimnames" => {
            let x = a.req(0, "x")?;
            Ok(with_host(|h| h.attr(&x, "dimnames")).unwrap_or_else(null))
        }

        // ── output ──────────────────────────────────────────────────────
        // ── inline-Rust FFI ──────────────────────────────────────────────
        ".rust" => {
            let code = a.req(0, "code")?;
            let src = str1(&code)
                .ok_or_else(|| "`.rust` expects a character string of Rust source".to_string())?;
            crate::ffi::register(&src)?;
            with_host(|h| h.visible = false);
            Ok(null())
        }
        ".Call" => {
            let name_v = a.req(0, ".NAME")?;
            let routine = str1(&name_v).ok_or_else(|| {
                "`.Call` expects a routine name as its first argument".to_string()
            })?;
            let mut fargs: Vec<Value> = Vec::new();
            for v in a.values().iter().skip(1) {
                fargs.push(r_to_fusevm(v)?);
            }
            let out = crate::ffi::call(&routine, &fargs)?;
            Ok(fusevm_to_r(out))
        }

        "print" => {
            let x = a.req(0, "x")?;
            // `print(x, digits = n)` overrides the significant-digit setting for
            // this one call, then restores the prior value.
            let restore = a
                .named("digits")
                .and_then(|v| num1(&v))
                .map(|d| crate::host::set_print_digits(d as usize));
            print_value(&x);
            if let Some(prev) = restore {
                crate::host::set_print_digits(prev);
            }
            with_host(|h| h.visible = false);
            Ok(x)
        }
        "cat" => {
            let sep = a
                .named("sep")
                .and_then(|v| str1(&v))
                .unwrap_or_else(|| " ".into());
            // R's `do_cat` walks the `...` objects, not a flattened element
            // list: a separator goes before every non-empty object after the
            // first, and between the elements within one. The two are not the
            // same — a leading zero-length argument still earns its successor a
            // separator, so `cat(NULL, "x")` prints " x", while
            // `cat("a", NULL, "b")` prints "a b" and not "a  b".
            let objs: Vec<&Value> = a
                .all
                .iter()
                .filter(|(t, _)| !CAT_CONTROL_ARGS.contains(&t.as_deref().unwrap_or("")))
                .map(|(_, v)| v)
                .collect();
            let mut out = String::new();
            for (i, v) in objs.iter().enumerate() {
                let n = len(v);
                if i != 0 && n > 0 {
                    out.push_str(&sep);
                }
                if n == 0 {
                    continue;
                }
                // A list or a function has no `cat` representation. R rejects it
                // *after* writing everything up to that point, so flush first.
                if let Some(kind) = uncatable(v) {
                    crate::host::emit(&out);
                    return Err(format!(
                        "argument {} (type '{kind}') cannot be handled by 'cat'",
                        i + 1
                    ));
                }
                for (k, s) in as_str(v).into_iter().enumerate() {
                    out.push_str(&s.unwrap_or_else(|| "NA".into()));
                    if k + 1 < n {
                        out.push_str(&sep);
                    }
                }
            }
            // R ends `cat` output with a newline whenever the separator itself
            // contains one — `cat(c("a", "b"), sep = "\n")` prints three lines'
            // worth of output, not two.
            let tail = if sep.contains('\n') { "\n" } else { "" };
            crate::host::emit(&format!("{out}{tail}"));
            with_host(|h| h.visible = false);
            Ok(null())
        }
        "message" | "warning" => {
            let text: Vec<String> = a.values().iter().flat_map(as_str).flatten().collect();
            // R's `message` appends a newline to the condition's message; a
            // warning's does not.
            let text = text.join("") + if name == "message" { "\n" } else { "" };
            let warn = name == "warning";
            let classes: Vec<String> = if warn {
                ["simpleWarning", "warning", "condition"]
            } else {
                ["simpleMessage", "message", "condition"]
            }
            .iter()
            .map(|s| s.to_string())
            .collect();
            // R establishes `muffleWarning` / `muffleMessage` around the signal,
            // so a calling handler can suppress the default action and let
            // evaluation resume from here.
            let muffle = if warn {
                "muffleWarning"
            } else {
                "muffleMessage"
            };
            // `warning()` and `message()` are closures in R, so `findCall`
            // starts one context out and the condition names their caller.
            let raised_in = with_host(|h| h.enclosing_call_source());
            match signal_condition_with_muffle(&text, &classes, muffle, raised_in.clone())? {
                // A `tryCatch` is waiting: raise it so the unwind reaches there.
                Signalled::Unwind => {
                    raise_condition(text, classes, raised_in);
                    return Ok(null());
                }
                // Nothing took it — R's default action is to report and carry on.
                Signalled::Fell => {
                    if warn {
                        user_warning(&text);
                    } else {
                        eprint!("{text}");
                    }
                }
                Signalled::Muffled => {}
            }
            // `warning()` returns its message, `message()` returns NULL; both
            // invisibly.
            let out = if warn { scalar_str(&text) } else { null() };
            with_host(|h| h.visible = false);
            Ok(out)
        }
        "stop" => {
            let text: Vec<String> = a.values().iter().flat_map(as_str).flatten().collect();
            // `stop(cond)` re-signals an existing condition object rather than
            // building a message out of it.
            let cond = a
                .get(0, "message")
                .filter(|v| class_of(v).iter().any(|c| c == "condition"));
            let (text, classes) = match &cond {
                Some(c) => (
                    element_field(c, "message")
                        .and_then(|m| str1(&m))
                        .unwrap_or_default(),
                    class_of(c),
                ),
                None => (
                    text.join(""),
                    ["simpleError", "error", "condition"]
                        .iter()
                        .map(|s| s.to_string())
                        .collect(),
                ),
            };
            // An error has no restart of its own, but calling handlers still see
            // it before the stack comes down — and one of them may transfer to a
            // restart established further out, which is not an error at all.
            let obj = match cond {
                Some(c) => c,
                None => mk_condition(
                    &text,
                    &classes.iter().map(String::as_str).collect::<Vec<_>>(),
                    with_host(|h| h.enclosing_call_source()),
                ),
            };
            signal_to_handlers(&obj, &classes)?;
            with_host(|h| {
                // R's `do_stop` calls `findCall`, which starts one context out:
                // `stop()` is itself a closure, so its own frame is skipped and
                // the error names the function that called it.
                let call = h.enclosing_call_source();
                h.set_error_call(call);
                h.error_classes = classes;
            });
            Err(text)
        }
        "stopifnot" => {
            // R names the *expression* that failed, deparsed out of its own
            // call — which is why the arguments come off the context stack here
            // rather than being carried alongside their values.
            let written = with_host(|h| h.current_call_source())
                .and_then(|src| call_arg_sources(&src))
                .unwrap_or_default();
            for (i, (tag, v)) in a.all.iter().enumerate() {
                let flags = as_lgl(v);
                // An empty vector has no FALSE in it, so it passes — R's
                // `stopifnot(logical(0))` is silent.
                if flags.iter().all(|e| *e == Some(true)) {
                    continue;
                }
                // R's `stopifnot` calls `stop` from inside itself, so the error
                // names *its* caller, not the `stopifnot(…)` call.
                with_host(|h| {
                    let c = h.enclosing_call_source();
                    h.set_error_call(c);
                });
                // `stopifnot("must be big" = x > 5)` uses the tag as the whole
                // message.
                if let Some(t) = tag {
                    return Err(t.clone());
                }
                let what = written.get(i).cloned().unwrap_or_default();
                let tail = match flags.len() > 1 {
                    true => "are not all TRUE",
                    false => "is not TRUE",
                };
                return Err(format!("{what} {tail}"));
            }
            with_host(|h| h.visible = false);
            Ok(null())
        }
        "invisible" => {
            let v = a.get(0, "x").unwrap_or_else(null);
            with_host(|h| h.visible = false);
            Ok(v)
        }
        "identity" => a.req(0, "x"),
        // `paste`, `toString` and `as.character` all coerce doubles at a fixed
        // 15 significant digits, NOT at `getOption("digits")` — which is why
        // `paste(pi)` stays "3.14159265358979" even under `options(digits = 3)`,
        // while `cat(pi)` and `format(pi)` follow the setting. `scipen` still
        // applies to them, so only the digit count is pinned here. Verified
        // against R 4.6.1 over 7120 comparisons (`digits` in {1,3,7,15,22} x
        // `scipen` in {-9,-5,-1,0,1,5,10,30} x 178 values): `as.character(x)` is
        // exactly `format(x)` at `digits = 15`, and `paste`/`toString` agree
        // with `as.character` on every one.
        "paste" | "paste0" => Ok(crate::host::with_print_digits(
            crate::host::AS_CHARACTER_DIGITS,
            || paste(&a, name == "paste0"),
        )),
        "toString" => {
            let x = a.req(0, "x")?;
            let parts: Vec<String> =
                crate::host::with_print_digits(crate::host::AS_CHARACTER_DIGITS, || {
                    as_str_labels(&x)
                })
                .into_iter()
                .map(|s| s.unwrap_or_else(|| "NA".into()))
                .collect();
            Ok(scalar_str(parts.join(", ")))
        }
        // `options(name = value, ...)` sets, `options("name")` queries, and
        // `options(old)` restores from a previously returned list. Either way
        // the result is a named list of the values as they were *before* the
        // call, in argument order, which is what makes the
        // `old <- options(...)`; `options(old)` round trip work. R returns it
        // invisibly when anything was set and visibly for a pure query.
        "options" => {
            let mut names: Vec<Option<String>> = Vec::new();
            let mut olds: Vec<Value> = Vec::new();
            let mut any_set = false;
            let mut record = |name: &str, old: Option<crate::host::OptionValue>| {
                names.push(Some(name.to_string()));
                olds.push(option_value_to_r(old));
            };
            for (tag, v) in a.all.iter() {
                match tag {
                    Some(name) => {
                        any_set = true;
                        let (prev, warning) = crate::host::option_set(name, r_to_option_value(v)?)?;
                        record(name, prev);
                        if let Some(w) = warning {
                            r_warning(&w, with_host(|h| h.current_call()));
                        }
                    }
                    // An untagged list is a restore: every named element is set.
                    None if kind(v) == RKind::List => {
                        let elems = match data(v) {
                            RData::List(xs) => xs,
                            _ => unreachable!(),
                        };
                        let tags = with_host(|h| h.attr(v, "names"))
                            .map(|n| as_str(&n))
                            .unwrap_or_default();
                        for (i, e) in elems.iter().enumerate() {
                            let Some(Some(name)) = tags.get(i) else {
                                return Err("invalid argument to 'options'".into());
                            };
                            any_set = true;
                            let (prev, warning) =
                                crate::host::option_set(name, r_to_option_value(e)?)?;
                            record(name, prev);
                            if let Some(w) = warning {
                                r_warning(&w, with_host(|h| h.current_call()));
                            }
                        }
                    }
                    // An untagged string is a query: report the current value
                    // without changing it.
                    None => {
                        for s in as_str(v).into_iter().flatten() {
                            record(&s, crate::host::option_get(&s));
                        }
                    }
                }
            }
            let out = mk_list(olds);
            set_names(&out, names);
            if any_set {
                with_host(|h| h.visible = false);
            }
            Ok(out)
        }
        // `getOption(x, default)` is the scalar read; an unset option gives the
        // default, which is itself `NULL` when not supplied.
        "getOption" => {
            let name = str1(&a.req(0, "x")?)
                .ok_or_else(|| "'x' must be a character string".to_string())?;
            match crate::host::option_get(&name) {
                Some(v) => Ok(option_value_to_r(Some(v))),
                None => Ok(a.get(1, "default").unwrap_or_else(null)),
            }
        }
        "deparse" => {
            let x = a.req(0, "expr")?;
            // A lone name deparses to its bare text — R backquotes a
            // non-syntactic name inside an expression (`` `my var` + 1 ``) but
            // not when it is the whole expression.
            if let RData::Sym(n) = data(&x) {
                return Ok(scalar_str(n));
            }
            // An expression deparses to its own source lines, one character
            // element per line, the way a function's does.
            if let Some(e) = lang_of(&x) {
                return Ok(mk_str(
                    crate::deparse::deparse_all_lines(&e)
                        .into_iter()
                        .map(Some)
                        .collect(),
                ));
            }
            // A function deparses to its source *lines* — one character element
            // per line, not one string with embedded newlines.
            if let Some(src) = function_src(&x) {
                return Ok(mk_str(src.into_iter().map(Some).collect()));
            }
            Ok(scalar_str(deparse_value(&x)))
        }
        "format" => {
            let x = a.req(0, "x")?;
            // `format` of a function is its deparsed source, like `deparse`.
            if let Some(src) = function_src(&x) {
                return Ok(mk_str(src.into_iter().map(Some).collect()));
            }
            // …and so is `format` of an expression.
            if let Some(e) = lang_of(&x) {
                return Ok(mk_str(
                    crate::deparse::deparse_all_lines(&e)
                        .into_iter()
                        .map(Some)
                        .collect(),
                ));
            }
            // `format(NULL)` is the *word* `"NULL"`, not the empty character
            // vector that formatting NULL's (zero) elements would produce. This
            // is `format.default`'s own special case, and it is what makes
            // `paste("x is", format(NULL))` read as text rather than vanish.
            if is_null(&x) {
                return Ok(mk_str(vec![Some("NULL".into())]));
            }
            let nsmall = a.named("nsmall").and_then(|v| num1(&v)).unwrap_or(0.0) as usize;
            let digits = a.named("digits").and_then(|v| num1(&v)).map(|d| d as i32);
            let big = a
                .named("big.mark")
                .and_then(|v| str1(&v))
                .unwrap_or_default();
            let numeric = matches!(kind(&x), RKind::Dbl | RKind::Int);
            // `scientific = TRUE/FALSE` forces a notation; otherwise the whole
            // vector takes whichever of the two is narrower — the same rule
            // `print` uses, which is why `format(1e6)` is "1e+06" and not
            // "1000000".
            let scientific = a.named("scientific").and_then(|v| lgl1(&v));
            let is_dbl = kind(&x) == RKind::Dbl;
            let base: Vec<Option<String>> = if numeric && (is_dbl || digits.is_some() || nsmall > 0)
            {
                // `digits` is the significant-digit count, which is exactly what
                // the print-layout renderers read, so set it for this call and
                // restore it afterwards.
                let restore = digits.map(|d| crate::host::set_print_digits(d.max(1) as usize));
                let dbl = as_dbl(&x);
                let finite: Vec<f64> = dbl
                    .iter()
                    .flatten()
                    .copied()
                    .filter(|v| v.is_finite())
                    .collect();
                // A common decimal count across the vector — R aligns the
                // decimal point.
                let fixed_d = finite
                    .iter()
                    .map(|v| crate::host::fixed_decimals(*v))
                    .max()
                    .unwrap_or(0);
                let sci_d = finite
                    .iter()
                    .map(|v| crate::host::sci_decimals(*v))
                    .max()
                    .unwrap_or(0);
                let width = |f: &dyn Fn(f64) -> String| {
                    finite
                        .iter()
                        .map(|v| f(*v).chars().count())
                        .max()
                        .unwrap_or(0)
                };
                let use_sci = scientific.unwrap_or_else(|| {
                    !crate::host::prefers_fixed(
                        width(&|v| render_fixed(v, fixed_d)),
                        width(&|v| render_sci(v, sci_d)),
                    )
                });
                // `nsmall` raises the decimal count only in fixed notation, and
                // only after the notation is chosen — R's `do_format` applies it
                // under `if (!e)`. So `format(1, nsmall = 5)` is "1.00000" while
                // `format(1e6, nsmall = 2)` stays "1e+06".
                let fixed_d = fixed_d.max(nsmall);
                let out = dbl
                    .iter()
                    .map(|e| {
                        e.map(|v| match (v.is_finite(), use_sci) {
                            (true, true) => render_sci(v, sci_d),
                            (true, false) => render_fixed(v, fixed_d),
                            (false, _) => render_fixed(v, 0),
                        })
                    })
                    .collect();
                if let Some(prev) = restore {
                    crate::host::set_print_digits(prev);
                }
                out
            } else {
                as_str(&x)
            };
            let out: Vec<Option<String>> = base
                .into_iter()
                .map(|s| {
                    let mut s = s.unwrap_or_else(|| "NA".into());
                    if numeric && !big.is_empty() {
                        s = insert_big_mark(&s, &big);
                    }
                    Some(s)
                })
                .collect();
            // R pads every element to a common width: numbers right-justified,
            // character left-justified. `width` raises that common width to a
            // minimum, and applies even to a length-1 vector (which otherwise
            // needs no alignment).
            let width = a
                .named("width")
                .and_then(|v| num1(&v))
                .unwrap_or(0.0)
                .max(0.0) as usize;
            // The common width is measured in terminal columns, so a CJK cell
            // lines up with an ASCII one: `format(c("日本語", "ab"))` pads "ab"
            // to six columns, not to three characters.
            // `justify` steers that padding for a character vector: "left"
            // (the default), "right", "centre", or "none" — which skips the
            // common width entirely, `width` included.
            let justify = a
                .named("justify")
                .and_then(|v| str1(&v))
                .unwrap_or_else(|| "left".into());
            let is_str = kind(&x) == RKind::Str;
            let out = if is_str && justify == "none" {
                out
            } else if out.len() > 1 || width > 0 {
                let w = out
                    .iter()
                    .flatten()
                    .map(|s| crate::strwidth::display_width(s))
                    .max()
                    .unwrap_or(0)
                    .max(width);
                if is_str && justify == "centre" {
                    out.into_iter()
                        .map(|s| s.map(|s| centre_display(&s, w)))
                        .collect()
                } else {
                    // Numbers right-justify; a character vector follows
                    // `justify`, which defaults to left.
                    let left = is_str && justify != "right";
                    out.into_iter()
                        .map(|s| s.map(|s| crate::strwidth::pad_display(&s, w, left)))
                        .collect()
                }
            } else {
                out
            };
            shaped_like(mk_str(out), &x)
        }
        "formatC" => format_c(&a),
        "prettyNum" => {
            let x = a.req(0, "x")?;
            let big = a
                .named("big.mark")
                .and_then(|v| str1(&v))
                .unwrap_or_default();
            Ok(mk_str(
                as_str(&x)
                    .into_iter()
                    .map(|s| s.map(|s| insert_big_mark(&s, &big)))
                    .collect(),
            ))
        }
        "sprintf" => sprintf(&a),

        // ── sequences ───────────────────────────────────────────────────
        "seq_len" => {
            let n = a.n(0, 0.0) as i64;
            Ok(mk_int((1..=n).map(Some).collect()))
        }
        "seq_along" => {
            let n = len(&a.req(0, "along.with")?) as i64;
            Ok(mk_int((1..=n).map(Some).collect()))
        }
        // `seq` is a generic in R; the error a bad `by` raises comes from
        // `seq.default`, and both the message's call and the chain name it.
        "seq" => in_method("seq.default", || seq(&a)),
        "seq.int" => seq(&a),
        "rep" => Ok(rep(&a)),
        "rep_len" => {
            let x = a.req(0, "x")?;
            let n = a.get(1, "length.out").and_then(|v| num1(&v)).unwrap_or(0.0) as usize;
            let src = len(&x).max(1);
            let pos: Vec<Option<usize>> = (0..n).map(|i| Some(i % src)).collect();
            let out = take_positions(&x, &pos);
            carry_factor(&out, &x);
            Ok(out)
        }
        "rev" => {
            let x = a.req(0, "x")?;
            let nm = names_of(&x);
            let pos: Vec<Option<usize>> = (0..len(&x)).rev().map(Some).collect();
            let out = take_positions(&x, &pos);
            // Reversing keeps the names, reversed too.
            if !nm.is_empty() {
                set_names(&out, nm.into_iter().rev().collect());
            }
            // `rev.default` is `x[length(x):1L]`, so a factor stays one.
            carry_factor(&out, &x);
            Ok(out)
        }
        "unname" => {
            let out = copy_of(&a.req(0, "obj")?);
            with_host(|h| {
                let nl = h.null();
                h.set_attr(&out, "names", nl);
            });
            Ok(out)
        }
        "all.equal" => {
            // Numeric near-equality within R's default tolerance (~1.5e-8);
            // returns TRUE or a short difference message. Non-numerics compare
            // with `identical`.
            let x = a.req(0, "target")?;
            let y = a.req(1, "current")?;
            let (xs, ys) = (as_dbl(&x), as_dbl(&y));
            let numeric = matches!(kind(&x), RKind::Dbl | RKind::Int)
                && matches!(kind(&y), RKind::Dbl | RKind::Int);
            if numeric && xs.len() == ys.len() {
                let tol = 1.5e-8;
                let mut sum_abs_diff = 0.0;
                let mut sum_abs_tgt = 0.0;
                for (a, b) in xs.iter().zip(ys.iter()) {
                    match (a, b) {
                        // R's default `countEQ = FALSE` scales only over the
                        // elements that actually differ.
                        (Some(a), Some(b)) if a != b => {
                            sum_abs_diff += (a - b).abs();
                            sum_abs_tgt += a.abs();
                        }
                        (Some(_), Some(_)) => {}
                        _ => return Ok(scalar_str("'is.NA' value mismatch")),
                    }
                }
                let rel = if sum_abs_tgt > tol {
                    sum_abs_diff / sum_abs_tgt
                } else {
                    sum_abs_diff
                };
                if rel <= tol {
                    Ok(scalar_lgl(true))
                } else {
                    Ok(scalar_str(format!(
                        "Mean relative difference: {}",
                        crate::host::format_dbl(rel)
                    )))
                }
            } else {
                Ok(if identical(&x, &y) {
                    scalar_lgl(true)
                } else {
                    scalar_str("objects differ")
                })
            }
        }
        "head" | "tail" => {
            let x = a.req(0, "x")?;
            let n = a.get(1, "n").and_then(|v| num1(&v)).unwrap_or(6.0) as i64;
            let total = len(&x) as i64;
            let k = if n < 0 {
                (total + n).max(0)
            } else {
                n.min(total)
            } as usize;
            let pos: Vec<Option<usize>> = if name == "head" {
                (0..k).map(Some).collect()
            } else {
                (total as usize - k..total as usize).map(Some).collect()
            };
            let out = take_positions(&x, &pos);
            // `head`/`tail` are `x[seq]`, which for a factor is `[.factor`.
            carry_factor(&out, &x);
            Ok(out)
        }
        "append" => {
            let x = a.req(0, "x")?;
            let y = a.req(1, "values")?;
            // `after` (default `length(x)`) is the position `values` is spliced
            // in *after*, so it is an insert, not an unconditional append:
            // `c(x[1:after], values, x[(after+1):length(x)])`. It was being
            // dropped, which pinned every insert to the tail.
            let n = len(&x);
            let after = a
                .get(2, "after")
                .and_then(|v| num1(&v))
                .map(|v| (v.max(0.0) as usize).min(n))
                .unwrap_or(n);
            let joined = if after == 0 {
                Args::new(vec![(None, y), (None, x)])
            } else if after >= n {
                Args::new(vec![(None, x), (None, y)])
            } else {
                let head = take_positions(&x, &(0..after).map(Some).collect::<Vec<_>>());
                let tail = take_positions(&x, &(after..n).map(Some).collect::<Vec<_>>());
                // `take_positions` rebuilds the payload only; a named `x` has to
                // keep the labels that travelled with each slice.
                let nm = names_of(&x);
                if !nm.is_empty() {
                    set_names(&head, nm[..after].to_vec());
                    set_names(&tail, nm[after..].to_vec());
                }
                Args::new(vec![(None, head), (None, y), (None, tail)])
            };
            Ok(concat(&joined))
        }

        // ── ordering and sets ───────────────────────────────────────────
        "sort" => {
            let x = a.req(0, "x")?;
            let decreasing = a
                .named("decreasing")
                .and_then(|v| lgl1(&v))
                .unwrap_or(false);
            // `na.last` defaults to `NA` for `sort` (drop the missing values)
            // and to `TRUE` for `order` (keep them, at the end).
            let na_last = na_last_arg(&a, None);
            let sorted = sort_value(&x, decreasing, na_last);
            // `index.return = TRUE` also returns the ordering as `$ix`.
            if a.named("index.return")
                .and_then(|v| lgl1(&v))
                .unwrap_or(false)
            {
                let ix = order_value(&x, decreasing, na_last);
                let out = mk_list(vec![sorted, ix]);
                set_names(&out, vec![Some("x".into()), Some("ix".into())]);
                Ok(out)
            } else {
                Ok(sorted)
            }
        }
        "order" => {
            // Every untagged argument is a sort key; later ones break ties.
            let keys: Vec<Value> = a
                .all
                .iter()
                .filter(|(t, _)| t.is_none())
                .map(|(_, v)| v.clone())
                .collect();
            if keys.is_empty() {
                return Err("argument \"x\" is missing, with no default".into());
            }
            let decreasing = a
                .named("decreasing")
                .and_then(|v| lgl1(&v))
                .unwrap_or(false);
            Ok(mk_int(
                order_by_keys(&keys, decreasing, na_last_arg(&a, Some(true)))
                    .into_iter()
                    .map(|i| Some(i as i64 + 1))
                    .collect(),
            ))
        }
        "sort.list" => {
            // `order` over a single key. With `na.last = NA` the missing values
            // are dropped *before* the positions are taken, so the answer
            // indexes the compacted vector: `sort.list(c("b", NA, "a"),
            // na.last = NA)` is `2 1`, not `3 1`.
            let x = a.req(0, "x")?;
            let decreasing = a
                .named("decreasing")
                .and_then(|v| lgl1(&v))
                .unwrap_or(false);
            let na_last = na_last_arg(&a, Some(true));
            let ord = order_by_keys(std::slice::from_ref(&x), decreasing, na_last);
            let out: Vec<Option<i64>> = if na_last.is_none() {
                let key = SortKey::of(&x);
                let mut compacted = vec![0i64; len(&x)];
                let mut next = 0;
                for (i, slot) in compacted.iter_mut().enumerate() {
                    if !key.missing(i) {
                        next += 1;
                        *slot = next;
                    }
                }
                ord.into_iter().map(|i| Some(compacted[i])).collect()
            } else {
                ord.into_iter().map(|i| Some(i as i64 + 1)).collect()
            };
            Ok(mk_int(out))
        }
        "xtfrm" => {
            // The numeric key that sorts `x`. A plain numeric vector already is
            // that key and comes back untouched, names and all; everything else
            // ranks with `ties.method = "min"` (so `c("b", "b", "a")` is
            // `2 2 1`, where `rank` would average to `2.5 2.5 1`) and keeps NA.
            let x = a.req(0, "x")?;
            if !is_factor(&x) && matches!(kind(&x), RKind::Int | RKind::Dbl) {
                return Ok(x);
            }
            let key = SortKey::of(&x);
            let n = len(&x);
            let mut good: Vec<usize> = (0..n).filter(|&i| !key.missing(i)).collect();
            good.sort_by(|&i, &j| key.cmp(i, j));
            let mut out: Vec<Option<i64>> = vec![None; n];
            let mut i = 0;
            while i < good.len() {
                let mut j = i + 1;
                while j < good.len() && key.cmp(good[j], good[i]).is_eq() {
                    j += 1;
                }
                for &k in &good[i..j] {
                    out[k] = Some(i as i64 + 1);
                }
                i = j;
            }
            Ok(mk_int(out))
        }
        "unique" => {
            let x = a.req(0, "x")?;
            let keys = as_str(&x);
            let mut seen: Vec<Option<String>> = Vec::new();
            let mut pos = Vec::new();
            for (i, k) in keys.iter().enumerate() {
                if !seen.contains(k) {
                    seen.push(k.clone());
                    pos.push(Some(i));
                }
            }
            let out = take_positions(&x, &pos);
            // R's `unique` keeps a factor's full level table — the levels are a
            // property of the variable, not of the values that happen to remain.
            carry_factor(&out, &x);
            Ok(out)
        }
        "setdiff" | "union" | "intersect" => {
            let x = a.req(0, "x")?;
            let y = a.req(1, "y")?;
            let (xs, ys) = (as_str_labels(&x), as_str_labels(&y));
            let mut pos = Vec::new();
            let mut seen: Vec<Option<String>> = Vec::new();
            for (i, k) in xs.iter().enumerate() {
                let keep = match name {
                    "setdiff" => !ys.contains(k),
                    "intersect" => ys.contains(k),
                    _ => true,
                };
                if keep && !seen.contains(k) {
                    seen.push(k.clone());
                    pos.push(Some(i));
                }
            }
            let head = take_positions(&x, &pos);
            carry_factor(&head, &x);
            if name == "setdiff" {
                // `setdiff` is `x[match(x, y, 0L) == 0L]` — a subset of `x`, so
                // it keeps exactly `x`'s levels.
                return Ok(head);
            }
            // `union` appends `y`'s new elements; `intersect` appends `y[0L]`.
            // Both go through `c`, which is what widens the level set to the
            // union — R's set operators are defined in terms of `c` precisely so
            // that a factor result spans both operands' levels.
            let mut ypos = Vec::new();
            if name == "union" {
                for (i, k) in ys.iter().enumerate() {
                    if !seen.contains(k) {
                        seen.push(k.clone());
                        ypos.push(Some(i));
                    }
                }
            }
            let tail = take_positions(&y, &ypos);
            carry_factor(&tail, &y);
            Ok(concat(&Args::new(vec![(None, head), (None, tail)])))
        }
        "match" => {
            // First position (1-based) of each `x` in `table`, else NA. String
            // coercion gives a type-agnostic equality that matches R here.
            let xs = as_str_labels(&a.req(0, "x")?);
            let table = as_str_labels(&a.req(1, "table")?);
            Ok(mk_int(
                xs.iter()
                    .map(|k| table.iter().position(|t| t == k).map(|p| p as i64 + 1))
                    .collect(),
            ))
        }
        "is.element" => {
            let el = as_str_labels(&a.req(0, "el")?);
            let table = as_str_labels(&a.req(1, "table")?);
            Ok(mk_lgl(el.iter().map(|k| Some(table.contains(k))).collect()))
        }
        "duplicated" => {
            let keys = as_str(&a.req(0, "x")?);
            let mut seen: Vec<Option<String>> = Vec::new();
            Ok(mk_lgl(
                keys.iter()
                    .map(|k| {
                        let dup = seen.contains(k);
                        if !dup {
                            seen.push(k.clone());
                        }
                        Some(dup)
                    })
                    .collect(),
            ))
        }
        "rank" => {
            // Average ranks with ties, like R's default `ties.method="average"`.
            // The key is typed rather than coerced to double: character input
            // ranks in collation order, so `rank(c("b", "b", "a"))` is
            // `2.5 2.5 1` and not one universal tie.
            let x = a.req(0, "x")?;
            let key = SortKey::of(&x);
            let n = len(&x);
            let mut good: Vec<usize> = (0..n).filter(|&i| !key.missing(i)).collect();
            good.sort_by(|&i, &j| key.cmp(i, j));
            let mut ranks = vec![0.0; n];
            let mut i = 0;
            while i < good.len() {
                let mut j = i + 1;
                while j < good.len() && key.cmp(good[j], good[i]).is_eq() {
                    j += 1;
                }
                // positions i..j are tied; their shared rank is the average of
                // the 1-based slots i+1..=j.
                let avg = ((i + 1 + j) as f64) / 2.0;
                for &k in &good[i..j] {
                    ranks[k] = avg;
                }
                i = j;
            }
            // `na.last = TRUE`, R's default for `rank`: the missing values take
            // the trailing ranks, in the order they appear.
            for (slot, i) in (0..n).filter(|&i| key.missing(i)).enumerate() {
                ranks[i] = (good.len() + slot + 1) as f64;
            }
            // Integer ranks (no ties) print as a double vector in R anyway.
            shaped_like(mk_dbl(ranks.into_iter().map(Some).collect()), &x)
        }
        "which" => {
            let x = a.req(0, "x")?;
            let nm = names_of(&x);
            let hits: Vec<usize> = as_lgl(&x)
                .iter()
                .enumerate()
                .filter(|(_, e)| **e == Some(true))
                .map(|(i, _)| i)
                .collect();
            // `arr.ind = TRUE` reports each hit as its per-margin subscripts
            // instead of one linear position (arrays only; a plain vector keeps
            // the linear answer).
            let arr_ind = a
                .named("arr.ind")
                .and_then(|v| as_lgl(&v).first().copied().flatten())
                .unwrap_or(false);
            if arr_ind {
                let d: Vec<usize> = with_host(|h| h.attr(&x, "dim"))
                    .map(|d| as_int(&d).iter().map(|e| e.unwrap_or(0) as usize).collect())
                    .unwrap_or_default();
                if d.len() >= 2 {
                    return Ok(which_arr_ind(&x, &hits, &d));
                }
            }
            let out = mk_int(hits.iter().map(|i| Some(*i as i64 + 1)).collect());
            if !nm.is_empty() {
                set_names(
                    &out,
                    hits.iter().map(|i| nm.get(*i).cloned().flatten()).collect(),
                );
            }
            Ok(out)
        }
        "which.max" | "which.min" => {
            let xs = as_dbl(&a.req(0, "x")?);
            let mut best: Option<(usize, f64)> = None;
            for (i, e) in xs.iter().enumerate() {
                let Some(v) = e else { continue };
                let better = match best {
                    None => true,
                    Some((_, b)) => {
                        if name == "which.max" {
                            *v > b
                        } else {
                            *v < b
                        }
                    }
                };
                if better {
                    best = Some((i, *v));
                }
            }
            Ok(match best {
                Some((i, _)) => scalar_int(i as i64 + 1),
                None => mk_int(vec![]),
            })
        }

        // ── numeric summaries ───────────────────────────────────────────
        "sum" | "prod" => {
            let mut acc = if name == "sum" { 0.0 } else { 1.0 };
            // Missing values propagate, and `NA` outranks `NaN`: R answers NA
            // for `sum(c(1, NaN, NA))` whichever order they appear in, but NaN
            // for `sum(c(1, NaN))`.
            let (mut na, mut nan) = (false, false);
            let narm = a.named("na.rm").and_then(|v| lgl1(&v)).unwrap_or(false);
            let all_int = a
                .all
                .iter()
                .filter(|(t, _)| t.as_deref() != Some("na.rm"))
                .all(|(_, v)| matches!(kind(v), RKind::Int | RKind::Lgl));
            for (tag, v) in a.all.iter() {
                if tag.as_deref() == Some("na.rm") {
                    continue;
                }
                for e in as_dbl(v) {
                    match e {
                        // NaN counts as missing, just like NA, for `na.rm`.
                        Some(x) if !x.is_nan() => {
                            if name == "sum" {
                                acc += x
                            } else {
                                acc *= x
                            }
                        }
                        Some(_) if !narm => nan = true,
                        None if !narm => na = true,
                        _ => {}
                    }
                }
            }
            Ok(if na {
                mk_dbl(vec![None])
            } else if nan {
                scalar_dbl(f64::NAN)
            } else if all_int && name == "sum" {
                scalar_int(acc as i64)
            } else {
                scalar_dbl(acc)
            })
        }
        "mean" => {
            if let Some(v) = missing_result(&a, &a.req(0, "x")?, true) {
                return Ok(v);
            }
            let xs = numeric_arg(&a, 0, "x")?;
            Ok(if xs.is_empty() {
                // Mean of nothing is NaN (0/0), not NA.
                scalar_dbl(f64::NAN)
            } else {
                scalar_dbl(xs.iter().sum::<f64>() / xs.len() as f64)
            })
        }
        "median" => {
            if let Some(v) = missing_result(&a, &a.req(0, "x")?, false) {
                return Ok(v);
            }
            let mut xs = numeric_arg(&a, 0, "x")?;
            if xs.is_empty() {
                return Ok(mk_dbl(vec![None]));
            }
            xs.sort_by(|p, q| p.partial_cmp(q).unwrap());
            let m = xs.len() / 2;
            Ok(scalar_dbl(if xs.len() % 2 == 1 {
                xs[m]
            } else {
                (xs[m - 1] + xs[m]) / 2.0
            }))
        }
        "quantile" => {
            let mut xs = numeric_arg(&a, 0, "x")?;
            xs.sort_by(|p, q| p.partial_cmp(q).unwrap_or(std::cmp::Ordering::Equal));
            let probs: Vec<f64> = match a.get(1, "probs") {
                Some(p) => as_dbl(&p).into_iter().flatten().collect(),
                None => vec![0.0, 0.25, 0.5, 0.75, 1.0],
            };
            let n = xs.len();
            // R's default type 7: h = (n-1)p, linear interpolation.
            let vals: Vec<Option<f64>> = probs
                .iter()
                .map(|&p| {
                    if n == 0 {
                        return None;
                    }
                    let h = (n as f64 - 1.0) * p;
                    let lo = h.floor() as usize;
                    let frac = h - lo as f64;
                    Some(if lo + 1 < n {
                        xs[lo] + frac * (xs[lo + 1] - xs[lo])
                    } else {
                        xs[lo]
                    })
                })
                .collect();
            let out = mk_dbl(vals);
            let names = a.named("names").and_then(|v| lgl1(&v)).unwrap_or(true);
            if names {
                set_names(
                    &out,
                    probs
                        .iter()
                        .map(|&p| Some(format!("{}%", crate::host::format_dbl(p * 100.0))))
                        .collect(),
                );
            }
            Ok(out)
        }
        "cor" => {
            // Pearson correlation of two equal-length numeric vectors.
            let x = numeric_arg(&a, 0, "x")?;
            let y = as_dbl(&a.req(1, "y")?)
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
            let n = x.len().min(y.len());
            if n < 2 {
                return Ok(mk_dbl(vec![None]));
            }
            let mx = x[..n].iter().sum::<f64>() / n as f64;
            let my = y[..n].iter().sum::<f64>() / n as f64;
            let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
            for i in 0..n {
                let (dx, dy) = (x[i] - mx, y[i] - my);
                sxy += dx * dy;
                sxx += dx * dx;
                syy += dy * dy;
            }
            // Zero variance in either vector makes the correlation undefined:
            // R returns NA there, not the `0/0 = NaN` the formula would give.
            if sxx == 0.0 || syy == 0.0 {
                Ok(mk_dbl(vec![None]))
            } else {
                Ok(scalar_dbl(sxy / (sxx.sqrt() * syy.sqrt())))
            }
        }
        "rle" => {
            let x = a.req(0, "x")?;
            let keys = as_str(&x);
            let mut starts: Vec<Option<usize>> = Vec::new();
            let mut lengths: Vec<Option<i64>> = Vec::new();
            let mut i = 0;
            while i < keys.len() {
                let mut j = i + 1;
                while j < keys.len() && keys[j] == keys[i] {
                    j += 1;
                }
                starts.push(Some(i));
                lengths.push(Some((j - i) as i64));
                i = j;
            }
            let values = take_positions(&x, &starts);
            let out = mk_list(vec![mk_int(lengths), values]);
            set_names(&out, vec![Some("lengths".into()), Some("values".into())]);
            let cls = scalar_str("rle");
            with_host(|h| h.set_attr(&out, "class", cls));
            Ok(out)
        }
        "inverse.rle" => {
            let x = a.req(0, "x")?;
            let lengths = element_field(&x, "lengths")
                .map(|v| as_int(&v))
                .unwrap_or_default();
            let values = element_field(&x, "values").unwrap_or_else(null);
            let mut pos: Vec<Option<usize>> = Vec::new();
            for (i, l) in lengths.iter().enumerate() {
                for _ in 0..l.unwrap_or(0).max(0) {
                    pos.push(Some(i));
                }
            }
            Ok(take_positions(&values, &pos))
        }
        "var" | "sd" => {
            let xs = numeric_arg(&a, 0, "x")?;
            if xs.len() < 2 {
                return Ok(mk_dbl(vec![None]));
            }
            let n = xs.len() as f64;
            let mean = xs.iter().sum::<f64>() / n;
            // R's two-pass variance (src/library/stats cov.c): the `t*t/n`
            // term cancels the rounding error left in `mean`, so the last
            // printed digit matches R rather than drifting by one ulp.
            let mut s = 0.0;
            let mut t = 0.0;
            for x in &xs {
                let d = x - mean;
                s += d * d;
                t += d;
            }
            let var = (s - t * t / n) / (n - 1.0);
            Ok(scalar_dbl(if name == "sd" { var.sqrt() } else { var }))
        }
        "min" | "max" | "range" => {
            // R's `Summary.factor` refuses every member of the group; only
            // `Summary.ordered` allows `min`/`max`/`range`, and it answers with
            // a factor over the same levels rather than a code.
            if let Some(f) = a.all.iter().map(|(_, v)| v).find(|v| is_factor(v)) {
                if !is_ordered(f) {
                    return Err(format!("'{name}' not meaningful for factors"));
                }
                let levels = levels_of(f);
                let codes: Vec<i64> = as_int(f).into_iter().flatten().collect();
                let pick: Vec<Option<i64>> = match name {
                    "min" => vec![codes.iter().min().copied()],
                    "max" => vec![codes.iter().max().copied()],
                    _ => vec![codes.iter().min().copied(), codes.iter().max().copied()],
                };
                return Ok(mk_factor(pick, levels, true));
            }
            let narm = a.named("na.rm").and_then(|v| lgl1(&v)).unwrap_or(false);
            let mut xs: Vec<Option<f64>> = Vec::new();
            let mut strings: Vec<Option<String>> = Vec::new();
            let mut is_text = false;
            for (tag, v) in a.all.iter() {
                if tag.as_deref() == Some("na.rm") {
                    continue;
                }
                if kind(v) == RKind::Str {
                    is_text = true;
                    strings.extend(as_str(v));
                } else {
                    xs.extend(as_dbl(v));
                }
            }
            if is_text {
                let mut ss: Vec<String> = strings.into_iter().flatten().collect();
                ss.sort_by(|a, b| crate::collate::str_cmp(a, b));
                return Ok(match name {
                    "min" => scalar_str(ss.first().cloned().unwrap_or_default()),
                    "max" => scalar_str(ss.last().cloned().unwrap_or_default()),
                    _ => mk_str(vec![ss.first().cloned(), ss.last().cloned()]),
                });
            }
            if !narm && xs.iter().any(|e| e.map(f64::is_nan).unwrap_or(true)) {
                // NA dominates NaN: `max(c(1, NA, NaN))` is NA, but with only a
                // NaN present the result is NaN.
                let marker = if xs.iter().any(|e| e.is_none()) {
                    None
                } else {
                    Some(f64::NAN)
                };
                return Ok(mk_dbl(if name == "range" {
                    vec![marker, marker]
                } else {
                    vec![marker]
                }));
            }
            let vals: Vec<f64> = xs.into_iter().flatten().filter(|x| !x.is_nan()).collect();
            if vals.is_empty() {
                // R: `max` of nothing is `-Inf`, `min` is `Inf`, `range` is
                // `c(Inf, -Inf)`; each extreme warns as it is taken, so `range`
                // warns twice, once per end.
                // `range` is R code in R (`range.default` ends in
                // `c(min(x), max(x))`), so its two warnings name *those* calls
                // rather than the `range(…)` the user wrote.
                let via_range = name == "range";
                let at = |own: &str| match via_range {
                    true => Some(format!("{own}(x)")),
                    false => with_host(|h| h.current_call()),
                };
                let warn_min = || {
                    signal_warning_in("no non-missing arguments to min; returning Inf", at("min"))
                };
                let warn_max = || {
                    signal_warning_in("no non-missing arguments to max; returning -Inf", at("max"))
                };
                return Ok(match name {
                    "min" => {
                        warn_min()?;
                        scalar_dbl(f64::INFINITY)
                    }
                    "max" => {
                        warn_max()?;
                        scalar_dbl(f64::NEG_INFINITY)
                    }
                    _ => {
                        warn_min()?;
                        warn_max()?;
                        mk_dbl(vec![Some(f64::INFINITY), Some(f64::NEG_INFINITY)])
                    }
                });
            }
            let lo = vals.iter().cloned().fold(f64::INFINITY, f64::min);
            let hi = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            Ok(match name {
                "min" => scalar_dbl(lo),
                "max" => scalar_dbl(hi),
                _ => mk_dbl(vec![Some(lo), Some(hi)]),
            })
        }
        "cumsum" | "cumprod" => {
            let x = a.req(0, "x")?;
            let xs = as_dbl(&x);
            let mut acc = if name == "cumsum" { 0.0 } else { 1.0 };
            // NA is cumulative: once a running total meets an NA, every later
            // element is NA too (R can no longer know the accumulated value).
            let mut na = false;
            let out: Vec<Option<f64>> = xs
                .iter()
                .map(|e| {
                    if na {
                        return None;
                    }
                    match e {
                        Some(v) => {
                            if name == "cumsum" {
                                acc += v
                            } else {
                                acc *= v
                            }
                            Some(acc)
                        }
                        None => {
                            na = true;
                            None
                        }
                    }
                })
                .collect();
            // `cumsum` of an integer/logical vector stays integer (R's rule);
            // `cumprod` is always double.
            let acc = if name == "cumsum" && matches!(kind(&x), RKind::Int | RKind::Lgl) {
                mk_int(out.into_iter().map(|e| e.map(|v| v as i64)).collect())
            } else {
                mk_dbl(out)
            };
            shaped_like(acc, &x)
        }
        "diff" => {
            let x = a.req(0, "x")?;
            let is_int = matches!(kind(&x), RKind::Int | RKind::Lgl);
            let lag = a
                .get(1, "lag")
                .and_then(|v| num1(&v))
                .unwrap_or(1.0)
                .max(1.0) as usize;
            let differences = a
                .get(2, "differences")
                .and_then(|v| num1(&v))
                .unwrap_or(1.0)
                .max(1.0) as usize;
            // Apply the lag-`lag` difference `differences` times.
            let mut cur = as_dbl(&x);
            for _ in 0..differences {
                if cur.len() <= lag {
                    cur = Vec::new();
                    break;
                }
                cur = (lag..cur.len())
                    .map(|i| match (cur[i - lag], cur[i]) {
                        (Some(p), Some(q)) => Some(q - p),
                        _ => None,
                    })
                    .collect();
            }
            // `diff` of an integer vector stays integer.
            if is_int {
                Ok(mk_int(
                    cur.into_iter().map(|e| e.map(|v| v as i64)).collect(),
                ))
            } else {
                Ok(mk_dbl(cur))
            }
        }

        // ── elementwise math ────────────────────────────────────────────
        "abs" | "sqrt" | "exp" | "log2" | "log10" | "floor" | "ceiling" | "trunc" | "sign"
        | "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "sinh" | "cosh" | "tanh" | "expm1"
        | "log1p" | "gamma" | "lgamma" | "factorial" | "lfactorial" => {
            let x = a.req(0, "x")?;
            require_numeric(&x)?;
            let f: fn(f64) -> f64 = match name {
                "abs" => f64::abs,
                "sqrt" => f64::sqrt,
                "exp" => f64::exp,
                "log2" => f64::log2,
                "log10" => f64::log10,
                "floor" => f64::floor,
                "ceiling" => f64::ceil,
                "trunc" => f64::trunc,
                "sin" => f64::sin,
                "cos" => f64::cos,
                "tan" => f64::tan,
                "asin" => f64::asin,
                "acos" => f64::acos,
                "atan" => f64::atan,
                "sinh" => f64::sinh,
                "cosh" => f64::cosh,
                "tanh" => f64::tanh,
                "expm1" => f64::exp_m1,
                "log1p" => f64::ln_1p,
                // `gamma`/`lgamma`/`factorial` route through the system libm
                // (the same one R links), so the printed result matches.
                "gamma" => r_tgamma,
                "lgamma" => r_lgamma,
                "factorial" => |v| r_tgamma(v + 1.0),
                "lfactorial" => |v| r_lgamma(v + 1.0),
                _ => r_sign,
            };
            // `abs` on an integer vector stays integer, like R.
            if name == "abs" {
                if let RData::Int(v) = data(&x) {
                    return Ok(mk_int(v.iter().map(|e| e.map(|n| n.abs())).collect()));
                }
            }
            let input = as_dbl(&x);
            let vals: Vec<Option<f64>> = input.iter().map(|e| e.map(f)).collect();
            // R warns when a math function turns a real number into NaN —
            // `sqrt(-1)`, `log(-1)`, `asin(2)`. An input that was already NaN
            // does not warn, because nothing was produced.
            if produced_nan(&vals, &input) {
                nan_warning()?;
            }
            let out = mk_dbl(vals);
            carry_attrs(&out, &x, &x);
            Ok(out)
        }
        "log" => {
            let x = a.req(0, "x")?;
            require_numeric(&x)?;
            let base = a.get(1, "base").and_then(|v| num1(&v));
            let input = as_dbl(&x);
            let vals: Vec<Option<f64>> = input
                .iter()
                .map(|e| {
                    e.map(|v| match base {
                        Some(b) => v.log(b),
                        None => v.ln(),
                    })
                })
                .collect();
            // R's `log` warns like every other `do_math1`: a real input that
            // comes back NaN produced it, so `log(-1)` warns and `log(NaN)`
            // does not.
            if produced_nan(&vals, &input) {
                nan_warning()?;
            }
            shaped_like(mk_dbl(vals), &x)
        }
        "atan2" => {
            let y = as_dbl(&a.req(0, "y")?);
            let x = as_dbl(&a.req(1, "x")?);
            let n = y.len().max(x.len());
            Ok(mk_dbl(
                (0..n)
                    .map(|i| match (y[i % y.len().max(1)], x[i % x.len().max(1)]) {
                        (Some(a), Some(b)) => Some(a.atan2(b)),
                        _ => None,
                    })
                    .collect(),
            ))
        }
        "choose" => {
            let ns = as_dbl(&a.req(0, "n")?);
            let ks = as_dbl(&a.req(1, "k")?);
            let n = ns.len().max(ks.len());
            Ok(mk_dbl(
                (0..n)
                    .map(
                        |i| match (ns[i % ns.len().max(1)], ks[i % ks.len().max(1)]) {
                            (Some(nn), Some(kk)) => Some(choose(nn, kk)),
                            _ => None,
                        },
                    )
                    .collect(),
            ))
        }
        "beta" | "lbeta" => {
            let av = as_dbl(&a.req(0, "a")?);
            let bv = as_dbl(&a.req(1, "b")?);
            let n = av.len().max(bv.len());
            Ok(mk_dbl(
                (0..n)
                    .map(
                        |i| match (av[i % av.len().max(1)], bv[i % bv.len().max(1)]) {
                            (Some(x), Some(y)) => Some({
                                // beta(a,b) = Γ(a)Γ(b)/Γ(a+b); via lgamma to stay finite.
                                let lb = r_lgamma(x) + r_lgamma(y) - r_lgamma(x + y);
                                if name == "lbeta" {
                                    lb
                                } else {
                                    lb.exp()
                                }
                            }),
                            _ => None,
                        },
                    )
                    .collect(),
            ))
        }
        "pmax" | "pmin" => {
            let narm = a.named("na.rm").and_then(|v| lgl1(&v)).unwrap_or(false);
            let cols: Vec<Vec<Option<f64>>> = a
                .all
                .iter()
                .filter(|(t, _)| t.as_deref() != Some("na.rm"))
                .map(|(_, v)| as_dbl(v))
                .collect();
            let n = cols.iter().map(|c| c.len()).max().unwrap_or(0);
            // R's `do_pmin` checks every argument against the longest and warns
            // once per one that does not tile it exactly — the same rule as
            // arithmetic recycling, under a different message.
            if cols.iter().any(|c| !c.is_empty() && n % c.len() != 0) {
                signal_warning("an argument will be fractionally recycled")?;
            }
            Ok(mk_dbl(
                (0..n)
                    .map(|i| {
                        let mut best: Option<f64> = None;
                        for c in &cols {
                            match c[i % c.len().max(1)] {
                                Some(v) => {
                                    best = Some(match best {
                                        None => v,
                                        Some(b) if name == "pmax" => b.max(v),
                                        Some(b) => b.min(v),
                                    })
                                }
                                None if !narm => return None,
                                None => {}
                            }
                        }
                        best
                    })
                    .collect(),
            ))
        }
        "cummax" | "cummin" => {
            let xv = a.req(0, "x")?;
            let xs = as_dbl(&xv);
            let mut acc: Option<f64> = None;
            // Like cumsum, an NA poisons every later element of the running
            // extremum.
            let mut na = false;
            let out: Vec<Option<f64>> = xs
                .iter()
                .map(|e| {
                    if na {
                        return None;
                    }
                    match e {
                        None => {
                            na = true;
                            None
                        }
                        Some(v) => {
                            acc = Some(match acc {
                                None => *v,
                                Some(a) if name == "cummax" => a.max(*v),
                                Some(a) => a.min(*v),
                            });
                            acc
                        }
                    }
                })
                .collect();
            shaped_like(mk_dbl(out), &xv)
        }
        "tabulate" => {
            let bins = as_int(&a.req(0, "bin")?);
            let nbins = a
                .get(1, "nbins")
                .and_then(|v| num1(&v))
                .map(|v| v as usize)
                .unwrap_or_else(|| {
                    bins.iter().flatten().copied().max().unwrap_or(0).max(0) as usize
                });
            let mut counts = vec![0i64; nbins];
            for b in bins.into_iter().flatten() {
                if b >= 1 && (b as usize) <= nbins {
                    counts[b as usize - 1] += 1;
                }
            }
            Ok(mk_int(counts.into_iter().map(Some).collect()))
        }
        "findInterval" => {
            let x = as_dbl(&a.req(0, "x")?);
            let vec = as_dbl(&a.req(1, "vec")?);
            Ok(mk_int(
                x.iter()
                    .map(|e| e.map(|v| vec.iter().flatten().filter(|&&b| b <= v).count() as i64))
                    .collect(),
            ))
        }
        "round" => {
            let x = a.req(0, "x")?;
            let digits = a.get(1, "digits").and_then(|v| num1(&v)).unwrap_or(0.0) as i32;
            shaped_like(
                mk_dbl(
                    as_dbl(&x)
                        .iter()
                        .map(|e| e.map(|v| r_round(v, digits)))
                        .collect(),
                ),
                &x,
            )
        }
        "signif" => {
            let x = a.req(0, "x")?;
            let digits = (a.get(1, "digits").and_then(|v| num1(&v)).unwrap_or(6.0) as i32).max(1);
            shaped_like(
                mk_dbl(
                    as_dbl(&x)
                        .iter()
                        .map(|e| e.map(|v| signif(v, digits)))
                        .collect(),
                ),
                &x,
            )
        }

        // ── predicates ──────────────────────────────────────────────────
        "is.null" => Ok(scalar_lgl(is_null(&a.get(0, "x").unwrap_or_else(null)))),
        "is.na" => {
            let x = a.req(0, "x")?;
            let out: Vec<Option<bool>> = match data(&x) {
                RData::Lgl(v) => v.iter().map(|e| Some(e.is_none())).collect(),
                RData::Int(v) => v.iter().map(|e| Some(e.is_none())).collect(),
                RData::Dbl(v) => v
                    .iter()
                    .map(|e| Some(e.map(f64::is_nan).unwrap_or(true)))
                    .collect(),
                RData::Str(v) => v.iter().map(|e| Some(e.is_none())).collect(),
                RData::List(v) => v
                    .iter()
                    .map(|e| Some(len(e) == 1 && as_dbl(e).first() == Some(&None)))
                    .collect(),
                _ => vec![],
            };
            shaped_like(mk_lgl(out), &x)
        }
        "is.nan" => {
            // Only doubles carry NaN; NA in other types is not NaN.
            let x = a.req(0, "x")?;
            let out = match data(&x) {
                RData::Dbl(v) => v.iter().map(|e| Some(e.is_some_and(f64::is_nan))).collect(),
                _ => vec![Some(false); len(&x)],
            };
            shaped_like(mk_lgl(out), &x)
        }
        "is.finite" => {
            let x = a.req(0, "x")?;
            let out = match data(&x) {
                RData::Dbl(v) => v
                    .iter()
                    .map(|e| Some(e.is_some_and(f64::is_finite)))
                    .collect(),
                RData::Int(v) => v.iter().map(|e| Some(e.is_some())).collect(),
                RData::Lgl(v) => v.iter().map(|e| Some(e.is_some())).collect(),
                _ => vec![Some(false); len(&x)],
            };
            shaped_like(mk_lgl(out), &x)
        }
        "is.infinite" => {
            let x = a.req(0, "x")?;
            let out = match data(&x) {
                RData::Dbl(v) => v
                    .iter()
                    .map(|e| Some(e.is_some_and(f64::is_infinite)))
                    .collect(),
                _ => vec![Some(false); len(&x)],
            };
            shaped_like(mk_lgl(out), &x)
        }
        "anyNA" => {
            let x = a.req(0, "x")?;
            let any = match data(&x) {
                RData::Dbl(v) => v.iter().any(|e| e.map(f64::is_nan).unwrap_or(true)),
                RData::Int(v) => v.iter().any(|e| e.is_none()),
                RData::Lgl(v) => v.iter().any(|e| e.is_none()),
                RData::Str(v) => v.iter().any(|e| e.is_none()),
                RData::List(items) => items
                    .iter()
                    .any(|e| len(e) == 1 && as_dbl(e).first() == Some(&None)),
                _ => false,
            };
            Ok(scalar_lgl(any))
        }
        "complete.cases" => {
            let x = a.req(0, "x")?;
            let na = match data(&x) {
                RData::Dbl(v) => v
                    .iter()
                    .map(|e| e.map(f64::is_nan).unwrap_or(true))
                    .collect(),
                RData::Int(v) => v.iter().map(|e| e.is_none()).collect(),
                RData::Lgl(v) => v.iter().map(|e| e.is_none()).collect(),
                RData::Str(v) => v.iter().map(|e| e.is_none()).collect(),
                _ => vec![false; len(&x)],
            };
            Ok(mk_lgl(na.into_iter().map(|n: bool| Some(!n)).collect()))
        }
        // `is.numeric` is true for both numeric types; `is.double` and
        // `is.integer` distinguish them (`is.double(1L)` is FALSE). All three
        // answer FALSE for a factor, which R excludes explicitly even though a
        // factor is stored as an integer vector.
        "is.numeric" => {
            let x = a.req(0, "x")?;
            Ok(scalar_lgl(
                matches!(kind(&x), RKind::Dbl | RKind::Int) && !is_factor(&x),
            ))
        }
        "is.double" => Ok(scalar_lgl(kind(&a.req(0, "x")?) == RKind::Dbl)),
        "is.integer" => {
            let x = a.req(0, "x")?;
            Ok(scalar_lgl(kind(&x) == RKind::Int && !is_factor(&x)))
        }
        "is.character" => Ok(scalar_lgl(kind(&a.req(0, "x")?) == RKind::Str)),
        "is.logical" => Ok(scalar_lgl(kind(&a.req(0, "x")?) == RKind::Lgl)),
        "is.list" => Ok(scalar_lgl(kind(&a.req(0, "x")?) == RKind::List)),
        "is.function" => Ok(scalar_lgl(with_host(|h| {
            h.is_function(&a.req(0, "x").unwrap_or(Value::Undef))
        }))),
        // R's `is.vector` is not just a type test: an object carrying any
        // attribute other than `names` is not a vector, so a matrix, a factor
        // and anything with a stray `attr` all answer FALSE.
        "is.vector" => {
            let x = a.req(0, "x")?;
            let plain = with_host(|h| h.attrs_of(&x))
                .iter()
                .all(|(k, _)| k == "names");
            Ok(scalar_lgl(
                plain
                    && matches!(
                        kind(&x),
                        RKind::Dbl | RKind::Int | RKind::Str | RKind::Lgl | RKind::List
                    ),
            ))
        }
        "any" | "all" => {
            let narm = a.named("na.rm").and_then(|v| lgl1(&v)).unwrap_or(false);
            let mut saw_na = false;
            let mut decided = false;
            for (tag, v) in a.all.iter() {
                if tag.as_deref() == Some("na.rm") {
                    continue;
                }
                for e in as_lgl(v) {
                    match e {
                        Some(b) if (name == "any") == b => decided = true,
                        Some(_) => {}
                        None => saw_na = true,
                    }
                }
            }
            Ok(if decided {
                scalar_lgl(name == "any")
            } else if saw_na && !narm {
                mk_lgl(vec![None])
            } else {
                scalar_lgl(name != "any")
            })
        }
        "isTRUE" => Ok(scalar_lgl(matches!(
            as_lgl(&a.req(0, "x")?).as_slice(),
            [Some(true)]
        ))),
        "isFALSE" => Ok(scalar_lgl(matches!(
            as_lgl(&a.req(0, "x")?).as_slice(),
            [Some(false)]
        ))),
        "xor" => {
            let x = as_lgl(&a.req(0, "x")?);
            let y = as_lgl(&a.req(1, "y")?);
            let n = x.len().max(y.len());
            Ok(mk_lgl(
                (0..n)
                    .map(|i| match (x[i % x.len().max(1)], y[i % y.len().max(1)]) {
                        (Some(a), Some(b)) => Some(a != b),
                        _ => None,
                    })
                    .collect(),
            ))
        }
        "bitwAnd" | "bitwOr" | "bitwXor" | "bitwShiftL" | "bitwShiftR" => {
            let x = as_int(&a.req(0, "a")?);
            let y = as_int(&a.req(1, "b")?);
            let n = x.len().max(y.len());
            Ok(mk_int(
                (0..n)
                    .map(|i| {
                        let a = x[i % x.len().max(1)]?;
                        let b = y[i % y.len().max(1)]?;
                        Some(match name {
                            "bitwAnd" => a & b,
                            "bitwOr" => a | b,
                            "bitwXor" => a ^ b,
                            "bitwShiftL" => a << b,
                            _ => a >> b,
                        })
                    })
                    .collect(),
            ))
        }
        "bitwNot" => Ok(mk_int(
            as_int(&a.req(0, "a")?)
                .iter()
                .map(|e| e.map(|v| !v))
                .collect(),
        )),
        "identical" => {
            let x = a.req(0, "x")?;
            let y = a.req(1, "y")?;
            Ok(scalar_lgl(identical(&x, &y)))
        }
        "ifelse" => {
            let test = as_lgl(&a.req(0, "test")?);
            let yes = a.req(1, "yes")?;
            let no = a.req(2, "no")?;
            let pos_yes: Vec<Option<usize>> = (0..len(&yes)).map(Some).collect();
            let pos_no: Vec<Option<usize>> = (0..len(&no)).map(Some).collect();
            let mut parts: Vec<(Option<String>, Value)> = Vec::new();
            for (i, t) in test.iter().enumerate() {
                let v = match t {
                    Some(true) => take_positions(&yes, &[pos_yes[i % pos_yes.len().max(1)]]),
                    Some(false) => take_positions(&no, &[pos_no[i % pos_no.len().max(1)]]),
                    None => mk_lgl(vec![None]),
                };
                parts.push((None, v));
            }
            Ok(concat(&Args::new(parts)))
        }

        // ── strings ─────────────────────────────────────────────────────
        "nchar" => {
            // `type=` names the unit: code points, UTF-8 bytes, or terminal
            // columns. They only coincide for ASCII.
            let ty = a
                .get(1, "type")
                .and_then(|v| str1(&v))
                .unwrap_or_else(|| "chars".into());
            let count: fn(&str) -> i64 = match ty.as_str() {
                "bytes" => |s: &str| s.len() as i64,
                "width" => |s: &str| crate::strwidth::display_width(s) as i64,
                _ => |s: &str| s.chars().count() as i64,
            };
            let x = a.req(0, "x")?;
            shaped_like(
                mk_int(as_str(&x).iter().map(|s| s.as_deref().map(count)).collect()),
                &x,
            )
        }
        "toupper" | "tolower" | "casefold" => {
            // `casefold(x, upper = FALSE)` is `tolower`/`toupper` behind a flag.
            let upper = if name == "casefold" {
                a.named("upper").and_then(|v| lgl1(&v)).unwrap_or(false)
            } else {
                name == "toupper"
            };
            // R maps one character to one character (`towupper`), so the result
            // never changes length — `toupper("straße")` is "STRAßE", not
            // "STRASSE" (see `strwidth::simple_upper`).
            let f: fn(char) -> char = if upper {
                crate::strwidth::simple_upper
            } else {
                crate::strwidth::simple_lower
            };
            let x = a.req(0, "x")?;
            shaped_like(
                mk_str(
                    as_str(&x)
                        .iter()
                        .map(|s| s.as_deref().map(|s| crate::strwidth::map_chars(s, f)))
                        .collect(),
                ),
                &x,
            )
        }
        "strtrim" => {
            // `strtrim(x, width)` cuts to *display columns*, recycling `width`.
            let x = as_str(&a.req(0, "x")?);
            let w = as_int(&a.req(1, "width")?);
            Ok(mk_str(
                x.iter()
                    .enumerate()
                    .map(|(i, s)| {
                        let wi = w.get(i % w.len().max(1)).and_then(|e| *e).unwrap_or(0);
                        s.as_deref()
                            .map(|s| crate::strwidth::trim_to_width(s, wi.max(0) as usize))
                    })
                    .collect(),
            ))
        }
        "utf8ToInt" => {
            // One string in, its code points out — `NA` for a non-string.
            let x = a.req(0, "x")?;
            let Some(s) = str1(&x) else {
                return Ok(mk_int(vec![None]));
            };
            Ok(mk_int(s.chars().map(|c| Some(c as i64)).collect()))
        }
        "intToUtf8" => {
            // The inverse. `multiple = TRUE` gives one string per code point
            // instead of one string of them all; a 0 code point is dropped.
            let x = as_int(&a.req(0, "x")?);
            let multiple = a.get(1, "multiple").and_then(|v| lgl1(&v)).unwrap_or(false);
            let chr = |e: &Option<i64>| -> Option<char> {
                u32::try_from(e.filter(|&v| v != 0)?)
                    .ok()
                    .and_then(char::from_u32)
            };
            if multiple {
                return Ok(mk_str(
                    x.iter()
                        .map(|e| e.map(|_| chr(e).map(String::from).unwrap_or_default()))
                        .collect(),
                ));
            }
            if x.iter().any(|e| e.is_none()) {
                return Ok(mk_str(vec![None]));
            }
            Ok(scalar_str(x.iter().filter_map(chr).collect::<String>()))
        }
        "trimws" => {
            let which = a
                .get(1, "which")
                .and_then(|v| str1(&v))
                .unwrap_or_else(|| "both".into());
            let xv = a.req(0, "x")?;
            shaped_like(
                mk_str(
                    as_str(&xv)
                        .iter()
                        .map(|s| {
                            s.as_ref().map(|s| match which.as_str() {
                                "left" => s.trim_start().to_string(),
                                "right" => s.trim_end().to_string(),
                                _ => s.trim().to_string(),
                            })
                        })
                        .collect(),
                ),
                &xv,
            )
        }
        "substr" => {
            let xv = a.req(0, "x")?;
            let start = a.get(1, "start").and_then(|v| num1(&v)).unwrap_or(1.0) as usize;
            let stop = a.get(2, "stop").and_then(|v| num1(&v)).unwrap_or(1e6) as usize;
            shaped_like(
                mk_str(
                    as_str(&xv)
                        .iter()
                        .map(|s| s.as_ref().map(|s| substr_of(s, start, stop)))
                        .collect(),
                ),
                &xv,
            )
        }
        "substring" => {
            // Unlike `substr`, `substring` recycles text/first/last to the
            // longest of the three: `substring("hello", 1:3)` is three pieces.
            let text = as_str(&a.req(0, "text")?);
            let first = as_dbl(&a.get(1, "first").unwrap_or_else(|| scalar_dbl(1.0)));
            let last = as_dbl(&a.get(2, "last").unwrap_or_else(|| scalar_dbl(1e6)));
            let n = text.len().max(first.len()).max(last.len()).max(1);
            let out = mk_str(
                (0..n)
                    .map(|i| {
                        let s = text.get(i % text.len().max(1)).cloned().flatten()?;
                        let f = first[i % first.len().max(1)].unwrap_or(1.0) as usize;
                        let l = last[i % last.len().max(1)].unwrap_or(1e6) as usize;
                        Some(substr_of(&s, f, l))
                    })
                    .collect(),
            );
            // Names survive only when nothing was recycled — once `first`/`last`
            // stretch the result past `text`, R drops them (`substring(c(a =
            // "hello"), 1:3)` is unnamed).
            let tv = a.req(0, "text")?;
            if n == text.len() {
                carry_attrs(&out, &tv, &tv);
            }
            Ok(out)
        }
        "strrep" => {
            let x = as_str(&a.req(0, "x")?);
            let times = as_int(&a.req(1, "times")?);
            let n = x.len().max(times.len());
            Ok(mk_str(
                (0..n)
                    .map(|i| {
                        let s = x.get(i % x.len().max(1)).cloned().flatten()?;
                        let t = times[i % times.len().max(1)].unwrap_or(0).max(0) as usize;
                        Some(s.repeat(t))
                    })
                    .collect(),
            ))
        }
        "encodeString" => Ok(mk_str(
            as_str(&a.req(0, "x")?)
                .into_iter()
                .map(|s| s.map(|s| encode_string(&s)))
                .collect(),
        )),
        "startsWith" | "endsWith" => {
            // Both `x` and the prefix/suffix recycle to the longer length.
            let x = as_str(&a.req(0, "x")?);
            let p = as_str(&a.req(1, "prefix")?);
            let n = x.len().max(p.len());
            Ok(mk_lgl(
                (0..n)
                    .map(|i| {
                        let s = x.get(i % x.len().max(1)).cloned().flatten()?;
                        let pre = p.get(i % p.len().max(1)).cloned().flatten()?;
                        Some(if name == "startsWith" {
                            s.starts_with(&pre)
                        } else {
                            s.ends_with(&pre)
                        })
                    })
                    .collect(),
            ))
        }
        "strsplit" => {
            let x = as_str(&a.req(0, "x")?);
            let sep = str1(&a.req(1, "split")?).unwrap_or_default();
            let fixed = a.named("fixed").and_then(|v| lgl1(&v)).unwrap_or(false);
            // R's `split` is a regular expression by default (POSIX ERE);
            // `fixed = TRUE` or an empty pattern falls back to character/literal
            // splitting. A blank pattern still means "every character".
            let re = if sep.is_empty() {
                None
            } else {
                let pat = if fixed {
                    regex::escape(&sep)
                } else {
                    sep.clone()
                };
                Some(
                    cached_regex(&pat)
                        .map_err(|e| format!("invalid regular expression '{sep}': {e}"))?,
                )
            };
            let parts: Vec<Value> = x
                .iter()
                .map(|s| match s {
                    Some(s) => {
                        let pieces: Vec<Option<String>> = match &re {
                            None => s.chars().map(|c| Some(c.to_string())).collect(),
                            Some(re) => r_strsplit(s, re),
                        };
                        mk_str(pieces)
                    }
                    None => mk_str(vec![None]),
                })
                .collect();
            Ok(mk_list(parts))
        }
        "sub" | "gsub" | "grepl" | "grep" => regex_op(name, &a),
        "chartr" => {
            // R expands `a-c` ranges in both `old` and `new`.
            let old = expand_char_ranges(&str1(&a.req(0, "old")?).unwrap_or_default());
            let new = expand_char_ranges(&str1(&a.req(1, "new")?).unwrap_or_default());
            // R errors only when `old` outruns `new`; extra `new` characters are
            // simply ignored (the `zip` below stops at the shorter).
            if old.len() > new.len() {
                return Err("'old' is longer than 'new'".into());
            }
            // A char repeated in `old` takes its LAST mapping, as R does; a
            // HashMap built in order overwrites earlier entries.
            let map: std::collections::HashMap<char, char> =
                old.iter().copied().zip(new.iter().copied()).collect();
            let xv = a.req(2, "x")?;
            shaped_like(
                mk_str(
                    as_str(&xv)
                        .iter()
                        .map(|s| {
                            s.as_ref()
                                .map(|s| s.chars().map(|c| *map.get(&c).unwrap_or(&c)).collect())
                        })
                        .collect(),
                ),
                &xv,
            )
        }
        "strtoi" => {
            let x = as_str(&a.req(0, "x")?);
            let base = a.get(1, "base").and_then(|v| num1(&v)).unwrap_or(10.0) as u32;
            Ok(mk_int(
                x.iter()
                    .map(|s| {
                        s.as_ref().and_then(|s| {
                            let t = s.trim();
                            // C strtol semantics: base 16 accepts an optional
                            // `0x`/`0X` prefix (which Rust's from_str_radix rejects).
                            let t = if base == 16 {
                                t.strip_prefix("0x")
                                    .or_else(|| t.strip_prefix("0X"))
                                    .unwrap_or(t)
                            } else {
                                t
                            };
                            i64::from_str_radix(t, base).ok()
                        })
                    })
                    .collect(),
            ))
        }
        "regexpr" | "gregexpr" => {
            let pat = str1(&a.req(0, "pattern")?).unwrap_or_default();
            let re = cached_regex(&pat)
                .map_err(|e| format!("invalid regular expression '{pat}': {e}"))?;
            let x = as_str(&a.req(1, "text")?);
            // R matches all-ASCII input on the byte path and reports that with
            // `index.type`/`useBytes`; multibyte input takes the char path and
            // carries `match.length` alone.
            let ascii = pat.is_ascii() && x.iter().flatten().all(|s| s.is_ascii());
            let tag = |v: &Value| {
                if ascii {
                    let it = scalar_str("chars");
                    let ub = scalar_lgl(true);
                    with_host(|h| {
                        h.set_attr(v, "index.type", it);
                        h.set_attr(v, "useBytes", ub);
                    });
                }
            };
            if name == "regexpr" {
                // First match per element: a 1-based char position (or -1), with
                // the match width carried on the `match.length` attribute so
                // `regmatches` can slice it back out.
                let (mut starts, mut lens) = (Vec::new(), Vec::new());
                for s in &x {
                    match s.as_ref().and_then(|s| re.find(s).map(|m| (s, m))) {
                        Some((s, m)) => {
                            starts.push(Some(char_pos(s, m.start()) as i64 + 1));
                            lens.push(Some(s[m.start()..m.end()].chars().count() as i64));
                        }
                        None => {
                            starts.push(Some(-1));
                            lens.push(Some(-1));
                        }
                    }
                }
                let out = mk_int(starts);
                let ml = mk_int(lens);
                with_host(|h| h.set_attr(&out, "match.length", ml));
                tag(&out);
                Ok(out)
            } else {
                // All matches per element, as a list of position vectors each
                // carrying its own `match.length`.
                let per: Vec<Value> = x
                    .iter()
                    .map(|s| {
                        let (mut starts, mut lens) = (Vec::new(), Vec::new());
                        if let Some(s) = s {
                            for m in re.find_iter(s) {
                                starts.push(Some(char_pos(s, m.start()) as i64 + 1));
                                lens.push(Some(s[m.start()..m.end()].chars().count() as i64));
                            }
                        }
                        if starts.is_empty() {
                            starts.push(Some(-1));
                            lens.push(Some(-1));
                        }
                        let v = mk_int(starts);
                        let ml = mk_int(lens);
                        with_host(|h| h.set_attr(&v, "match.length", ml));
                        tag(&v);
                        v
                    })
                    .collect();
                Ok(mk_list(per))
            }
        }
        "regmatches" => {
            let x = as_str(&a.req(0, "x")?);
            let m = a.req(1, "m")?;
            let extract = |s: &str, start: i64, len: i64| -> String {
                s.chars()
                    .skip((start - 1).max(0) as usize)
                    .take(len.max(0) as usize)
                    .collect()
            };
            match data(&m) {
                // `gregexpr` result: one character vector of all matches per
                // element, returned as a list.
                RData::List(items) => {
                    let out: Vec<Value> = items
                        .iter()
                        .enumerate()
                        .map(|(i, mi)| {
                            let starts = as_int(mi);
                            let lens = with_host(|h| h.attr(mi, "match.length"))
                                .map(|v| as_int(&v))
                                .unwrap_or_default();
                            let s = x.get(i).cloned().flatten().unwrap_or_default();
                            let hits: Vec<Option<String>> = starts
                                .iter()
                                .zip(lens.iter())
                                .filter_map(|(st, ln)| {
                                    let st = (*st)?;
                                    (st >= 1).then(|| Some(extract(&s, st, ln.unwrap_or(0))))
                                })
                                .collect();
                            mk_str(hits)
                        })
                        .collect();
                    Ok(mk_list(out))
                }
                // `regexpr` result: drop the non-matches (start == -1).
                _ => {
                    let starts = as_int(&m);
                    let lens = with_host(|h| h.attr(&m, "match.length"))
                        .map(|v| as_int(&v))
                        .unwrap_or_default();
                    let mut out = Vec::new();
                    for (i, s) in x.iter().enumerate() {
                        let st = starts.get(i).and_then(|e| *e).unwrap_or(-1);
                        if st < 1 {
                            continue;
                        }
                        let ln = lens.get(i).and_then(|e| *e).unwrap_or(0);
                        if let Some(s) = s {
                            out.push(Some(extract(s, st, ln)));
                        }
                    }
                    Ok(mk_str(out))
                }
            }
        }

        // ── apply family ────────────────────────────────────────────────
        "lapply" | "sapply" => {
            let x = a.req(0, "X")?;
            let f = a.req(1, "FUN")?;
            // `simplify`/`USE.NAMES` are sapply's own controls — not `...` args
            // to forward to FUN.
            let extra: Vec<(Option<String>, Value)> = a
                .rest(2)
                .into_iter()
                .filter(|(t, _)| !matches!(t.as_deref(), Some("simplify") | Some("USE.NAMES")))
                .collect();
            let items = elements(&x);
            // `sapply` is R code whose body is `lapply(X = X, FUN = FUN, ...)`,
            // so a condition it does not attribute to `FUN` itself reports
            // *that* call rather than the `sapply(…)` the user wrote.
            let via_lapply = name == "sapply";
            if via_lapply {
                push_context("lapply(X = X, FUN = FUN, ...)");
            }
            let mut out = Vec::with_capacity(items.len());
            let mut err = None;
            for it in items {
                let mut call_args = vec![(None, it)];
                call_args.extend(extra.clone());
                match call_fun(&f, call_args, FUN_CALL) {
                    Ok(v) => out.push(v),
                    Err(e) => {
                        err = Some(e);
                        break;
                    }
                }
            }
            if via_lapply {
                with_host(|h| {
                    h.calls.pop();
                });
            }
            if let Some(e) = err {
                return Err(e);
            }
            let res = mk_list(out);
            let nm = names_of(&x);
            if !nm.is_empty() {
                set_names(&res, nm.clone());
            } else if kind(&x) == RKind::Str && name == "sapply" {
                set_names(&res, as_str(&x));
            }
            Ok(if name == "sapply" {
                simplify(&res)
            } else {
                res
            })
        }
        "vapply" => {
            let x = a.req(0, "X")?;
            let f = a.req(1, "FUN")?;
            let items = elements(&x);
            // With nothing to map over there is no result to infer a type from,
            // so `FUN.VALUE` — which exists precisely to pin the result type —
            // supplies it. `simplify` on an empty list returned `list()`, but R
            // answers `integer(0)` for a `FUN.VALUE` of `integer(1)`. A
            // character `X` also still contributes its (empty) names, which is
            // what makes R print `named integer(0)`.
            if items.is_empty() {
                let proto = a.req(2, "FUN.VALUE")?;
                let out = take_positions(&proto, &[]);
                if kind(&x) == RKind::Str {
                    // `mk_str` borrows the host itself, so it has to be built
                    // before the `set_attr` borrow, not inside it.
                    let nv = mk_str(vec![]);
                    with_host(|h| h.set_attr(&out, "names", nv));
                }
                return Ok(out);
            }
            let mut out = Vec::with_capacity(items.len());
            for it in items {
                out.push(call_fun(&f, vec![(None, it)], FUN_CALL)?);
            }
            // `vapply` carries names exactly as `sapply` does — from `X`, or from
            // a character `X` used as its own labels (`USE.NAMES`).
            let res = mk_list(out);
            let nm = names_of(&x);
            if !nm.is_empty() {
                set_names(&res, nm);
            } else if kind(&x) == RKind::Str {
                set_names(&res, as_str(&x));
            }
            Ok(simplify(&res))
        }
        "Map" => {
            let f = a.req(0, "f")?;
            let lists: Vec<Vec<Value>> = a.rest(1).iter().map(|(_, v)| elements(v)).collect();
            let n = lists.iter().map(|l| l.len()).min().unwrap_or(0);
            let mut out = Vec::with_capacity(n);
            for i in 0..n {
                let call_args: Vec<(Option<String>, Value)> =
                    lists.iter().map(|l| (None, l[i].clone())).collect();
                out.push(call_value(&f, call_args, None)?);
            }
            Ok(mk_list(out))
        }
        "mapply" => {
            // Like `Map` but simplified to an atomic vector when every result
            // is a scalar, matching R's default `SIMPLIFY = TRUE`.
            let f = a.req(0, "FUN")?;
            let rest = a.rest(1);
            let lists: Vec<Vec<Value>> = rest.iter().map(|(_, v)| elements(v)).collect();
            let n = lists.iter().map(|l| l.len()).max().unwrap_or(0);
            let mut out = Vec::with_capacity(n);
            for i in 0..n {
                let call_args: Vec<(Option<String>, Value)> = lists
                    .iter()
                    .map(|l| (None, l[i % l.len().max(1)].clone()))
                    .collect();
                out.push(call_value(&f, call_args, None)?);
            }
            // `USE.NAMES`: the result is labelled from the FIRST mapped argument,
            // by its names or — for a character vector — by its own values.
            let res = mk_list(out);
            if let Some((_, first)) = rest.first() {
                let nm = names_of(first);
                if !nm.is_empty() {
                    set_names(&res, nm);
                } else if kind(first) == RKind::Str {
                    set_names(&res, as_str(first));
                }
            }
            Ok(simplify(&res))
        }
        "Filter" => {
            let f = a.req(0, "f")?;
            let x = a.req(1, "x")?;
            let items = elements(&x);
            let nm = names_of(&x);
            let mut keep = Vec::new();
            for (i, it) in items.iter().enumerate() {
                let r = call_fun(&f, vec![(None, it.clone())], FUN_CALL)?;
                if as_lgl(&r).first() == Some(&Some(true)) {
                    keep.push(Some(i));
                }
            }
            let out = take_positions(&x, &keep);
            if !nm.is_empty() {
                set_names(
                    &out,
                    keep.iter()
                        .map(|p| p.and_then(|i| nm.get(i).cloned().flatten()))
                        .collect(),
                );
            }
            Ok(out)
        }
        "Position" | "Find" => {
            let f = a.req(0, "f")?;
            let x = a.req(1, "x")?;
            for (i, it) in elements(&x).into_iter().enumerate() {
                let r = call_fun(&f, vec![(None, it.clone())], FUN_CALL)?;
                if as_lgl(&r).first() == Some(&Some(true)) {
                    return Ok(if name == "Position" {
                        scalar_int(i as i64 + 1)
                    } else {
                        it
                    });
                }
            }
            // No match: `Position` yields integer NA, `Find` yields NULL.
            Ok(if name == "Position" {
                mk_int(vec![None])
            } else {
                null()
            })
        }
        "Reduce" => {
            let f = a.req(0, "f")?;
            let x = a.req(1, "x")?;
            let accumulate = a
                .named("accumulate")
                .and_then(|v| lgl1(&v))
                .unwrap_or(false);
            let from_right = a.named("right").and_then(|v| lgl1(&v)).unwrap_or(false);
            let mut seq = elements(&x);
            if from_right {
                seq.reverse();
            }
            let mut it = seq.into_iter();
            let mut acc = match a.get(2, "init") {
                Some(v) => v,
                None => match it.next() {
                    Some(v) => v,
                    None => return Ok(null()),
                },
            };
            let mut steps = vec![acc.clone()];
            for e in it {
                // `right = TRUE` folds as f(elem, acc); otherwise f(acc, elem).
                let args = if from_right {
                    vec![(None, e), (None, acc)]
                } else {
                    vec![(None, acc), (None, e)]
                };
                acc = call_fun(&f, args, "f(init, x[[i]])")?;
                steps.push(acc.clone());
            }
            if accumulate {
                // A right fold's accumulated steps read back in original order.
                if from_right {
                    steps.reverse();
                }
                Ok(simplify(&mk_list(steps)))
            } else {
                Ok(acc)
            }
        }
        "split" => {
            let x = a.req(0, "x")?;
            let f = as_str_labels(&a.req(1, "f")?);
            // Groups appear in sorted-level order (R uses the factor levels).
            let levels = factor_levels(&a.req(1, "f")?);
            let groups: Vec<Value> = levels
                .iter()
                .map(|lev| {
                    let pos: Vec<Option<usize>> = f
                        .iter()
                        .enumerate()
                        .filter(|(_, k)| k.as_deref() == Some(lev.as_str()))
                        .map(|(i, _)| Some(i))
                        .collect();
                    let group = take_positions(&x, &pos);
                    // Splitting a factor yields factors, levels intact.
                    carry_factor(&group, &x);
                    group
                })
                .collect();
            let out = mk_list(groups);
            set_names(&out, levels.into_iter().map(Some).collect());
            Ok(out)
        }
        "tapply" => {
            let x = a.req(0, "X")?;
            let index = as_str_labels(&a.req(1, "INDEX")?);
            let f = a.req(2, "FUN")?;
            let levels = factor_levels(&a.req(1, "INDEX")?);
            let mut results = Vec::with_capacity(levels.len());
            for lev in &levels {
                let pos: Vec<Option<usize>> = index
                    .iter()
                    .enumerate()
                    .filter(|(_, k)| k.as_deref() == Some(lev.as_str()))
                    .map(|(i, _)| Some(i))
                    .collect();
                let group = take_positions(&x, &pos);
                results.push(call_value(&f, vec![(None, group)], None)?);
            }
            let out = simplify(&mk_list(results));
            set_names(&out, levels.into_iter().map(Some).collect());
            Ok(out)
        }
        "modifyList" => {
            let x = a.req(0, "x")?;
            let val = a.req(1, "val")?;
            let mut names = names_of(&x);
            let mut items = elements(&x);
            let vnames = names_of(&val);
            for (i, v) in elements(&val).into_iter().enumerate() {
                let key = vnames.get(i).cloned().flatten();
                // Replace an existing key by name, else append.
                match key
                    .as_ref()
                    .and_then(|k| names.iter().position(|n| n.as_deref() == Some(k.as_str())))
                {
                    Some(p) => items[p] = v,
                    None => {
                        items.push(v);
                        names.push(key);
                    }
                }
            }
            let out = mk_list(items);
            set_names(&out, names);
            Ok(out)
        }
        "rapply" => {
            // Apply FUN to each leaf, keeping the shape. `how = "list"` hands
            // the nested list back as it is; `"unlist"` flattens it, and the
            // names ride along either way — `rapply(list(a = 1:3), length)` is
            // a *named* 3, not a bare one.
            let x = a.req(0, "object")?;
            let f = a.req(1, "f")?;
            let how = a
                .named("how")
                .and_then(|v| str1(&v))
                .unwrap_or_else(|| "unlist".into());
            fn walk(v: &Value, f: &Value) -> Result<Value, String> {
                match data(v) {
                    RData::List(items) => {
                        let mapped: Result<Vec<Value>, String> =
                            items.iter().map(|it| walk(it, f)).collect();
                        let out = mk_list(mapped?);
                        let nm = names_of(v);
                        if !nm.is_empty() {
                            set_names(&out, nm);
                        }
                        Ok(out)
                    }
                    _ => call_value(f, vec![(None, v.clone())], None),
                }
            }
            let out = walk(&x, &f)?;
            Ok(match how.as_str() {
                "unlist" => unlist(&out),
                _ => out,
            })
        }
        "do.call" => {
            let f = a.req(0, "what")?;
            let f = match str1(&f) {
                Some(fname) if !with_host(|h| h.is_function(&f)) => {
                    with_host(|h| h.lookup_function(&fname))
                        .or_else(|| primitive_value(&fname))
                        .ok_or_else(|| format!("could not find function \"{fname}\""))?
                }
                _ => f,
            };
            let argl = a.req(1, "args")?;
            let nm = names_of(&argl);
            let items = elements(&argl);
            let call_args: Vec<(Option<String>, Value)> = items
                .into_iter()
                .enumerate()
                .map(|(i, v)| (nm.get(i).cloned().flatten(), v))
                .collect();
            // R builds the call itself, with the function inline, so a
            // condition raised inside reports the deparsed function rather than
            // the `do.call(…)` that arranged it.
            call_fun(&f, call_args, &inline_call_text(&f))
        }
        "Negate" => {
            let f = a.req(0, "f")?;
            Ok(with_host(|h| {
                h.alloc(RData::Combinator {
                    kind: CombinatorKind::Negate,
                    inner: f,
                })
            }))
        }
        "Vectorize" => {
            let f = a.req(0, "FUN")?;
            Ok(with_host(|h| {
                h.alloc(RData::Combinator {
                    kind: CombinatorKind::Vectorize,
                    inner: f,
                })
            }))
        }

        // ── matrices ────────────────────────────────────────────────────
        "matrix" => {
            let x = a.get(0, "data").unwrap_or_else(|| mk_lgl(vec![None]));
            let n = len(&x).max(1);
            let nrow = a.get(1, "nrow").and_then(|v| num1(&v)).map(|v| v as usize);
            let ncol = a.get(2, "ncol").and_then(|v| num1(&v)).map(|v| v as usize);
            let (nr, nc) = match (nrow, ncol) {
                (Some(r), Some(c)) => (r, c),
                (Some(r), None) => (r, n.div_ceil(r)),
                (None, Some(c)) => (n.div_ceil(c), c),
                (None, None) => (n, 1),
            };
            let total = nr * nc;
            if let Some(msg) = matrix_fill_warning(len(&x), nr, nc) {
                signal_warning(&msg)?;
            }
            let byrow = a.named("byrow").and_then(|v| lgl1(&v)).unwrap_or(false);
            // R stores matrices column-major. With `byrow = TRUE` the data fills
            // rows first, so column-major slot `c*nr + r` draws from row-major
            // source index `r*nc + c`.
            let pos: Vec<Option<usize>> = if byrow {
                let mut p = vec![Some(0usize); total];
                for r in 0..nr {
                    for c in 0..nc {
                        p[c * nr + r] = Some((r * nc + c) % n);
                    }
                }
                p
            } else {
                (0..total).map(|i| Some(i % n)).collect()
            };
            let out = take_positions(&x, &pos);
            let dim = mk_int(vec![Some(nr as i64), Some(nc as i64)]);
            with_host(|h| h.set_attr(&out, "dim", dim));
            // `dimnames = list(rownames, colnames)`, either element possibly NULL.
            if let Some(dn) = a.get(4, "dimnames").filter(|v| kind(v) != RKind::Null) {
                let parts = elements(&dn);
                let pick = |i: usize| -> Option<Vec<Option<String>>> {
                    parts.get(i).filter(|e| kind(e) != RKind::Null).map(as_str)
                };
                set_dimnames(&out, pick(0), pick(1));
            }
            Ok(out)
        }
        "t" => {
            let x = a.req(0, "x")?;
            let d = with_host(|h| h.attr(&x, "dim"))
                .map(|d| as_int(&d))
                .unwrap_or_default();
            // A dim-less vector transposes as an n×1 *column*, so `t(1:3)` is the
            // 1×3 row matrix — treating it as 1×n instead produced a 3×1 column,
            // the transpose of R's answer. Its `names` become the result's
            // column labels (`t(c(a=1,b=2))` heads the columns `a b`).
            let (nr, nc) = match d.as_slice() {
                [Some(r), Some(c)] => (*r as usize, *c as usize),
                _ => (len(&x), 1),
            };
            let mut pos = Vec::with_capacity(nr * nc);
            for r in 0..nr {
                for c in 0..nc {
                    pos.push(Some(c * nr + r));
                }
            }
            let out = take_positions(&x, &pos);
            let dim = mk_int(vec![Some(nc as i64), Some(nr as i64)]);
            with_host(|h| h.set_attr(&out, "dim", dim));
            // Transposing swaps the margins, so the row and column labels swap
            // with them; a plain named vector contributes its names as the
            // single row's column labels.
            let (rn, cn) = match d.as_slice() {
                [Some(_), Some(_)] => {
                    let mut dn = dimnames_of(&x);
                    dn.resize(2, None);
                    (dn[1].clone(), dn[0].clone())
                }
                _ => {
                    let nm = names_of(&x);
                    (None, (!nm.is_empty()).then_some(nm))
                }
            };
            if rn.is_some() || cn.is_some() {
                set_dimnames(&out, rn, cn);
            }
            Ok(out)
        }
        "array" => {
            let data = a.get(0, "data").unwrap_or_else(|| mk_lgl(vec![None]));
            let dims: Vec<usize> = match a.get(1, "dim") {
                Some(d) => as_int(&d)
                    .into_iter()
                    .map(|e| e.unwrap_or(0) as usize)
                    .collect(),
                None => vec![len(&data)],
            };
            let total: usize = dims.iter().product();
            let n = len(&data).max(1);
            let pos: Vec<Option<usize>> = (0..total).map(|i| Some(i % n)).collect();
            let out = take_positions(&data, &pos);
            let dim = mk_int(dims.iter().map(|&d| Some(d as i64)).collect());
            with_host(|h| h.set_attr(&out, "dim", dim));
            // `dimnames` is one label vector per margin, of any rank — the same
            // list the N-D subscript and print paths read back.
            if let Some(dn) = a.get(2, "dimnames").filter(|v| !is_null(v)) {
                with_host(|h| h.set_attr(&out, "dimnames", dn));
            }
            Ok(out)
        }
        "aperm" => {
            // Permute an array's dimensions (default: reverse — a transpose).
            let x = a.req(0, "a")?;
            let dims = dims_of(&x);
            let k = dims.len();
            let perm: Vec<usize> = match a.get(1, "perm") {
                Some(p) => as_int(&p)
                    .into_iter()
                    .flatten()
                    .map(|m| (m - 1) as usize)
                    .collect(),
                None => (0..k).rev().collect(),
            };
            let mut stride = vec![1usize; k];
            for d in 1..k {
                stride[d] = stride[d - 1] * dims[d - 1];
            }
            let new_dims: Vec<usize> = perm.iter().map(|&p| dims[p]).collect();
            let total: usize = dims.iter().product();
            let mut pos = Vec::with_capacity(total);
            // Walk the OUTPUT in column-major order, mapping each cell back to the
            // source linear index via the permuted strides.
            let mut idx = vec![0usize; k];
            for _ in 0..total {
                let lin: usize = (0..k).map(|d| idx[d] * stride[perm[d]]).sum();
                pos.push(Some(lin));
                for d in 0..k {
                    idx[d] += 1;
                    if idx[d] < new_dims[d] {
                        break;
                    }
                    idx[d] = 0;
                }
            }
            let out = take_positions(&x, &pos);
            let dim = mk_int(new_dims.iter().map(|&n| Some(n as i64)).collect());
            with_host(|h| h.set_attr(&out, "dim", dim));
            Ok(out)
        }
        "rowSums" | "colSums" | "rowMeans" | "colMeans" => {
            let x = a.req(0, "x")?;
            let dims = dims_of(&x);
            let data = as_dbl(&x);
            let by_row = name.starts_with("row");
            let mean = name.ends_with("Means");
            // `row*` keeps the first dimension and reduces the rest; `col*`
            // reduces the first and keeps the rest (a matrix for 3-D+ input).
            let keep: Vec<usize> = if by_row {
                vec![0]
            } else {
                (1..dims.len()).collect()
            };
            let reduce: Vec<usize> = (0..dims.len()).filter(|d| !keep.contains(d)).collect();
            let mut stride = vec![1usize; dims.len()];
            for d in 1..dims.len() {
                stride[d] = stride[d - 1] * dims[d - 1];
            }
            let keep_shape: Vec<usize> = keep.iter().map(|&d| dims[d]).collect();
            let red_shape: Vec<usize> = reduce.iter().map(|&d| dims[d]).collect();
            let ktotal: usize = keep_shape.iter().product::<usize>().max(1);
            let rtotal: usize = red_shape.iter().product::<usize>().max(1);
            let mut out = Vec::with_capacity(ktotal);
            let mut kidx = vec![0usize; keep.len()];
            for _ in 0..ktotal {
                let base: usize = keep
                    .iter()
                    .enumerate()
                    .map(|(i, &d)| kidx[i] * stride[d])
                    .sum();
                let mut acc = 0.0;
                let mut ridx = vec![0usize; reduce.len()];
                for _ in 0..rtotal {
                    let off: usize = reduce
                        .iter()
                        .enumerate()
                        .map(|(i, &d)| ridx[i] * stride[d])
                        .sum();
                    acc += data.get(base + off).and_then(|e| *e).unwrap_or(f64::NAN);
                    for i in 0..reduce.len() {
                        ridx[i] += 1;
                        if ridx[i] < red_shape[i] {
                            break;
                        }
                        ridx[i] = 0;
                    }
                }
                out.push(Some(if mean { acc / rtotal as f64 } else { acc }));
                for i in 0..keep.len() {
                    kidx[i] += 1;
                    if kidx[i] < keep_shape[i] {
                        break;
                    }
                    kidx[i] = 0;
                }
            }
            let res = mk_dbl(out);
            if keep.len() >= 2 {
                let d = mk_int(keep_shape.iter().map(|&n| Some(n as i64)).collect());
                with_host(|h| h.set_attr(&res, "dim", d));
            } else if keep.len() == 1 {
                // A 1-D reduction keeps the retained dimension's labels as names.
                if let Some(Some(names)) = dimnames_of(&x).get(keep[0]) {
                    set_names(&res, names.clone());
                }
            }
            Ok(res)
        }
        "apply" => {
            let x = a.req(0, "X")?;
            let dims = dims_of(&x);
            let margins: Vec<usize> = as_int(&a.req(1, "MARGIN")?)
                .into_iter()
                .flatten()
                .map(|m| (m - 1) as usize)
                .collect();
            let f = a.req(2, "FUN")?;
            let others: Vec<usize> = (0..dims.len()).filter(|d| !margins.contains(d)).collect();
            let mut stride = vec![1usize; dims.len()];
            for d in 1..dims.len() {
                stride[d] = stride[d - 1] * dims[d - 1];
            }
            let dn = dimnames_of(&x);
            let margin_shape: Vec<usize> = margins.iter().map(|&d| dims[d]).collect();
            let other_shape: Vec<usize> = others.iter().map(|&d| dims[d]).collect();
            let mtotal: usize = margin_shape.iter().product::<usize>().max(1);
            let ototal: usize = other_shape.iter().product::<usize>().max(1);
            let mut results = Vec::with_capacity(mtotal);
            let mut midx = vec![0usize; margins.len()];
            for _ in 0..mtotal {
                let base: usize = margins
                    .iter()
                    .enumerate()
                    .map(|(k, &d)| midx[k] * stride[d])
                    .sum();
                let mut slice_pos = Vec::with_capacity(ototal);
                let mut oidx = vec![0usize; others.len()];
                for _ in 0..ototal {
                    let off: usize = others
                        .iter()
                        .enumerate()
                        .map(|(k, &d)| oidx[k] * stride[d])
                        .sum();
                    slice_pos.push(Some(base + off));
                    for k in 0..others.len() {
                        oidx[k] += 1;
                        if oidx[k] < other_shape[k] {
                            break;
                        }
                        oidx[k] = 0;
                    }
                }
                let slice = take_positions(&x, &slice_pos);
                // A rank ≥ 2 sub-array keeps its `dim` so FUN sees a matrix; a
                // 1-D slice keeps the labels of the dimension it runs along, so
                // `apply(m, 1, f)` hands `f` a *named* row.
                if other_shape.len() >= 2 {
                    let d = mk_int(other_shape.iter().map(|&n| Some(n as i64)).collect());
                    with_host(|h| h.set_attr(&slice, "dim", d));
                    let labels = mk_list(
                        others
                            .iter()
                            .map(|&d| match dn.get(d).cloned().flatten() {
                                Some(names) => mk_str(names),
                                None => null(),
                            })
                            .collect(),
                    );
                    if elements(&labels).iter().any(|e| !is_null(e)) {
                        with_host(|h| h.set_attr(&slice, "dimnames", labels));
                    }
                } else if let Some(Some(names)) =
                    others.first().map(|&d| dn.get(d).cloned().flatten())
                {
                    set_names(&slice, names);
                }
                results.push(call_fun(&f, vec![(None, slice)], "FUN(newX[, i], ...)")?);
                for k in 0..margins.len() {
                    midx[k] += 1;
                    if midx[k] < margin_shape[k] {
                        break;
                    }
                    midx[k] = 0;
                }
            }
            // The labels of the margins iterated over, and of one result — R
            // carries both onto the answer: the margin labels name the slots the
            // results landed in, the result's own names label what FUN returned.
            let margin_dn: Vec<Option<Vec<Option<String>>>> = margins
                .iter()
                .map(|&d| dn.get(d).cloned().flatten())
                .collect();
            let result_names = results
                .first()
                .map(names_of)
                .filter(|n| n.iter().any(|e| e.is_some()));
            let out = simplify(&mk_list(results));
            let has_dim = with_host(|h| h.attr(&out, "dim")).is_some();
            if margins.len() >= 2 && !has_dim {
                // Several margins with scalar results reshape to an array over
                // those margins, labels and all.
                let d = mk_int(margin_shape.iter().map(|&n| Some(n as i64)).collect());
                with_host(|h| h.set_attr(&out, "dim", d));
                if margin_dn.iter().any(|m| m.is_some()) {
                    let labels = mk_list(
                        margin_dn
                            .iter()
                            .map(|m| match m {
                                Some(names) => mk_str(names.clone()),
                                None => null(),
                            })
                            .collect(),
                    );
                    with_host(|h| h.set_attr(&out, "dimnames", labels));
                }
            } else if !has_dim {
                // One margin, one value per slice: a vector named by the margin.
                if let Some(Some(names)) = margin_dn.first() {
                    if len(&out) == names.len() {
                        set_names(&out, names.clone());
                    }
                }
            } else if margins.len() == 1 {
                // One margin, a vector per slice: R stacks them column-wise, so
                // the result's own names become the rows and the margin's the
                // columns.
                set_dimnames(&out, result_names, margin_dn[0].clone());
            }
            Ok(out)
        }
        "diag" => {
            let x = a.req(0, "x")?;
            let d = with_host(|h| h.attr(&x, "dim"));
            match d {
                // `diag(matrix)` extracts the main diagonal.
                Some(_) => {
                    let (nr, nc) = mat_dim(&x);
                    let k = nr.min(nc);
                    let pos: Vec<Option<usize>> = (0..k).map(|i| Some(i * nr + i)).collect();
                    Ok(take_positions(&x, &pos))
                }
                // `diag(n)` for a length-1 numeric builds the n×n identity.
                None if matches!(kind(&x), RKind::Int | RKind::Dbl) && len(&x) == 1 => {
                    let n = num1(&x).unwrap_or(0.0) as usize;
                    let mut vals = vec![Some(0.0); n * n];
                    for i in 0..n {
                        vals[i * n + i] = Some(1.0);
                    }
                    let out = mk_dbl(vals);
                    let dim = mk_int(vec![Some(n as i64), Some(n as i64)]);
                    with_host(|h| h.set_attr(&out, "dim", dim));
                    Ok(out)
                }
                // `diag(vector)` builds a diagonal matrix from the values.
                None => {
                    let v = as_dbl(&x);
                    let n = v.len();
                    let mut vals = vec![Some(0.0); n * n];
                    for (i, e) in v.iter().enumerate() {
                        vals[i * n + i] = *e;
                    }
                    let out = mk_dbl(vals);
                    let dim = mk_int(vec![Some(n as i64), Some(n as i64)]);
                    with_host(|h| h.set_attr(&out, "dim", dim));
                    Ok(out)
                }
            }
        }
        "%*%" => {
            let x = a.req(0, "x")?;
            let y = a.req(1, "y")?;
            Ok(mat_mul(&x, &y))
        }
        "crossprod" | "tcrossprod" => {
            // crossprod(x, y) = t(x) %*% y ; tcrossprod(x, y) = x %*% t(y).
            let x = a.req(0, "x")?;
            let y = a.get(1, "y").unwrap_or_else(|| x.clone());
            let out = if name == "crossprod" {
                mat_mul(&transpose(&x), &y)
            } else {
                mat_mul(&x, &transpose(&y))
            };
            Ok(out)
        }
        "outer" | "%o%" => {
            let xv = a.req(0, "X")?;
            let yv = a.req(1, "Y")?;
            let (nx, ny) = (len(&xv), len(&yv));
            // R tiles the inputs (X repeated ny times, each Y element repeated nx
            // times) and calls FUN *once* on the pair, so the result keeps FUN's
            // own type — strings from `paste0`, logicals from `==`, etc. — rather
            // than being forced to double element-by-element. Column-major slot
            // j*nx + i pairs X[i] with Y[j].
            let xe = take_positions(&xv, &(0..nx * ny).map(|k| Some(k % nx)).collect::<Vec<_>>());
            let ye = take_positions(&yv, &(0..nx * ny).map(|k| Some(k / nx)).collect::<Vec<_>>());
            let fun = a.get(2, "FUN");
            let res = match &fun {
                Some(f) if with_host(|h| h.is_function(f)) => {
                    call_value(f, vec![(None, xe), (None, ye)], None)?
                }
                // A bare operator name (the default is "*").
                other => {
                    let op = other.as_ref().and_then(str1);
                    binop(op.as_deref().unwrap_or("*"), &xe, &ye)?
                }
            };
            let dim = mk_int(vec![Some(nx as i64), Some(ny as i64)]);
            with_host(|h| h.set_attr(&res, "dim", dim));
            // R labels the result's margins with the inputs' names, so
            // `outer(c(a=1,b=2), c(x=1,y=2))` prints with `a`/`b` rows and
            // `x`/`y` columns rather than `[1,]`/`[,1]`.
            let (xn, yn) = (names_of(&xv), names_of(&yv));
            if !xn.is_empty() || !yn.is_empty() {
                let side = |nm: Vec<Option<String>>| match nm.is_empty() {
                    true => null(),
                    false => mk_str(nm),
                };
                let dn = mk_list(vec![side(xn), side(yn)]);
                with_host(|h| h.set_attr(&res, "dimnames", dn));
            }
            Ok(res)
        }
        "cbind" | "rbind" => Ok(bind_matrix(&a, name == "cbind")),

        // ── environments and dispatch ───────────────────────────────────
        "exists" => {
            let n = str1(&a.req(0, "x")?).unwrap_or_default();
            // The base constants (`pi`, `letters`, `month.name`, …) are bindings
            // in R's base environment just as the primitives are, so `exists`
            // has to see them too — it reported FALSE for `pi` while `get("pi")`
            // returned 3.141593.
            Ok(scalar_lgl(
                with_host(|h| h.exists(&n)) || is_primitive(&n) || base_constant(&n).is_some(),
            ))
        }
        "get" => {
            let n = str1(&a.req(0, "x")?).unwrap_or_default();
            with_host(|h| h.lookup(&n))
                .or_else(|| primitive_value(&n))
                .ok_or_else(|| format!("object '{n}' not found"))
        }
        "assign" => {
            let n = str1(&a.req(0, "x")?).unwrap_or_default();
            let v = a.req(1, "value")?;
            // `envir = e` binds in *that* environment, not the caller's:
            // `assign("v", 9, envir = e)` leaves `v` unbound where it was
            // written, which is the whole reason for passing one.
            let env = a.named("envir").and_then(|e| env_of(&e));
            with_host(|h| {
                match &env {
                    Some(e) => h.bind(e, &n, v.clone()),
                    None => h.set_var(&n, v.clone()),
                }
                h.visible = false;
            });
            Ok(v)
        }
        "environment" | "new.env" => {
            let e = if name == "new.env" {
                Rc::new(std::cell::RefCell::new(crate::host::EnvData {
                    vars: NameMap::default(),
                    parent: Some(with_host(|h| h.global.clone())),
                }))
            } else {
                with_host(|h| h.env())
            };
            Ok(with_host(|h| h.alloc(RData::Environment(e))))
        }
        "missing" => {
            let n = str1(&a.req(0, "x")?).unwrap_or_default();
            Ok(scalar_lgl(!with_host(|h| {
                h.env().borrow().vars.contains_key(&n)
            })))
        }
        "return" => {
            let v = a.get(0, "value").unwrap_or_else(null);
            with_host(|h| h.signal = Some(Signal::Return(v.clone())));
            Ok(v)
        }
        "UseMethod" => use_method(&a),
        "NextMethod" => next_method(),
        "tryCatch" => try_catch(&a),
        "withCallingHandlers" => with_calling_handlers(&a),
        "try" => r_try(&a),
        "on.exit" => {
            let thunk = a.req(0, "expr")?;
            let add = a.named("add").and_then(|v| lgl1(&v)).unwrap_or(false);
            // `after = FALSE` puts the new expression *first*, so a cleanup
            // registered later runs before the ones already there.
            let after = a.named("after").and_then(|v| lgl1(&v)).unwrap_or(true);
            with_host(|h| {
                if let Some(f) = h.frames.last_mut() {
                    // Without `add = TRUE` a later `on.exit` replaces the
                    // registered expression rather than joining it.
                    if !add {
                        f.on_exit.clear();
                    }
                    match after {
                        true => f.on_exit.push(thunk),
                        false => f.on_exit.insert(0, thunk),
                    }
                }
                h.visible = false;
            });
            Ok(null())
        }
        "conditionMessage" => Ok(scalar_str(
            element_field(&a.req(0, "c")?, "message")
                .and_then(|m| str1(&m))
                .unwrap_or_default(),
        )),
        "conditionCall" => Ok(element_field(&a.req(0, "c")?, "call").unwrap_or_else(null)),
        "simpleError" | "simpleWarning" | "simpleMessage" | "simpleCondition" => {
            let msg = str1(&a.req(0, "message")?).unwrap_or_default();
            let classes: &[&str] = match name {
                "simpleError" => &["simpleError", "error", "condition"],
                "simpleWarning" => &["simpleWarning", "warning", "condition"],
                "simpleMessage" => &["simpleMessage", "message", "condition"],
                _ => &["simpleCondition", "condition"],
            };
            Ok(mk_condition(&msg, classes, None))
        }
        "signalCondition" => {
            let c = a.req(0, "cond")?;
            let msg = element_field(&c, "message")
                .and_then(|m| str1(&m))
                .unwrap_or_default();
            let classes = class_of(&c);
            // `signalCondition` establishes no restart and has no default
            // action: with nothing waiting it just returns NULL — visibly, so a
            // bare call at top level echoes it.
            if let Signalled::Unwind = signal_to_handlers(&c, &classes)? {
                raise_condition(msg, classes, element_field(&c, "call").and_then(|cl| lang_of(&cl)).map(|e| crate::deparse::deparse_lines(&e)));
            }
            with_host(|h| h.visible = true);
            Ok(null())
        }
        "withRestarts" => with_restarts(&a),
        "invokeRestart" => invoke_restart(&a),
        "computeRestarts" => Ok(compute_restarts()),
        // `restartDescription(r)` is `r$description`, so the `abort` restart —
        // which carries no such field — answers NULL rather than "".
        "restartDescription" => {
            Ok(element_field(&a.req(0, "r")?, "description").unwrap_or_else(null))
        }
        "isRestart" => Ok(mk_lgl(vec![Some(
            class_of(&a.req(0, "x")?).iter().any(|c| c == "restart"),
        )])),
        "Recall" => {
            // Re-invoke the closure that is currently executing, one frame down
            // (the top frame is Recall's own primitive call is not pushed, so the
            // last closure frame is the caller).
            let here = with_host(|h| h.frames.last().and_then(|f| f.fun.clone()));
            match here {
                Some((id, env)) => {
                    let f = with_host(|h| h.alloc(RData::Closure { id, env }));
                    call_value(&f, a.all.clone(), None)
                }
                None => Err("Recall called from outside a closure".into()),
            }
        }
        "factor" => {
            let x = a.req(0, "x")?;
            let levels: Vec<String> = match a.named("levels") {
                Some(l) => as_str(&l).into_iter().flatten().collect(),
                None => factor_levels(&x),
            };
            // Each value's code is the 1-based index of its label in `levels`
            // (NA when the value is not among the levels). Re-factoring an
            // existing factor matches on its labels, never its codes.
            let codes: Vec<Option<i64>> = as_str_labels(&x)
                .iter()
                .map(|c| {
                    c.as_ref()
                        .and_then(|c| levels.iter().position(|l| l == c))
                        .map(|p| p as i64 + 1)
                })
                .collect();
            // R's default is `ordered = is.ordered(x)`, so re-factoring an
            // ordered factor keeps it ordered.
            let ordered = a
                .named("ordered")
                .and_then(|v| lgl1(&v))
                .unwrap_or_else(|| is_ordered(&x));
            Ok(mk_factor(codes, levels, ordered))
        }
        "levels" => {
            let x = a.req(0, "x")?;
            Ok(with_host(|h| h.attr(&x, "levels")).unwrap_or_else(null))
        }
        "nlevels" => {
            let x = a.req(0, "x")?;
            let n = with_host(|h| h.attr(&x, "levels"))
                .map(|l| len(&l))
                .unwrap_or(0);
            Ok(scalar_int(n as i64))
        }
        // `droplevels.factor` is `factor(x, exclude = …)` — drop the levels that
        // no longer occur and renumber the codes.
        "droplevels" => Ok(drop_unused_levels(&a.req(0, "x")?)),
        "cut" => cut(&a),
        "table" => {
            let x = a.req(0, "x")?;
            let is_factor = class_of(&x).iter().any(|c| c == "factor");
            let levels: Vec<String> = if is_factor {
                with_host(|h| h.attr(&x, "levels"))
                    .map(|l| as_str(&l).into_iter().flatten().collect())
                    .unwrap_or_default()
            } else {
                factor_levels(&x)
            };
            // The observed labels, dropping NA the way R's `table` does.
            let obs: Vec<String> = if is_factor {
                let codes = as_int(&x);
                codes
                    .iter()
                    .filter_map(|c| c.and_then(|i| levels.get((i - 1) as usize).cloned()))
                    .collect()
            } else {
                as_str(&x).into_iter().flatten().collect()
            };
            let counts: Vec<Option<i64>> = levels
                .iter()
                .map(|l| Some(obs.iter().filter(|o| *o == l).count() as i64))
                .collect();
            let out = mk_int(counts);
            // R's `table` is a 1-D array: the labels live in `dimnames`, and the
            // name OF that dimnames element is the deparsed argument, which is
            // the header `print` puts above the counts (`table(z)` heads `z`).
            // The compiler passes that symbol as `.dnn`; a non-symbol argument
            // has none and R heads the table with a blank line.
            let dim = mk_int(vec![Some(levels.len() as i64)]);
            let labels = mk_str(levels.into_iter().map(Some).collect());
            let dn = mk_list(vec![labels]);
            if let Some(sym) = a.named(".dnn").and_then(|v| str1(&v)) {
                set_names(&dn, vec![Some(sym)]);
            }
            let cls = scalar_str("table");
            with_host(|h| {
                h.set_attr(&out, "dim", dim);
                h.set_attr(&out, "dimnames", dn);
                h.set_attr(&out, "class", cls);
            });
            Ok(out)
        }

        // Package loaders and unknown functions fall through to the CRAN bridge
        // (an embedded GNU R), so `library(pkg)` and any package routine work
        // without rlang re-implementing them.
        "library" | "require" | "requireNamespace" | "loadNamespace" => {
            let pkg = str1(&a.req(0, "package")?).unwrap_or_default();
            let r = cran_eval(&format!("suppressMessages({name}({pkg}))"))?;
            with_host(|h| h.visible = false);
            // `require`/`requireNamespace` yield a logical; `library` loads
            // invisibly.
            if name.starts_with("require") {
                Ok(r)
            } else {
                Ok(null())
            }
        }
        "suppressWarnings" => suppress_conditions(&a, "warning"),
        "suppressMessages" | "suppressPackageStartupMessages" => suppress_conditions(&a, "message"),
        // A model formula (compiled from `lhs ~ rhs`): build the real formula
        // object in embedded R so `lm`/`glm`/`aggregate` receive it intact.
        // `quote(x)` / `bquote(x)`: the compiler has replaced the argument with
        // its deparse (see `compiler::quote_call`), so what arrives is R source
        // to parse back into the expression the caller wrote.
        ".rlang_quote" | "quote" | "bquote" => {
            let src = str1(&a.req(0, "expr")?).unwrap_or_default();
            let mut exprs = crate::parser::parse(&src)?;
            match exprs.len() {
                1 => Ok(mk_lang(exprs.remove(0))),
                _ => Err(format!("invalid expression in quote: {src}")),
            }
        }
        // `eval(expr)` runs an expression that was held rather than evaluated.
        // Anything that is not one is already a value and evaluates to itself,
        // which is R's rule too: `eval(1 + 2)` sees the number 3.
        "eval" | "evalq" => {
            let x = a.req(0, "expr")?;
            let Some(e) = lang_of(&x) else { return Ok(x) };
            // `envir = e` runs the expression *in* that environment; without
            // one it runs where the caller stands.
            match a.get(1, "envir").and_then(|v| env_of(&v)) {
                Some(env) => in_env(env, || eval_expr(&e)),
                None => eval_expr(&e),
            }
        }
        // `substitute(expr)`: the expression with every symbol the current
        // frame can account for replaced by what it stands for. The compiler
        // has already carried the argument across as source (see
        // `compiler`'s rewrite), so what arrives is R text to parse back.
        ".rlang_substitute" | "substitute" => {
            let src = str1(&a.req(0, "expr")?).unwrap_or_default();
            let mut exprs = crate::parser::parse(&src)?;
            if exprs.len() != 1 {
                return Err(format!("invalid expression in substitute: {src}"));
            }
            let e = exprs.remove(0);
            // An explicit table is consulted instead of the frame — R's
            // `substitute(x, list(x = 1))`.
            if let Some(table) = a.get(1, "env") {
                let names = names_of(&table);
                let items = elements(&table);
                let map: Vec<(String, Expr)> = names
                    .iter()
                    .zip(&items)
                    .filter_map(|(n, v)| Some((n.clone()?, value_as_expr(v)?)))
                    .collect();
                return Ok(mk_lang(substitute_in(&e, &map, false)));
            }
            // At top level R substitutes nothing at all, however much is bound:
            // `y <- 9; substitute(y + 1)` is `y + 1`.
            let Some(map) = frame_substitutions() else {
                return Ok(mk_lang(e));
            };
            Ok(mk_lang(substitute_in(&e, &map, true)))
        }
        // `sys.call()` is the call that made the frame now running, verbatim.
        // The context stack holds its deparse; parsing that back is what makes
        // it an expression rather than a string.
        "sys.call" => Ok(match with_host(|h| h.enclosing_source()) {
            Some(src) => parse_one(&src)?,
            None => null(),
        }),
        // `match.call()` is the same call with every argument named and put in
        // formal order — what R's own `match.arg` and `update` read to find out
        // how they were called.
        "match.call" => {
            let Some(src) = with_host(|h| h.enclosing_source()) else {
                return Ok(null());
            };
            let Some(params) = with_host(|h| h.current_formals()) else {
                return parse_one(&src);
            };
            match matched_call(&src, &params)? {
                Some(e) => Ok(mk_lang(e)),
                None => parse_one(&src),
            }
        }
        // `sys.function()` is the closure being executed, not its call.
        "sys.function" => Ok(with_host(|h| {
            match h.frames.last().and_then(|f| f.fun.clone()) {
                Some((id, env)) => h.alloc(RData::Closure { id, env }),
                None => h.null(),
            }
        })),
        "as.name" | "as.symbol" => {
            let n = str1(&a.req(0, "x")?).unwrap_or_default();
            Ok(with_host(|h| h.alloc(RData::Sym(n))))
        }
        "is.call" => Ok(scalar_lgl(kind(&a.req(0, "x")?) == RKind::Lang)),
        "is.name" | "is.symbol" => Ok(scalar_lgl(kind(&a.req(0, "x")?) == RKind::Sym)),
        ".rlang_formula" => {
            let src = str1(&a.req(0, "src")?).unwrap_or_default();
            cran_eval(&format!("stats::as.formula({src:?})"))
        }
        other => cran_call(other, &a.all),
    }
}

/// Delegate `name(args…)` to the embedded GNU R when available, preserving
/// rlang's own "could not find function" error otherwise.
#[cfg(not(target_arch = "wasm32"))]
fn cran_call(name: &str, args: &[(Option<String>, Value)]) -> Result<Value, String> {
    let r = crate::rembed::call(name, args)?;
    // The print/cat family writes to stdout and returns invisibly; make sure the
    // delegated result is not auto-printed a second time by rlang.
    if name.starts_with("print")
        || matches!(name, "cat" | "message" | "writeLines" | "str" | "invisible")
    {
        with_host(|h| h.visible = false);
    }
    Ok(r)
}
#[cfg(target_arch = "wasm32")]
fn cran_call(name: &str, _: &[(Option<String>, Value)]) -> Result<Value, String> {
    Err(format!("could not find function \"{name}\""))
}

/// Evaluate R source in the embedded GNU R (used by the package loaders).
#[cfg(not(target_arch = "wasm32"))]
fn cran_eval(code: &str) -> Result<Value, String> {
    crate::rembed::eval_source(code)
}
#[cfg(target_arch = "wasm32")]
fn cran_eval(_: &str) -> Result<Value, String> {
    Err("package loading needs an R installation (unavailable on wasm)".into())
}

/// An operator invoked through its function name: ``\`+\`(1, 2)``, ``\`[\`(x, 2)``.
/// A one-argument call of `-`/`+`/`!` is the unary form.
fn call_operator(name: &str, args: &[(Option<String>, Value)]) -> Result<Value, String> {
    let vals: Vec<Value> = args.iter().map(|(_, v)| v.clone()).collect();
    let first = vals
        .first()
        .cloned()
        .ok_or_else(|| format!("argument to '{name}' is missing"))?;
    match name {
        "[" => return index_single(&first, &args[1..]),
        "[[" => return index_double(&first, &args[1..]),
        "$" => {
            let key = vals.get(1).and_then(str1).unwrap_or_default();
            let names = names_of(&first);
            return Ok(
                match names
                    .iter()
                    .position(|n| n.as_deref() == Some(key.as_str()))
                {
                    Some(i) => element_at(&first, i),
                    None => null(),
                },
            );
        }
        _ => {}
    }
    match vals.len() {
        1 => match name {
            "-" => Ok(mk_dbl(
                as_dbl(&first).iter().map(|e| e.map(|n| -n)).collect(),
            )),
            "+" => Ok(first),
            "!" => Ok(mk_lgl(
                as_lgl(&first).iter().map(|e| e.map(|b| !b)).collect(),
            )),
            other => Err(format!("invalid unary operator '{other}'")),
        },
        _ => binop(name, &first, &vals[1]),
    }
}

/// Positional/named argument access for primitives.
struct Args {
    all: Vec<(Option<String>, Value)>,
}

impl Args {
    fn new(all: Vec<(Option<String>, Value)>) -> Self {
        Args { all }
    }
    /// Every argument value, in order.
    fn values(&self) -> Vec<Value> {
        self.all.iter().map(|(_, v)| v.clone()).collect()
    }
    /// Every argument tag, in order.
    fn tags(&self) -> Vec<Option<String>> {
        self.all.iter().map(|(t, _)| t.clone()).collect()
    }
    fn named(&self, name: &str) -> Option<Value> {
        self.all
            .iter()
            .find(|(t, _)| t.as_deref() == Some(name))
            .map(|(_, v)| v.clone())
    }
    /// The argument matching `name`, else the `i`-th untagged one.
    fn get(&self, i: usize, name: &str) -> Option<Value> {
        if let Some(v) = self.named(name) {
            return Some(v);
        }
        self.all
            .iter()
            .filter(|(t, _)| t.is_none())
            .nth(i)
            .map(|(_, v)| v.clone())
    }
    fn req(&self, i: usize, name: &str) -> Result<Value, String> {
        self.get(i, name)
            .ok_or_else(|| format!("argument \"{name}\" is missing, with no default"))
    }
    /// A numeric argument with a fallback.
    fn n(&self, i: usize, default: f64) -> f64 {
        self.get(i, "length.out")
            .or_else(|| self.get(i, "n"))
            .and_then(|v| num1(&v))
            .unwrap_or(default)
    }
    /// Every argument from untagged position `i` onward, tags preserved.
    fn rest(&self, i: usize) -> Vec<(Option<String>, Value)> {
        let mut seen = 0usize;
        self.all
            .iter()
            .filter(|(t, _)| {
                if t.is_none() {
                    seen += 1;
                    seen > i
                } else {
                    true
                }
            })
            .cloned()
            .collect()
    }
}

/// The missing value a summary of `x` yields when `na.rm` is off, or `None`
/// when `x` has none.
///
/// R distinguishes `NA` from `NaN` here and the two summaries disagree on how.
/// `mean` accumulates in IEEE arithmetic, so the *first* missing value met
/// decides: `mean(c(1, NA, NaN))` is NA and `mean(c(1, NaN, NA))` is NaN — pass
/// `first_kind = true` for that. `median` tests `is.na` up front and returns a
/// typed `NA` whichever it found, so `median(c(NaN, 1, 2))` is NA.
fn missing_result(a: &Args, x: &Value, first_kind: bool) -> Option<Value> {
    if a.named("na.rm").and_then(|v| lgl1(&v)).unwrap_or(false) {
        return None;
    }
    let xs = as_dbl(x);
    let first = xs.iter().find(|e| e.map(f64::is_nan).unwrap_or(true))?;
    Some(if first_kind && first.is_some() {
        scalar_dbl(f64::NAN)
    } else {
        mk_dbl(vec![None])
    })
}

fn numeric_arg(a: &Args, i: usize, name: &str) -> Result<Vec<f64>, String> {
    let v = a.req(i, name)?;
    let narm = a.named("na.rm").and_then(|x| lgl1(&x)).unwrap_or(false);
    let xs = as_dbl(&v);
    // `na.rm` removes NaN as well as NA (R treats them the same here); without
    // it, either one poisons the whole result.
    let is_missing = |e: &Option<f64>| e.map(f64::is_nan).unwrap_or(true);
    if !narm && xs.iter().any(is_missing) {
        return Ok(vec![f64::NAN]);
    }
    Ok(xs.into_iter().flatten().filter(|x| !x.is_nan()).collect())
}

fn empty_vector(mode: &str, n: usize) -> Value {
    match mode {
        "numeric" | "double" => mk_dbl(vec![Some(0.0); n]),
        "integer" => mk_int(vec![Some(0); n]),
        "character" => mk_str(vec![Some(String::new()); n]),
        "list" => mk_list((0..n).map(|_| null()).collect()),
        _ => mk_lgl(vec![Some(false); n]),
    }
}

/// `c(...)` — concatenate, promoting to the widest type present and building
/// the combined `names` (including `c(a = 1)` tags).
fn concat(a: &Args) -> Value {
    let parts: Vec<(Option<String>, Value)> =
        a.all.iter().filter(|(_, v)| !is_null(v)).cloned().collect();
    if parts.is_empty() {
        return null();
    }
    let rank = parts
        .iter()
        .map(|(_, v)| with_host(|h| h.kind_of(v).rank()))
        .max()
        .unwrap_or(1);

    let mut names: Vec<Option<String>> = Vec::new();
    let mut any_named = false;
    for (tag, v) in &parts {
        let inner = names_of(v);
        let n = len(v);
        for i in 0..n {
            let base = inner.get(i).cloned().flatten();
            let nm = match (tag, &base) {
                (Some(t), Some(b)) => Some(format!("{t}.{b}")),
                (Some(t), None) if n == 1 => Some(t.clone()),
                (Some(t), None) => Some(format!("{t}{}", i + 1)),
                (None, b) => b.clone(),
            };
            any_named |= nm.is_some();
            names.push(nm);
        }
    }

    // `c.factor`: when *every* argument is a factor the result is a factor over
    // the union of their level sets, so the codes are re-mapped rather than
    // concatenated. A single non-factor argument drops the whole result to
    // `unlist`'s plain coercion — which is why `c(f, "q")` yields the codes as
    // text, not the labels.
    if parts.iter().all(|(_, v)| is_factor(v)) {
        let mut levels: Vec<String> = Vec::new();
        for (_, v) in &parts {
            for l in levels_of(v) {
                if !levels.contains(&l) {
                    levels.push(l);
                }
            }
        }
        let codes = parts
            .iter()
            .flat_map(|(_, v)| factor_labels(v))
            .map(|s| {
                s.and_then(|s| levels.iter().position(|l| *l == s))
                    .map(|p| p as i64 + 1)
            })
            .collect();
        // The result stays ordered only when every argument is ordered over the
        // *same* levels; otherwise the combined order would be invented.
        let ordered = parts
            .iter()
            .all(|(_, v)| is_ordered(v) && levels_of(v) == levels);
        let out = mk_factor(codes, levels, ordered);
        if any_named {
            set_names(&out, names);
        }
        return out;
    }
    let out = if rank >= 5 {
        mk_list(parts.iter().flat_map(|(_, v)| elements(v)).collect())
    } else {
        match rank {
            1 => mk_lgl(parts.iter().flat_map(|(_, v)| as_lgl(v)).collect()),
            2 => mk_int(parts.iter().flat_map(|(_, v)| as_int(v)).collect()),
            4 => mk_str(parts.iter().flat_map(|(_, v)| as_str(v)).collect()),
            _ => mk_dbl(parts.iter().flat_map(|(_, v)| as_dbl(v)).collect()),
        }
    };
    if any_named {
        set_names(&out, names);
    }
    out
}

/// `unlist(x)` — flatten a list to an atomic vector of the widest type,
/// recursively, composing names the way R does (`list(a = 1, b = list(2, 3))`
/// unlists to `a b1 b2`).
fn unlist(x: &Value) -> Value {
    match data(x) {
        RData::List(items) => {
            let names = names_of(x);
            let parts: Vec<(Option<String>, Value)> = items
                .iter()
                .enumerate()
                .map(|(i, v)| (names.get(i).cloned().flatten(), unlist(v)))
                .collect();
            concat(&Args::new(parts))
        }
        _ => x.clone(),
    }
}

/// `sapply` simplification: a list of length-1 values of one type becomes an
/// atomic vector; anything else stays a list.
fn simplify(list: &Value) -> Value {
    let items = match data(list) {
        RData::List(v) => v,
        _ => return list.clone(),
    };
    if items.is_empty() {
        return list.clone();
    }
    let k = len(&items[0]);
    // R's `simplify2array` collapses uniformly length-1 results with
    // `unlist(recursive = FALSE)`, which flattens exactly one level — so a
    // result that is itself a length-1 *list* still simplifies, to a flat list.
    // Bailing on any list element left `sapply(1:2, \(x) list(x))` doubly
    // nested. Longer list results have no matrix form, so those still stay put.
    if k != 1 && items.iter().any(|v| kind(v) == RKind::List) {
        return list.clone();
    }
    // Uniform scalar results collapse to a vector; uniform length-k (k > 1)
    // results become a k×n matrix, each result a column — R's sapply/vapply
    // rule. Ragged results stay a list.
    if k >= 1 && items.iter().all(|v| len(v) == k) {
        let parts: Vec<(Option<String>, Value)> = items.iter().map(|v| (None, v.clone())).collect();
        let out = concat(&Args::new(parts));
        if k == 1 {
            let nm = names_of(list);
            if !nm.is_empty() {
                set_names(&out, nm);
            }
        } else {
            let dim = mk_int(vec![Some(k as i64), Some(items.len() as i64)]);
            with_host(|h| h.set_attr(&out, "dim", dim));
            // R labels the rows with the first result's own names and the
            // columns with the names of what was mapped over.
            let rn = names_of(&items[0]);
            let cn = names_of(list);
            set_dimnames(
                &out,
                (!rn.is_empty()).then_some(rn),
                (!cn.is_empty()).then_some(cn),
            );
        }
        return out;
    }
    list.clone()
}

/// `paste`/`paste0` — elementwise, with recycling, and an optional `collapse`.
fn paste(a: &Args, zero: bool) -> Value {
    let sep = if zero {
        String::new()
    } else {
        a.named("sep")
            .and_then(|v| str1(&v))
            .unwrap_or_else(|| " ".into())
    };
    let collapse = a
        .named("collapse")
        .filter(|v| !is_null(v))
        .and_then(|v| str1(&v));
    let parts: Vec<Vec<Option<String>>> = a
        .all
        .iter()
        .filter(|(t, _)| !matches!(t.as_deref(), Some("sep") | Some("collapse")))
        .map(|(_, v)| as_str_labels(v))
        .collect();
    // Every argument contributes a field, even a zero-length one — it just
    // contributes the empty string. That is why `paste("a", NULL, "b")` is
    // "a  b" (two separators, one empty field) and not "a b". If *every*
    // argument is zero-length the result is `character(0)`.
    // `collapse` always yields one string, so an empty result collapses to ""
    // rather than to `character(0)`.
    let n = parts.iter().map(|p| p.len()).max().unwrap_or(0);
    if n == 0 && collapse.is_none() {
        return mk_str(vec![]);
    }
    let joined: Vec<String> = (0..n)
        .map(|i| {
            parts
                .iter()
                .map(|p| match p.get(i % p.len().max(1)) {
                    Some(Some(s)) => s.clone(),
                    Some(None) => "NA".into(),
                    None => String::new(),
                })
                .collect::<Vec<_>>()
                .join(&sep)
        })
        .collect();
    match collapse {
        Some(c) => scalar_str(joined.join(&c)),
        None => mk_str(joined.into_iter().map(Some).collect()),
    }
}

/// `seq(from, to, by=, length.out=)`.
fn seq(a: &Args) -> Result<Value, String> {
    // `seq(along.with = x)` is `seq_along(x)`: the indices of `x`, whatever its
    // values are. It short-circuits the rest of the signature.
    if let Some(v) = a.named("along.with") {
        return Ok(mk_int((1..=len(&v) as i64).map(Some).collect()));
    }
    let from = a.get(0, "from").and_then(|v| num1(&v)).unwrap_or(1.0);
    let to = a.get(1, "to").and_then(|v| num1(&v));
    // R's signature is `seq(from, to, by, length.out, ...)`, so a third
    // positional argument is `by` — `seq(0, 1, 0.25)` must see by = 0.25, not
    // fall through to the default step of 1.
    let by = a.get(2, "by").and_then(|v| num1(&v));
    let length_out = a.named("length.out").and_then(|v| num1(&v));
    // With no `to`, R's signature supplies one: `length.out` counts terms
    // forward from `from`, a bare `by` leaves `to` at its default of 1 (so
    // `seq(5, by = 2)` is a wrong-sign error while `seq(5, by = -2)` is 5 3 1),
    // and the one-argument form is `1:n` — which counts *down* when n < 1, so
    // `seq(0)` is `c(1, 0)` and not the empty sequence `seq_len(0)` gives.
    let (from, to) = match to {
        Some(t) => (from, t),
        None => match (length_out, by) {
            (Some(n), _) => (from, from + by.unwrap_or(1.0) * (n - 1.0).max(0.0)),
            (None, Some(_)) => (from, 1.0),
            (None, None) => (1.0, from),
        },
    };
    // R refuses a step that cannot reach `to` rather than emitting one element.
    if let Some(b) = by {
        if b == 0.0 && to != from {
            return Err("invalid '(to - from)/by'".into());
        }
        if (to - from) * b < 0.0 {
            return Err("wrong sign in 'by' argument".into());
        }
    }
    let step = match (by, length_out) {
        (Some(b), _) => b,
        (None, Some(n)) if n > 1.0 => (to - from) / (n - 1.0),
        (None, Some(_)) => 0.0,
        (None, None) => {
            if to >= from {
                1.0
            } else {
                -1.0
            }
        }
    };
    let mut out = Vec::new();
    if let Some(n) = length_out {
        // `length.out` fixes the count exactly, even when the step is zero:
        // `seq(5, 5, length.out = 4)` repeats the value four times.
        out = (0..n.max(0.0) as usize)
            .map(|k| Some(from + step * k as f64))
            .collect();
    } else if step == 0.0 {
        out.push(Some(from));
    } else {
        // R's `seq.default`: the term count is `as.integer((to - from)/by +
        // 1e-10)` — truncation with a nudge, not a floor. Without the nudge a
        // step that divides the span exactly in decimal but not in binary
        // (`seq(0.1, 3, by = 0.1)`, whose quotient is 28.999999999999996)
        // loses its last term. The terms are then clamped to `to`, which is
        // R's closing `pmin`/`pmax`.
        let count = ((to - from) / step + 1e-10) as i64;
        for k in 0..=count.max(0) {
            let v = from + step * k as f64;
            out.push(Some(if step > 0.0 { v.min(to) } else { v.max(to) }));
        }
    }
    let whole = out
        .iter()
        .flatten()
        .all(|x| *x == x.trunc() && x.abs() < 1e15);
    Ok(if whole && by.map(|b| b == b.trunc()).unwrap_or(true) {
        mk_int(out.into_iter().map(|e| e.map(|x| x as i64)).collect())
    } else {
        mk_dbl(out)
    })
}

/// `rep(x, times=, each=)`.
fn rep(a: &Args) -> Value {
    let x = match a.get(0, "x") {
        Some(v) => v,
        None => return null(),
    };
    let each = a
        .named("each")
        .and_then(|v| num1(&v))
        .unwrap_or(1.0)
        .max(0.0) as usize;
    let n = len(&x);

    // R applies `each` first: every element is repeated in place.
    let mut base: Vec<Option<usize>> = Vec::with_capacity(n * each);
    for i in 0..n {
        for _ in 0..each {
            base.push(Some(i));
        }
    }

    // `times` is either a scalar whole-vector count or a per-element vector of
    // counts (`rep(1:3, times = c(1, 2, 3))` -> 1 2 2 3 3 3).
    let times_arg = a.get(1, "times");
    let mut pos: Vec<Option<usize>> = Vec::new();
    match &times_arg {
        Some(t) if len(t) > 1 => {
            let counts = as_int(t);
            for (k, p) in base.iter().enumerate() {
                let c = counts.get(k).copied().flatten().unwrap_or(0).max(0) as usize;
                for _ in 0..c {
                    pos.push(*p);
                }
            }
        }
        _ => {
            let times = times_arg.and_then(|v| num1(&v)).unwrap_or(1.0).max(0.0) as usize;
            for _ in 0..times {
                pos.extend_from_slice(&base);
            }
        }
    }

    // `length.out` then truncates, or recycles, to an exact length.
    if let Some(want) = a.named("length.out").and_then(|v| num1(&v)) {
        let want = want.max(0.0) as usize;
        pos = match pos.len() {
            0 => vec![None; want],
            m => (0..want).map(|i| pos[i % m]).collect(),
        };
    }
    let out = take_positions(&x, &pos);
    let nm = names_of(&x);
    if !nm.is_empty() {
        set_names(
            &out,
            pos.iter()
                .map(|p| p.and_then(|i| nm.get(i).cloned().flatten()))
                .collect(),
        );
    }
    // `rep.factor` is `structure(NextMethod(), class = class(x), levels = levels(x))`.
    carry_factor(&out, &x);
    out
}

fn sort_value(x: &Value, decreasing: bool, na_last: Option<bool>) -> Value {
    let idx = order_positions(x, decreasing, na_last);
    let pos: Vec<Option<usize>> = idx.into_iter().map(Some).collect();
    let out = take_positions(x, &pos);
    let nm = names_of(x);
    if !nm.is_empty() {
        set_names(
            &out,
            pos.iter()
                .map(|p| p.and_then(|i| nm.get(i).cloned().flatten()))
                .collect(),
        );
    }
    // `sort.default` sorts an object with `x[order(x, ...)]`, so a factor's
    // levels and class survive the reorder.
    carry_factor(&out, x);
    out
}

fn order_value(x: &Value, decreasing: bool, na_last: Option<bool>) -> Value {
    mk_int(
        order_positions(x, decreasing, na_last)
            .into_iter()
            .map(|i| Some(i as i64 + 1))
            .collect(),
    )
}

/// Read a `na.last` argument. An explicit `NA` means "drop the missing values",
/// which is distinct from the argument being absent — hence the two layers of
/// `Option`, collapsed here against the caller's default.
fn na_last_arg(a: &Args, default: Option<bool>) -> Option<bool> {
    match a.named("na.last") {
        Some(v) => lgl1(&v),
        None => default,
    }
}

/// One sort key: a character vector compares lexically, anything else numerically.
enum SortKey {
    Text(Vec<Option<String>>),
    Num(Vec<Option<f64>>),
}

impl SortKey {
    fn of(x: &Value) -> SortKey {
        if kind(x) == RKind::Str {
            SortKey::Text(as_str(x))
        } else {
            SortKey::Num(as_dbl(x))
        }
    }
    /// Whether position `i` is missing. `NaN` counts alongside `NA`: R sorts
    /// both to the same place.
    fn missing(&self, i: usize) -> bool {
        match self {
            SortKey::Text(v) => v[i].is_none(),
            SortKey::Num(v) => v[i].map(f64::is_nan).unwrap_or(true),
        }
    }
    fn cmp(&self, p: usize, q: usize) -> std::cmp::Ordering {
        match self {
            SortKey::Text(v) => match (&v[p], &v[q]) {
                (Some(a), Some(b)) => crate::collate::str_cmp(a, b),
                (a, b) => a.cmp(b),
            },
            SortKey::Num(v) => v[p].partial_cmp(&v[q]).unwrap_or(std::cmp::Ordering::Equal),
        }
    }
}

/// The ordering permutation over one or more keys, later keys breaking earlier
/// ties (`order(a, b)`).
///
/// `na_last` places the missing values: `Some(true)` at the end (what `order`
/// defaults to), `Some(false)` at the front, `None` dropped (what `sort`
/// defaults to). A position is missing if *any* key is missing there.
///
/// `decreasing` reverses the comparison rather than the result, so equal
/// elements keep their original ascending index order in both directions —
/// `order(c(1, 1, 2), decreasing = TRUE)` is `3 1 2`, not `3 2 1`.
fn order_by_keys(keys: &[Value], decreasing: bool, na_last: Option<bool>) -> Vec<usize> {
    let n = keys.first().map(len).unwrap_or(0);
    let ks: Vec<SortKey> = keys.iter().map(SortKey::of).collect();
    let missing = |i: usize| ks.iter().any(|k| k.missing(i));
    let mut good: Vec<usize> = (0..n).filter(|i| !missing(*i)).collect();
    good.sort_by(|p, q| {
        let ord = ks
            .iter()
            .map(|k| k.cmp(*p, *q))
            .find(|o| o.is_ne())
            .unwrap_or(std::cmp::Ordering::Equal);
        if decreasing {
            ord.reverse()
        } else {
            ord
        }
    });
    let bad = || (0..n).filter(|i| missing(*i));
    match na_last {
        None => good,
        Some(true) => {
            good.extend(bad());
            good
        }
        Some(false) => bad().chain(good).collect(),
    }
}

fn order_positions(x: &Value, decreasing: bool, na_last: Option<bool>) -> Vec<usize> {
    order_by_keys(std::slice::from_ref(x), decreasing, na_last)
}

/// `identical(x, y)` — same type, same attributes, same elements.
fn identical(x: &Value, y: &Value) -> bool {
    let (dx, dy) = (data(x), data(y));
    if std::mem::discriminant(&dx) != std::mem::discriminant(&dy) {
        return false;
    }
    if names_of(x) != names_of(y) {
        return false;
    }
    match (dx, dy) {
        (RData::Null, RData::Null) => true,
        (RData::Lgl(a), RData::Lgl(b)) => a == b,
        (RData::Int(a), RData::Int(b)) => a == b,
        (RData::Dbl(a), RData::Dbl(b)) => a == b,
        (RData::Str(a), RData::Str(b)) => a == b,
        (RData::List(a), RData::List(b)) => {
            a.len() == b.len() && a.iter().zip(b.iter()).all(|(p, q)| identical(p, q))
        }
        _ => x == y,
    }
}

/// Round half to even, matching R's `round`.
/// `round(x, digits)` at R 4.x semantics: round half to even on the *true*
/// decimal value, not on `x * 10^digits` (whose multiplication error made
/// `round(0.15, 1)` come out `0.2` instead of `0.1`). Rust's float formatting
/// already rounds ties to even on the true value, so for non-negative digits we
/// format and parse back; negative digits round to tens/hundreds by scaling.
fn r_round(v: f64, digits: i32) -> f64 {
    if !v.is_finite() {
        return v;
    }
    if digits >= 0 {
        format!("{:.*}", digits as usize, v).parse().unwrap_or(v)
    } else {
        let scale = 10f64.powi(-digits);
        round_half_even(v / scale) * scale
    }
}

/// R's `%%` (`myfmod`): the exact `fmod` remainder — computed against the
/// stored divisor, so it never rounds the quotient — re-signed to follow the
/// divisor. `10 %% 0.04` is `0.04`, `-7 %% 3` is `2`, `7 %% -3` is `-2`. A zero
/// result is normalized to `+0` (R never yields `-0`, which would flip the sign
/// of a later `x / (a %% b)`).
fn r_mod(x: f64, y: f64) -> f64 {
    if y == 0.0 {
        return f64::NAN;
    }
    let r = x % y;
    let r = if r != 0.0 && (r < 0.0) != (y < 0.0) {
        r + y
    } else {
        r
    };
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

/// R's `%/%`, kept consistent with `%%` via `(x - (x %% y)) / y`. A zero divisor
/// or a non-finite dividend yields R's `x / y` directly (`49 %/% 0` and
/// `Inf %/% 3` are both `Inf`, not `NaN`), and a zero quotient normalizes to
/// `+0` for the same reason as `r_mod`.
fn r_idiv(x: f64, y: f64) -> f64 {
    if y == 0.0 || !x.is_finite() {
        return x / y;
    }
    let q = ((x - r_mod(x, y)) / y).round();
    if q == 0.0 {
        0.0
    } else {
        q
    }
}

/// R's `^`, which is not IEEE `pow`.
///
/// Two rules fire before the operands are even examined for NA/NaN, which is why
/// they cannot be expressed by the caller's `Option` matching alone:
/// `x ^ 0` is 1 for *every* `x` (`NA ^ 0` is 1), and `1 ^ y` is 1 for every `y`
/// (`1 ^ NA` is 1). The caller applies those two to its `None` operands.
///
/// The third rule is where IEEE and R actively disagree: for a negative base, R
/// yields `NaN` unless the exponent is a finite whole number, while IEEE `pow`
/// defines a value for the infinite cases. `(-1) ^ Inf` is 1 under IEEE (every
/// value near -1 raised to a large power tends to 1 in magnitude) but `NaN`
/// under R, and `(-Inf) ^ 0.5` is `Inf` under IEEE but `NaN` under R. Verified
/// against the reference `Rscript` over the full
/// `{-Inf,-2,-1,-0.5,0,0.5,1,2,Inf,NaN,NA}` square.
fn r_pow(x: f64, y: f64) -> f64 {
    if y == 0.0 || x == 1.0 {
        return 1.0;
    }
    if x < 0.0 && !(y.is_finite() && y == y.trunc()) {
        return f64::NAN;
    }
    x.powf(y)
}

fn round_half_even(x: f64) -> f64 {
    let r = x.round();
    if (x - x.trunc()).abs() == 0.5 && r % 2.0 != 0.0 {
        r - x.signum()
    } else {
        r
    }
}

// libm's gamma functions aren't in the `libc` crate bindings; declare them
// against the system libm directly — the same one R links, so `gamma`/`lgamma`
// match R to the printed precision. `lgamma_r` is the reentrant form (no
// `signgam` global), safe under the fuzzer's parallel workers.
extern "C" {
    fn tgamma(x: f64) -> f64;
    fn lgamma_r(x: f64, sign: *mut i32) -> f64;
}
fn r_tgamma(x: f64) -> f64 {
    unsafe { tgamma(x) }
}
fn r_lgamma(x: f64) -> f64 {
    let mut sign = 0i32;
    unsafe { lgamma_r(x, &mut sign) }
}

/// R's `sign`: -1 / 0 / 1, with `sign(0) == 0` (unlike `f64::signum`, which
/// returns +1 for +0), and NaN preserved.
fn r_sign(x: f64) -> f64 {
    if x.is_nan() {
        f64::NAN
    } else if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// The binomial coefficient `choose(n, k)` R-style: 0 for negative `k`,
/// integer-valued for non-negative integer `n` via a product to limit rounding.
fn choose(n: f64, k: f64) -> f64 {
    let k = k.round();
    if k < 0.0 {
        return 0.0;
    }
    if k == 0.0 {
        return 1.0;
    }
    let ki = k as i64;
    let mut r = 1.0;
    for i in 0..ki {
        r *= (n - i as f64) / (ki - i) as f64;
    }
    // Integer inputs give an integer result; round off the accumulated error.
    if n == n.round() {
        r.round()
    } else {
        r
    }
}

/// Transpose a matrix value (or a bare vector treated as a single row), the
/// column-major reshuffle behind both `t()` and `crossprod`.
fn transpose(x: &Value) -> Value {
    let (nr, nc) = mat_dim(x);
    let mut pos = Vec::with_capacity(nr * nc);
    for r in 0..nr {
        for c in 0..nc {
            pos.push(Some(c * nr + r));
        }
    }
    let out = take_positions(x, &pos);
    let dim = mk_int(vec![Some(nc as i64), Some(nr as i64)]);
    with_host(|h| h.set_attr(&out, "dim", dim));
    out
}

/// `cat`'s own named arguments, which are not part of the `...` it prints.
const CAT_CONTROL_ARGS: &[&str] = &["sep", "fill", "file", "append", "labels"];

/// The R type name `cat` reports for a value it cannot print, or `None` when it
/// can. R handles atomic vectors and symbols only; everything else is an error.
fn uncatable(v: &Value) -> Option<&'static str> {
    match data(v) {
        RData::List(_) | RData::Args(_) => Some("list"),
        RData::Builtin(_) => Some("builtin"),
        RData::Closure { .. } | RData::Combinator { .. } => Some("closure"),
        _ => None,
    }
}

/// The control arguments `rbind`/`cbind` accept alongside the data: R's own
/// `deparse.level`, plus the two vectors the compiler threads in so the seam
/// labels R derives from the *argument expressions* are reachable from a
/// builtin that only ever sees values (see `Compiler::expr`).
const BIND_CONTROL_ARGS: &[&str] = &["deparse.level", ".deparse.sym", ".deparse.txt"];

/// Recycle `v` to exactly `n` elements, keeping its type. A zero-length input
/// yields all-`NA` (callers drop such arguments before this, so it is a guard).
fn recycle_to(v: &Value, n: usize) -> Value {
    let l = len(v);
    let pos: Vec<Option<usize>> = (0..n)
        .map(|i| if l == 0 { None } else { Some(i % l) })
        .collect();
    take_positions(v, &pos)
}

/// `cbind`/`rbind` of vectors and matrices into a single matrix. Each argument
/// contributes its columns (or rows); shorter inputs recycle to the common
/// length, and the result takes the widest type present, as `c()` does.
///
/// Labels along the binding seam follow R exactly: a matrix argument brings its
/// own dimnames for that margin, a tagged vector brings its tag, and an
/// untagged vector brings the deparsed argument expression — a bare symbol at
/// the default `deparse.level = 1`, any expression at `2`, nothing at `0`. If
/// no argument supplies a label the result gets no dimnames at all, which is
/// what makes `rbind(1:3, 4:6)` print `[1,]`/`[2,]` while `rbind(x, x)` prints
/// `x`/`x`. Unlabelled rows in an otherwise labelled result print as blank.
fn bind_matrix(a: &Args, by_col: bool) -> Value {
    let level = a
        .named("deparse.level")
        .and_then(|v| num1(&v))
        .unwrap_or(1.0) as i64;
    let sym = a
        .named(".deparse.sym")
        .map(|v| as_str(&v))
        .unwrap_or_default();
    let txt = a
        .named(".deparse.txt")
        .map(|v| as_str(&v))
        .unwrap_or_default();
    // R drops `NULL` and zero-length arguments outright — they contribute
    // neither a row/column nor a label.
    let inputs: Vec<(usize, Option<String>, Value)> = a
        .all
        .iter()
        .enumerate()
        .filter(|(_, (t, v))| {
            !BIND_CONTROL_ARGS.contains(&t.as_deref().unwrap_or("")) && len(v) > 0
        })
        .map(|(i, (t, v))| (i, t.clone(), v.clone()))
        .collect();
    if inputs.is_empty() {
        return null();
    }
    let is_matrix = |v: &Value| with_host(|h| h.attr(v, "dim")).is_some();
    // The length along the seam: a matrix contributes its cross-margin extent,
    // a vector its length.
    let cross = inputs
        .iter()
        .map(|(_, _, v)| {
            if is_matrix(v) {
                let (nr, nc) = mat_dim(v);
                if by_col {
                    nr
                } else {
                    nc
                }
            } else {
                len(v)
            }
        })
        .max()
        .unwrap_or(0)
        .max(1);

    let mut strips: Vec<Value> = Vec::new();
    let mut seam: Vec<Option<String>> = Vec::new();
    let mut cross_names: Option<Vec<Option<String>>> = None;
    for (i, tag, v) in &inputs {
        if is_matrix(v) {
            let (nr, nc) = mat_dim(v);
            let (outer, inner) = if by_col { (nc, nr) } else { (nr, nc) };
            let dn = dimnames_of(v);
            let margin = |k: usize| dn.get(k).cloned().flatten();
            let (seam_dn, cross_dn) = if by_col {
                (margin(1), margin(0))
            } else {
                (margin(0), margin(1))
            };
            for o in 0..outer {
                let pos: Vec<Option<usize>> = (0..inner)
                    .map(|k| {
                        let (r, c) = if by_col { (k, o) } else { (o, k) };
                        Some(c * nr + r)
                    })
                    .collect();
                strips.push(recycle_to(&take_positions(v, &pos), cross));
                // A tag on a matrix argument is ignored: R labels its rows from
                // the matrix's own dimnames or not at all.
                seam.push(seam_dn.as_ref().and_then(|d| d.get(o).cloned().flatten()));
            }
            if cross_names.is_none() {
                cross_names = cross_dn.filter(|d| d.len() == cross);
            }
        } else {
            strips.push(recycle_to(v, cross));
            seam.push(match tag {
                Some(t) => Some(t.clone()),
                None => match level {
                    0 => None,
                    2 => txt.get(*i).cloned().flatten(),
                    _ => sym.get(*i).cloned().flatten(),
                },
            });
            if cross_names.is_none() {
                let nm = names_of(v);
                if nm.iter().any(|e| e.is_some()) && nm.len() == cross {
                    cross_names = Some(nm);
                }
            }
        }
    }

    // `concat` promotes to the widest type; it lays the strips out end to end,
    // which is already column-major for `cbind` and row-major for `rbind`.
    let flat = concat(&Args::new(
        strips.into_iter().map(|s| (None, s)).collect::<Vec<_>>(),
    ));
    let n = seam.len();
    let (nr, nc) = if by_col { (cross, n) } else { (n, cross) };
    let out = if by_col {
        flat
    } else {
        let pos: Vec<Option<usize>> = (0..nr * nc).map(|k| Some((k % nr) * nc + k / nr)).collect();
        take_positions(&flat, &pos)
    };
    let dim = mk_int(vec![Some(nr as i64), Some(nc as i64)]);
    with_host(|h| h.set_attr(&out, "dim", dim));
    // One label anywhere along the seam gives every position a label; the ones
    // with none print blank.
    let seam = seam.iter().any(|s| s.is_some()).then(|| {
        seam.into_iter()
            .map(|s| Some(s.unwrap_or_default()))
            .collect::<Vec<_>>()
    });
    if by_col {
        set_dimnames(&out, cross_names, seam);
    } else {
        set_dimnames(&out, seam, cross_names);
    }
    out
}

/// The sorted, de-duplicated labels R uses as factor/table levels: numeric
/// input sorts by value (so `10` follows `2`, not precedes it), character input
/// sorts lexically.
fn factor_levels(x: &Value) -> Vec<String> {
    // An existing factor already carries its level table, in the order that
    // defines it — re-deriving it from the codes would both re-sort it and
    // report the codes as the labels.
    if is_factor(x) {
        return levels_of(x);
    }
    // Logicals level as their labels, not their 0/1 codes: R's
    // `levels(factor(c(TRUE, FALSE)))` is `"FALSE" "TRUE"`, which the string
    // branch's sort already yields.
    if matches!(kind(x), RKind::Int | RKind::Dbl) {
        let mut u: Vec<f64> = Vec::new();
        for v in as_dbl(x).into_iter().flatten() {
            if !u.contains(&v) {
                u.push(v);
            }
        }
        u.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        as_str(&mk_dbl(u.into_iter().map(Some).collect()))
            .into_iter()
            .flatten()
            .collect()
    } else {
        let mut u: Vec<String> = Vec::new();
        for v in as_str(x).into_iter().flatten() {
            if !u.contains(&v) {
                u.push(v);
            }
        }
        u.sort_by(|a, b| crate::collate::str_cmp(a, b));
        u
    }
}

/// A value's `dim` as a `Vec<usize>`, or `[length]` for a plain vector.
fn dims_of(x: &Value) -> Vec<usize> {
    with_host(|h| h.attr(x, "dim"))
        .map(|d| {
            as_int(&d)
                .into_iter()
                .map(|e| e.unwrap_or(0) as usize)
                .collect()
        })
        .unwrap_or_else(|| vec![len(x)])
}

/// `which(x, arr.ind = TRUE)` — one row per hit, one column per dimension,
/// holding that hit's 1-based subscript along the margin. R heads the columns
/// `row`/`col` for a matrix and `dim1`…`dimN` for a higher-rank array, and
/// carries the first margin's labels over as row names.
fn which_arr_ind(x: &Value, hits: &[usize], dims: &[usize]) -> Value {
    let k = dims.len();
    // Column-major: the subscript along margin `d` is the linear position
    // divided by the product of the dimensions before it, modulo its own.
    let mut cells: Vec<Option<i64>> = Vec::with_capacity(hits.len() * k);
    for d in 0..k {
        let stride: usize = dims[..d].iter().product();
        for &h in hits {
            cells.push(Some((h / stride % dims[d]) as i64 + 1));
        }
    }
    let out = mk_int(cells);
    let dim = mk_int(vec![Some(hits.len() as i64), Some(k as i64)]);
    with_host(|h| h.set_attr(&out, "dim", dim));
    let colnames: Vec<Option<String>> = if k == 2 {
        vec![Some("row".into()), Some("col".into())]
    } else {
        (1..=k).map(|i| Some(format!("dim{i}"))).collect()
    };
    let rownames = dimnames_of(x).first().cloned().flatten().map(|all| {
        hits.iter()
            .map(|&h| all.get(h % dims[0]).cloned().flatten())
            .collect()
    });
    set_dimnames(&out, rownames, Some(colnames));
    out
}

/// A value's `dimnames` as one optional label vector per dimension. An absent
/// `dimnames` attribute yields an empty list; a `NULL` entry within it yields
/// `None` for that dimension.
fn dimnames_of(x: &Value) -> Vec<Option<Vec<Option<String>>>> {
    match with_host(|h| h.attr(x, "dimnames")) {
        Some(dn) => elements(&dn)
            .iter()
            .map(|e| {
                if kind(e) == RKind::Null {
                    None
                } else {
                    Some(as_str(e))
                }
            })
            .collect(),
        None => Vec::new(),
    }
}

/// Store row/column labels as a `dimnames` list (a `NULL` element for a
/// dimension with no labels). Setting nothing leaves the attribute absent.
fn set_dimnames(v: &Value, rn: Option<Vec<Option<String>>>, cn: Option<Vec<Option<String>>>) {
    if rn.is_none() && cn.is_none() {
        return;
    }
    let to_val = |o: Option<Vec<Option<String>>>| match o {
        Some(names) => mk_str(names),
        None => null(),
    };
    let dn = mk_list(vec![to_val(rn), to_val(cn)]);
    with_host(|h| h.set_attr(v, "dimnames", dn));
}

/// The `(nrow, ncol)` of a value's `dim`, treating a bare vector as a single
/// column (`n × 1`) the way R's matrix ops coerce one.
fn mat_dim(x: &Value) -> (usize, usize) {
    let d = with_host(|h| h.attr(x, "dim"))
        .map(|d| as_int(&d))
        .unwrap_or_default();
    match d.as_slice() {
        [Some(r), Some(c)] => (*r as usize, *c as usize),
        _ => (len(x), 1),
    }
}

/// Matrix product `A %*% B`, column-major, at R's `%*%` semantics: a bare
/// vector on the left is a row, on the right a column, so it conforms.
fn mat_mul(x: &Value, y: &Value) -> Value {
    let has_dim = |v: &Value| with_host(|h| h.attr(v, "dim")).is_some();
    let (ar, ac) = if has_dim(x) { mat_dim(x) } else { (1, len(x)) };
    let (br, bc) = if has_dim(y) { mat_dim(y) } else { (len(y), 1) };
    let a = as_dbl(x);
    let b = as_dbl(y);
    if ac != br {
        return mk_dbl(vec![None]);
    }
    let mut out = vec![Some(0.0); ar * bc];
    for i in 0..ar {
        for j in 0..bc {
            let mut acc = 0.0;
            for k in 0..ac {
                let av = a.get(k * ar + i).and_then(|e| *e).unwrap_or(0.0);
                let bv = b.get(j * br + k).and_then(|e| *e).unwrap_or(0.0);
                acc += av * bv;
            }
            out[j * ar + i] = Some(acc);
        }
    }
    let res = mk_dbl(out);
    let dim = mk_int(vec![Some(ar as i64), Some(bc as i64)]);
    with_host(|h| h.set_attr(&res, "dim", dim));
    res
}

/// R's `deparse` for a value (not a language object): the source text that would
/// recreate it — `1:3`, `c(1.5, 2.5)`, `"a"`, `c("a", NA)`, `TRUE`, `NULL`.
fn deparse_value(v: &Value) -> String {
    let wrap = |parts: Vec<String>| {
        if parts.len() == 1 {
            parts.into_iter().next().unwrap()
        } else {
            format!("c({})", parts.join(", "))
        }
    };
    match data(v) {
        RData::Null => "NULL".into(),
        // An expression deparses to itself, not to a quoted string.
        RData::Lang(e) => crate::deparse::deparse_lines(&e),
        RData::Sym(n) => n,
        RData::Int(xs) => {
            if let Some(seq) = int_colon(&xs) {
                return seq;
            }
            wrap(
                xs.iter()
                    .map(|e| e.map(|i| format!("{i}L")).unwrap_or_else(|| "NA".into()))
                    .collect(),
            )
        }
        RData::Str(xs) => wrap(
            xs.iter()
                .map(|e| {
                    e.as_ref()
                        .map(|s| format!("\"{}\"", encode_string(s)))
                        .unwrap_or_else(|| "NA".into())
                })
                .collect(),
        ),
        _ => wrap(
            as_str(v)
                .into_iter()
                .map(|s| s.unwrap_or_else(|| "NA".into()))
                .collect(),
        ),
    }
}

/// If `xs` is a fully-defined run of consecutive integers (ascending or
/// descending, length ≥ 2), its `a:b` form; otherwise `None`.
fn int_colon(xs: &[Option<i64>]) -> Option<String> {
    if xs.len() < 2 || xs.iter().any(|e| e.is_none()) {
        return None;
    }
    let v: Vec<i64> = xs.iter().flatten().copied().collect();
    let step = v[1] - v[0];
    if (step == 1 || step == -1) && v.windows(2).all(|w| w[1] - w[0] == step) {
        Some(format!("{}:{}", v[0], v[v.len() - 1]))
    } else {
        None
    }
}

/// `cut(x, breaks, labels)` — bin numeric `x` into a factor of half-open
/// intervals `(a, b]`. A single-number `breaks` means that many equal-width
/// intervals over the (slightly widened) range, exactly as R computes them.
fn cut(a: &Args) -> Result<Value, String> {
    let x = as_dbl(&a.req(0, "x")?);
    let breaks_arg = a.req(1, "breaks")?;
    let breaks: Vec<f64> = if len(&breaks_arg) == 1 {
        let nb = num1(&breaks_arg).unwrap_or(1.0) as usize;
        let vals: Vec<f64> = x.iter().flatten().copied().collect();
        let mn = vals.iter().cloned().fold(f64::INFINITY, f64::min);
        let mx = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        // R spaces the breaks evenly across the *un-widened* data range and only
        // then nudges the first and last outward, so the interior cut points sit
        // on round data values (`cut(1:10, 3)` breaks at 4 and 7). Spreading
        // them evenly across the widened range instead shifted every interior
        // break inward — 4 landed in `(4,7]` rather than R's `(0.991,4]`.
        let dx = mx - mn;
        if dx == 0.0 {
            // A constant `x` has no range to divide, so R substitutes a width
            // (`abs(x)`, or 1 at zero) and spreads the whole sequence across the
            // padded interval instead of pinning the interior to a single point.
            let pad = if mn != 0.0 { mn.abs() } else { 1.0 } / 1000.0;
            let (lo, hi) = (mn - pad, mx + pad);
            (0..=nb)
                .map(|i| lo + (hi - lo) * i as f64 / nb as f64)
                .collect()
        } else {
            let mut b: Vec<f64> = (0..=nb).map(|i| mn + dx * i as f64 / nb as f64).collect();
            b[0] = mn - dx / 1000.0;
            b[nb] = mx + dx / 1000.0;
            b
        }
    } else {
        as_dbl(&breaks_arg).into_iter().flatten().collect()
    };
    let dig_lab = a
        .named("dig.lab")
        .and_then(|v| num1(&v))
        .map(|v| v as i32)
        .unwrap_or(3);
    let break_text = break_labels(&breaks, dig_lab);
    let labels: Vec<String> = match a.named("labels") {
        Some(l) if kind(&l) == RKind::Str => as_str(&l).into_iter().flatten().collect(),
        _ => (0..breaks.len().saturating_sub(1))
            .map(|i| format!("({},{}]", break_text[i], break_text[i + 1]))
            .collect(),
    };
    // Right-closed intervals `(a, b]`.
    let codes: Vec<Option<i64>> = x
        .iter()
        .map(|e| {
            e.and_then(|v| {
                (0..breaks.len().saturating_sub(1))
                    .find(|&i| v > breaks[i] && v <= breaks[i + 1])
                    .map(|i| i as i64 + 1)
            })
        })
        .collect();
    // `labels = FALSE` asks for the bare bin numbers rather than a factor — the
    // argument was only consulted when it held a character vector, so `FALSE`
    // fell through to the generated interval labels.
    if a.named("labels").and_then(|l| lgl1(&l)) == Some(false) {
        return Ok(mk_int(codes));
    }
    Ok(mk_factor(codes, labels, false))
}

/// Render `cut`'s break points as the endpoint text its interval labels quote.
///
/// R formats them with `formatC(breaks, digits = dig, width = 1)` — plain `%g` —
/// starting at `dig.lab` and *raising* the precision until no two adjacent
/// endpoints render alike, capped at 12:
///
/// ```text
/// for(dig in dig.lab:max(12L, dig.lab)) {
///     ch.br <- formatC(0+breaks, digits = dig, width = 1L)
///     if(ok <- all(ch.br[-1L] != ch.br[-nb])) break
/// }
/// ```
///
/// Without the escalation a constant `x` collapses every endpoint to the same
/// text: `cut(c(5,5,5), 2)` breaks at 4.995/5/5.005, which all print as `5` at
/// three digits, and the labels came out as the degenerate `(5,5]` twice instead
/// of R's `(4.995,5]` and `(5,5.005]`.
fn break_labels(breaks: &[f64], dig_lab: i32) -> Vec<String> {
    let render = |dig: i32| -> Vec<String> {
        breaks
            .iter()
            .map(|&v| format!("{:.*}", dig.max(1) as usize, FmtG(v)))
            .collect()
    };
    for dig in dig_lab..=12.max(dig_lab) {
        let text = render(dig);
        if text.windows(2).all(|w| w[0] != w[1]) {
            return text;
        }
    }
    render(12.max(dig_lab))
}

/// `%g`-style rendering of a break point: the precision is *significant digits*,
/// and trailing zeros are dropped. Rust has no `%g`, so the shortest of the
/// fixed and exponential renderings is chosen the way C's `%g` chooses.
struct FmtG(f64);

impl std::fmt::Display for FmtG {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let digits = f.precision().unwrap_or(6).max(1);
        let v = self.0;
        if v == 0.0 || !v.is_finite() {
            return write!(f, "{}", crate::host::format_dbl(v));
        }
        // C's `%g` uses the exponential form when the decimal exponent is below
        // -4 or at least the requested significant-digit count.
        let exp = v.abs().log10().floor() as i32;
        if exp < -4 || exp >= digits as i32 {
            let s = format!("{:.*e}", digits - 1, v);
            return write!(f, "{}", trim_g(&s));
        }
        let decimals = (digits as i32 - 1 - exp).max(0) as usize;
        write!(f, "{}", trim_g(&format!("{v:.decimals$}")))
    }
}

/// Drop the trailing zeros (and a bare trailing `.`) `%g` suppresses, in the
/// mantissa only so an exponent survives.
fn trim_g(s: &str) -> String {
    let (mant, exp) = match s.split_once(['e', 'E']) {
        Some((m, e)) => (m, Some(e)),
        None => (s, None),
    };
    let mant = if mant.contains('.') {
        mant.trim_end_matches('0').trim_end_matches('.')
    } else {
        mant
    };
    match exp {
        Some(e) => format!("{mant}e{e}"),
        None => mant.to_string(),
    }
}

/// `signif(x, digits)` — round to `digits` significant figures, half-to-even
/// like R. `signif(123.456, 2)` is `120`, `signif(0.0034219, 3)` is `0.00342`.
fn signif(v: f64, digits: i32) -> f64 {
    if v == 0.0 || !v.is_finite() {
        return v;
    }
    let power = digits as f64 - 1.0 - v.abs().log10().floor();
    let factor = 10f64.powf(power);
    round_half_even(v * factor) / factor
}

/// Compile an R pattern to a `regex::Regex`, honoring `fixed` (literal) and
/// `ignore.case` (the `(?i)` inline flag).
fn compile_re(pattern: &str, fixed: bool, ignore_case: bool) -> Result<Rc<regex::Regex>, String> {
    let body = if fixed {
        regex::escape(pattern)
    } else {
        pattern.to_string()
    };
    let full = if ignore_case {
        format!("(?i){body}")
    } else {
        body
    };
    cached_regex(&full).map_err(|e| format!("invalid regular expression '{pattern}': {e}"))
}

/// How many distinct patterns the cache keeps before it starts over. A script
/// works with a handful; a program that builds patterns from data could
/// otherwise grow the cache without bound.
const RE_CACHE_LIMIT: usize = 512;

thread_local! {
    /// Compiled regexes, keyed by the exact text handed to the engine.
    ///
    /// R's regex primitives are vectorised, so one call already compiles once
    /// for the whole vector — but a *call in a loop* recompiled every
    /// iteration, and building the engine (its literal prefilter above all)
    /// dwarfs matching against a short subject. The failure is cached
    /// alongside the engine so an invalid pattern keeps erroring on every call
    /// instead of being quietly accepted the second time.
    static RE_CACHE: RefCell<
        std::collections::HashMap<String, Result<Rc<regex::Regex>, String>, rustc_hash::FxBuildHasher>,
    > = RefCell::new(std::collections::HashMap::default());
}

/// The compiled form of `full`, built once per distinct pattern.
fn cached_regex(full: &str) -> Result<Rc<regex::Regex>, String> {
    RE_CACHE.with(|c| {
        if let Some(hit) = c.borrow().get(full) {
            return hit.clone();
        }
        let built = regex::Regex::new(full)
            .map(Rc::new)
            .map_err(|e| e.to_string());
        let mut cache = c.borrow_mut();
        if cache.len() >= RE_CACHE_LIMIT {
            cache.clear();
        }
        cache.insert(full.to_string(), built.clone());
        built
    })
}

/// `sub`, `gsub`, `grepl`, `grep` over R's default (POSIX-flavored) regex.
fn regex_op(name: &str, a: &Args) -> Result<Value, String> {
    let pattern = str1(&a.req(0, "pattern")?).unwrap_or_default();
    let fixed = a.named("fixed").and_then(|v| lgl1(&v)).unwrap_or(false);
    let ignore_case = a
        .named("ignore.case")
        .and_then(|v| lgl1(&v))
        .unwrap_or(false);
    let (subject_idx, subject_name) = if name == "sub" || name == "gsub" {
        (2, "x")
    } else {
        (1, "x")
    };
    let subject = a.req(subject_idx, subject_name)?;
    let x = as_str(&subject);
    let re = compile_re(&pattern, fixed, ignore_case)?;

    match name {
        "grepl" => Ok(mk_lgl(
            x.iter()
                .map(|s| s.as_ref().map(|s| re.is_match(s)))
                .collect(),
        )),
        "grep" => {
            // `value = TRUE` returns the matching strings rather than positions.
            let value = a.named("value").and_then(|v| lgl1(&v)).unwrap_or(false);
            let hits = x
                .iter()
                .enumerate()
                .filter(|(_, s)| s.as_ref().is_some_and(|s| re.is_match(s)));
            if value {
                Ok(mk_str(hits.map(|(_, s)| s.clone()).collect()))
            } else {
                Ok(mk_int(hits.map(|(i, _)| Some(i as i64 + 1)).collect()))
            }
        }
        _ => {
            let replacement = str1(&a.req(1, "replacement")?).unwrap_or_default();
            // R writes back-references as \1; the regex crate wants ${1}. The
            // brace form is required (bare `$1_` would read `1_` as the group
            // name), and `\\` collapses to a literal backslash as R does.
            let rep = if fixed {
                replacement.replace('$', "$$")
            } else {
                let mut out = String::new();
                let mut chars = replacement.chars().peekable();
                while let Some(c) = chars.next() {
                    if c == '\\' {
                        match chars.next() {
                            Some(d) if d.is_ascii_digit() => {
                                out.push_str("${");
                                out.push(d);
                                out.push('}');
                            }
                            Some(d) => out.push(d),
                            None => out.push('\\'),
                        }
                    } else if c == '$' {
                        out.push_str("$$");
                    } else {
                        out.push(c);
                    }
                }
                out
            };
            // `sub`/`gsub` rewrite each element in place, so the result keeps
            // the subject's names/dim (`grepl`/`grep` do not: one selects, the
            // other renumbers).
            shaped_like(
                mk_str(
                    x.iter()
                        .map(|s| {
                            s.as_ref().map(|s| {
                                if name == "sub" {
                                    re.replace(s, rep.as_str()).into_owned()
                                } else {
                                    re.replace_all(s, rep.as_str()).into_owned()
                                }
                            })
                        })
                        .collect(),
                ),
                &subject,
            )
        }
    }
}

/// `sprintf(fmt, ...)` — vectorized over the arguments, with R's `%d %i %s %f
/// %e %g %x %%` plus width/precision/flags.
fn sprintf(a: &Args) -> Result<Value, String> {
    let fmts = as_str(&a.req(0, "fmt")?);
    let rest = a.rest(1);
    let n = rest
        .iter()
        .map(|(_, v)| len(v))
        .chain(std::iter::once(fmts.len()))
        .max()
        .unwrap_or(1);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let Some(fmt) = fmts[i % fmts.len().max(1)].clone() else {
            out.push(None);
            continue;
        };
        let mut argi = 0usize;
        let mut s = String::new();
        let mut chars = fmt.chars().peekable();
        while let Some(c) = chars.next() {
            if c != '%' {
                s.push(c);
                continue;
            }
            if chars.peek() == Some(&'%') {
                chars.next();
                s.push('%');
                continue;
            }
            let mut spec = String::new();
            while let Some(&d) = chars.peek() {
                spec.push(d);
                chars.next();
                if d.is_ascii_alphabetic() {
                    break;
                }
            }
            let conv = spec.pop().unwrap_or('s');
            // `%*d` / `%.*f` read the field width (or the precision) from the
            // next argument, ahead of the value it formats. R allows at most one
            // `*` per specification, so one substitution is enough.
            if let Some(pos) = spec.find('*') {
                let Some((_, wv)) = rest.get(argi).cloned() else {
                    return Err("too few arguments for sprintf format".into());
                };
                argi += 1;
                let k = i % len(&wv).max(1);
                let w = as_int(&wv).get(k).and_then(|e| *e).unwrap_or(0);
                // A negative width means left-justify (C's `-` flag); a negative
                // precision means no precision at all.
                let repl = if spec[..pos].contains('.') {
                    w.max(0).to_string()
                } else if w < 0 {
                    format!("-{}", w.unsigned_abs())
                } else {
                    w.to_string()
                };
                spec.replace_range(pos..pos + 1, &repl);
            }
            let (flags, width, precision) = parse_spec(&spec);
            let arg = rest.get(argi).map(|(_, v)| v.clone());
            argi += 1;
            let Some(arg) = arg else {
                return Err("too few arguments for sprintf format".into());
            };
            let k = i % len(&arg).max(1);
            // Integer/float conversions split into sign + magnitude so the `+`/
            // space sign flag and `0` zero-padding compose the C way (zeros go
            // *after* the sign: `%05d` of -5 is `-0005`, not `000-5`).
            let field = match conv {
                'd' | 'i' => match as_int(&arg).get(k).and_then(|e| *e) {
                    Some(v) => num_field(v < 0, v.unsigned_abs().to_string(), width, &flags),
                    None => pad("NA", width, ""),
                },
                'f' | 'e' | 'E' | 'g' | 'G' => match as_dbl(&arg).get(k).and_then(|e| *e) {
                    Some(v) => {
                        let p = precision.unwrap_or(6);
                        // R spells the non-finite doubles itself rather than
                        // deferring to C's lowercase `inf`/`nan`, and still runs
                        // them through the numeric field, so `%010.2f` of `Inf`
                        // is `0000000Inf`.
                        let mag = if v.is_nan() {
                            "NaN".to_string()
                        } else if v.is_infinite() {
                            "Inf".to_string()
                        } else {
                            match conv {
                                'f' => format!("{:.p$}", v.abs()),
                                'e' | 'E' => fmt_exp(v.abs(), p, conv == 'E'),
                                _ => fmt_g(v.abs(), p, conv == 'G'),
                            }
                        };
                        num_field(v < 0.0, mag, width, &flags)
                    }
                    None => pad("NA", width, ""),
                },
                'x' | 'X' | 'o' => {
                    let v = as_int(&arg).get(k).and_then(|e| *e).unwrap_or(0);
                    let mag = match conv {
                        'x' => format!("{v:x}"),
                        'X' => format!("{v:X}"),
                        _ => format!("{v:o}"),
                    };
                    // Radix conversions take the `0` flag but no sign flag here.
                    num_field(false, mag, width, &flags.replace(['+', ' '], ""))
                }
                _ => {
                    let v = as_str(&arg)
                        .get(k)
                        .cloned()
                        .flatten()
                        .unwrap_or_else(|| "NA".into());
                    // A `%.Ns` precision is a byte budget in C. Cut on the last
                    // character boundary that fits rather than mid-sequence:
                    // R emits the partial bytes, which is not a representable
                    // Rust string.
                    let text: String = match precision {
                        Some(p) => {
                            let end = (0..=p.min(v.len())).rev().find(|&i| v.is_char_boundary(i));
                            v[..end.unwrap_or(0)].to_string()
                        }
                        None => v,
                    };
                    // Strings ignore the `0` flag (they pad with spaces).
                    pad(&text, width, &flags.replace('0', ""))
                }
            };
            s.push_str(&field);
        }
        out.push(Some(s));
    }
    Ok(mk_str(out))
}

/// `formatC(x, digits, width, format, flag)` — R's C-style number formatter.
///
/// The argument order is R's: `digits` comes before `width`, so
/// `formatC(pi, 3)` sets *digits*, not the field. The defaults are the ones
/// R's own `formatC` computes before reaching C — width and digits both
/// unset gives `width = 1`, an unset `digits` is 2 for an integer format and 4
/// for a real one, a negative `digits` is C's 6, and an unset width becomes
/// `digits + 1`. Everything finite then routes through `sprintf` so the
/// numeric-field rules (sign, zero-pad, exponent) are shared; the non-finite
/// entries are right-justified as a group the way `format.char` does.
fn format_c(a: &Args) -> Result<Value, String> {
    let x = a.req(0, "x")?;
    let digits = a.get(1, "digits").and_then(|v| num1(&v)).map(|d| d as i64);
    let mut width = a.get(2, "width").and_then(|v| num1(&v)).map(|w| w as i64);
    let flag = a.named("flag").and_then(|v| str1(&v)).unwrap_or_default();
    let left = flag.contains('-');
    let is_int = matches!(kind(&x), RKind::Int | RKind::Lgl);
    let format = a
        .get(3, "format")
        .and_then(|v| str1(&v))
        .unwrap_or_else(|| if is_int { "d".into() } else { "g".into() });

    // A character `x`, or an explicit `format = "s"`, is padded to a *display*
    // width and right-justified unless the `-` flag says otherwise — R's
    // `format.char`, not a printf spec. `sprintf` would count bytes.
    if kind(&x) == RKind::Str || format == "s" {
        let w = width.unwrap_or(0);
        let left = left || w < 0;
        let w = w.unsigned_abs() as usize;
        return Ok(mk_str(
            as_str(&x)
                .iter()
                .map(|s| {
                    s.as_deref()
                        .map(|s| crate::strwidth::pad_display(s, w, left))
                })
                .collect(),
        ));
    }

    if width.is_none() && digits.is_none() {
        width = Some(1);
    }
    let digits = match digits {
        None => {
            if format == "d" {
                2
            } else {
                4
            }
        }
        Some(d) if d < 0 => 6,
        Some(d) => d,
    };
    let width = match width {
        None => digits + 1,
        Some(0) => digits,
        Some(w) => w,
    };

    // R's `fg` is `f` with `digits` *significant* digits: the decimal count is
    // derived per element, and trailing zeros are dropped unless the `#` flag
    // asks to keep them.
    if format == "fg" {
        let out = as_dbl(&x)
            .iter()
            .map(|v| {
                let v = (*v)?;
                if !v.is_finite() {
                    return Some(special_str(v));
                }
                let e = if v == 0.0 {
                    0
                } else {
                    v.abs().log10().floor() as i64
                };
                let dec = (digits - 1 - e).max(0) as usize;
                let mut t = format!("{:.dec$}", v);
                if !flag.contains('#') && t.contains('.') {
                    t = t.trim_end_matches('0').trim_end_matches('.').to_string();
                }
                Some(t)
            })
            .collect::<Vec<_>>();
        let w = width.unsigned_abs() as usize;
        let left = left || width < 0;
        return shaped_like(
            mk_str(
                out.into_iter()
                    .map(|s| s.map(|s| crate::strwidth::pad_display(&s, w, left)))
                    .collect(),
            ),
            &x,
        );
    }

    // `format = "d"` formats the integer part, so a double `x` truncates first.
    let xv = if format == "d" && !is_int {
        mk_int(as_dbl(&x).iter().map(|v| v.map(|v| v as i64)).collect())
    } else {
        x.clone()
    };

    let mut spec = String::from("%");
    spec.push_str(&flag);
    spec.push_str(&width.to_string());
    if format != "d" {
        spec.push('.');
        spec.push_str(&digits.to_string());
    }
    spec.push_str(&format);
    let formatted = sprintf(&Args::new(vec![
        (None, scalar_str(spec)),
        (None, xv.clone()),
    ]))?;

    // Non-finite entries never go through the printf field: R formats them as a
    // group, padded to their own common width (and at least `width`).
    let raw = as_dbl(&x);
    if raw.iter().any(|v| !matches!(v, Some(v) if v.is_finite())) {
        let specials: Vec<String> = raw
            .iter()
            .filter(|v| !matches!(v, Some(v) if v.is_finite()))
            .map(|v| match v {
                Some(v) => special_str(*v),
                None => "NA".to_string(),
            })
            .collect();
        let w = specials
            .iter()
            .map(|s| s.chars().count())
            .max()
            .unwrap_or(0)
            .max(width.unsigned_abs() as usize);
        let mut cur = as_str(&formatted);
        for (i, v) in raw.iter().enumerate() {
            if matches!(v, Some(v) if v.is_finite()) {
                continue;
            }
            let text = match v {
                Some(v) => special_str(*v),
                None => "NA".to_string(),
            };
            cur[i] = Some(crate::strwidth::pad_display(&text, w, left));
        }
        return shaped_like(mk_str(big_marked(a, mk_str(cur))), &x);
    }
    let marked = big_marked(a, formatted.clone());
    shaped_like(mk_str(marked), &x)
}

/// Apply `formatC`'s `big.mark` to an already-padded field, preserving the
/// field width the way R's `preserve.width = "individual"` does: every
/// separator inserted eats one leading blank, if there is one to eat.
fn big_marked(a: &Args, v: Value) -> Vec<Option<String>> {
    let mark = a
        .named("big.mark")
        .and_then(|v| str1(&v))
        .unwrap_or_default();
    let cur = as_str(&v);
    if mark.is_empty() {
        return cur;
    }
    cur.into_iter()
        .map(|s| {
            s.map(|s| {
                let lead = s.len() - s.trim_start().len();
                let marked = insert_big_mark(s.trim_start(), &mark);
                let grew = marked.len() - (s.len() - lead);
                format!("{}{marked}", " ".repeat(lead.saturating_sub(grew)))
            })
        })
        .collect()
}

/// R's spelling of a non-finite double: `Inf`, `-Inf`, `NaN`.
fn special_str(v: f64) -> String {
    if v.is_nan() {
        "NaN".into()
    } else if v > 0.0 {
        "Inf".into()
    } else {
        "-Inf".into()
    }
}

/// Centre `s` in a `w`-column field, R's `justify = "centre"`: an odd amount of
/// padding puts the extra column on the right.
fn centre_display(s: &str, w: usize) -> String {
    let pad = w.saturating_sub(crate::strwidth::display_width(s));
    format!("{}{s}{}", " ".repeat(pad / 2), " ".repeat(pad - pad / 2))
}

/// Insert `mark` between every third digit of the integer part of a formatted
/// number, preserving any sign and fractional part: `1234567` → `1,234,567`.
fn insert_big_mark(s: &str, mark: &str) -> String {
    if mark.is_empty() {
        return s.to_string();
    }
    let (sign, body) = match s.strip_prefix('-') {
        Some(r) => ("-", r),
        None => ("", s),
    };
    let (int_part, frac) = match body.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (body, None),
    };
    if !int_part.chars().all(|c| c.is_ascii_digit()) {
        return s.to_string();
    }
    let digits: Vec<char> = int_part.chars().collect();
    let mut grouped = String::new();
    for (i, c) in digits.iter().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push_str(mark);
        }
        grouped.push(*c);
    }
    match frac {
        Some(f) => format!("{sign}{grouped}.{f}"),
        None => format!("{sign}{grouped}"),
    }
}

/// Character offset of byte position `byte` within `s` (R indexes by character,
/// not byte).
fn char_pos(s: &str, byte: usize) -> usize {
    s[..byte].chars().count()
}

/// The 1-based inclusive character slice `s[start..=stop]`, R's `substr`/
/// `substring` rule (out-of-range bounds clamp, not error).
fn substr_of(s: &str, start: usize, stop: usize) -> String {
    let skip = start.saturating_sub(1);
    s.chars()
        .skip(skip)
        .take(stop.saturating_sub(skip))
        .collect()
}

/// Expand `a-c`-style character ranges the way `chartr` does: `"a-cx"` becomes
/// `['a','b','c','x']`. A dash that is not between two ascending characters is
/// kept literal.
fn expand_char_ranges(s: &str) -> Vec<char> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if i + 2 < chars.len() && chars[i + 1] == '-' && chars[i] <= chars[i + 2] {
            for c in chars[i]..=chars[i + 2] {
                out.push(c);
            }
            i += 3;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// R's `encodeString`: render the C escapes for the control/quote characters so
/// the result round-trips as a source literal.
pub(crate) fn encode_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            _ => out.push(c),
        }
    }
    out
}

/// R's `strsplit` regex algorithm: emit the text before each match, advance
/// past it (`^` re-anchors because each search runs on the remaining slice), and
/// drop a trailing empty piece — but keep a leading one. A zero-width match
/// emits one character and steps forward one, so an empty-capable pattern
/// degenerates to a character split.
fn r_strsplit(s: &str, re: &regex::Regex) -> Vec<Option<String>> {
    let mut out = Vec::new();
    let mut rest = s;
    while !rest.is_empty() {
        match re.find(rest) {
            Some(m) if m.start() != m.end() => {
                out.push(Some(rest[..m.start()].to_string()));
                rest = &rest[m.end()..];
            }
            Some(_) => {
                let mut it = rest.char_indices();
                let (_, c) = it.next().unwrap();
                out.push(Some(c.to_string()));
                let next = it.next().map(|(i, _)| i).unwrap_or(rest.len());
                rest = &rest[next..];
            }
            None => {
                out.push(Some(rest.to_string()));
                break;
            }
        }
    }
    out
}

/// Split a `%` conversion spec into flags, width, and precision.
fn parse_spec(spec: &str) -> (String, Option<usize>, Option<usize>) {
    let mut flags = String::new();
    let mut rest = spec;
    while let Some(c) = rest.chars().next() {
        if "-+ 0#".contains(c) {
            flags.push(c);
            rest = &rest[1..];
        } else {
            break;
        }
    }
    let (w, p) = match rest.split_once('.') {
        Some((w, p)) => (w, p.parse::<usize>().ok()),
        None => (rest, None),
    };
    (flags, w.parse::<usize>().ok(), p)
}

fn pad(text: &str, width: Option<usize>, flags: &str) -> String {
    let Some(w) = width else {
        return text.to_string();
    };
    // C — and therefore R's `sprintf` — counts a field width in BYTES, so
    // `sprintf("%6s", "café")` pads by one, not two (the string is 5 bytes and 4
    // characters). Numeric fields are ASCII, where the two agree.
    if text.len() >= w {
        return text.to_string();
    }
    let fill = w - text.len();
    if flags.contains('-') {
        format!("{text}{}", " ".repeat(fill))
    } else if flags.contains('0') {
        format!("{}{text}", "0".repeat(fill))
    } else {
        format!("{}{text}", " ".repeat(fill))
    }
}

/// Assemble a printf numeric field from a sign and an already-formatted
/// magnitude, then apply width padding. The `0` flag zero-fills *between* the
/// sign and the magnitude (`-0005`), the `-` flag left-justifies with spaces,
/// and a positive value takes `+` or a leading space when those flags are set.
fn num_field(neg: bool, mag: String, width: Option<usize>, flags: &str) -> String {
    let sign = if neg {
        "-"
    } else if flags.contains('+') {
        "+"
    } else if flags.contains(' ') {
        " "
    } else {
        ""
    };
    let core = sign.len() + mag.chars().count();
    match width {
        Some(w) if w > core => {
            let fill = w - core;
            if flags.contains('-') {
                format!("{sign}{mag}{}", " ".repeat(fill))
            } else if flags.contains('0') {
                format!("{sign}{}{mag}", "0".repeat(fill))
            } else {
                format!("{}{sign}{mag}", " ".repeat(fill))
            }
        }
        _ => format!("{sign}{mag}"),
    }
}

/// C's `%e`: a mantissa with `p` fractional digits and an exponent that always
/// carries a sign and at least two digits (`1.500000e+00`), unlike Rust's
/// `{:e}` which prints `1.5e0`. `v` is the non-negative magnitude.
fn fmt_exp(v: f64, p: usize, upper: bool) -> String {
    let s = format!("{:.*e}", p, v);
    let e = if upper { 'E' } else { 'e' };
    match s.split_once('e') {
        Some((mant, exp)) => {
            let (esign, digits) = match exp.strip_prefix('-') {
                Some(d) => ('-', d),
                None => ('+', exp),
            };
            format!("{mant}{e}{esign}{digits:0>2}")
        }
        None => s,
    }
}

/// C's `%g`: pick `%e` when the decimal exponent is `< -4` or `>= p`, else
/// `%f`, with `p` significant digits (min 1), then strip trailing zeros (and a
/// trailing `.`). `v` is the non-negative magnitude.
fn fmt_g(v: f64, p: usize, upper: bool) -> String {
    let p = p.max(1);
    if v == 0.0 {
        return "0".to_string();
    }
    let exp = v.log10().floor() as i32;
    if exp < -4 || exp >= p as i32 {
        let s = fmt_exp(v, p - 1, upper);
        let (mant, rest) = s.split_once(['e', 'E']).unwrap_or((&s, ""));
        let mant = strip_g_zeros(mant);
        let e = if upper { 'E' } else { 'e' };
        if rest.is_empty() {
            mant
        } else {
            format!("{mant}{e}{rest}")
        }
    } else {
        let prec = (p as i32 - 1 - exp).max(0) as usize;
        strip_g_zeros(&format!("{v:.prec$}"))
    }
}

/// Drop the trailing zeros (and a now-dangling decimal point) that C's `%g`
/// suppresses: `1.230` → `1.23`, `100.0` → `100`.
fn strip_g_zeros(s: &str) -> String {
    if !s.contains('.') {
        return s.to_string();
    }
    let t = s.trim_end_matches('0');
    t.trim_end_matches('.').to_string()
}

/// The primitives R dispatches S3 methods for. A call to one of these on an
/// object carrying an explicit `class` attribute looks for `<name>.<class>`
/// before falling back to the built-in implementation — which is how a user's
/// `print.myclass` / `format.myclass` / `as.character.myclass` takes over.
///
/// Only names rlang implements as primitives are listed; a generic with no
/// primitive behind it is reached through `UseMethod` instead.
const INTERNAL_GENERICS: &[&str] = &[
    "print",
    "format",
    "summary",
    "toString",
    "as.character",
    "as.numeric",
    "as.double",
    "as.integer",
    "as.logical",
    "as.vector",
    "as.list",
    "length",
    "levels",
    "dim",
    "dimnames",
    "names",
    "c",
    "rev",
    "sort",
    "unique",
    "mean",
    "median",
    "seq",
    "rep",
    "t",
    "head",
    "tail",
    "all.equal",
    "cbind",
    "rbind",
    "split",
];

/// The result of an S3 method for `name`, or `None` when there is no method to
/// dispatch to (the primitive then runs as usual).
fn s3_primitive_method(
    name: &str,
    args: &[(Option<String>, Value)],
) -> Option<Result<Value, String>> {
    if !INTERNAL_GENERICS.contains(&name) {
        return None;
    }
    let obj = args.iter().find(|(t, _)| t.is_none()).map(|(_, v)| v)?;
    // Dispatch is driven by an explicit `class` attribute. The implicit class of
    // a plain vector ("numeric", "character", …) does not select a method here,
    // matching R, where those reach `print.default`.
    with_host(|h| h.attr(obj, "class"))?;
    let mut classes = class_of(obj);
    classes.push("default".to_string());
    // Only take over when a method actually exists; otherwise the primitive's
    // own implementation is the answer and `dispatch_from` would just bounce
    // straight back into it.
    classes
        .iter()
        .any(|c| with_host(|h| h.lookup_function(&format!("{name}.{c}")).is_some()))
        .then(|| dispatch_from(name, &classes, args.to_vec()))
}

// ── conditions ──────────────────────────────────────────────────────────

/// R's condition object: a two-element list `list(message =, call =)` carrying
/// the condition's S3 class vector. `conditionMessage` reads the first field.
fn mk_condition(message: &str, classes: &[&str], call: Option<String>) -> Value {
    // The call is held as source on the context stack; the condition carries it
    // as the expression itself, which is what `conditionCall` hands back and
    // what `print(cond)` shows in its `in f()` clause.
    let call = match call.as_deref().and_then(|src| parse_one(src).ok()) {
        Some(v) => v,
        None => null(),
    };
    let out = mk_list(vec![scalar_str(message), call]);
    set_names(
        &out,
        vec![Some("message".to_string()), Some("call".to_string())],
    );
    let cls = mk_str(classes.iter().map(|c| Some((*c).to_string())).collect());
    with_host(|h| h.set_attr(&out, "class", cls));
    out
}

/// What walking the handler stack for a condition concluded.
enum Signalled {
    /// Nothing took it: the signalling site performs R's default action —
    /// print for `warning`/`message`, unwind for `stop`, return for
    /// `signalCondition`.
    Fell,
    /// A `tryCatch` frame matched. The signalling site records the condition so
    /// the normal error unwind carries it out to that handler.
    Unwind,
    /// A `suppressWarnings` / `suppressMessages` frame swallowed it, which is R
    /// invoking the built-in muffle restart on the signaller's behalf.
    Muffled,
}

/// Offer a condition to every enclosing handler, innermost frame first — the
/// heart of R's condition system.
///
/// A *calling* handler (`withCallingHandlers`) runs right here, on a nested VM,
/// with the signalling frame still on the stack; when it returns normally the
/// walk carries on outward and the signaller resumes. An *exiting* handler
/// (`tryCatch`) is not run here at all: the walk stops and reports `Unwind`, so
/// the stack is torn down before the handler sees the condition. A handler that
/// transfers control to a restart comes back as `Err(RESTART_UNWIND)`, which
/// propagates untouched to the frame that established it.
///
/// While a frame's handler runs, that frame and every frame inside it is
/// disabled, so a condition signalled *by* a handler is only offered further
/// out. Without that, `warning()` inside a warning handler re-enters it forever.
fn signal_to_handlers(cond: &Value, classes: &[String]) -> Result<Signalled, String> {
    let mut i = with_host(|h| h.handlers.len());
    while i > 0 {
        i -= 1;
        let (calling, skip, muffle, matched) = with_host(|h| {
            let f = &h.handlers[i];
            let matched: Vec<usize> = f
                .handlers
                .iter()
                .enumerate()
                .filter(|(_, (c, _))| classes.iter().any(|k| k == c))
                .map(|(j, _)| j)
                .collect();
            (f.calling, f.disabled, f.muffle.clone(), matched)
        });
        if skip {
            continue;
        }
        if let Some(m) = muffle {
            if classes.contains(&m) {
                return Ok(Signalled::Muffled);
            }
            continue;
        }
        if matched.is_empty() {
            continue;
        }
        if !calling {
            return Ok(Signalled::Unwind);
        }
        let saved: Vec<bool> = with_host(|h| {
            let s = h.handlers[i..].iter().map(|f| f.disabled).collect();
            h.handlers[i..].iter_mut().for_each(|f| f.disabled = true);
            s
        });
        // Every matching handler of one `withCallingHandlers` call runs, in the
        // order written — R does not stop at the first match the way `tryCatch`
        // does.
        let mut out = Ok(());
        // Running a handler must not decide whether the *signalling* expression
        // prints: `withCallingHandlers(signalCondition(c), … = function(c)
        // cat("x"))` still echoes NULL.
        let vis = with_host(|h| h.visible);
        for j in matched {
            let f = with_host(|h| h.handlers[i].handlers[j].1.clone());
            if let Err(e) = call_value(&f, vec![(None, cond.clone())], None) {
                out = Err(e);
                break;
            }
        }
        with_host(|h| {
            h.visible = vis;
            for (f, was) in h.handlers[i..].iter_mut().zip(saved) {
                f.disabled = was;
            }
        });
        out?;
    }
    Ok(Signalled::Fell)
}

/// R's "NaNs produced" warning, as a catchable condition. Returns `Err` when a
/// `tryCatch(warning = )` is waiting for it, which unwinds to that handler.
/// Whether a math function turned a real number into `NaN` — R's condition for
/// "NaNs produced". An input that was already `NaN` does not warn, because
/// nothing was produced.
/// R's `do_math1` guard: only logical, integer and double reach a math
/// function. Anything else — text, a list, `NULL` — stops rather than coercing,
/// so `sqrt("x")` is an error where `as.numeric("x")` is a warning. A factor
/// stops too, but with `Math.factor`'s own message.
fn require_numeric(x: &Value) -> Result<(), String> {
    if is_factor(x) {
        return Err("\u{2018}sqrt\u{2019} not meaningful for factors".into());
    }
    match kind(x) {
        RKind::Lgl | RKind::Int | RKind::Dbl => Ok(()),
        _ => Err("non-numeric argument to mathematical function".into()),
    }
}

fn produced_nan(out: &[Option<f64>], input: &[Option<f64>]) -> bool {
    out.iter()
        .zip(input)
        .any(|(o, i)| matches!((o, i), (Some(o), Some(i)) if o.is_nan() && !i.is_nan()))
}

fn nan_warning() -> Result<(), String> {
    signal_warning("NaNs produced")
}

/// Signal one of rlang's own internal warnings the way `warning("…")` would:
/// calling handlers see it, `tryCatch` unwinds to it, `suppressWarnings` eats
/// it, and with nothing in scope it prints and evaluation carries on.
fn signal_warning(msg: &str) -> Result<(), String> {
    signal_warning_in(msg, with_host(|h| h.current_call()))
}

/// As [`signal_warning`], but naming `call` rather than the primitive's own.
/// R's internal warnings do not all report the same context: `do_summary`
/// passes its own call, `range.default` is R code so its warnings name the
/// `min(x)` / `max(x)` it calls, and a coercion warning names its caller.
fn signal_warning_in(msg: &str, call: Option<String>) -> Result<(), String> {
    let classes: Vec<String> = ["simpleWarning", "warning", "condition"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    match signal_condition_with_muffle(msg, &classes, "muffleWarning", call.clone())? {
        Signalled::Fell => r_warning(msg, call),
        Signalled::Muffled => {}
        Signalled::Unwind => {
            raise_condition(msg.to_string(), classes, call);
            return Err(msg.to_string());
        }
    }
    Ok(())
}

/// Signal `msg` with `classes`, with the built-in restart `muffle` established
/// around the walk — R establishes `muffleWarning` around `warning()` and
/// `muffleMessage` around `message()` so a calling handler can suppress the
/// default action and let evaluation resume.
///
/// A handler invoking *that* restart comes back as `Ok(Muffled)`; a handler
/// invoking any other restart keeps unwinding.
fn signal_condition_with_muffle(
    msg: &str,
    classes: &[String],
    muffle: &str,
    call: Option<String>,
) -> Result<Signalled, String> {
    let cond = mk_condition(
        msg,
        &classes.iter().map(String::as_str).collect::<Vec<_>>(),
        call,
    );
    let id = push_restarts(vec![(muffle.to_string(), String::new(), None)])[0];
    let out = signal_to_handlers(&cond, classes);
    with_host(|h| {
        h.restarts.pop();
    });
    match out {
        Err(e) => match with_host(|h| h.restart_invoke.take_if(|(t, _)| *t == id)) {
            Some(_) => Ok(Signalled::Muffled),
            None => Err(e),
        },
        Ok(v) => Ok(v),
    }
}

// ── restarts ────────────────────────────────────────────────────────────

/// The id reserved for R's top-level `abort` restart, which is established by
/// the evaluator itself rather than by any `withRestarts` call. Nothing ever
/// catches a transfer to it, so it reaches the top and ends the program.
const ABORT_RESTART: u64 = 0;

/// Establish a group of restarts — one `withRestarts` call, or the single
/// muffle restart a signalling builtin puts around itself — and return their
/// ids in order. The caller pops the group with `h.restarts.pop()`.
fn push_restarts(specs: Vec<(String, String, Option<Value>)>) -> Vec<u64> {
    with_host(|h| {
        let group: Vec<crate::host::RestartFrame> = specs
            .into_iter()
            .map(|(name, description, handler)| {
                h.next_restart_id += 1;
                crate::host::RestartFrame {
                    id: h.next_restart_id,
                    name,
                    description,
                    handler,
                }
            })
            .collect();
        let ids = group.iter().map(|r| r.id).collect();
        h.restarts.push(group);
        ids
    })
}

/// Every established restart as `(id, name, description)`, innermost group
/// first — the order `computeRestarts` reports and `invokeRestart` searches.
fn visible_restarts() -> Vec<(u64, String, String)> {
    with_host(|h| {
        h.restarts
            .iter()
            .rev()
            .flatten()
            .map(|r| (r.id, r.name.clone(), r.description.clone()))
            .collect()
    })
}

/// R's restart object: a six-element list of
/// `name, exit, handler, description, test, interactive` with class `"restart"`.
/// rlang keeps the establishing frame's id in the `exit` slot, which is what
/// lets `invokeRestart(restartObject)` target one *particular* frame when two
/// nested `withRestarts` share a name.
fn mk_restart(id: u64, name: &str, description: &str) -> Value {
    let out = mk_list(vec![
        scalar_str(name),
        scalar_dbl(id as f64),
        null(),
        scalar_str(description),
        null(),
        null(),
    ]);
    set_names(
        &out,
        [
            "name",
            "exit",
            "handler",
            "description",
            "test",
            "interactive",
        ]
        .iter()
        .map(|s| Some((*s).to_string()))
        .collect(),
    );
    let cls = scalar_str("restart");
    with_host(|h| h.set_attr(&out, "class", cls));
    out
}

/// R's top-level `abort` restart object, which is *not* shaped like the ones
/// `withRestarts` builds: an unnamed two-element list, so `r$name` is `NULL`.
fn mk_abort_restart() -> Value {
    let out = mk_list(vec![scalar_str("abort"), null()]);
    let cls = scalar_str("restart");
    with_host(|h| h.set_attr(&out, "class", cls));
    out
}

/// `computeRestarts(cond = NULL)` — every restart in scope, innermost first,
/// ending with the `abort` restart the evaluator always provides.
fn compute_restarts() -> Value {
    let mut items: Vec<Value> = visible_restarts()
        .into_iter()
        .map(|(id, name, desc)| mk_restart(id, &name, &desc))
        .collect();
    items.push(mk_abort_restart());
    mk_list(items)
}

/// `invokeRestart(r, ...)` — transfer control to the restart named (or held) by
/// `r`, passing the remaining arguments to its handler.
///
/// This does not return: it records the target and unwinds with
/// [`RESTART_UNWIND`], which no handler absorbs, until the establishing frame
/// recognises the id. `on.exit` cleanups and `finally` blocks along the way
/// still run, because the transfer rides R's ordinary unwind.
fn invoke_restart(a: &Args) -> Result<Value, String> {
    let r = a.req(0, "r")?;
    let args = a.rest(1);
    let named = str1(&r).unwrap_or_default();
    let id = if class_of(&r).iter().any(|c| c == "restart") {
        // A restart object names an exact frame; the `abort` restart carries no
        // `exit` slot, so falling through to its reserved id is correct.
        element_field(&r, "exit")
            .and_then(|v| num1(&v))
            .map(|v| v as u64)
            .or(Some(ABORT_RESTART))
            .filter(|id| *id == ABORT_RESTART || visible_restarts().iter().any(|(i, ..)| i == id))
    } else if named == "abort" {
        Some(ABORT_RESTART)
    } else {
        visible_restarts()
            .into_iter()
            .find(|(_, n, _)| *n == named)
            .map(|(id, ..)| id)
    };
    let name = if named.is_empty() {
        element_field(&r, "name")
            .and_then(|v| str1(&v))
            .unwrap_or_default()
    } else {
        named
    };
    let Some(id) = id else {
        return Err(format!("no 'restart' '{name}' found"));
    };
    with_host(|h| h.restart_invoke = Some((id, args)));
    Err(crate::host::RESTART_UNWIND.to_string())
}

/// `withRestarts(expr, name = handler, …)` — run `expr` with restarts
/// established, and if one is invoked, return its handler's value as the value
/// of this call.
///
/// A restart spec is the handler itself, a `list(handler = , description = )`,
/// or a bare description string — R's `makeRestartList` accepts all three, and
/// a string one leaves the handler as `function(...) NULL`.
fn with_restarts(a: &Args) -> Result<Value, String> {
    let body = a.req(0, "expr")?;
    let specs: Vec<(String, String, Option<Value>)> = a
        .all
        .iter()
        .filter_map(|(t, v)| match t.as_deref() {
            Some("expr") | None => None,
            Some(name) => {
                let (handler, description) = match (element_field(v, "handler"), str1(v)) {
                    (Some(h), _) => (
                        Some(h),
                        element_field(v, "description")
                            .and_then(|d| str1(&d))
                            .unwrap_or_default(),
                    ),
                    (None, Some(desc)) if !with_host(|h| h.is_function(v)) => (None, desc),
                    _ => (Some(v.clone()), String::new()),
                };
                Some((name.to_string(), description, handler))
            }
        })
        .collect();
    let ids = push_restarts(specs);
    let out = call_value(&body, Vec::new(), None);
    let group = with_host(|h| h.restarts.pop()).unwrap_or_default();
    let Err(e) = out else { return out };
    // A transfer to one of *our* restarts stops here and becomes this call's
    // value; anything else — a real error, or a transfer aimed further out —
    // keeps going.
    let Some((id, args)) = with_host(|h| h.restart_invoke.take_if(|(t, _)| ids.contains(t))) else {
        return Err(e);
    };
    match group
        .into_iter()
        .find(|r| r.id == id)
        .and_then(|r| r.handler)
    {
        Some(f) => call_value(&f, args, None),
        None => Ok(null()),
    }
}

/// Whether the unwind currently in flight is a restart transfer rather than an
/// error. No handler — `tryCatch`, `try`, or the top-level reporter — may
/// absorb one; only the frame that established the target restart may.
fn restart_in_flight() -> bool {
    with_host(|h| h.restart_invoke.is_some())
}

/// Raise a condition: record the message and its class vector, and let the
/// normal error unwind carry it out to the nearest `tryCatch`.
fn raise_condition(msg: String, classes: Vec<String>, call: Option<String>) {
    with_host(|h| {
        if h.error.is_none() {
            h.set_error_call(call);
            h.error = Some(msg);
            h.error_classes = classes;
        }
    });
}

/// `tryCatch(function() expr, <class> = handler, …, finally = function() f)`.
///
/// The compiler has already wrapped `expr` and `finally` in zero-argument
/// closures (see `thunk_lazy_args`), so this decides *when* they run. The body
/// executes on its own nested VM, which is what bounds the unwind: an error
/// inside it surfaces here as an `Err` with the host's error state already
/// cleared, so a matching handler simply returns a value instead.
fn try_catch(a: &Args) -> Result<Value, String> {
    let body = a.req(0, "expr")?;
    let finally = a.named("finally");
    // Every named argument other than `expr`/`finally` is a handler keyed by the
    // condition class it catches.
    let handlers: Vec<(String, Value)> = a
        .all
        .iter()
        .filter_map(|(t, v)| match t.as_deref() {
            Some("finally") | Some("expr") | None => None,
            Some(t) => Some((t.to_string(), v.clone())),
        })
        .collect();
    with_host(|h| {
        h.handlers.push(crate::host::HandlerFrame {
            calling: false,
            handlers: handlers.clone(),
            disabled: false,
            muffle: None,
        })
    });
    let out = call_value(&body, Vec::new(), None);
    // The error's own call comes off the host with its classes: the unwind has
    // already cut the context stack back past the frame that raised it, so the
    // condition handed to the handler can only be built from what was recorded.
    let (raised, raised_call) = with_host(|h| {
        h.handlers.pop();
        let call = h.error_call.clone();
        h.clear_error_call();
        (std::mem::take(&mut h.error_classes), call)
    });
    let result = match out {
        Ok(v) => Ok(v),
        // A restart transfer is passing through on its way to the frame that
        // established it; `finally` still runs, but no handler here may claim it.
        Err(msg) if restart_in_flight() => Err(msg),
        Err(msg) => {
            // An error with no class vector is a plain `stop()`.
            let classes: Vec<String> = if raised.is_empty() {
                ["simpleError", "error", "condition"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect()
            } else {
                raised
            };
            match handlers
                .iter()
                .find(|(c, _)| classes.iter().any(|k| k == c))
            {
                Some((_, f)) => {
                    let cond = mk_condition(
                        &msg,
                        &classes.iter().map(String::as_str).collect::<Vec<_>>(),
                        raised_call.clone(),
                    );
                    call_value(f, vec![(None, cond)], None)
                }
                // Nothing here handles it — keep unwinding.
                None => Err(msg),
            }
        }
    };
    // `finally` runs whichever way the body went, before the result leaves. Its
    // own visibility is not the call's: `tryCatch(42, finally = cat("x\n"))`
    // still prints 42.
    if let Some(f) = finally {
        let vis = with_host(|h| h.visible);
        call_value(&f, Vec::new(), None)?;
        with_host(|h| h.visible = vis);
    }
    result
}

/// `withCallingHandlers(expr, <class> = handler, …)` — run `expr` with handlers
/// that do **not** unwind.
///
/// This installs the frame and nothing else: the handlers fire at the point the
/// condition is signalled (see `signal_to_handlers`), with the signalling frame
/// still live, and when one returns normally the signaller carries on. That is
/// the whole difference from `tryCatch`, and it is what makes the standard
/// muffling idiom — handle the warning, `invokeRestart("muffleWarning")`,
/// resume — behave as R's does.
fn with_calling_handlers(a: &Args) -> Result<Value, String> {
    let body = a.req(0, "expr")?;
    let handlers: Vec<(String, Value)> = a
        .all
        .iter()
        .filter_map(|(t, v)| match t.as_deref() {
            Some("expr") | None => None,
            Some(t) => Some((t.to_string(), v.clone())),
        })
        .collect();
    with_host(|h| {
        h.handlers.push(crate::host::HandlerFrame {
            calling: true,
            handlers,
            disabled: false,
            muffle: None,
        })
    });
    let out = call_value(&body, Vec::new(), None);
    with_host(|h| {
        h.handlers.pop();
    });
    out
}

/// `suppressWarnings(expr)` / `suppressMessages(expr)` — R defines these as a
/// `withCallingHandlers` whose handler does nothing but invoke the built-in
/// muffle restart, so the condition is discarded and `expr` resumes.
///
/// rlang installs a handler frame with no R-level handler at all and muffles on
/// the spot, which is the same observable behaviour without building a closure
/// per call.
fn suppress_conditions(a: &Args, class: &str) -> Result<Value, String> {
    let body = a.req(0, "expr")?;
    with_host(|h| {
        h.handlers.push(crate::host::HandlerFrame {
            calling: true,
            handlers: Vec::new(),
            disabled: false,
            muffle: Some(class.to_string()),
        })
    });
    let out = call_value(&body, Vec::new(), None);
    with_host(|h| {
        h.handlers.pop();
    });
    out
}

/// `try(expr, silent = FALSE)` — run `expr`, and on error return the message as
/// an invisible `"try-error"` string instead of aborting.
fn r_try(a: &Args) -> Result<Value, String> {
    let body = a.req(0, "expr")?;
    let silent = a.named("silent").and_then(|v| lgl1(&v)).unwrap_or(false);
    match call_value(&body, Vec::new(), None) {
        Ok(v) => Ok(v),
        // A restart transfer is not an error and `try` does not catch one.
        Err(msg) if restart_in_flight() => Err(msg),
        Err(msg) => {
            let (classes, raised_call) = with_host(|h| {
                let call = h.error_call.clone();
                h.clear_error_call();
                (std::mem::take(&mut h.error_classes), call)
            });
            // R heads the string with the call when the error carries one:
            // `Error in f() : msg`, and a bare `Error : msg` when it does not.
            let text = match raised_call.as_deref().and_then(|c| c.lines().next()) {
                Some(c) => format!("Error in {c} : {msg}\n"),
                None => format!("Error : {msg}\n"),
            };
            if !silent {
                eprint!("{text}");
            }
            let out = scalar_str(text);
            let classes: Vec<&str> = if classes.is_empty() {
                vec!["simpleError", "error", "condition"]
            } else {
                classes.iter().map(String::as_str).collect()
            };
            // Every value is built before the host is borrowed — allocating one
            // borrows it too, and `with_host` is not re-entrant.
            let cls = scalar_str("try-error");
            let cond = mk_condition(&msg, &classes, raised_call);
            with_host(|h| {
                h.set_attr(&out, "class", cls);
                h.set_attr(&out, "condition", cond);
                h.visible = false;
            });
            Ok(out)
        }
    }
}

/// `NextMethod()` — continue S3 dispatch at the class *after* the one whose
/// method is running. `UseMethod` records the remaining class vector on the
/// method's frame; this walks the rest of it, ending at `<generic>.default`.
fn next_method() -> Result<Value, String> {
    let (dispatch, args) = with_host(|h| {
        let f = h.frames.last();
        (
            f.and_then(|f| f.dispatch.clone()),
            f.map(|f| f.args.clone()).unwrap_or_default(),
        )
    });
    let (generic, rest) =
        dispatch.ok_or_else(|| "NextMethod called from outside a method".to_string())?;
    dispatch_from(&generic, &rest, args)
}

/// Run the first `<generic>.<class>` found in `classes`, recording the classes
/// after it so a `NextMethod` inside that method continues where this left off.
/// With no method left, the generic's own primitive is the default.
fn dispatch_from(
    generic: &str,
    classes: &[String],
    args: Vec<(Option<String>, Value)>,
) -> Result<Value, String> {
    for (i, cls) in classes.iter().enumerate() {
        let method = format!("{generic}.{cls}");
        if let Some(f) = with_host(|h| h.lookup_function(&method)) {
            with_host(|h| {
                h.pending_dispatch = Some((generic.to_string(), classes[i + 1..].to_vec()))
            });
            let out = call_value(&f, args, Some(method));
            with_host(|h| h.pending_dispatch = None);
            return out;
        }
    }
    // Nothing further defined: fall through to the primitive behind the generic,
    // which is what R's internal default dispatch does. The flag stops that call
    // re-entering S3 dispatch on the same object and looping forever.
    with_host(|h| h.suppress_s3 = true);
    call_primitive(generic, args)
}

/// `UseMethod("generic")` — S3 dispatch on the class vector of the first
/// argument of the *calling* function, falling back to `generic.default`.
fn use_method(a: &Args) -> Result<Value, String> {
    let generic = str1(&a.req(0, "generic")?).unwrap_or_default();
    let frame_args = with_host(|h| h.frames.last().map(|f| f.args.clone()).unwrap_or_default());
    let obj = match a.get(1, "object") {
        Some(v) => v,
        None => frame_args
            .first()
            .map(|(_, v)| v.clone())
            .ok_or_else(|| format!("UseMethod(\"{generic}\") applied to an object-less call"))?,
    };
    let mut classes = class_of(&obj);
    classes.push("default".to_string());
    if !classes
        .iter()
        .any(|c| with_host(|h| h.lookup_function(&format!("{generic}.{c}")).is_some()))
    {
        return Err(format!(
            "no applicable method for '{generic}' applied to an object of class \"{}\"",
            class_of(&obj).first().cloned().unwrap_or_default()
        ));
    }
    let out = dispatch_from(&generic, &classes, frame_args)?;
    // The generic returns whatever the method returned.
    with_host(|h| h.signal = Some(Signal::Return(out.clone())));
    Ok(out)
}

// ===========================================================================
// Printing — R's own layout.
// ===========================================================================

/// Print a value the way R's default `print` does.
pub fn print_value(v: &Value) {
    for line in format_value(v) {
        crate::host::emit(&format!("{line}\n"));
    }
}

/// Render a value into the lines `print` would emit, including the trailing
/// `attr(,"name")` blocks R appends for every non-structural attribute.
pub fn format_value(v: &Value) -> Vec<String> {
    let mut out = format_value_body(v);
    out.extend(format_extra_attrs(v));
    out
}

/// R's `print.default` follows a value with an `attr(,"name")` block for every
/// attribute that is not part of the value's own structure — the structural
/// ones are already shown as names, a matrix layout, or a `Levels:` line.
/// Whether one of `v`'s classes has a print layout of its own in
/// `format_value_body`, rather than falling through to `print.default`.
fn has_print_layout(v: &Value) -> bool {
    class_of(v).iter().any(|c| {
        matches!(
            c.as_str(),
            "factor" | "ordered" | "table" | "rle" | "condition" | "restart"
        )
    })
}

fn format_extra_attrs(v: &Value) -> Vec<String> {
    const STRUCTURAL: [&str; 6] = ["names", "dim", "dimnames", "levels", "row.names", "tsp"];
    // `class` is structural only for the classes that have their own layout
    // above; a class rlang has no method for is shown, the way R's
    // `print.default` shows one it has no method for.
    let laid_out = has_print_layout(v);
    let mut out = Vec::new();
    for (k, val) in with_host(|h| h.attrs_of(v)) {
        if STRUCTURAL.contains(&k.as_str()) || (k == "class" && laid_out) {
            continue;
        }
        out.push(format!("attr(,\"{k}\")"));
        out.extend(format_value(&val));
    }
    out
}

/// The value's own layout, before any `attr(,…)` tail.
fn format_value_body(v: &Value) -> Vec<String> {
    // Class-based print methods that override the default vector layout.
    let classes = class_of(v);
    if classes.iter().any(|c| c == "factor") {
        return format_factor(v);
    }
    if classes.iter().any(|c| c == "table") {
        // R heads a 1-D table with the name of its `dimnames` element — the
        // deparsed argument, e.g. `z` for `table(z)` — then the named-vector
        // body. A table built from a non-symbol argument has no such name and
        // is headed with a blank line.
        let header = with_host(|h| h.attr(v, "dimnames"))
            .map(|dn| names_of(&dn))
            .and_then(|n| n.first().cloned().flatten())
            .unwrap_or_default();
        let mut out = vec![header];
        out.extend(format_vector(v));
        return out;
    }
    if classes.iter().any(|c| c == "rle") {
        return format_rle(v);
    }
    // `print.restart`: the name only — R's method shows neither the handler nor
    // the description.
    if classes.iter().any(|c| c == "restart") {
        let name = element_field(v, "name")
            .and_then(|n| str1(&n))
            .or_else(|| str1(&element_at(v, 0)))
            .unwrap_or_default();
        return vec![format!("<restart: {name} >")];
    }
    // `print.condition`: `<simpleError in f(): msg>`, or without the `in` clause
    // when the condition carries no call — which is always here, since rlang
    // records none.
    if classes.iter().any(|c| c == "condition") {
        let msg = element_field(v, "message")
            .and_then(|m| str1(&m))
            .unwrap_or_default();
        let call = element_field(v, "call").filter(|c| !is_null(c));
        let head = classes.first().cloned().unwrap_or_default();
        return match call {
            Some(c) => vec![format!("<{head} in {}: {msg}>", deparse_value(&c))],
            None => vec![format!("<{head}: {msg}>")],
        };
    }
    match data(v) {
        RData::Null => vec!["NULL".into()],
        RData::Closure { .. } | RData::Builtin(_) | RData::Combinator { .. } => format_function(v),
        // A foreign R object prints the way R would print it.
        #[cfg(not(target_arch = "wasm32"))]
        RData::RForeign(ptr) => crate::rembed::print_foreign(ptr),
        #[cfg(target_arch = "wasm32")]
        RData::RForeign(_) => vec!["<R object>".into()],
        RData::Environment(_) => vec!["<environment>".into()],
        // An unevaluated expression prints as its source, unquoted and
        // unindexed — `print(quote(f(1)))` is `f(1)`, not `[1] "f(1)"`.
        RData::Lang(e) => crate::deparse::deparse_all_lines(&e),
        // A name prints backquoted unless the parser would read it back as the
        // same symbol — `as.name("+")` shows as `+` in backticks.
        RData::Sym(n) => vec![match crate::deparse::is_syntactic_name(&n) {
            true => n,
            false => format!("`{n}`"),
        }],
        RData::Args(_) => format_list(v),
        RData::List(_) => format_list(v),
        _ => {
            if let Some(dim) = with_host(|h| h.attr(v, "dim")) {
                let d: Vec<usize> = as_int(&dim)
                    .iter()
                    .map(|e| e.unwrap_or(0) as usize)
                    .collect();
                if d.len() == 2 {
                    return format_matrix(v, d[0], d[1]);
                }
                if d.len() >= 3 {
                    return format_array(v, &d);
                }
            }
            format_vector(v)
        }
    }
}

/// Print a 3-D+ array as a sequence of 2-D slices headed `, , k` (`, , k, l` for
/// higher ranks), each slice being the first two dimensions at the fixed outer
/// indices — R's `print.default` for arrays.
fn format_array(v: &Value, dims: &[usize]) -> Vec<String> {
    let (nr, nc) = (dims[0], dims[1]);
    let plane = nr * nc;
    let outer: Vec<usize> = dims[2..].to_vec();
    let n_planes: usize = outer.iter().product::<usize>().max(1);
    let dn = dimnames_of(v);
    let margin = |d: usize| dn.get(d).cloned().flatten();
    let mut out = Vec::new();
    let mut oidx = vec![0usize; outer.len()];
    for p in 0..n_planes {
        // A labelled outer margin heads its plane with the label, else the index.
        let labels: Vec<String> = oidx
            .iter()
            .enumerate()
            .map(|(k, i)| {
                margin(k + 2)
                    .and_then(|l| l.get(*i).cloned().flatten())
                    .unwrap_or_else(|| (i + 1).to_string())
            })
            .collect();
        out.push(format!(", , {}", labels.join(", ")));
        out.push(String::new());
        // Extract this plane (contiguous in column-major order) and print it as
        // a matrix, carrying the first two margins' labels onto it.
        let base = p * plane;
        let pos: Vec<Option<usize>> = (0..plane).map(|i| Some(base + i)).collect();
        let slice = take_positions(v, &pos);
        set_dimnames(&slice, margin(0), margin(1));
        out.extend(format_matrix(&slice, nr, nc));
        out.push(String::new());
        for k in 0..outer.len() {
            oidx[k] += 1;
            if oidx[k] < outer[k] {
                break;
            }
            oidx[k] = 0;
        }
    }
    out
}

/// The deparsed source lines of `v` if it is a function, else `None` — the
/// shared path behind `print(f)`, `deparse(f)` and `format(f)`.
fn function_src(v: &Value) -> Option<Vec<String>> {
    match data(v) {
        // A primitive deparses to just its `.Primitive` call — R shows the
        // formals only when *printing* it, not when deparsing it.
        RData::Builtin(name) => Some(vec![format!(".Primitive(\"{name}\")")]),
        RData::Closure { .. } | RData::Combinator { .. } => Some(format_function(v)),
        _ => None,
    }
}

/// The lines `print` shows for a function. A closure shows its deparsed source
/// — `Rscript` runs with `keep.source = FALSE`, so R re-renders the parse tree
/// rather than echoing the original text, and `ClosureDef::src` holds exactly
/// that rendering (see [`crate::deparse`]).
fn format_function(v: &Value) -> Vec<String> {
    match data(v) {
        RData::Builtin(name) => vec![format!("function (...) .Primitive(\"{name}\")")],
        RData::Closure { id, .. } => with_host(|h| h.closures.get(id).map(|c| c.src.clone()))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| vec!["function (...) ...".into()]),
        _ => vec!["function".into()],
    }
}

/// One element as `print` shows it: strings quoted, NA unquoted.
fn print_element(v: &Value, i: usize) -> String {
    match data(v) {
        RData::Str(xs) => match &xs[i] {
            // `print` shows the escaped source form (`cat` shows the raw text).
            Some(s) => format!("\"{}\"", escape_string(s)),
            None => "NA".into(),
        },
        RData::Lgl(xs) => match xs[i] {
            Some(true) => "TRUE".into(),
            Some(false) => "FALSE".into(),
            None => "NA".into(),
        },
        RData::Int(xs) => match xs[i] {
            Some(n) => n.to_string(),
            None => "NA".into(),
        },
        RData::Dbl(xs) => match xs[i] {
            Some(x) => x.to_string(),
            None => "NA".into(),
        },
        _ => String::new(),
    }
}

/// Escape a string the way R's `print` renders it: backslash, quote, and the
/// control characters become their source escapes.
fn escape_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            _ => out.push(c),
        }
    }
    out
}

/// Format the elements of a vector with one shared layout — what makes R print
/// `c(1, 2.5)` as `1.0 2.5` and `c(1e10, 1)` as `1e+10 1e+00`: the decimal count
/// and the fixed-vs-scientific choice are decided once for the whole vector.
fn format_elements(v: &Value) -> Vec<String> {
    let n = len(v);
    if let RData::Dbl(xs) = data(v) {
        return format_dbl_run(&xs);
    }
    (0..n).map(|i| print_element(v, i)).collect()
}

/// Render one run of doubles that share a layout: a common decimal count and a
/// single fixed-versus-scientific choice for all of them. R decides both once
/// per `formatReal` call, so the caller's choice of run *is* the choice of what
/// lines up — a whole vector for `print(x)`, but one **column** at a time for a
/// matrix, which is why `matrix(c(1.5, 2.25, 1, 2), 2)` prints its second column
/// as `1` / `2` and not `1.00` / `2.00`.
fn format_dbl_run(xs: &[Option<f64>]) -> Vec<String> {
    let finite: Vec<f64> = xs
        .iter()
        .flatten()
        .copied()
        .filter(|x| x.is_finite())
        .collect();
    let fixed_d = finite.iter().map(|x| fixed_decimals(*x)).max().unwrap_or(0);
    let sci_d = finite.iter().map(|x| sci_decimals(*x)).max().unwrap_or(0);
    let width = |f: &dyn Fn(f64) -> String| {
        finite
            .iter()
            .map(|x| f(*x).chars().count())
            .max()
            .unwrap_or(0)
    };
    let use_sci = !crate::host::prefers_fixed(
        width(&|x| render_fixed(x, fixed_d)),
        width(&|x| render_sci(x, sci_d)),
    );
    let cells: Vec<String> = xs
        .iter()
        .map(|e| match e {
            Some(x) if x.is_finite() && use_sci => render_sci(*x, sci_d),
            Some(x) if x.is_finite() => render_fixed(*x, fixed_d),
            Some(x) => render_fixed(*x, 0),
            None => "NA".into(),
        })
        .collect();
    if !use_sci {
        // In fixed notation the run's column is just as wide as its widest
        // rendering, so there is nothing to reserve.
        return cells;
    }
    // In scientific notation R sizes the column *structurally* rather than from
    // the widest rendering: a sign column when any element is negative, the
    // leading digit, the shared mantissa decimals, `e` and its sign, and the
    // widest exponent. The two differ whenever a negative element carries a
    // narrower exponent than a positive one — `c(10^100, -1e5)` renders as
    // `1e+100` and `-1e+05`, both six wide, yet R lays the column out seven
    // wide, so the positive one gains a leading space. Reserving the width here
    // means every caller that pads to the widest cell (vector, named vector and
    // matrix layouts alike) gets R's width for free.
    let exp_digits = cells
        .iter()
        .filter_map(|c| {
            c.split_once(['e'])
                .map(|(_, e)| e.trim_start_matches(['+', '-']).len())
        })
        .max()
        .unwrap_or(2);
    let neg = usize::from(cells.iter().any(|c| c.starts_with('-')));
    let w = neg + 1 + if sci_d > 0 { sci_d + 1 } else { 0 } + 2 + exp_digits;
    cells
        .into_iter()
        .map(|c| crate::strwidth::pad_display(&c, w, false))
        .collect()
}

/// Print an `rle` object the way R's `print.rle` does: a header, then the
/// `lengths` and `values` as one-line `str`-style summaries.
fn format_rle(v: &Value) -> Vec<String> {
    let lengths = element_field(v, "lengths").unwrap_or_else(null);
    let values = element_field(v, "values").unwrap_or_else(null);
    let line = |field: &Value| -> String {
        let n = len(field);
        let abbr = match data(field) {
            RData::Int(_) => "int",
            RData::Dbl(_) => "num",
            RData::Str(_) => "chr",
            RData::Lgl(_) => "logi",
            _ => "num",
        };
        let cells: Vec<String> = as_str(field)
            .into_iter()
            .map(|s| {
                let s = s.unwrap_or_else(|| "NA".into());
                if kind(field) == RKind::Str {
                    format!("\"{s}\"")
                } else {
                    s
                }
            })
            .collect();
        // R's `str` shows the `[1:n]` index range only for length > 1.
        if n > 1 {
            format!("{abbr} [1:{n}] {}", cells.join(" "))
        } else {
            format!("{abbr} {}", cells.join(" "))
        }
    };
    vec![
        "Run Length Encoding".to_string(),
        format!("  lengths: {}", line(&lengths)),
        format!("  values : {}", line(&values)),
    ]
}

/// Print a factor: the level labels (unquoted, `[i]`-indexed like a character
/// vector) followed by a `Levels:` line.
fn format_factor(v: &Value) -> Vec<String> {
    let levels: Vec<String> = with_host(|h| h.attr(v, "levels"))
        .map(|l| as_str(&l).into_iter().flatten().collect())
        .unwrap_or_default();
    let labels: Vec<String> = as_int(v)
        .iter()
        .map(|c| {
            c.and_then(|i| levels.get((i - 1) as usize).cloned())
                .unwrap_or_else(|| "<NA>".into())
        })
        .collect();
    let ordered = class_of(v).iter().any(|c| c == "ordered");
    let names = names_of(v);
    let mut out = if labels.is_empty() {
        // A zero-length factor prints as `factor()` / `ordered()` — the empty
        // *call*, not the `type(0)` form a plain empty vector uses.
        vec![format!("{}()", if ordered { "ordered" } else { "factor" })]
    } else if names.is_empty() {
        layout_indexed(&labels, true)
    } else {
        // A named factor prints its names above the labels, like any named
        // vector — the `Levels:` line still follows.
        layout_named(&names, &labels)
    };
    // An ordered factor separates its levels with `<`.
    let sep = if ordered { " < " } else { " " };
    out.push(format!("Levels: {}", levels.join(sep)));
    out
}

/// The `[i]`-prefixed, width-wrapped cell layout R uses for an unnamed vector.
/// `left_align` left-justifies each cell (character-style) rather than
/// right-justifying (numeric-style).
fn layout_indexed(cells: &[String], left_align: bool) -> Vec<String> {
    let n = cells.len();
    const WIDTH: usize = 80;
    let cell_w = cells.iter().map(|c| c.chars().count()).max().unwrap_or(1);
    let idx_w = format!("[{n}]").len();
    let per_line = ((WIDTH - idx_w) / (cell_w + 1)).max(1);
    let mut out = Vec::new();
    let mut i = 0;
    while i < n {
        let take = per_line.min(n - i);
        let cells_row: Vec<String> = (i..i + take)
            .map(|k| {
                if left_align {
                    format!("{:<cell_w$}", cells[k])
                } else {
                    format!("{:>cell_w$}", cells[k])
                }
            })
            .collect();
        out.push(format!(
            "{:>idx_w$} {}",
            format!("[{}]", i + 1),
            cells_row.join(" ")
        ));
        i += take;
    }
    out
}

/// A named vector's two-row layout: names above values, sharing one column
/// width, both right-justified, wrapped at 80 columns. Shared by the plain
/// vector printer and the factor printer, which differ only in how they render
/// a cell.
fn layout_named(names: &[Option<String>], cells: &[String]) -> Vec<String> {
    const WIDTH: usize = 80;
    let n = cells.len();
    let labels: Vec<String> = (0..n)
        .map(|i| {
            names
                .get(i)
                .cloned()
                .flatten()
                .unwrap_or_else(|| "<NA>".into())
        })
        .collect();
    let w = labels
        .iter()
        .chain(cells.iter())
        .map(|s| crate::strwidth::display_width(s))
        .max()
        .unwrap_or(1);
    let per_line = (WIDTH / (w + 1)).max(1);
    let mut out = Vec::new();
    let mut i = 0;
    while i < n {
        let take = per_line.min(n - i);
        let row = |src: &[String]| {
            (i..i + take)
                .map(|k| crate::strwidth::pad_display(&src[k], w, false))
                .collect::<Vec<_>>()
                .join(" ")
                + " "
        };
        out.push(row(&labels));
        out.push(row(cells));
        i += take;
    }
    out
}

fn format_vector(v: &Value) -> Vec<String> {
    let n = len(v);
    if n == 0 {
        let kind = match data(v) {
            RData::Str(_) => "character",
            RData::Int(_) => "integer",
            RData::Lgl(_) => "logical",
            RData::List(_) => "list",
            _ => "numeric",
        };
        // An empty vector that still carries a `names` attribute prints as
        // `named integer(0)` — `names_of` flattens both "no attribute" and
        // "empty attribute" to an empty vec, so the attribute is read directly.
        let named = with_host(|h| h.attr(v, "names")).is_some_and(|n| !is_null(&n));
        return vec![match named {
            true => format!("named {kind}(0)"),
            false => format!("{kind}(0)"),
        }];
    }
    let cells = format_elements(v);
    let names = names_of(v);
    const WIDTH: usize = 80;

    // Character vectors are left-justified; everything else is right-justified.
    // A *named* vector right-justifies both rows regardless of type.
    let left_align = kind(v) == RKind::Str && names.is_empty();
    let justify = |cell: &str, w: usize| crate::strwidth::pad_display(cell, w, left_align);

    if !names.is_empty() {
        return layout_named(&names, &cells);
    }

    // Column budget is terminal columns, not code points — a double-width
    // character occupies two.
    let cell_w = cells
        .iter()
        .map(|c| crate::strwidth::display_width(c))
        .max()
        .unwrap_or(1);
    let idx_w = format!("[{n}]").len();
    let per_line = ((WIDTH - idx_w) / (cell_w + 1)).max(1);
    let mut out = Vec::new();
    let mut i = 0;
    while i < n {
        let take = per_line.min(n - i);
        let body = (i..i + take)
            .map(|k| justify(&cells[k], cell_w))
            .collect::<Vec<_>>()
            .join(" ");
        out.push(format!("{:>w$} {body}", format!("[{}]", i + 1), w = idx_w));
        i += take;
    }
    out
}

fn format_matrix(v: &Value, nr: usize, nc: usize) -> Vec<String> {
    // With no rows *and* no columns there is nothing to head, so R names the
    // shape instead of printing an empty header line.
    if nr == 0 && nc == 0 {
        return vec!["<0 x 0 matrix>".into()];
    }
    // R's `printRealMatrix` calls `formatReal` once per column, so each column
    // gets its own decimal count and its own fixed-versus-scientific choice —
    // a column of whole numbers stays whole next to a column of decimals.
    // Only doubles carry a shared layout; every other type formats element by
    // element, where the run boundary makes no difference.
    let cells = match data(v) {
        RData::Dbl(xs) if nr > 0 => (0..nc)
            .flat_map(|c| {
                let lo = (c * nr).min(xs.len());
                let hi = (lo + nr).min(xs.len());
                format_dbl_run(&xs[lo..hi])
            })
            .collect(),
        _ => format_elements(v),
    };
    let dn = dimnames_of(v);
    let dn_at = |dim: usize, k: usize| -> Option<String> {
        dn.get(dim)
            .and_then(|o| o.as_ref())
            .and_then(|names| names.get(k).cloned().flatten())
    };
    let row_labels: Vec<String> = (0..nr)
        .map(|r| dn_at(0, r).unwrap_or_else(|| format!("[{},]", r + 1)))
        .collect();
    // R's `MatrixRowLabel` pads a *name* on the right and a generated `[i,]` on
    // the left, so a 10-row matrix's labels line up on the comma: ` [9,]` then
    // `[10,]`.
    let row_names = dn.first().and_then(|o| o.as_ref()).is_some();
    let col_labels: Vec<String> = (0..nc)
        .map(|c| dn_at(1, c).unwrap_or_else(|| format!("[,{}]", c + 1)))
        .collect();
    // Every width here is terminal columns: a dimname or a cell holding a
    // double-width character claims two per character.
    let dw = |s: &str| crate::strwidth::display_width(s);
    // A matrix with no rows still prints its column header over a row-label
    // gutter, and R fixes that gutter at the width of `[1,]` regardless of what
    // the dimnames would have been — `matrix(1:4, 2)[0, ]` is `     [,1] [,2]`.
    // Deriving the width from the (empty) label list collapsed it to zero.
    let label_w = match nr {
        0 => "[1,]".len(),
        _ => row_labels.iter().map(|s| dw(s)).max().unwrap_or(0),
    };
    let widths: Vec<usize> = (0..nc)
        .map(|c| {
            (0..nr)
                .map(|r| cells.get(c * nr + r).map(|s| dw(s)).unwrap_or(2))
                .chain(std::iter::once(dw(&col_labels[c])))
                .max()
                .unwrap_or(1)
        })
        .collect();
    // Character matrices are left-justified (cells and column headers alike),
    // like character vectors; numeric matrices are right-justified.
    let left = kind(v) == RKind::Str;
    let just = |s: &str, w: usize| crate::strwidth::pad_display(s, w, left);
    let mut out = Vec::with_capacity(nr + 1);
    // With no columns there is nothing after the label gutter, and R does not
    // leave the separating space dangling — the header is a bare run of spaces
    // and each row is its label alone.
    let sep = if nc == 0 { "" } else { " " };
    for (first, last) in column_blocks(&widths, label_w) {
        let header = (first..last)
            .map(|c| just(&col_labels[c], widths[c]))
            .collect::<Vec<_>>()
            .join(" ");
        out.push(format!("{:w$}{sep}{header}", "", w = label_w));
        for (r, label) in row_labels.iter().enumerate() {
            let row = (first..last)
                .map(|c| {
                    just(
                        &cells.get(c * nr + r).cloned().unwrap_or_default(),
                        widths[c],
                    )
                })
                .collect::<Vec<_>>()
                .join(" ");
            out.push(format!(
                "{}{sep}{row}",
                crate::strwidth::pad_display(label, label_w, row_names)
            ));
        }
    }
    out
}

/// Split a matrix's columns into the runs R prints one under the other —
/// `printarray.c`'s `_PRINT_MATRIX_` loop, which lays down as many columns as
/// fit `getOption("width")` and starts a fresh header for the rest.
///
/// Each column costs its own width plus the one-space gap, and the row-label
/// gutter is charged once per block. A block always takes at least one column,
/// however wide, and then takes the next only while the running width *plus*
/// that column stays strictly under the limit — R's `<`, not `<=`, which is
/// what leaves its full lines one column short of 80.
fn column_blocks(widths: &[usize], label_w: usize) -> Vec<(usize, usize)> {
    if widths.is_empty() {
        return vec![(0, 0)];
    }
    const WIDTH: usize = 80;
    let mut blocks = Vec::new();
    let mut first = 0;
    while first < widths.len() {
        let mut last = first;
        let mut width = label_w;
        loop {
            width += widths[last] + 1;
            last += 1;
            if last >= widths.len() || width + widths[last] + 1 >= WIDTH {
                break;
            }
        }
        blocks.push((first, last));
        first = last;
    }
    blocks
}

/// R's `do_matrix` complaint when the data does not tile the matrix — the
/// recycling still happens, and R names the extent that does not divide it.
///
/// The two extents are tried in order and only one is reported: rows first,
/// then columns, then the fall-through for a length that fits neither but
/// divides both. `matrix(1:5, 2, 2)` names the rows, `matrix(1:4, 2, 3)` the
/// columns.
fn matrix_fill_warning(lendat: usize, nr: usize, nc: usize) -> Option<String> {
    if lendat <= 1 {
        return None;
    }
    let total = nr * nc;
    if total == 0 {
        return Some("non-empty data for zero-extent matrix".into());
    }
    if total % lendat == 0 {
        return None;
    }
    // "Divides" in both directions: a length longer than the extent must be a
    // whole number of them, and a shorter one must divide it exactly.
    let tiles = |extent: usize| lendat % extent == 0 || extent % lendat == 0;
    if !tiles(nr) {
        return Some(format!(
            "data length [{lendat}] is not a sub-multiple or multiple of the number of rows [{nr}]"
        ));
    }
    if !tiles(nc) {
        return Some(format!(
            "data length [{lendat}] is not a sub-multiple or multiple of the number of columns [{nc}]"
        ));
    }
    (total != lendat)
        .then(|| format!("data length differs from size of matrix: [{lendat} != {nr} x {nc}]"))
}

/// R quotes a `$name` list header in backticks when the name is not a syntactic
/// R identifier — starts with a digit or `.` followed by a digit, contains a
/// non-`[A-Za-z0-9._]` character, is empty, or is a reserved word.
fn is_syntactic_name(n: &str) -> bool {
    let mut chars = n.chars();
    let first = match chars.next() {
        Some(c) => c,
        None => return false,
    };
    if !(first.is_ascii_alphabetic() || first == '.') {
        return false;
    }
    // `.1` is non-syntactic (a dot immediately followed by a digit).
    if first == '.' {
        if let Some(c2) = n.chars().nth(1) {
            if c2.is_ascii_digit() {
                return false;
            }
        }
    }
    if !n
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
    {
        return false;
    }
    !matches!(
        n,
        "if" | "else"
            | "repeat"
            | "while"
            | "function"
            | "for"
            | "in"
            | "next"
            | "break"
            | "TRUE"
            | "FALSE"
            | "NULL"
            | "Inf"
            | "NaN"
            | "NA"
            | "NA_integer_"
            | "NA_real_"
            | "NA_character_"
            | "NA_complex_"
    )
}

fn format_list(v: &Value) -> Vec<String> {
    format_list_at(v, "")
}

/// Whether `format_value` would lay this value out as a list, i.e. it is a bare
/// list with no class-specific print method of its own.
fn is_plain_list(v: &Value) -> bool {
    !has_print_layout(v) && matches!(kind(v), RKind::List | RKind::Args)
}

/// Render a list, heading each element with the full path taken to reach it.
/// R does not restart the tags inside a nested list: `list(a = list(b = 1))`
/// prints `$a` and then `$a$b`, not a second bare `$b`.
fn format_list_at(v: &Value, prefix: &str) -> Vec<String> {
    let items = elements(v);
    if items.is_empty() {
        return vec!["list()".into()];
    }
    let names = names_of(v);
    let mut out = Vec::new();
    for (i, it) in items.iter().enumerate() {
        // A missing *name* and a missing *names attribute* head differently: an
        // unnamed list numbers its elements `[[i]]`, and so does an element whose
        // name is the empty string (`list(a = 1, 2)`), but an `NA_character_`
        // name — what an unmatched label subscript leaves behind, `l["zz"]` —
        // heads as `$<NA>`.
        let tag = match names.get(i) {
            Some(Some(n)) if n.is_empty() => format!("[[{}]]", i + 1),
            Some(Some(n)) if is_syntactic_name(n) => format!("${n}"),
            Some(Some(n)) => format!("$`{n}`"),
            Some(None) => "$<NA>".to_string(),
            None => format!("[[{}]]", i + 1),
        };
        let path = format!("{prefix}{tag}");
        out.push(path.clone());
        if is_plain_list(it) {
            out.extend(format_list_at(it, &path));
            out.extend(format_extra_attrs(it));
        } else {
            out.extend(format_value(it));
        }
        out.push(String::new());
    }
    out
}

#[cfg(test)]
mod tests {
    use crate::eval_to_string;

    #[test]
    fn arithmetic_recycles_and_keeps_integer_type() {
        assert_eq!(eval_to_string("c(1L, 2L) + 1L").unwrap(), "[1] 2 3");
        assert_eq!(eval_to_string("1:6 * c(1, 0)").unwrap(), "[1] 1 0 3 0 5 0");
    }

    #[test]
    fn na_propagates_but_logic_stays_three_valued() {
        assert_eq!(eval_to_string("NA + 1").unwrap(), "[1] NA");
        assert_eq!(eval_to_string("NA & FALSE").unwrap(), "[1] FALSE");
        assert_eq!(eval_to_string("NA | TRUE").unwrap(), "[1] TRUE");
    }

    #[test]
    fn modulo_follows_the_sign_of_the_divisor() {
        // R: -5 %% 3 is 1, not -2.
        assert_eq!(eval_to_string("-5 %% 3").unwrap(), "[1] 1");
        assert_eq!(eval_to_string("-5 %/% 3").unwrap(), "[1] -2");
    }

    #[test]
    fn negative_subscripts_exclude() {
        assert_eq!(eval_to_string("(1:5)[-1]").unwrap(), "[1] 2 3 4 5");
        assert_eq!(
            eval_to_string("(1:5)[c(TRUE, FALSE)]").unwrap(),
            "[1] 1 3 5"
        );
    }

    #[test]
    fn doubles_share_a_decimal_width_when_printed() {
        assert_eq!(eval_to_string("c(1, 2.5)").unwrap(), "[1] 1.0 2.5");
    }
}
