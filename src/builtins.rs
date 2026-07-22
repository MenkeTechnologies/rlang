//! Registered fusevm builtins, R's vectorized operators, and the primitive
//! function library.
//!
//! Every `Op::CallBuiltin(id, argc)` the compiler emits lands in a `b_*`
//! function here: it marshals values off the VM stack, calls into the
//! thread-local `RHost`, and pushes the result. Host borrows are taken in small
//! scopes — never across a call back into R — so `lapply` can run a closure body
//! on a nested VM while the outer builtin is still on the stack.

use crate::host::{
    call_value, fixed_decimals, format_dbl, ops, render_fixed, render_sci, sci_decimals, with_host,
    RData, Signal,
};
use fusevm::{Value, VM};
use indexmap::IndexMap;
use std::rc::Rc;

/// Register every rlang builtin on `vm`.
pub fn install(vm: &mut VM) {
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
    vm.register_builtin(ops::AUTOPRINT, b_autoprint);
    vm.register_builtin(ops::IS_FALSE, b_is_false);
    vm.register_builtin(ops::IS_TRUE, b_is_true);
    vm.register_builtin(ops::MISSING, b_missing);
    vm.register_builtin(ops::NULL_INVISIBLE, b_null_invisible);
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
fn mk_dbl(xs: Vec<Option<f64>>) -> Value {
    with_host(|h| h.dbl(xs))
}
fn mk_int(xs: Vec<Option<i64>>) -> Value {
    with_host(|h| h.int(xs))
}
fn mk_lgl(xs: Vec<Option<bool>>) -> Value {
    with_host(|h| h.lgl(xs))
}
fn mk_str(xs: Vec<Option<String>>) -> Value {
    with_host(|h| h.str_vec(xs))
}
fn mk_list(xs: Vec<Value>) -> Value {
    with_host(|h| h.list(xs))
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
fn null() -> Value {
    with_host(|h| h.null())
}
fn names_of(v: &Value) -> Vec<Option<String>> {
    with_host(|h| h.names(v))
}
fn class_of(v: &Value) -> Vec<String> {
    with_host(|h| h.class_of(v))
}
fn elements(v: &Value) -> Vec<Value> {
    with_host(|h| h.elements(v))
}
fn element_at(v: &Value, i: usize) -> Value {
    with_host(|h| h.element_at(v, i))
}
fn set_names(v: &Value, names: Vec<Option<String>>) {
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
    match with_host(|h| h.lookup(&name)) {
        Some(v) => v,
        None => match primitive_value(&name) {
            Some(v) => v,
            None => abort(vm, format!("object '{name}' not found")),
        },
    }
}

fn b_getfun(vm: &mut VM, _: u8) -> Value {
    let name = name_of(&vm.pop());
    match with_host(|h| h.lookup_function(&name)) {
        Some(v) => v,
        None => match primitive_value(&name) {
            Some(v) => v,
            None => abort(vm, format!("could not find function \"{name}\"")),
        },
    }
}

/// A primitive as a first-class value, so `sapply(x, sqrt)` works.
fn primitive_value(name: &str) -> Option<Value> {
    is_primitive(name).then(|| with_host(|h| h.alloc(RData::Builtin(name.to_string()))))
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

fn b_call(vm: &mut VM, _: u8) -> Value {
    let argv = vm.pop();
    let f = vm.pop();
    let args = match data(&argv) {
        RData::Args(a) => a,
        _ => Vec::new(),
    };
    let name = match data(&f) {
        RData::Builtin(n) => Some(n),
        _ => None,
    };
    with_host(|h| h.visible = true);
    match call_value(&f, args, name) {
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
    match op.as_str() {
        "-" => match data(&x) {
            RData::Int(v) => mk_int(v.iter().map(|e| e.map(|n| -n)).collect()),
            _ => mk_dbl(as_dbl(&x).iter().map(|e| e.map(|n| -n)).collect()),
        },
        "+" => x,
        "!" => mk_lgl(as_lgl(&x).iter().map(|e| e.map(|b| !b)).collect()),
        other => abort(vm, format!("invalid unary operator '{other}'")),
    }
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
            match with_host(|h| h.lookup_function(&fname)) {
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
    let hay: Vec<Option<String>> = as_str(table);
    let out = as_str(x)
        .into_iter()
        .map(|e| Some(hay.contains(&e)))
        .collect();
    mk_lgl(out)
}

/// R's binary operators, vectorized with recycling and NA propagation.
pub fn binop(op: &str, lhs: &Value, rhs: &Value) -> Result<Value, String> {
    match op {
        "+" | "-" | "*" | "/" | "^" | "%%" | "%/%" => arith(op, lhs, rhs),
        "==" | "!=" | "<" | ">" | "<=" | ">=" => compare(op, lhs, rhs),
        "&" | "|" => logic(op, lhs, rhs),
        ":" => Ok(colon(lhs, rhs)),
        other => Err(format!("invalid operator '{other}'")),
    }
}

/// The recycled length of a binary operation, and whether it is empty.
fn recycle_len(a: usize, b: usize) -> usize {
    if a == 0 || b == 0 {
        0
    } else {
        a.max(b)
    }
}

/// Copy `names`/`dim` from the operand that shaped the result.
fn carry_attrs(out: &Value, lhs: &Value, rhs: &Value) {
    let src = if len(lhs) >= len(rhs) { lhs } else { rhs };
    for key in ["names", "dim"] {
        if let Some(a) = with_host(|h| h.attr(src, key)) {
            with_host(|h| h.set_attr(out, key, a));
        }
    }
}

fn arith(op: &str, lhs: &Value, rhs: &Value) -> Result<Value, String> {
    if matches!(data(lhs), RData::Str(_)) || matches!(data(rhs), RData::Str(_)) {
        return Err("non-numeric argument to binary operator".into());
    }
    let n = recycle_len(len(lhs), len(rhs));
    // Integer arithmetic stays integer for `+ - * %% %/%`; `/` and `^` always
    // produce doubles, exactly as R does.
    let int_result = matches!(op, "+" | "-" | "*" | "%%" | "%/%")
        && matches!(data(lhs), RData::Int(_) | RData::Lgl(_))
        && matches!(data(rhs), RData::Int(_) | RData::Lgl(_));
    let (a, b) = (as_dbl(lhs), as_dbl(rhs));
    let mut out: Vec<Option<f64>> = Vec::with_capacity(n);
    for i in 0..n {
        let (x, y) = (a[i % a.len()], b[i % b.len()]);
        out.push(match (x, y) {
            (Some(x), Some(y)) => Some(match op {
                "+" => x + y,
                "-" => x - y,
                "*" => x * y,
                "/" => x / y,
                "^" => x.powf(y),
                // R's %% and %/% follow the sign of the divisor (floored),
                // unlike C's truncated remainder.
                "%%" => x - y * (x / y).floor(),
                _ => (x / y).floor(),
            }),
            _ => None,
        });
    }
    let v = if int_result {
        mk_int(
            out.into_iter()
                .map(|e| e.and_then(|x| x.is_finite().then_some(x as i64)))
                .collect(),
        )
    } else {
        mk_dbl(out)
    };
    carry_attrs(&v, lhs, rhs);
    Ok(v)
}

fn compare(op: &str, lhs: &Value, rhs: &Value) -> Result<Value, String> {
    let n = recycle_len(len(lhs), len(rhs));
    let as_text = matches!(data(lhs), RData::Str(_)) || matches!(data(rhs), RData::Str(_));
    let mut out: Vec<Option<bool>> = Vec::with_capacity(n);
    if as_text {
        let (a, b) = (as_str(lhs), as_str(rhs));
        for i in 0..n {
            let (x, y) = (&a[i % a.len()], &b[i % b.len()]);
            out.push(match (x, y) {
                (Some(x), Some(y)) => Some(cmp_result(op, x.cmp(y))),
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
    let n = recycle_len(len(lhs), len(rhs));
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
        h.visible = true;
        s
    });
    if show {
        print_value(&v);
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

fn b_index(vm: &mut VM, _: u8) -> Value {
    let argv = vm.pop();
    let x = vm.pop();
    match index_single(&x, &args_of(&argv)) {
        Ok(v) => v,
        Err(e) => abort(vm, e),
    }
}

fn b_index2(vm: &mut VM, _: u8) -> Value {
    let argv = vm.pop();
    let x = vm.pop();
    match index_double(&x, &args_of(&argv)) {
        Ok(v) => v,
        Err(e) => abort(vm, e),
    }
}

fn b_dollar(vm: &mut VM, _: u8) -> Value {
    let name = name_of(&vm.pop());
    let x = vm.pop();
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
    // Matrix indexing `m[i, j]`.
    if args.len() == 2 {
        if let Some(dim) = with_host(|h| h.attr(x, "dim")) {
            let d = as_int(&dim);
            if d.len() == 2 {
                let (nr, nc) = (d[0].unwrap_or(0) as usize, d[1].unwrap_or(0) as usize);
                return matrix_index(x, args, nr, nc);
            }
        }
    }
    if supplied.is_empty() {
        return Ok(x.clone());
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
        set_names(&out, sel);
    }
    Ok(out)
}

/// Build a new vector/list from zero-based positions (`None` → NA element).
fn take_positions(x: &Value, pos: &[Option<usize>]) -> Value {
    match data(x) {
        RData::Lgl(v) => mk_lgl(
            pos.iter()
                .map(|p| p.and_then(|i| v.get(i).copied().flatten()))
                .collect(),
        ),
        RData::Int(v) => mk_int(
            pos.iter()
                .map(|p| p.and_then(|i| v.get(i).copied().flatten()))
                .collect(),
        ),
        RData::Dbl(v) => mk_dbl(
            pos.iter()
                .map(|p| p.and_then(|i| v.get(i).copied().flatten()))
                .collect(),
        ),
        RData::Str(v) => mk_str(
            pos.iter()
                .map(|p| p.and_then(|i| v.get(i).cloned().flatten()))
                .collect(),
        ),
        RData::List(v) => mk_list(
            pos.iter()
                .map(|p| p.and_then(|i| v.get(i).cloned()).unwrap_or_else(null))
                .collect(),
        ),
        _ => null(),
    }
}

/// `m[i, j]` over a 2-D `dim` attribute; an empty index selects the whole
/// margin, and a single remaining row/column drops to a plain vector.
fn matrix_index(
    x: &Value,
    args: &[(Option<String>, Value)],
    nr: usize,
    nc: usize,
) -> Result<Value, String> {
    let rows: Vec<usize> = match &args[0].1 {
        Value::Undef => (0..nr).collect(),
        v => resolve_index(v, nr, &[])?.into_iter().flatten().collect(),
    };
    let cols: Vec<usize> = match &args[1].1 {
        Value::Undef => (0..nc).collect(),
        v => resolve_index(v, nc, &[])?.into_iter().flatten().collect(),
    };
    // Column-major storage, like R.
    let mut pos = Vec::with_capacity(rows.len() * cols.len());
    for c in &cols {
        for r in &rows {
            pos.push(Some(c * nr + r));
        }
    }
    let out = take_positions(x, &pos);
    if rows.len() > 1 && cols.len() > 1 {
        let dim = mk_int(vec![Some(rows.len() as i64), Some(cols.len() as i64)]);
        with_host(|h| h.set_attr(&out, "dim", dim));
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
        _ => Ok(element_at(x, i)),
    }
}

fn b_index_set(vm: &mut VM, _: u8) -> Value {
    let value = vm.pop();
    let argv = vm.pop();
    let x = vm.pop();
    match assign_index(&x, &args_of(&argv), &value, false) {
        Ok(v) => v,
        Err(e) => abort(vm, e),
    }
}

fn b_index2_set(vm: &mut VM, _: u8) -> Value {
    let value = vm.pop();
    let argv = vm.pop();
    let x = vm.pop();
    match assign_index(&x, &args_of(&argv), &value, true) {
        Ok(v) => v,
        Err(e) => abort(vm, e),
    }
}

fn b_dollar_set(vm: &mut VM, _: u8) -> Value {
    let value = vm.pop();
    let name = name_of(&vm.pop());
    let x = vm.pop();
    if let RData::Environment(e) = data(&x) {
        e.borrow_mut().vars.insert(name, value);
        return x;
    }
    let key = scalar_str(name);
    let args = vec![(None, key)];
    match assign_index(&x, &args, &value, true) {
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
) -> Result<Value, String> {
    let Some((_, idx)) = args.iter().find(|(_, v)| !matches!(v, Value::Undef)) else {
        return Ok(x.clone());
    };
    let is_list = matches!(data(x), RData::List(_))
        || (single_slot && !is_null(value) && len(value) > 1)
        || matches!(
            data(value),
            RData::List(_) | RData::Closure { .. } | RData::Builtin(_)
        );
    let mut names = names_of(x);
    let n = len(x);

    // Character index that names a new element appends it.
    let mut positions: Vec<usize> = Vec::new();
    let mut new_names: Vec<(usize, String)> = Vec::new();
    match data(idx) {
        RData::Str(keys) => {
            let mut next = n;
            for k in keys.iter().flatten() {
                match names
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
            for p in resolve_index(idx, n.max(highest), &names)? {
                match p {
                    Some(i) => positions.push(i),
                    None => return Err("NAs are not allowed in subscripted assignments".into()),
                }
            }
        }
    }

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
                names.push(None);
            }
            items[*p] = vals[k % vals.len().max(1)].clone();
        }
        for (i, nm) in new_names {
            while names.len() <= i {
                names.push(None);
            }
            names[i] = Some(nm);
        }
        let out = mk_list(items);
        if !names.is_empty() {
            set_names(&out, names.clone());
        }
        for (k, v) in with_host(|h| h.attrs_of(x)) {
            if k != "names" {
                with_host(|h| h.set_attr(&out, &k, v));
            }
        }
        return Ok(out);
    }

    // Atomic assignment: promote to the wider of the two types.
    let rank = with_host(|h| {
        crate::host::type_rank(&h.data_of(x)).max(crate::host::type_rank(&h.data_of(value)))
    });
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
            names.push(None);
        }
        names[i] = Some(nm);
    }
    if !names.is_empty() {
        while names.len() < grow {
            names.push(None);
        }
        set_names(&out, names);
    }
    for (k, v) in with_host(|h| h.attrs_of(x)) {
        if k != "names" {
            with_host(|h| h.set_attr(&out, &k, v));
        }
    }
    Ok(out)
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
    PRIMITIVES.contains(&name) || OPERATORS.contains(&name)
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
    "sprintf",
    "message",
    "warning",
    "stop",
    "invisible",
    "identity",
    "seq",
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
    "any",
    "all",
    "sum",
    "prod",
    "mean",
    "median",
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
    "trunc",
    "sign",
    "cumsum",
    "cumprod",
    "diff",
    "is.null",
    "is.na",
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
    "Reduce",
    "Filter",
    "do.call",
    "nchar",
    "substr",
    "substring",
    "toupper",
    "tolower",
    "strsplit",
    "sub",
    "gsub",
    "grepl",
    "grep",
    "trimws",
    "startsWith",
    "endsWith",
    "matrix",
    "dim",
    "nrow",
    "ncol",
    "t",
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
    "nlevels",
    "typeof",
    "mode",
    "Recall",
    "Negate",
    "toString",
    "rownames",
    "colnames",
];

/// Call a primitive by name with evaluated arguments.
pub fn call_primitive(name: &str, args: Vec<(Option<String>, Value)>) -> Result<Value, String> {
    if OPERATORS.contains(&name) {
        return call_operator(name, &args);
    }
    let a = Args::new(args);
    match name {
        // ── construction and coercion ───────────────────────────────────
        "c" => Ok(concat(&a)),
        "list" => {
            let out = mk_list(a.values());
            let nm = a.tags();
            if nm.iter().any(|n| n.is_some()) {
                set_names(&out, nm);
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
        "as.numeric" | "as.double" => Ok(mk_dbl(as_dbl(&a.req(0, "x")?))),
        "as.integer" => Ok(mk_int(as_int(&a.req(0, "x")?))),
        "as.character" => Ok(mk_str(as_str(&a.req(0, "x")?))),
        "as.logical" => Ok(mk_lgl(as_lgl(&a.req(0, "x")?))),
        "as.vector" => Ok(a.req(0, "x")?),
        "as.list" => {
            let x = a.req(0, "x")?;
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
        "names" => {
            let x = a.req(0, "x")?;
            let nm = names_of(&x);
            Ok(if nm.is_empty() { null() } else { mk_str(nm) })
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
                other => other,
            }))
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
        "rownames" | "colnames" => Ok(null()),

        // ── output ──────────────────────────────────────────────────────
        "print" => {
            let x = a.req(0, "x")?;
            print_value(&x);
            with_host(|h| h.visible = false);
            Ok(x)
        }
        "cat" => {
            let sep = a
                .named("sep")
                .and_then(|v| str1(&v))
                .unwrap_or_else(|| " ".into());
            let mut parts: Vec<String> = Vec::new();
            for (tag, v) in a.all.iter() {
                if tag.as_deref() == Some("sep") || tag.as_deref() == Some("fill") {
                    continue;
                }
                for s in as_str(v) {
                    parts.push(s.unwrap_or_else(|| "NA".into()));
                }
            }
            // R ends `cat` output with a newline whenever the separator itself
            // contains one — `cat(c("a", "b"), sep = "\n")` prints three lines'
            // worth of output, not two.
            let tail = if sep.contains('\n') { "\n" } else { "" };
            print!("{}{tail}", parts.join(&sep));
            with_host(|h| h.visible = false);
            Ok(null())
        }
        "message" | "warning" => {
            let text: Vec<String> = a.values().iter().flat_map(as_str).flatten().collect();
            let text = text.join("");
            if name == "warning" {
                eprintln!("Warning message:\n{text}");
            } else {
                eprintln!("{text}");
            }
            with_host(|h| h.visible = false);
            Ok(null())
        }
        "stop" => {
            let text: Vec<String> = a.values().iter().flat_map(as_str).flatten().collect();
            Err(text.join(""))
        }
        "stopifnot" => {
            for (_, v) in a.all.iter() {
                if !as_lgl(v).iter().all(|e| *e == Some(true)) || len(v) == 0 {
                    return Err("not all arguments are TRUE".into());
                }
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
        "paste" | "paste0" => Ok(paste(&a, name == "paste0")),
        "toString" => {
            let parts: Vec<String> = as_str(&a.req(0, "x")?)
                .into_iter()
                .map(|s| s.unwrap_or_else(|| "NA".into()))
                .collect();
            Ok(scalar_str(parts.join(", ")))
        }
        "format" => Ok(mk_str(
            as_str(&a.req(0, "x")?)
                .into_iter()
                .map(|s| s.or_else(|| Some("NA".into())))
                .collect(),
        )),
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
        "seq" => Ok(seq(&a)),
        "rep" => Ok(rep(&a)),
        "rev" => {
            let x = a.req(0, "x")?;
            let pos: Vec<Option<usize>> = (0..len(&x)).rev().map(Some).collect();
            Ok(take_positions(&x, &pos))
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
            Ok(take_positions(&x, &pos))
        }
        "append" => {
            let x = a.req(0, "x")?;
            let y = a.req(1, "values")?;
            let joined = Args::new(vec![(None, x), (None, y)]);
            Ok(concat(&joined))
        }

        // ── ordering and sets ───────────────────────────────────────────
        "sort" => Ok(sort_value(
            &a.req(0, "x")?,
            a.named("decreasing")
                .and_then(|v| lgl1(&v))
                .unwrap_or(false),
        )),
        "order" => Ok(order_value(
            &a.req(0, "x")?,
            a.named("decreasing")
                .and_then(|v| lgl1(&v))
                .unwrap_or(false),
        )),
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
            Ok(take_positions(&x, &pos))
        }
        "setdiff" | "union" | "intersect" => {
            let x = a.req(0, "x")?;
            let y = a.req(1, "y")?;
            let (xs, ys) = (as_str(&x), as_str(&y));
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
            if name != "union" {
                return Ok(head);
            }
            let mut ypos = Vec::new();
            for (i, k) in ys.iter().enumerate() {
                if !seen.contains(k) {
                    seen.push(k.clone());
                    ypos.push(Some(i));
                }
            }
            let tail = take_positions(&y, &ypos);
            Ok(concat(&Args::new(vec![(None, head), (None, tail)])))
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
            let mut na = false;
            let narm = a.named("na.rm").and_then(|v| lgl1(&v)).unwrap_or(false);
            let all_int = a
                .all
                .iter()
                .filter(|(t, _)| t.as_deref() != Some("na.rm"))
                .all(|(_, v)| matches!(data(v), RData::Int(_) | RData::Lgl(_)));
            for (tag, v) in a.all.iter() {
                if tag.as_deref() == Some("na.rm") {
                    continue;
                }
                for e in as_dbl(v) {
                    match e {
                        Some(x) => {
                            if name == "sum" {
                                acc += x
                            } else {
                                acc *= x
                            }
                        }
                        None if !narm => na = true,
                        None => {}
                    }
                }
            }
            Ok(if na {
                mk_dbl(vec![None])
            } else if all_int && name == "sum" {
                scalar_int(acc as i64)
            } else {
                scalar_dbl(acc)
            })
        }
        "mean" => {
            let xs = numeric_arg(&a, 0, "x")?;
            Ok(if xs.is_empty() {
                mk_dbl(vec![None])
            } else {
                scalar_dbl(xs.iter().sum::<f64>() / xs.len() as f64)
            })
        }
        "median" => {
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
        "var" | "sd" => {
            let xs = numeric_arg(&a, 0, "x")?;
            if xs.len() < 2 {
                return Ok(mk_dbl(vec![None]));
            }
            let mean = xs.iter().sum::<f64>() / xs.len() as f64;
            // The sample variance (n-1 denominator), which is R's default.
            let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (xs.len() - 1) as f64;
            Ok(scalar_dbl(if name == "sd" { var.sqrt() } else { var }))
        }
        "min" | "max" | "range" => {
            let narm = a.named("na.rm").and_then(|v| lgl1(&v)).unwrap_or(false);
            let mut xs: Vec<Option<f64>> = Vec::new();
            let mut strings: Vec<Option<String>> = Vec::new();
            let mut is_text = false;
            for (tag, v) in a.all.iter() {
                if tag.as_deref() == Some("na.rm") {
                    continue;
                }
                if matches!(data(v), RData::Str(_)) {
                    is_text = true;
                    strings.extend(as_str(v));
                } else {
                    xs.extend(as_dbl(v));
                }
            }
            if is_text {
                let mut ss: Vec<String> = strings.into_iter().flatten().collect();
                ss.sort();
                return Ok(match name {
                    "min" => scalar_str(ss.first().cloned().unwrap_or_default()),
                    "max" => scalar_str(ss.last().cloned().unwrap_or_default()),
                    _ => mk_str(vec![ss.first().cloned(), ss.last().cloned()]),
                });
            }
            if !narm && xs.iter().any(|e| e.is_none()) {
                return Ok(mk_dbl(if name == "range" {
                    vec![None, None]
                } else {
                    vec![None]
                }));
            }
            let vals: Vec<f64> = xs.into_iter().flatten().collect();
            if vals.is_empty() {
                return Ok(mk_dbl(vec![None]));
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
            let xs = as_dbl(&a.req(0, "x")?);
            let mut acc = if name == "cumsum" { 0.0 } else { 1.0 };
            let out = xs
                .iter()
                .map(|e| {
                    e.map(|x| {
                        if name == "cumsum" {
                            acc += x
                        } else {
                            acc *= x
                        }
                        acc
                    })
                })
                .collect();
            Ok(mk_dbl(out))
        }
        "diff" => {
            let xs = as_dbl(&a.req(0, "x")?);
            let out = xs
                .windows(2)
                .map(|w| match (w[0], w[1]) {
                    (Some(p), Some(q)) => Some(q - p),
                    _ => None,
                })
                .collect();
            Ok(mk_dbl(out))
        }

        // ── elementwise math ────────────────────────────────────────────
        "abs" | "sqrt" | "exp" | "log2" | "log10" | "floor" | "ceiling" | "trunc" | "sign" => {
            let x = a.req(0, "x")?;
            let f: fn(f64) -> f64 = match name {
                "abs" => f64::abs,
                "sqrt" => f64::sqrt,
                "exp" => f64::exp,
                "log2" => f64::log2,
                "log10" => f64::log10,
                "floor" => f64::floor,
                "ceiling" => f64::ceil,
                "trunc" => f64::trunc,
                _ => f64::signum,
            };
            // `abs` on an integer vector stays integer, like R.
            if name == "abs" {
                if let RData::Int(v) = data(&x) {
                    return Ok(mk_int(v.iter().map(|e| e.map(|n| n.abs())).collect()));
                }
            }
            let out = mk_dbl(as_dbl(&x).iter().map(|e| e.map(f)).collect());
            carry_attrs(&out, &x, &x);
            Ok(out)
        }
        "log" => {
            let x = a.req(0, "x")?;
            let base = a.get(1, "base").and_then(|v| num1(&v));
            Ok(mk_dbl(
                as_dbl(&x)
                    .iter()
                    .map(|e| {
                        e.map(|v| match base {
                            Some(b) => v.log(b),
                            None => v.ln(),
                        })
                    })
                    .collect(),
            ))
        }
        "round" => {
            let x = a.req(0, "x")?;
            let digits = a.get(1, "digits").and_then(|v| num1(&v)).unwrap_or(0.0);
            let scale = 10f64.powf(digits);
            Ok(mk_dbl(
                as_dbl(&x)
                    .iter()
                    // R rounds half to even ("banker's rounding").
                    .map(|e| e.map(|v| round_half_even(v * scale) / scale))
                    .collect(),
            ))
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
            Ok(mk_lgl(out))
        }
        "is.numeric" => Ok(scalar_lgl(matches!(
            data(&a.req(0, "x")?),
            RData::Dbl(_) | RData::Int(_)
        ))),
        "is.character" => Ok(scalar_lgl(matches!(data(&a.req(0, "x")?), RData::Str(_)))),
        "is.logical" => Ok(scalar_lgl(matches!(data(&a.req(0, "x")?), RData::Lgl(_)))),
        "is.list" => Ok(scalar_lgl(matches!(data(&a.req(0, "x")?), RData::List(_)))),
        "is.function" => Ok(scalar_lgl(with_host(|h| {
            h.is_function(&a.req(0, "x").unwrap_or(Value::Undef))
        }))),
        "is.vector" => Ok(scalar_lgl(matches!(
            data(&a.req(0, "x")?),
            RData::Dbl(_) | RData::Int(_) | RData::Str(_) | RData::Lgl(_) | RData::List(_)
        ))),
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
        "nchar" => Ok(mk_int(
            as_str(&a.req(0, "x")?)
                .iter()
                .map(|s| s.as_ref().map(|s| s.chars().count() as i64))
                .collect(),
        )),
        "toupper" | "tolower" => {
            let f: fn(&str) -> String = if name == "toupper" {
                |s| s.to_uppercase()
            } else {
                |s| s.to_lowercase()
            };
            Ok(mk_str(
                as_str(&a.req(0, "x")?)
                    .iter()
                    .map(|s| s.as_deref().map(f))
                    .collect(),
            ))
        }
        "trimws" => Ok(mk_str(
            as_str(&a.req(0, "x")?)
                .iter()
                .map(|s| s.as_ref().map(|s| s.trim().to_string()))
                .collect(),
        )),
        "substr" | "substring" => {
            let x = as_str(&a.req(0, "x")?);
            let start = a
                .get(1, "start")
                .or_else(|| a.named("first"))
                .and_then(|v| num1(&v))
                .unwrap_or(1.0) as usize;
            let stop = a
                .get(2, "stop")
                .or_else(|| a.named("last"))
                .and_then(|v| num1(&v))
                .unwrap_or(1e6) as usize;
            Ok(mk_str(
                x.iter()
                    .map(|s| {
                        s.as_ref().map(|s| {
                            s.chars()
                                .skip(start.saturating_sub(1))
                                .take(stop.saturating_sub(start.saturating_sub(1)))
                                .collect::<String>()
                        })
                    })
                    .collect(),
            ))
        }
        "startsWith" | "endsWith" => {
            let x = as_str(&a.req(0, "x")?);
            let p = str1(&a.req(1, "prefix")?).unwrap_or_default();
            Ok(mk_lgl(
                x.iter()
                    .map(|s| {
                        s.as_ref().map(|s| {
                            if name == "startsWith" {
                                s.starts_with(&p)
                            } else {
                                s.ends_with(&p)
                            }
                        })
                    })
                    .collect(),
            ))
        }
        "strsplit" => {
            let x = as_str(&a.req(0, "x")?);
            let sep = str1(&a.req(1, "split")?).unwrap_or_default();
            let parts: Vec<Value> = x
                .iter()
                .map(|s| match s {
                    Some(s) => {
                        let pieces: Vec<Option<String>> = if sep.is_empty() {
                            s.chars().map(|c| Some(c.to_string())).collect()
                        } else {
                            s.split(sep.as_str()).map(|p| Some(p.to_string())).collect()
                        };
                        mk_str(pieces)
                    }
                    None => mk_str(vec![None]),
                })
                .collect();
            Ok(mk_list(parts))
        }
        "sub" | "gsub" | "grepl" | "grep" => regex_op(name, &a),

        // ── apply family ────────────────────────────────────────────────
        "lapply" | "sapply" => {
            let x = a.req(0, "X")?;
            let f = a.req(1, "FUN")?;
            let extra = a.rest(2);
            let items = elements(&x);
            let mut out = Vec::with_capacity(items.len());
            for it in items {
                let mut call_args = vec![(None, it)];
                call_args.extend(extra.clone());
                out.push(call_value(&f, call_args, None)?);
            }
            let res = mk_list(out);
            let nm = names_of(&x);
            if !nm.is_empty() {
                set_names(&res, nm.clone());
            } else if matches!(data(&x), RData::Str(_)) && name == "sapply" {
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
            let mut out = Vec::with_capacity(items.len());
            for it in items {
                out.push(call_value(&f, vec![(None, it)], None)?);
            }
            Ok(simplify(&mk_list(out)))
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
        "Filter" => {
            let f = a.req(0, "f")?;
            let x = a.req(1, "x")?;
            let items = elements(&x);
            let nm = names_of(&x);
            let mut keep = Vec::new();
            for (i, it) in items.iter().enumerate() {
                let r = call_value(&f, vec![(None, it.clone())], None)?;
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
        "Reduce" => {
            let f = a.req(0, "f")?;
            let x = a.req(1, "x")?;
            let mut items = elements(&x).into_iter();
            let mut acc = match a.get(2, "init") {
                Some(v) => v,
                None => match items.next() {
                    Some(v) => v,
                    None => return Ok(null()),
                },
            };
            for it in items {
                acc = call_value(&f, vec![(None, acc), (None, it)], None)?;
            }
            Ok(acc)
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
            call_value(&f, call_args, None)
        }
        "Negate" => {
            let _f = a.req(0, "f")?;
            Err("Negate() is not implemented yet".into())
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
            let pos: Vec<Option<usize>> = (0..total).map(|i| Some(i % n)).collect();
            let out = take_positions(&x, &pos);
            let dim = mk_int(vec![Some(nr as i64), Some(nc as i64)]);
            with_host(|h| h.set_attr(&out, "dim", dim));
            Ok(out)
        }
        "t" => {
            let x = a.req(0, "x")?;
            let d = with_host(|h| h.attr(&x, "dim"))
                .map(|d| as_int(&d))
                .unwrap_or_default();
            let (nr, nc) = match d.as_slice() {
                [Some(r), Some(c)] => (*r as usize, *c as usize),
                _ => (1, len(&x)),
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
            Ok(out)
        }

        // ── environments and dispatch ───────────────────────────────────
        "exists" => {
            let n = str1(&a.req(0, "x")?).unwrap_or_default();
            Ok(scalar_lgl(with_host(|h| h.exists(&n)) || is_primitive(&n)))
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
            with_host(|h| {
                h.set_var(&n, v.clone());
                h.visible = false;
            });
            Ok(v)
        }
        "environment" | "new.env" => {
            let e = if name == "new.env" {
                Rc::new(std::cell::RefCell::new(crate::host::EnvData {
                    vars: IndexMap::new(),
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
        "Recall" => Err("Recall() is not implemented yet".into()),
        "nlevels" => Ok(scalar_int(0)),

        other => Err(format!("could not find function \"{other}\"")),
    }
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

fn numeric_arg(a: &Args, i: usize, name: &str) -> Result<Vec<f64>, String> {
    let v = a.req(i, name)?;
    let narm = a.named("na.rm").and_then(|x| lgl1(&x)).unwrap_or(false);
    let xs = as_dbl(&v);
    if !narm && xs.iter().any(|e| e.is_none()) {
        return Ok(vec![f64::NAN]);
    }
    Ok(xs.into_iter().flatten().collect())
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
        .map(|(_, v)| with_host(|h| crate::host::type_rank(&h.data_of(v))))
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
    if items.is_empty()
        || items
            .iter()
            .any(|v| len(v) != 1 || matches!(data(v), RData::List(_)))
    {
        return list.clone();
    }
    let parts: Vec<(Option<String>, Value)> = items.into_iter().map(|v| (None, v)).collect();
    let out = concat(&Args::new(parts));
    let nm = names_of(list);
    if !nm.is_empty() {
        set_names(&out, nm);
    }
    out
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
        .map(|(_, v)| as_str(v))
        .filter(|v| !v.is_empty())
        .collect();
    if parts.is_empty() {
        return mk_str(vec![]);
    }
    let n = parts.iter().map(|p| p.len()).max().unwrap_or(0);
    let joined: Vec<String> = (0..n)
        .map(|i| {
            parts
                .iter()
                .map(|p| p[i % p.len()].clone().unwrap_or_else(|| "NA".into()))
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
fn seq(a: &Args) -> Value {
    let from = a.get(0, "from").and_then(|v| num1(&v)).unwrap_or(1.0);
    let to = a.get(1, "to").and_then(|v| num1(&v));
    let by = a.named("by").and_then(|v| num1(&v));
    let length_out = a.named("length.out").and_then(|v| num1(&v));
    // `seq(n)` with one argument means `seq_len(n)`.
    let Some(to) = to else {
        return match length_out {
            Some(n) => mk_int((1..=n as i64).map(Some).collect()),
            None => mk_int((1..=from as i64).map(Some).collect()),
        };
    };
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
    if step == 0.0 {
        out.push(Some(from));
    } else {
        let count = ((to - from) / step).floor() as i64;
        for k in 0..=count.max(0) {
            out.push(Some(from + step * k as f64));
        }
    }
    let whole = out
        .iter()
        .flatten()
        .all(|x| *x == x.trunc() && x.abs() < 1e15);
    if whole && by.map(|b| b == b.trunc()).unwrap_or(true) {
        mk_int(out.into_iter().map(|e| e.map(|x| x as i64)).collect())
    } else {
        mk_dbl(out)
    }
}

/// `rep(x, times=, each=)`.
fn rep(a: &Args) -> Value {
    let x = match a.get(0, "x") {
        Some(v) => v,
        None => return null(),
    };
    let times = a.get(1, "times").and_then(|v| num1(&v)).unwrap_or(1.0) as usize;
    let each = a.named("each").and_then(|v| num1(&v)).unwrap_or(1.0) as usize;
    let n = len(&x);
    let mut pos = Vec::with_capacity(n * times * each);
    for _ in 0..times {
        for i in 0..n {
            for _ in 0..each {
                pos.push(Some(i));
            }
        }
    }
    take_positions(&x, &pos)
}

fn sort_value(x: &Value, decreasing: bool) -> Value {
    let idx = order_positions(x, decreasing);
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
    out
}

fn order_value(x: &Value, decreasing: bool) -> Value {
    mk_int(
        order_positions(x, decreasing)
            .into_iter()
            .map(|i| Some(i as i64 + 1))
            .collect(),
    )
}

/// The ordering permutation, with NA values dropped (R's `sort` default).
fn order_positions(x: &Value, decreasing: bool) -> Vec<usize> {
    let text = matches!(data(x), RData::Str(_));
    let mut idx: Vec<usize> = (0..len(x)).collect();
    if text {
        let keys = as_str(x);
        idx.retain(|i| keys[*i].is_some());
        idx.sort_by(|p, q| keys[*p].cmp(&keys[*q]));
    } else {
        let keys = as_dbl(x);
        idx.retain(|i| keys[*i].is_some_and(|v| !v.is_nan()));
        idx.sort_by(|p, q| keys[*p].partial_cmp(&keys[*q]).unwrap());
    }
    if decreasing {
        idx.reverse();
    }
    idx
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
fn round_half_even(x: f64) -> f64 {
    let r = x.round();
    if (x - x.trunc()).abs() == 0.5 && r % 2.0 != 0.0 {
        r - x.signum()
    } else {
        r
    }
}

/// `sub`, `gsub`, `grepl`, `grep` over R's default (POSIX-flavored) regex.
fn regex_op(name: &str, a: &Args) -> Result<Value, String> {
    let pattern = str1(&a.req(0, "pattern")?).unwrap_or_default();
    let fixed = a.named("fixed").and_then(|v| lgl1(&v)).unwrap_or(false);
    let (subject_idx, subject_name) = if name == "sub" || name == "gsub" {
        (2, "x")
    } else {
        (1, "x")
    };
    let x = as_str(&a.req(subject_idx, subject_name)?);
    let re = if fixed {
        regex::Regex::new(&regex::escape(&pattern))
    } else {
        regex::Regex::new(&pattern)
    }
    .map_err(|e| format!("invalid regular expression '{pattern}': {e}"))?;

    match name {
        "grepl" => Ok(mk_lgl(
            x.iter()
                .map(|s| s.as_ref().map(|s| re.is_match(s)))
                .collect(),
        )),
        "grep" => Ok(mk_int(
            x.iter()
                .enumerate()
                .filter(|(_, s)| s.as_ref().is_some_and(|s| re.is_match(s)))
                .map(|(i, _)| Some(i as i64 + 1))
                .collect(),
        )),
        _ => {
            let replacement = str1(&a.req(1, "replacement")?).unwrap_or_default();
            // R writes back-references as \1; the regex crate wants $1.
            let rep = if fixed {
                replacement.replace('$', "$$")
            } else {
                let mut out = String::new();
                let mut chars = replacement.chars().peekable();
                while let Some(c) = chars.next() {
                    if c == '\\' && chars.peek().is_some_and(|d| d.is_ascii_digit()) {
                        out.push('$');
                    } else if c == '$' {
                        out.push_str("$$");
                    } else {
                        out.push(c);
                    }
                }
                out
            };
            Ok(mk_str(
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
            ))
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
            let (flags, width, precision) = parse_spec(&spec);
            let arg = rest.get(argi).map(|(_, v)| v.clone());
            argi += 1;
            let Some(arg) = arg else {
                return Err("too few arguments for sprintf format".into());
            };
            let k = i % len(&arg).max(1);
            let text = match conv {
                'd' | 'i' => as_int(&arg)
                    .get(k)
                    .and_then(|e| *e)
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "NA".into()),
                'f' | 'e' | 'g' => {
                    let v = as_dbl(&arg).get(k).and_then(|e| *e);
                    match v {
                        Some(v) => {
                            let p = precision.unwrap_or(6);
                            match conv {
                                'f' => format!("{v:.p$}"),
                                'e' => format!("{v:.p$e}"),
                                _ => format_dbl(v),
                            }
                        }
                        None => "NA".into(),
                    }
                }
                'x' => format!("{:x}", as_int(&arg).get(k).and_then(|e| *e).unwrap_or(0)),
                'X' => format!("{:X}", as_int(&arg).get(k).and_then(|e| *e).unwrap_or(0)),
                _ => {
                    let v = as_str(&arg)
                        .get(k)
                        .cloned()
                        .flatten()
                        .unwrap_or_else(|| "NA".into());
                    match precision {
                        Some(p) => v.chars().take(p).collect(),
                        None => v,
                    }
                }
            };
            s.push_str(&pad(&text, width, &flags));
        }
        out.push(Some(s));
    }
    Ok(mk_str(out))
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
    if text.chars().count() >= w {
        return text.to_string();
    }
    let fill = w - text.chars().count();
    if flags.contains('-') {
        format!("{text}{}", " ".repeat(fill))
    } else if flags.contains('0') {
        format!("{}{text}", "0".repeat(fill))
    } else {
        format!("{}{text}", " ".repeat(fill))
    }
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
    for cls in classes {
        let method = format!("{generic}.{cls}");
        if let Some(f) = with_host(|h| h.lookup_function(&method)) {
            let out = call_value(&f, frame_args, Some(method))?;
            // The generic returns whatever the method returned.
            with_host(|h| h.signal = Some(Signal::Return(out.clone())));
            return Ok(out);
        }
    }
    Err(format!(
        "no applicable method for '{generic}' applied to an object of class \"{}\"",
        class_of(&obj).first().cloned().unwrap_or_default()
    ))
}

// ===========================================================================
// Printing — R's own layout.
// ===========================================================================

/// Print a value the way R's default `print` does.
pub fn print_value(v: &Value) {
    for line in format_value(v) {
        println!("{line}");
    }
}

/// Render a value into the lines `print` would emit.
pub fn format_value(v: &Value) -> Vec<String> {
    match data(v) {
        RData::Null => vec!["NULL".into()],
        RData::Closure { .. } | RData::Builtin(_) => vec![format_function(v)],
        RData::Environment(_) => vec!["<environment>".into()],
        RData::Args(_) => format_list(v),
        RData::List(_) => format_list(v),
        _ => {
            if let Some(dim) = with_host(|h| h.attr(v, "dim")) {
                let d = as_int(&dim);
                if d.len() == 2 {
                    return format_matrix(
                        v,
                        d[0].unwrap_or(0) as usize,
                        d[1].unwrap_or(0) as usize,
                    );
                }
            }
            format_vector(v)
        }
    }
}

fn format_function(v: &Value) -> String {
    match data(v) {
        RData::Builtin(name) => format!("function (...) .Primitive(\"{name}\")"),
        RData::Closure { id, .. } => {
            let params =
                with_host(|h| h.closures.get(id).map(|c| c.params.join(", "))).unwrap_or_default();
            format!("function ({params}) ...")
        }
        _ => "function".into(),
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
        let use_sci = width(&|x| render_sci(x, sci_d)) < width(&|x| render_fixed(x, fixed_d));
        return xs
            .iter()
            .map(|e| match e {
                Some(x) if x.is_finite() && use_sci => render_sci(*x, sci_d),
                Some(x) if x.is_finite() => render_fixed(*x, fixed_d),
                Some(x) => render_fixed(*x, 0),
                None => "NA".into(),
            })
            .collect();
    }
    (0..n).map(|i| print_element(v, i)).collect()
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
        return vec![format!("{kind}(0)")];
    }
    let cells = format_elements(v);
    let names = names_of(v);
    const WIDTH: usize = 80;

    // Character vectors are left-justified; everything else is right-justified.
    // A *named* vector right-justifies both rows regardless of type.
    let left_align = matches!(data(v), RData::Str(_)) && names.is_empty();
    let justify = |cell: &str, w: usize| {
        if left_align {
            format!("{cell:<w$}")
        } else {
            format!("{cell:>w$}")
        }
    };

    if !names.is_empty() {
        // Named vectors print as name/value rows sharing one column width.
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
            .map(|s| s.chars().count())
            .max()
            .unwrap_or(1);
        let per_line = (WIDTH / (w + 1)).max(1);
        let mut out = Vec::new();
        let mut i = 0;
        while i < n {
            let take = per_line.min(n - i);
            let row = |src: &[String]| {
                (i..i + take)
                    .map(|k| format!("{:>w$}", src[k], w = w))
                    .collect::<Vec<_>>()
                    .join(" ")
                    + " "
            };
            out.push(row(&labels));
            out.push(row(&cells));
            i += take;
        }
        return out;
    }

    let cell_w = cells.iter().map(|c| c.chars().count()).max().unwrap_or(1);
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
    let cells = format_elements(v);
    let row_labels: Vec<String> = (1..=nr).map(|r| format!("[{r},]")).collect();
    let col_labels: Vec<String> = (1..=nc).map(|c| format!("[,{c}]")).collect();
    let label_w = row_labels.iter().map(|s| s.len()).max().unwrap_or(0);
    let widths: Vec<usize> = (0..nc)
        .map(|c| {
            (0..nr)
                .map(|r| {
                    cells
                        .get(c * nr + r)
                        .map(|s| s.chars().count())
                        .unwrap_or(2)
                })
                .chain(std::iter::once(col_labels[c].len()))
                .max()
                .unwrap_or(1)
        })
        .collect();
    let mut out = Vec::with_capacity(nr + 1);
    let header = (0..nc)
        .map(|c| format!("{:>w$}", col_labels[c], w = widths[c]))
        .collect::<Vec<_>>()
        .join(" ");
    out.push(format!("{:w$} {header}", "", w = label_w));
    for (r, label) in row_labels.iter().enumerate() {
        let row = (0..nc)
            .map(|c| {
                format!(
                    "{:>w$}",
                    cells.get(c * nr + r).cloned().unwrap_or_default(),
                    w = widths[c]
                )
            })
            .collect::<Vec<_>>()
            .join(" ");
        out.push(format!("{label:<label_w$} {row}"));
    }
    out
}

fn format_list(v: &Value) -> Vec<String> {
    let items = elements(v);
    if items.is_empty() {
        return vec!["list()".into()];
    }
    let names = names_of(v);
    let mut out = Vec::new();
    for (i, it) in items.iter().enumerate() {
        let header = match names.get(i).cloned().flatten() {
            Some(n) => format!("${n}"),
            None => format!("[[{}]]", i + 1),
        };
        out.push(header);
        out.extend(format_value(it));
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
