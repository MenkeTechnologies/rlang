//! The R value heap and runtime, reached from fusevm through registered
//! builtins (`register_builtin`).
//!
//! rlang owns no VM and no JIT: the compiler lowers R to `fusevm::Chunk`, and
//! every R-specific operation lands in a builtin that calls in here.
//!
//! Value representation: **every** R value is a heap object behind a
//! `Value::Obj(u32)` handle, because R has no scalars — `1` is a
//! double vector of length one, and any value can carry attributes (`names`,
//! `dim`, `class`). Native `Value::Int` appears only in compiler-internal loop
//! counters, never as an R value.
//!
//! Environments are `Rc<RefCell<..>>` chained child-to-parent, so a closure
//! captures its defining environment by reference — R's lexical scoping, and
//! what makes `<<-` reach the enclosing frame.

use fusevm::{Chunk, VMResult, Value, VM};
use indexmap::IndexMap;
use std::cell::RefCell;
use std::rc::Rc;

/// Builtin ids emitted by the compiler and registered on every VM.
pub mod ops {
    /// `[name]` → value bound to a name (any binding).
    pub const GETVAR: u16 = 1;
    /// `[name]` → the nearest *function* binding (R searches past non-functions).
    pub const GETFUN: u16 = 2;
    /// `[name, value]` → value (assignment is invisible).
    pub const SETVAR: u16 = 3;
    /// `[name, value]` → value, assigning in an enclosing frame (`<<-`).
    pub const SETSUPER: u16 = 4;
    /// `[(name|NULL), value, ...]` argc=2n → argument list.
    pub const MKARGS: u16 = 5;
    /// `[fn, args]` → call result.
    pub const CALL: u16 = 6;
    /// `[closure_id]` → closure capturing the current environment.
    pub const MKCLOSURE: u16 = 7;
    /// `[f64]` → double vector of length 1.
    pub const CONST_DBL: u16 = 8;
    /// `[i64]` → integer vector of length 1.
    pub const CONST_INT: u16 = 9;
    /// `[Str]` → character vector of length 1.
    pub const CONST_STR: u16 = 10;
    /// `[bool]` → logical vector of length 1.
    pub const CONST_LGL: u16 = 11;
    /// `[]` → NULL.
    pub const CONST_NULL: u16 = 12;
    /// `[i64 kind]` → typed NA.
    pub const CONST_NA: u16 = 13;
    /// `[]` → the `...` list of the current frame.
    pub const DOTS: u16 = 14;

    /// `[op_name, lhs, rhs]` → vectorized binary operation.
    pub const BINOP: u16 = 20;
    /// `[op_name, operand]` → vectorized unary operation.
    pub const UNOP: u16 = 21;
    /// `[name, lhs, rhs]` → `%op%` dispatch (`%%`, `%in%`, user-defined).
    pub const SPECIAL: u16 = 22;

    /// `[x, args]` → `x[...]`.
    pub const INDEX: u16 = 30;
    /// `[x, args]` → `x[[...]]`.
    pub const INDEX2: u16 = 31;
    /// `[x, name]` → `x$name`.
    pub const DOLLAR: u16 = 32;
    /// `[x, args, value]` → the modified `x` (`x[...] <- value`).
    pub const INDEX_SET: u16 = 33;
    /// `[x, args, value]` → the modified `x` (`x[[...]] <- value`).
    pub const INDEX2_SET: u16 = 34;
    /// `[x, name, value]` → the modified `x` (`x$name <- value`).
    pub const DOLLAR_SET: u16 = 35;
    /// `[fname, x, args, value]` → the modified `x` via the replacement
    /// function `fname<-` (`names(x) <- v`, `dim(x) <- v`, …).
    pub const REPLACE: u16 = 36;

    /// `[v]` → native `Bool` for a native `JumpIfFalse`.
    pub const TRUTHY: u16 = 40;
    /// `[]` → halt the loop body chunk with a `break` signal.
    pub const SIG_BREAK: u16 = 41;
    /// `[]` → halt the loop body chunk with a `next` signal.
    pub const SIG_NEXT: u16 = 42;
    /// `[v]` → halt the closure body with a `return` signal.
    pub const SIG_RETURN: u16 = 43;
    /// `[v]` → native `Int` length, for the native `for` loop counter.
    pub const SEQ_LEN: u16 = 44;
    /// `[v, Int]` → the i-th element as an R value (`for` iteration).
    pub const SEQ_ELEM: u16 = 45;
    /// `[v]` → v, printing it first if the value is visible (top-level echo).
    pub const AUTOPRINT: u16 = 46;
    /// `[v]` → native `Bool`: is this a definite FALSE? (`&&` short-circuit).
    pub const IS_FALSE: u16 = 47;
    /// `[v]` → native `Bool`: is this a definite TRUE? (`||` short-circuit).
    pub const IS_TRUE: u16 = 48;
    /// `[name]` → native `Bool`: was this formal left unsupplied? (default
    /// prologue, and the `missing()` primitive).
    pub const MISSING: u16 = 49;
    /// `[]` → NULL, marked invisible. The value of a loop and of an `if` with no
    /// `else`, neither of which R echoes at top level.
    pub const NULL_INVISIBLE: u16 = 50;
}

/// A variable environment: a frame's bindings plus a link to its enclosure.
pub struct EnvData {
    pub vars: IndexMap<String, Value>,
    pub parent: Option<Env>,
}
/// A reference-counted environment, shared between a frame and every closure
/// that captured it.
pub type Env = Rc<RefCell<EnvData>>;

fn new_env(parent: Option<Env>) -> Env {
    Rc::new(RefCell::new(EnvData {
        vars: IndexMap::new(),
        parent,
    }))
}

/// A compiled `function(...)`: its formals and its body chunk. Defaults are not
/// stored here — the compiler emits them as a body prologue
/// (`if (missing(p)) p <- <default>`), which is what makes an R default able to
/// refer to another argument.
#[derive(Clone)]
pub struct ClosureDef {
    pub params: Vec<String>,
    pub chunk: Chunk,
}

/// The non-local control transfers a builtin can raise.
#[derive(Debug, Clone)]
pub enum Signal {
    Break,
    Next,
    Return(Value),
}

/// One R value: its data and its attributes (`names`, `dim`, `class`, …).
#[derive(Clone)]
pub struct RObj {
    pub data: RData,
    pub attrs: IndexMap<String, Value>,
}

/// The R data types rlang represents. Atomic vectors hold `Option<T>`, where
/// `None` is `NA` — R's missing value is part of every atomic type, not a
/// separate one.
#[derive(Clone)]
pub enum RData {
    Null,
    Lgl(Vec<Option<bool>>),
    Int(Vec<Option<i64>>),
    Dbl(Vec<Option<f64>>),
    Str(Vec<Option<String>>),
    /// A generic vector (`list(...)`); elements are handles to other objects.
    List(Vec<Value>),
    Closure {
        id: usize,
        env: Env,
    },
    /// A primitive implemented in Rust, named for dispatch and printing.
    Builtin(String),
    Environment(Env),
    /// A call-site argument list: `(tag, value)` pairs, produced by `MKARGS`.
    /// `Value::Undef` is an empty argument (`x[, 1]`).
    Args(Vec<(Option<String>, Value)>),
}

/// The scalar type ladder used for promotion in `c()` and arithmetic:
/// logical < integer < double < character.
pub fn type_rank(d: &RData) -> u8 {
    match d {
        RData::Null => 0,
        RData::Lgl(_) => 1,
        RData::Int(_) => 2,
        RData::Dbl(_) => 3,
        RData::Str(_) => 4,
        _ => 5,
    }
}

/// One entry on the R call stack.
pub struct Frame {
    pub env: Env,
    /// The call's arguments, kept for `UseMethod` and `sys.call`-style needs.
    pub args: Vec<(Option<String>, Value)>,
    /// The name the function was called by, when known.
    pub fun_name: Option<String>,
    /// Set by `UseMethod` so `NextMethod` can continue down the class vector.
    pub dispatch: Option<(String, Vec<String>)>,
}

/// The R runtime: heap, environments, call stack, and pending control state.
pub struct RHost {
    heap: Vec<RObj>,
    pub global: Env,
    pub frames: Vec<Frame>,
    pub closures: Vec<ClosureDef>,
    pub error: Option<String>,
    pub signal: Option<Signal>,
    /// R's "visibility" flag: assignment and `invisible()` clear it, so the
    /// top-level echo knows not to print.
    pub visible: bool,
    /// Whether the top-level echo prints at all. On for `Rscript` and the REPL,
    /// off for embedding and for tests that want the value, not the transcript.
    pub echo: bool,
    /// Interned singletons so `NULL` and the common constants do not allocate a
    /// fresh heap cell on every evaluation.
    null: Option<Value>,
}

thread_local! {
    static HOST: RefCell<RHost> = RefCell::new(RHost::new());
}

/// Run `f` with mutable access to the thread-local host.
pub fn with_host<R>(f: impl FnOnce(&mut RHost) -> R) -> R {
    HOST.with(|h| f(&mut h.borrow_mut()))
}

/// Reset the host to a clean slate (fresh global environment and heap).
pub fn reset_host() {
    with_host(|h| *h = RHost::new());
}

impl Default for RHost {
    fn default() -> Self {
        Self::new()
    }
}

impl RHost {
    /// A fresh runtime with an empty heap and an empty global environment.
    pub fn new() -> Self {
        let global = new_env(None);
        RHost {
            heap: Vec::new(),
            global: global.clone(),
            frames: vec![Frame {
                env: global,
                args: Vec::new(),
                fun_name: None,
                dispatch: None,
            }],
            closures: Vec::new(),
            error: None,
            signal: None,
            visible: true,
            echo: true,
            null: None,
        }
    }

    /// Install the compiled closure bodies for a program.
    pub fn load_closures(&mut self, closures: Vec<ClosureDef>) {
        self.closures = closures;
    }

    // ── heap ───────────────────────────────────────────────────────────

    /// Allocate `data` with no attributes and return its handle.
    pub fn alloc(&mut self, data: RData) -> Value {
        self.alloc_with(data, IndexMap::new())
    }

    /// Allocate `data` carrying `attrs`.
    pub fn alloc_with(&mut self, data: RData, attrs: IndexMap<String, Value>) -> Value {
        self.heap.push(RObj { data, attrs });
        Value::Obj((self.heap.len() - 1) as u32)
    }

    /// The object behind a handle, if the value is one.
    pub fn get(&self, v: &Value) -> Option<&RObj> {
        match v {
            Value::Obj(i) => self.heap.get(*i as usize),
            _ => None,
        }
    }

    fn get_mut(&mut self, v: &Value) -> Option<&mut RObj> {
        match v {
            Value::Obj(i) => self.heap.get_mut(*i as usize),
            _ => None,
        }
    }

    /// The data behind a handle, cloned. R has copy-on-assign value semantics,
    /// so every mutation path here rebuilds the object rather than aliasing it.
    pub fn data_of(&self, v: &Value) -> RData {
        self.get(v).map(|o| o.data.clone()).unwrap_or(RData::Null)
    }

    /// The attributes of a value.
    pub fn attrs_of(&self, v: &Value) -> IndexMap<String, Value> {
        self.get(v).map(|o| o.attrs.clone()).unwrap_or_default()
    }

    /// Read one attribute.
    pub fn attr(&self, v: &Value, name: &str) -> Option<Value> {
        self.get(v).and_then(|o| o.attrs.get(name).cloned())
    }

    /// Set (or, with `NULL`, remove) one attribute in place.
    pub fn set_attr(&mut self, v: &Value, name: &str, val: Value) {
        let is_null = matches!(self.data_of(&val), RData::Null);
        if let Some(o) = self.get_mut(v) {
            if is_null {
                o.attrs.shift_remove(name);
            } else {
                o.attrs.insert(name.to_string(), val);
            }
        }
    }

    /// The shared `NULL` singleton.
    pub fn null(&mut self) -> Value {
        if let Some(v) = &self.null {
            return v.clone();
        }
        let v = self.alloc(RData::Null);
        self.null = Some(v.clone());
        v
    }

    // ── constructors ───────────────────────────────────────────────────

    /// A double vector.
    pub fn dbl(&mut self, xs: Vec<Option<f64>>) -> Value {
        self.alloc(RData::Dbl(xs))
    }
    /// An integer vector.
    pub fn int(&mut self, xs: Vec<Option<i64>>) -> Value {
        self.alloc(RData::Int(xs))
    }
    /// A logical vector.
    pub fn lgl(&mut self, xs: Vec<Option<bool>>) -> Value {
        self.alloc(RData::Lgl(xs))
    }
    /// A character vector.
    pub fn str_vec(&mut self, xs: Vec<Option<String>>) -> Value {
        self.alloc(RData::Str(xs))
    }
    /// A generic vector (list).
    pub fn list(&mut self, xs: Vec<Value>) -> Value {
        self.alloc(RData::List(xs))
    }
    /// A length-1 double.
    pub fn scalar_dbl(&mut self, x: f64) -> Value {
        self.dbl(vec![Some(x)])
    }
    /// A length-1 integer.
    pub fn scalar_int(&mut self, x: i64) -> Value {
        self.int(vec![Some(x)])
    }
    /// A length-1 logical.
    pub fn scalar_lgl(&mut self, x: bool) -> Value {
        self.lgl(vec![Some(x)])
    }
    /// A length-1 character vector.
    pub fn scalar_str(&mut self, x: impl Into<String>) -> Value {
        self.str_vec(vec![Some(x.into())])
    }

    // ── lengths, coercion, accessors ───────────────────────────────────

    /// `length(x)`.
    pub fn length(&self, v: &Value) -> usize {
        match self.get(v).map(|o| &o.data) {
            Some(RData::Null) | None => 0,
            Some(RData::Lgl(x)) => x.len(),
            Some(RData::Int(x)) => x.len(),
            Some(RData::Dbl(x)) => x.len(),
            Some(RData::Str(x)) => x.len(),
            Some(RData::List(x)) => x.len(),
            Some(RData::Args(x)) => x.len(),
            Some(RData::Environment(e)) => e.borrow().vars.len(),
            _ => 1,
        }
    }

    /// Coerce to logicals (`as.logical`); non-convertible strings become NA.
    pub fn as_lgl(&self, v: &Value) -> Vec<Option<bool>> {
        match self.get(v).map(|o| &o.data) {
            Some(RData::Lgl(x)) => x.clone(),
            Some(RData::Int(x)) => x.iter().map(|e| e.map(|n| n != 0)).collect(),
            Some(RData::Dbl(x)) => x.iter().map(|e| e.map(|n| n != 0.0)).collect(),
            Some(RData::Str(x)) => x
                .iter()
                .map(|e| match e.as_deref() {
                    Some("TRUE") | Some("true") | Some("T") => Some(true),
                    Some("FALSE") | Some("false") | Some("F") => Some(false),
                    _ => None,
                })
                .collect(),
            Some(RData::List(x)) => x.iter().flat_map(|e| self.as_lgl(e)).collect(),
            _ => Vec::new(),
        }
    }

    /// Coerce to integers (`as.integer`); doubles truncate toward zero.
    pub fn as_int(&self, v: &Value) -> Vec<Option<i64>> {
        match self.get(v).map(|o| &o.data) {
            Some(RData::Lgl(x)) => x.iter().map(|e| e.map(|b| b as i64)).collect(),
            Some(RData::Int(x)) => x.clone(),
            Some(RData::Dbl(x)) => x
                .iter()
                .map(|e| e.and_then(|n| n.is_finite().then_some(n.trunc() as i64)))
                .collect(),
            Some(RData::Str(x)) => x
                .iter()
                .map(|e| e.as_ref().and_then(|s| s.trim().parse::<f64>().ok()))
                .map(|o| o.map(|n| n.trunc() as i64))
                .collect(),
            Some(RData::List(x)) => x.iter().flat_map(|e| self.as_int(e)).collect(),
            _ => Vec::new(),
        }
    }

    /// Coerce to doubles (`as.numeric`).
    pub fn as_dbl(&self, v: &Value) -> Vec<Option<f64>> {
        match self.get(v).map(|o| &o.data) {
            Some(RData::Lgl(x)) => x.iter().map(|e| e.map(|b| b as i64 as f64)).collect(),
            Some(RData::Int(x)) => x.iter().map(|e| e.map(|n| n as f64)).collect(),
            Some(RData::Dbl(x)) => x.clone(),
            Some(RData::Str(x)) => x
                .iter()
                .map(|e| e.as_ref().and_then(|s| parse_num(s.trim())))
                .collect(),
            Some(RData::List(x)) => x.iter().flat_map(|e| self.as_dbl(e)).collect(),
            _ => Vec::new(),
        }
    }

    /// Coerce to strings (`as.character`).
    pub fn as_str(&self, v: &Value) -> Vec<Option<String>> {
        match self.get(v).map(|o| &o.data) {
            Some(RData::Lgl(x)) => x
                .iter()
                .map(|e| e.map(|b| if b { "TRUE" } else { "FALSE" }.to_string()))
                .collect(),
            Some(RData::Int(x)) => x.iter().map(|e| e.map(|n| n.to_string())).collect(),
            Some(RData::Dbl(x)) => x.iter().map(|e| e.map(format_dbl)).collect(),
            Some(RData::Str(x)) => x.clone(),
            Some(RData::List(x)) => x.iter().flat_map(|e| self.as_str(e)).collect(),
            _ => Vec::new(),
        }
    }

    /// The single string of a length-1 character vector (argument helper).
    pub fn str1(&self, v: &Value) -> Option<String> {
        self.as_str(v).into_iter().next().flatten()
    }
    /// The single number of a length-1 numeric vector (argument helper).
    pub fn num1(&self, v: &Value) -> Option<f64> {
        self.as_dbl(v).into_iter().next().flatten()
    }
    /// The single logical of a length-1 vector (argument helper).
    pub fn lgl1(&self, v: &Value) -> Option<bool> {
        self.as_lgl(v).into_iter().next().flatten()
    }

    /// The list elements of a value: a list yields its elements; an atomic
    /// vector yields one length-1 value per element (what `lapply` iterates).
    pub fn elements(&mut self, v: &Value) -> Vec<Value> {
        match self.data_of(v) {
            RData::List(xs) => xs,
            RData::Null => Vec::new(),
            _ => (0..self.length(v)).map(|i| self.element_at(v, i)).collect(),
        }
    }

    /// The i-th element of a vector or list as a standalone R value.
    pub fn element_at(&mut self, v: &Value, i: usize) -> Value {
        match self.data_of(v) {
            RData::Lgl(x) => {
                let e = x.get(i).cloned().flatten();
                self.lgl(vec![e])
            }
            RData::Int(x) => {
                let e = x.get(i).cloned().flatten();
                self.int(vec![e])
            }
            RData::Dbl(x) => {
                let e = x.get(i).cloned().flatten();
                self.dbl(vec![e])
            }
            RData::Str(x) => {
                let e = x.get(i).cloned().flatten();
                self.str_vec(vec![e])
            }
            RData::List(x) => x.get(i).cloned().unwrap_or_else(|| {
                let n = self.null.clone();
                n.unwrap_or(Value::Undef)
            }),
            _ => self.null(),
        }
    }

    /// `names(x)`, as a plain vector of `Option<String>` (`None` where unnamed).
    pub fn names(&self, v: &Value) -> Vec<Option<String>> {
        match self.attr(v, "names") {
            Some(n) => self.as_str(&n),
            None => Vec::new(),
        }
    }

    /// Whether the value is `NULL`.
    pub fn is_null(&self, v: &Value) -> bool {
        matches!(self.get(v).map(|o| &o.data), Some(RData::Null) | None)
    }

    /// Whether the value is callable.
    pub fn is_function(&self, v: &Value) -> bool {
        matches!(
            self.get(v).map(|o| &o.data),
            Some(RData::Closure { .. }) | Some(RData::Builtin(_))
        )
    }

    /// The implicit or explicit class vector, as `class(x)` reports it.
    pub fn class_of(&self, v: &Value) -> Vec<String> {
        if let Some(c) = self.attr(v, "class") {
            let cs: Vec<String> = self.as_str(&c).into_iter().flatten().collect();
            if !cs.is_empty() {
                return cs;
            }
        }
        if self.attr(v, "dim").map(|d| self.length(&d)) == Some(2) {
            return vec!["matrix".into(), "array".into()];
        }
        vec![match self.get(v).map(|o| &o.data) {
            Some(RData::Null) | None => "NULL",
            Some(RData::Lgl(_)) => "logical",
            Some(RData::Int(_)) => "integer",
            Some(RData::Dbl(_)) => "numeric",
            Some(RData::Str(_)) => "character",
            Some(RData::List(_)) => "list",
            Some(RData::Closure { .. }) | Some(RData::Builtin(_)) => "function",
            Some(RData::Environment(_)) => "environment",
            Some(RData::Args(_)) => "list",
        }
        .to_string()]
    }

    /// `typeof(x)`.
    pub fn type_of(&self, v: &Value) -> &'static str {
        match self.get(v).map(|o| &o.data) {
            Some(RData::Null) | None => "NULL",
            Some(RData::Lgl(_)) => "logical",
            Some(RData::Int(_)) => "integer",
            Some(RData::Dbl(_)) => "double",
            Some(RData::Str(_)) => "character",
            Some(RData::List(_)) | Some(RData::Args(_)) => "list",
            Some(RData::Closure { .. }) => "closure",
            Some(RData::Builtin(_)) => "builtin",
            Some(RData::Environment(_)) => "environment",
        }
    }

    // ── environments ───────────────────────────────────────────────────

    /// The environment of the innermost frame.
    pub fn env(&self) -> Env {
        self.frames
            .last()
            .map(|f| f.env.clone())
            .unwrap_or_else(|| self.global.clone())
    }

    /// Look a name up through the environment chain.
    pub fn lookup(&self, name: &str) -> Option<Value> {
        let mut e = Some(self.env());
        while let Some(cur) = e {
            if let Some(v) = cur.borrow().vars.get(name) {
                return Some(v.clone());
            }
            e = cur.borrow().parent.clone();
        }
        None
    }

    /// Look up a name, skipping non-function bindings — R's function-position
    /// rule, which lets `c <- 1; c(1, 2)` still call the concatenate function.
    pub fn lookup_function(&self, name: &str) -> Option<Value> {
        let mut e = Some(self.env());
        while let Some(cur) = e {
            let hit = cur.borrow().vars.get(name).cloned();
            if let Some(v) = hit {
                if self.is_function(&v) {
                    return Some(v);
                }
            }
            e = cur.borrow().parent.clone();
        }
        None
    }

    /// Bind `name` in the current environment.
    pub fn set_var(&mut self, name: &str, val: Value) {
        self.env().borrow_mut().vars.insert(name.to_string(), val);
    }

    /// Bind `name` in a binding environment (`<<-`): the first enclosing frame
    /// that already has it, else the global environment.
    pub fn set_super(&mut self, name: &str, val: Value) {
        let mut e = self.env().borrow().parent.clone();
        while let Some(cur) = e {
            let has = cur.borrow().vars.contains_key(name);
            if has {
                cur.borrow_mut().vars.insert(name.to_string(), val);
                return;
            }
            e = cur.borrow().parent.clone();
        }
        self.global.borrow_mut().vars.insert(name.to_string(), val);
    }

    /// Whether a name is bound anywhere in the chain.
    pub fn exists(&self, name: &str) -> bool {
        self.lookup(name).is_some()
    }

    /// The `...` bindings of the current frame, if any.
    pub fn dots(&self) -> Vec<(Option<String>, Value)> {
        match self.lookup("...").map(|v| self.data_of(&v)) {
            Some(RData::Args(a)) => a,
            _ => Vec::new(),
        }
    }

    /// Record an R error; the VM run unwinds and the message surfaces as the
    /// program's error.
    pub fn fail<T>(&mut self, msg: impl Into<String>) -> Option<T> {
        if self.error.is_none() {
            self.error = Some(msg.into());
        }
        None
    }

    /// Take and clear a pending error.
    pub fn take_error(&mut self) -> Option<String> {
        self.error.take()
    }
}

/// Parse a numeric literal the way `as.numeric` does, including `Inf`/`NaN`.
fn parse_num(s: &str) -> Option<f64> {
    match s {
        "Inf" => Some(f64::INFINITY),
        "-Inf" => Some(f64::NEG_INFINITY),
        "NaN" => Some(f64::NAN),
        _ => s.parse().ok(),
    }
}

/// Decimal places a fixed-notation rendering of `x` needs at R's default
/// 7 significant digits, with trailing zeros dropped.
pub fn fixed_decimals(x: f64) -> usize {
    if !x.is_finite() || (x == x.trunc() && x.abs() < 1e15) {
        return 0;
    }
    let mag = x.abs().log10().floor() as i32;
    let d = (6 - mag).clamp(0, 15) as usize;
    let s = format!("{x:.d$}");
    let trimmed = s.trim_end_matches('0');
    match trimmed.split_once('.') {
        Some((_, frac)) => frac.len(),
        None => 0,
    }
}

/// Mantissa decimal places a scientific rendering of `x` needs at 7 significant
/// digits, with trailing zeros dropped (`1e+05` needs none, `1.5e-05` needs one).
pub fn sci_decimals(x: f64) -> usize {
    if !x.is_finite() || x == 0.0 {
        return 0;
    }
    let mag = x.abs().log10().floor() as i32;
    let mant = x / 10f64.powi(mag);
    let s = format!("{mant:.6}");
    let trimmed = s.trim_end_matches('0');
    match trimmed.split_once('.') {
        Some((_, frac)) => frac.len(),
        None => 0,
    }
}

/// Fixed-notation rendering with a given number of decimals; the non-finite
/// values keep R's spellings.
pub fn render_fixed(x: f64, decimals: usize) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Inf" } else { "-Inf" }.into();
    }
    format!("{x:.decimals$}")
}

/// Scientific rendering with a given mantissa width, in R's shape: at least two
/// exponent digits and an explicit sign (`1e+05`, `1.5e-05`).
pub fn render_sci(x: f64, decimals: usize) -> String {
    if !x.is_finite() {
        return render_fixed(x, 0);
    }
    if x == 0.0 {
        return format!("{:.*}e+00", decimals, 0.0);
    }
    let mut mag = x.abs().log10().floor() as i32;
    let mut mant = x / 10f64.powi(mag);
    // Rounding the mantissa can carry it to 10; renormalize so `9.9999999`
    // prints as `1e+01` rather than `10e+00`.
    if format!("{mant:.decimals$}")
        .trim_start_matches('-')
        .starts_with("10")
    {
        mant /= 10.0;
        mag += 1;
    }
    format!(
        "{mant:.decimals$}e{}{:02}",
        if mag < 0 { '-' } else { '+' },
        mag.abs()
    )
}

/// Format one double the way R prints it: 7 significant digits, and whichever
/// of fixed or scientific notation is *narrower* — R's `scipen = 0` rule, which
/// is why `1e5` prints as `1e+05` but `123456789` stays fixed. Ties go to fixed.
pub fn format_dbl(x: f64) -> String {
    let fixed = render_fixed(x, fixed_decimals(x));
    if !x.is_finite() {
        return fixed;
    }
    let sci = render_sci(x, sci_decimals(x));
    if sci.chars().count() < fixed.chars().count() {
        sci
    } else {
        fixed
    }
}

// ===========================================================================
// Running chunks: the program, closure bodies, loop bodies.
// ===========================================================================

/// Register every rlang builtin on a fresh VM and run `chunk` on it.
pub fn run_chunk(chunk: Chunk) -> Result<Value, String> {
    let mut vm = VM::new(chunk);
    crate::builtins::install(&mut vm);
    vm.enable_tracing_jit();
    let outcome = vm.run();
    if let Some(e) = with_host(|h| h.take_error()) {
        return Err(e);
    }
    match outcome {
        VMResult::Ok(v) => Ok(v),
        VMResult::Halted => Ok(vm.stack.last().cloned().unwrap_or(Value::Undef)),
        VMResult::Error(e) => Err(e),
    }
}

/// Run the top-level program chunk.
pub fn run_main(chunk: Chunk) -> Result<Value, String> {
    let r = run_chunk(chunk);
    with_host(|h| h.signal = None);
    r
}

/// Call any callable value with an already-evaluated argument list.
pub fn call_value(
    f: &Value,
    args: Vec<(Option<String>, Value)>,
    fun_name: Option<String>,
) -> Result<Value, String> {
    match with_host(|h| h.data_of(f)) {
        RData::Builtin(name) => crate::builtins::call_primitive(&name, args),
        RData::Closure { id, env } => call_closure(id, env, args, fun_name),
        _ => Err("attempt to apply non-function".into()),
    }
}

/// Call a closure: match the arguments to the formals, push a frame whose
/// environment encloses the closure's captured environment, and run the body.
pub fn call_closure(
    id: usize,
    env: Env,
    args: Vec<(Option<String>, Value)>,
    fun_name: Option<String>,
) -> Result<Value, String> {
    let def = with_host(|h| h.closures.get(id).cloned())
        .ok_or_else(|| format!("internal: unknown closure #{id}"))?;
    let frame_env = new_env(Some(env));
    let bindings = match_args(&def.params, &args)?;
    {
        let mut e = frame_env.borrow_mut();
        for (k, v) in bindings {
            e.vars.insert(k, v);
        }
    }
    with_host(|h| {
        h.frames.push(Frame {
            env: frame_env,
            args: args.clone(),
            fun_name,
            dispatch: None,
        })
    });
    let out = run_chunk(def.chunk.clone());
    with_host(|h| {
        h.frames.pop();
    });
    let out = out?;
    // A `return()` inside the body halted the chunk; its value is the result.
    let sig = with_host(|h| h.signal.take());
    match sig {
        Some(Signal::Return(v)) => Ok(v),
        Some(other) => {
            with_host(|h| h.signal = Some(other));
            Ok(out)
        }
        None => Ok(out),
    }
}

/// R's argument matching: exact tags first, then unique partial tags, then
/// positional fill of the still-unmatched formals. Everything left over goes to
/// `...` when the function has it, and is an error when it does not.
pub fn match_args(
    params: &[String],
    args: &[(Option<String>, Value)],
) -> Result<Vec<(String, Value)>, String> {
    let mut bound: Vec<Option<Value>> = vec![None; params.len()];
    let mut used = vec![false; args.len()];
    let dots_at = params.iter().position(|p| p == "...");

    // 1. exact tag matches
    for (ai, (tag, val)) in args.iter().enumerate() {
        let Some(tag) = tag else { continue };
        if let Some(pi) = params.iter().position(|p| p == tag) {
            if bound[pi].is_some() {
                return Err(format!(
                    "formal argument \"{tag}\" matched by multiple actual arguments"
                ));
            }
            bound[pi] = Some(val.clone());
            used[ai] = true;
        }
    }
    // 2. partial tag matches, only against formals before `...`
    let partial_limit = dots_at.unwrap_or(params.len());
    for (ai, (tag, val)) in args.iter().enumerate() {
        if used[ai] {
            continue;
        }
        let Some(tag) = tag else { continue };
        let hits: Vec<usize> = (0..partial_limit)
            .filter(|&pi| bound[pi].is_none() && params[pi].starts_with(tag.as_str()))
            .collect();
        match hits.len() {
            0 => {}
            1 => {
                bound[hits[0]] = Some(val.clone());
                used[ai] = true;
            }
            _ => return Err(format!("argument {tag} matches multiple formal arguments")),
        }
    }
    // 3. positional fill
    let mut pi = 0usize;
    for (ai, (tag, val)) in args.iter().enumerate() {
        if used[ai] || tag.is_some() {
            continue;
        }
        while pi < params.len() && (bound[pi].is_some() || params[pi] == "...") {
            if params[pi] == "..." {
                break;
            }
            pi += 1;
        }
        if pi >= params.len() || params[pi] == "..." {
            break;
        }
        bound[pi] = Some(val.clone());
        used[ai] = true;
        pi += 1;
    }

    let mut out = Vec::new();
    for (i, p) in params.iter().enumerate() {
        if p == "..." {
            continue;
        }
        if let Some(v) = &bound[i] {
            out.push((p.clone(), v.clone()));
        }
    }
    // 4. leftovers → `...`, or an error
    let rest: Vec<(Option<String>, Value)> = args
        .iter()
        .enumerate()
        .filter(|(i, _)| !used[*i])
        .map(|(_, a)| a.clone())
        .collect();
    if dots_at.is_some() {
        let dots = with_host(|h| h.alloc(RData::Args(rest)));
        out.push(("...".to_string(), dots));
    } else if let Some((tag, _)) = rest.first() {
        return Err(match tag {
            Some(t) => format!("unused argument ({t} = ...)"),
            None => "unused arguments".to_string(),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_dbl_matches_r_seven_significant_digits() {
        assert_eq!(format_dbl(1.0), "1");
        assert_eq!(format_dbl(1.5), "1.5");
        assert_eq!(format_dbl(1.0 / 3.0), "0.3333333");
        assert_eq!(format_dbl(123.456), "123.456");
        assert_eq!(format_dbl(f64::INFINITY), "Inf");
        assert_eq!(format_dbl(f64::NAN), "NaN");
    }

    #[test]
    fn exact_tags_beat_position() {
        reset_host();
        let (a, b) = with_host(|h| (h.scalar_dbl(1.0), h.scalar_dbl(2.0)));
        let bound = match_args(
            &["x".into(), "y".into()],
            &[(Some("y".into()), a.clone()), (None, b.clone())],
        )
        .unwrap();
        assert_eq!(bound[0].0, "x");
        assert_eq!(bound[0].1, b);
        assert_eq!(bound[1].0, "y");
        assert_eq!(bound[1].1, a);
    }

    #[test]
    fn partial_tags_match_unique_prefixes_only() {
        reset_host();
        let v = with_host(|h| h.scalar_dbl(1.0));
        let ok = match_args(&["verbose".into()], &[(Some("verb".into()), v.clone())]).unwrap();
        assert_eq!(ok[0].0, "verbose");
        let ambiguous = match_args(
            &["value".into(), "verbose".into()],
            &[(Some("v".into()), v.clone())],
        );
        assert!(ambiguous.is_err());
    }

    #[test]
    fn extra_arguments_need_dots() {
        reset_host();
        let v = with_host(|h| h.scalar_dbl(1.0));
        assert!(match_args(&["x".into()], &[(None, v.clone()), (None, v.clone())]).is_err());
        let with_dots =
            match_args(&["x".into(), "...".into()], &[(None, v.clone()), (None, v)]).unwrap();
        assert_eq!(with_dots.len(), 2);
        assert_eq!(with_dots[1].0, "...");
    }
}
