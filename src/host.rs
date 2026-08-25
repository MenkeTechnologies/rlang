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
/// Every map keyed by an R *name* — an environment's bindings, an object's
/// attributes — hashes with `FxHasher` rather than the standard library's
/// SipHash. R resolves a variable by name on every read, so this is the
/// interpreter's hottest hash; the keys are program identifiers, never
/// untrusted input, so SipHash's collision resistance was pure overhead.
/// Insertion order — which `ls()` and `attributes()` expose — is IndexMap's,
/// independent of the hasher.
pub type NameMap = IndexMap<String, Value, rustc_hash::FxBuildHasher>;

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

    /// `switch` branch selection: pops the `EXPR` value plus the branch names and
    /// pushes the raw integer index of the branch to run (or -1 for none), which
    /// the compiled jump table then dispatches on. Keeps `switch` lazy — only the
    /// selected branch's code is ever executed.
    pub const SWITCH_INDEX: u16 = 51;
    pub const RANGE_FROM: u16 = 52;
    pub const RANGE_STEP: u16 = 53;
    pub const RANGE_LEN: u16 = 54;
    /// `[v]` → v, marked visible. What `(` does in R: it is a function whose
    /// value is its argument, and evaluating a call sets the visibility flag —
    /// so `(x <- 5)` echoes while `x <- 5` does not.
    pub const SET_VISIBLE: u16 = 55;
    /// `[]` → NULL. Emitted after every top-level statement: R's default
    /// `options(warn = 0)` holds warnings until the statement that raised them
    /// finishes, then prints the whole batch, so this is where a top-level
    /// expression ends as far as the warning machinery is concerned.
    pub const WARN_FLUSH: u16 = 56;
    /// `[v]` → v, marked permanently shared. Emitted for the intermediate of a
    /// nested assignment target (`x$a[i] <- v`), whose object is an element of
    /// a container rather than the value of a binding, so the reference count
    /// on it does not describe how many bindings can see it.
    pub const MARK_SHARED: u16 = 57;
    /// `[text]` → text, with `text` opened as a call context *before* the
    /// arguments are evaluated. Emitted for a callee R implements as a closure:
    /// R makes the closure's context first and forces the argument promises
    /// inside it, so a condition raised while evaluating an argument reports
    /// the enclosing call. A callee R implements as a primitive makes no
    /// context at all, and gets no `OPEN_CALL`.
    pub const OPEN_CALL: u16 = 58;
    /// `[text, fun, args]` → result, for a call whose context [`OPEN_CALL`]
    /// already opened. Identical to [`CALL`] but for adopting that entry
    /// instead of pushing a second one.
    pub const CALL_OPENED: u16 = 59;
}

/// A variable environment: a frame's bindings plus a link to its enclosure.
pub struct EnvData {
    pub vars: NameMap,
    pub parent: Option<Env>,
}
/// A reference-counted environment, shared between a frame and every closure
/// that captured it.
pub type Env = Rc<RefCell<EnvData>>;

fn new_env(parent: Option<Env>) -> Env {
    Rc::new(RefCell::new(EnvData {
        vars: NameMap::default(),
        parent,
    }))
}

/// A compiled `function(...)`: its formals, its body chunk, and its deparsed
/// source. Defaults are not stored as expressions here — the compiler emits
/// them as a body prologue (`if (missing(p)) p <- <default>`), which is what
/// makes an R default able to refer to another argument.
///
/// `src` is the closure rendered by [`crate::deparse::deparse_closure`] at
/// compile time — the lines `print(f)`, `deparse(f)` and `format(f)` return.
/// It is computed from the AST before the body is lowered, because the AST does
/// not survive compilation and the default-argument expressions survive only as
/// prologue bytecode.
#[derive(Clone)]
pub struct ClosureDef {
    pub params: Vec<String>,
    pub chunk: Chunk,
    pub src: Vec<String>,
}

/// The non-local control transfers a builtin can raise.
#[derive(Debug, Clone)]
pub enum Signal {
    Break,
    Next,
    Return(Value),
}

/// One R value: its data, its attributes (`names`, `dim`, `class`, …), and the
/// count of durable references to it.
///
/// `refcnt` is R's `REFCNT` (true reference counting since R 4.0, where `NAMED`
/// used to be a saturating 0/1/2 approximation). It counts the places that
/// *keep* the object — an environment binding, an attribute slot, a list
/// element, a call frame's argument list — and deliberately not the operand
/// stack, exactly as R does not count C-stack references. A value with a count
/// of one is mutated in place by `x[i] <- v`; anything above one is copied
/// first. See [`RHost::may_mutate`].
#[derive(Clone)]
pub struct RObj {
    pub data: RData,
    pub attrs: NameMap,
    pub refcnt: u32,
}

/// The count at which reference counting stops. R's `REFCNTMAX` works the same
/// way: once a value has been referenced this many times the count saturates
/// and never decrements again, so the value stays permanently shared rather
/// than risking an undercount.
const REFCNT_MAX: u32 = u32::MAX;

/// Take a durable reference to `v` — R's `INCREMENT_REFCNT`.
///
/// A call-site argument list (`RData::Args`) is the one container that does not
/// count its elements when it is built: it exists for the duration of one call
/// and counting there would leave every argument of every call permanently
/// shared, which is the pessimisation this whole mechanism exists to avoid.
/// Binding one to a name — which is how `...` becomes durable — counts them
/// here instead, and [`release`] gives them back.
fn retain(heap: &mut [RObj], v: &Value) {
    let Value::Obj(i) = v else { return };
    let nested = {
        let Some(o) = heap.get_mut(*i as usize) else {
            return;
        };
        if o.refcnt == REFCNT_MAX {
            return;
        }
        o.refcnt += 1;
        match &o.data {
            RData::Args(items) => items.iter().map(|(_, v)| v.clone()).collect::<Vec<_>>(),
            _ => Vec::new(),
        }
    };
    for v in &nested {
        retain(heap, v);
    }
}

/// Give up a durable reference to `v` — R's `DECREMENT_REFCNT`.
fn release(heap: &mut [RObj], v: &Value) {
    let Value::Obj(i) = v else { return };
    let nested = {
        let Some(o) = heap.get_mut(*i as usize) else {
            return;
        };
        if o.refcnt == REFCNT_MAX || o.refcnt == 0 {
            return;
        }
        o.refcnt -= 1;
        match &o.data {
            RData::Args(items) => items.iter().map(|(_, v)| v.clone()).collect::<Vec<_>>(),
            _ => Vec::new(),
        }
    };
    for v in &nested {
        release(heap, v);
    }
}

/// Bind `name` to `val` in `env`, keeping the counts R's `SETCAR` keeps: the
/// new value gains a reference, the one it displaces loses one, and rebinding a
/// name to the object it already holds changes neither. That last rule is what
/// lets `x[i] <- v` re-bind `x` to the vector it just mutated in place without
/// marking it shared — R's `SETCAR` has the same early return.
fn bind_in(heap: &mut [RObj], env: &Env, name: &str, val: Value) {
    let old = env.borrow_mut().vars.insert(name.to_string(), val.clone());
    if old.as_ref() == Some(&val) {
        return;
    }
    if let Some(o) = old {
        release(heap, &o);
    }
    retain(heap, &val);
}

/// The R data types rlang represents. Atomic vectors hold `Option<T>`, where
/// `None` is `NA` — R's missing value is part of every atomic type, not a
/// separate one.
/// Which runtime function-combinator an [`RData::Combinator`] applies.
#[derive(Clone, Copy)]
pub enum CombinatorKind {
    /// `Negate(f)` — logical negation of `f`'s result.
    Negate,
    /// `Vectorize(f)` — apply `f` elementwise over recycled arguments.
    Vectorize,
}

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
    /// An opaque handle to an R object living in the embedded GNU R (a raw
    /// vector, S4 object, data frame, environment — anything rlang has no type
    /// for). The `usize` is the preserved `SEXP` pointer; it flows back into R
    /// verbatim, so a value rlang cannot inspect can still pass through it.
    RForeign(usize),
    /// A function built at runtime by wrapping another — `Negate(f)` /
    /// `Vectorize(f)`. The closure model uses compile-time chunk ids, so a
    /// runtime-constructed function is represented as data: the combinator kind
    /// plus a handle to the wrapped function.
    Combinator {
        kind: CombinatorKind,
        inner: Value,
    },
    Environment(Env),
    /// A call-site argument list: `(tag, value)` pairs, produced by `MKARGS`.
    /// `Value::Undef` is an empty argument (`x[, 1]`).
    Args(Vec<(Option<String>, Value)>),
    /// An unevaluated R expression — R's `LANGSXP`. What `quote(f(x))` builds,
    /// what `sys.call()` hands back, and what a condition's `call` slot holds.
    /// The payload is the parse tree itself, so it deparses back to source and
    /// takes apart the way R's does.
    Lang(crate::ast::Expr),
    /// An R name — `SYMSXP`. `quote(x)`, and the head of every call.
    Sym(String),
}

/// Which [`RData`] variant a value holds, without its payload.
///
/// [`RHost::data_of`] *clones* the data — R's copy-on-assign semantics mean a
/// caller that keeps it must own it — so asking `matches!(data(x), RData::Str(_))`
/// copied the whole vector to read its tag. `RHost::kind_of` answers the same
/// question in O(1), which is what the vector paths (arithmetic, comparison,
/// subscript assignment) ask on every operation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RKind {
    Null,
    Lgl,
    Int,
    Dbl,
    Str,
    List,
    Closure,
    Builtin,
    RForeign,
    Combinator,
    Environment,
    Args,
    Lang,
    Sym,
}

impl RKind {
    /// The kind of an already-materialised `RData`.
    pub fn of(d: &RData) -> RKind {
        match d {
            RData::Null => RKind::Null,
            RData::Lgl(_) => RKind::Lgl,
            RData::Int(_) => RKind::Int,
            RData::Dbl(_) => RKind::Dbl,
            RData::Str(_) => RKind::Str,
            RData::List(_) => RKind::List,
            RData::Closure { .. } => RKind::Closure,
            RData::Builtin(_) => RKind::Builtin,
            RData::RForeign(_) => RKind::RForeign,
            RData::Combinator { .. } => RKind::Combinator,
            RData::Environment(_) => RKind::Environment,
            RData::Args(_) => RKind::Args,
            RData::Lang(_) => RKind::Lang,
            RData::Sym(_) => RKind::Sym,
        }
    }

    /// This kind's place on the promotion ladder — see [`type_rank`].
    pub fn rank(self) -> u8 {
        match self {
            RKind::Null => 0,
            RKind::Lgl => 1,
            RKind::Int => 2,
            RKind::Dbl => 3,
            RKind::Str => 4,
            _ => 5,
        }
    }
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
    /// The closure being executed, as the `(id, environment)` pair it was built
    /// from, so `Recall` can re-invoke it — anonymous functions included. Kept
    /// as the pair rather than an allocated closure value because only `Recall`
    /// ever reads it, and building the value allocated a heap cell on every
    /// call. `None` for the top-level frame, which is in no closure.
    pub fun: Option<(usize, Env)>,
    /// Set by `UseMethod` so `NextMethod` can continue down the class vector.
    pub dispatch: Option<(String, Vec<String>)>,
    /// Thunks registered by `on.exit`, run when this frame is left — whether it
    /// returns normally or unwinds with an error.
    pub on_exit: Vec<Value>,
}

/// One `tryCatch` or `withCallingHandlers` frame, innermost last.
///
/// The two differ only in what happens when a handler matches: an *exiting*
/// handler (`tryCatch`) unwinds the stack first and runs the handler in the
/// `tryCatch`'s own frame, while a *calling* handler (`withCallingHandlers`)
/// runs at the point the condition was signalled and — unless it transfers
/// control to a restart — lets evaluation resume there.
pub struct HandlerFrame {
    /// `true` for `withCallingHandlers`.
    pub calling: bool,
    /// `(condition class, handler)` in the order written, which is the order R
    /// tries them: `tryCatch` runs the first match, `withCallingHandlers` runs
    /// every match.
    pub handlers: Vec<(String, Value)>,
    /// Set while one of this frame's handlers runs, so a condition signalled
    /// from inside a handler is only offered to frames established *outside*
    /// it. Without this, a `warning()` in a warning handler re-enters it.
    pub disabled: bool,
    /// A frame established by `suppressWarnings`/`suppressMessages`, which have
    /// no R-level handler: a matching condition is muffled outright. Holds the
    /// condition class to muffle (`"warning"` / `"message"`).
    pub muffle: Option<String>,
}

/// One restart, as established by `withRestarts` or by the `warning()` /
/// `message()` builtins around their own signalling.
pub struct RestartFrame {
    /// Identity for `invokeRestart`, which names a *particular* established
    /// restart — two nested `withRestarts` may use the same name.
    pub id: u64,
    pub name: String,
    pub description: String,
    /// The function to run after control transfers here. `None` for the
    /// built-in muffle restarts, which are consumed by the signalling builtin
    /// itself rather than by an R-level handler.
    pub handler: Option<Value>,
}

/// A restart transfer in flight: the target restart's id, and the arguments
/// `invokeRestart` was given to pass on to its handler.
pub type RestartInvoke = (u64, Vec<(Option<String>, Value)>);

/// The `Err` payload that carries a restart transfer out to its establishing
/// frame. It is not a message anyone sees: every site that catches errors
/// (`tryCatch`, `try`, the top-level reporter) re-propagates while
/// [`RHost::restart_invoke`] is set, and the establishing frame clears it.
pub const RESTART_UNWIND: &str = "\u{1}restart";

/// The R runtime: heap, environments, call stack, and pending control state.
/// A queued warning: its message and the call it was raised from, which R
/// prints as the `In <call> :` prefix. `None` for one raised at top level,
/// where R's `findCall` finds no enclosing call either.
/// A context's call text, read back out of the constant holding it.
fn ctx_text(v: &Value) -> String {
    // A diagnostic shows one line: R's `deparse1s(call)` keeps only the first,
    // so `withCallingHandlers({ … })` reports as `withCallingHandlers({`. The
    // rest of the deparse is there for `sys.call()`, which needs source that
    // parses back.
    ctx_source(v).lines().next().unwrap_or_default().to_string()
}

/// A context's call as whole R source — every line of the deparse.
pub fn ctx_source(v: &Value) -> String {
    match v {
        Value::Str(s) => s.to_string(),
        _ => String::new(),
    }
}

/// One entry on the call context stack.
pub struct CallCtx {
    /// The call's deparsed source, as `In <call> :` renders it — held as the
    /// compiler's own string constant, so opening a context is an `Arc` bump
    /// rather than a copy. Every call pays for this; only a warning reads it.
    pub text: Value,
    /// Whether the call has actually been entered, or is only open around the
    /// evaluation of its own arguments. rlang evaluates arguments eagerly but
    /// opens a closure's context before them, so both states exist here where
    /// R has a promise instead. The diagnostics want the open call — a warning
    /// raised while evaluating an argument to `print` reports `print(…)`, as it
    /// does in R — while `sys.call()` wants the entered one, because a promise
    /// belongs to the frame whose body wrote it: `h <- function() g(sys.call())`
    /// answers `h()`, not `g(…)`.
    pub entered: bool,
    /// Whether *R* would make a function context for this call — R's
    /// `CTXT_FUNCTION`, which every closure sets and no primitive does. It is
    /// decided by the compiler from R's primitive list rather than by what
    /// rlang implements the callee as, because rlang implements most of R's
    /// base *closures* as Rust builtins and the condition machinery has to see
    /// R's shape, not rlang's.
    pub closure: bool,
}

pub struct Warning {
    pub msg: String,
    pub call: Option<String>,
}

pub struct RHost {
    heap: Vec<RObj>,
    pub global: Env,
    pub frames: Vec<Frame>,
    pub closures: Vec<ClosureDef>,
    pub error: Option<String>,
    /// The S3 class vector of the condition currently being signalled — R's
    /// `c("simpleError", "error", "condition")` and friends. It rides alongside
    /// `error` so a `tryCatch` handler can be selected by class.
    pub error_classes: Vec<String>,
    /// The call the pending error was raised in — R's `errorcall`. Held as
    /// whole R source: the `Error in <call> :` line shows its first line, and
    /// the condition object the error carries needs the rest. It has to be taken where the error is
    /// recorded: by the time the unwind reaches the top the context stack has
    /// been cut back past it. Written through [`RHost::set_error_call`].
    pub error_call: Option<String>,
    /// The deparsed call of every *function* context the pending error was
    /// raised under, innermost last — what R's `Calls:` line is built from.
    /// Snapshotted with [`RHost::set_error_call`], for the same reason.
    pub error_trace: Vec<String>,
    /// Whether the pending error's call has been decided. `error_call` alone
    /// cannot say: a top-level `stop()` decides on *no* call, which is an
    /// answer rather than an absence, and every frame the unwind passes through
    /// would otherwise overwrite it with its own.
    pub error_call_known: bool,
    /// The enclosing `tryCatch` / `withCallingHandlers` frames, innermost last.
    /// Every signalling site walks it outward: a calling handler runs in place,
    /// an exiting handler unwinds, and reaching the bottom with nothing matched
    /// is what lets `warning()` and `message()` keep R's default action.
    pub handlers: Vec<HandlerFrame>,
    /// The established restarts, innermost group last. One group per
    /// `withRestarts` call (or per signalling builtin), because the restarts in
    /// a single call are searched in the order written while the groups
    /// themselves are searched innermost-first.
    pub restarts: Vec<Vec<RestartFrame>>,
    /// Source of [`RestartFrame::id`].
    pub next_restart_id: u64,
    /// Set by `invokeRestart` and cleared by the frame that established the
    /// target: the restart's id and the arguments to pass its handler. While it
    /// is set the current unwind is a control transfer, not an error, so no
    /// handler may absorb it.
    pub restart_invoke: Option<RestartInvoke>,
    /// Set by S3 dispatch immediately before invoking a method, and consumed by
    /// the frame that method pushes, so `NextMethod` inside it knows which
    /// classes are still to try.
    pub pending_dispatch: Option<(String, Vec<String>)>,
    /// Set for exactly one `call_primitive` by `NextMethod`'s fall-through, so
    /// the primitive behind a generic runs its own implementation instead of
    /// dispatching S3 on the same object again.
    pub suppress_s3: bool,
    pub signal: Option<Signal>,
    /// R's "visibility" flag: assignment and `invisible()` clear it, so the
    /// top-level echo knows not to print.
    pub visible: bool,
    /// Whether the top-level echo prints at all. On for `Rscript` and the REPL,
    /// off for embedding and for tests that want the value, not the transcript.
    pub echo: bool,
    /// One entry per call now being evaluated, innermost last — R's context
    /// stack, narrowed to what the condition machinery reads off it. R's
    /// `findCall` walks it for the innermost *function* context, which is the
    /// call `warning()` reports; a warning raised inside a primitive reports
    /// that primitive's own call instead.
    pub calls: Vec<CallCtx>,
    /// Warnings raised by the top-level statement now running, held until it
    /// finishes — R's default `options(warn = 0)`. Capped at [`MAX_WARNINGS`]
    /// entries the way R caps `nwarnings`.
    pub warnings: Vec<Warning>,
    /// How many warnings that statement raised, which keeps counting past the
    /// cap so the "50 or more" banner can say so.
    pub warn_count: usize,
    /// Interned singletons so `NULL` and the common constants do not allocate a
    /// fresh heap cell on every evaluation.
    null: Option<Value>,
}

thread_local! {
    static HOST: RefCell<RHost> = RefCell::new(RHost::new());
    /// When `Some`, R's stdout output (`print`/`cat`/autoprint) is appended here
    /// instead of written to the process stdout. `wasm32` has no real stdout, so
    /// the wasm entry point (`crate::eval_capture`) drains this buffer; native
    /// callers leave it `None` and print straight through.
    static CAPTURE: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Write R program output. Appended to the capture buffer when one is active
/// (see [`start_capture`]), otherwise written to the process stdout. Every
/// R-visible stdout write (`print`, `cat`, top-level autoprint) goes through
/// here so a single switch redirects them all.
pub fn emit(s: &str) {
    CAPTURE.with(|c| {
        if let Some(buf) = c.borrow_mut().as_mut() {
            buf.push_str(s);
        } else {
            use std::io::Write as _;
            let mut out = std::io::stdout();
            let _ = out.write_all(s.as_bytes());
        }
    });
}

/// Begin capturing R stdout into an in-memory buffer, replacing any prior one.
pub fn start_capture() {
    CAPTURE.with(|c| *c.borrow_mut() = Some(String::new()));
}

/// Stop capturing and return everything written since [`start_capture`].
pub fn take_capture() -> String {
    CAPTURE.with(|c| c.borrow_mut().take().unwrap_or_default())
}

thread_local! {
    /// Where R's diagnostics go. `None` writes them straight to the process
    /// stderr; `Rscript` installs a hook instead, because it buffers stdout for
    /// the CRAN fallback and has to replay the two streams in the order they
    /// were written rather than one after the other. See `main.rs`.
    #[allow(clippy::type_complexity)]
    static STDERR_HOOK: RefCell<Option<Box<dyn Fn(&str)>>> = const { RefCell::new(None) };
}

/// Route R's diagnostics (the warning batches) through `f` instead of stderr.
pub fn set_stderr_hook(f: Box<dyn Fn(&str)>) {
    STDERR_HOOK.with(|h| *h.borrow_mut() = Some(f));
}

/// Stop diverting diagnostics, so later ones go straight to stderr. Without
/// this the batch a *failed* statement queued was written into a sink that had
/// already been replayed, and vanished: R prints it after the error line, under
/// `In addition:`, which is after the replay.
pub fn clear_stderr_hook() {
    STDERR_HOOK.with(|h| *h.borrow_mut() = None);
}

/// Write one diagnostic — through the installed hook, or to stderr.
pub fn emit_stderr(s: &str) {
    let handled = STDERR_HOOK.with(|h| h.borrow().as_ref().map(|f| f(s)).is_some());
    if !handled {
        eprint!("{s}");
    }
}

/// Run `f` with mutable access to the thread-local host.
pub fn with_host<R>(f: impl FnOnce(&mut RHost) -> R) -> R {
    HOST.with(|h| f(&mut h.borrow_mut()))
}

/// Reset the host to a clean slate (fresh global environment and heap).
pub fn reset_host() {
    CLOSURE_VMS.with(|c| c.borrow_mut().clear());
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
                fun: None,
                dispatch: None,
                on_exit: Vec::new(),
            }],
            closures: Vec::new(),
            error: None,
            error_classes: Vec::new(),
            error_call: None,
            error_trace: Vec::new(),
            error_call_known: false,
            handlers: Vec::new(),
            restarts: Vec::new(),
            next_restart_id: 0,
            restart_invoke: None,
            pending_dispatch: None,
            suppress_s3: false,
            signal: None,
            visible: true,
            echo: true,
            calls: Vec::new(),
            warnings: Vec::new(),
            warn_count: 0,
            null: None,
        }
    }

    /// Install the compiled closure bodies for a program.
    pub fn load_closures(&mut self, closures: Vec<ClosureDef>) {
        // The parked VMs are keyed by closure id and still hold the *old*
        // program's chunk for that id, so a new closure table invalidates every
        // one of them.
        CLOSURE_VMS.with(|c| c.borrow_mut().clear());
        self.closures = closures;
    }

    // ── heap ───────────────────────────────────────────────────────────

    /// Allocate `data` with no attributes and return its handle.
    pub fn alloc(&mut self, data: RData) -> Value {
        self.alloc_with(data, NameMap::default())
    }

    /// Allocate `data` carrying `attrs`.
    pub fn alloc_with(&mut self, data: RData, attrs: NameMap) -> Value {
        // A container holds a durable reference to each element, the way R's
        // `SET_VECTOR_ELT` and `setAttrib` increment what they store. rlang
        // builds a container by allocating a fresh one, so this is the single
        // site where a list's or a combinator's elements are counted.
        match &data {
            RData::List(xs) => {
                for v in xs {
                    retain(&mut self.heap, v);
                }
            }
            RData::Combinator { inner, .. } => retain(&mut self.heap, inner),
            _ => {}
        }
        for v in attrs.values() {
            retain(&mut self.heap, v);
        }
        self.heap.push(RObj {
            data,
            attrs,
            refcnt: 0,
        });
        Value::Obj((self.heap.len() - 1) as u32)
    }

    /// R's `REFCNT`: how many durable references the value has. An unboxed
    /// scalar has no heap cell at all, so it answers `REFCNT_MAX` — never
    /// eligible for in-place mutation.
    pub fn refcnt(&self, v: &Value) -> u32 {
        match v {
            Value::Obj(i) => self.heap.get(*i as usize).map_or(REFCNT_MAX, |o| o.refcnt),
            _ => REFCNT_MAX,
        }
    }

    /// R's `!MAYBE_SHARED`: whether nothing but the binding being assigned
    /// through holds this object, so `x[i] <- v` may write into it rather than
    /// rebuilding it.
    pub fn may_mutate(&self, v: &Value) -> bool {
        self.refcnt(v) <= 1
    }

    /// Run `f` on a heap object's data in place. The caller must have
    /// established [`RHost::may_mutate`] first — this is the write half of R's
    /// reference-count rule and there is no other way to reach it.
    pub fn with_data_mut<R>(&mut self, v: &Value, f: impl FnOnce(&mut RData) -> R) -> Option<R> {
        self.get_mut(v).map(|o| f(&mut o.data))
    }

    /// Write `val` into a list's slots in place, keeping the element counts:
    /// each slot written takes a reference to `val` and gives back the one it
    /// held. A position past the end extends the list, filling the slots the
    /// assignment skipped with `NULL` the way R's `EnlargeVector` does. `None`
    /// when the value is not a list, which leaves the caller on the rebuild
    /// path.
    pub fn set_list_elements(&mut self, x: &Value, positions: &[usize], val: &Value) -> Option<()> {
        let null = self.null();
        let displaced = self.with_data_mut(x, |d| match d {
            RData::List(items) => Some(
                positions
                    .iter()
                    .map(|p| {
                        while items.len() <= *p {
                            items.push(null.clone());
                        }
                        std::mem::replace(&mut items[*p], val.clone())
                    })
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })??;
        for old in &displaced {
            release(&mut self.heap, old);
        }
        for _ in 0..positions.len() {
            retain(&mut self.heap, val);
        }
        Some(())
    }

    /// Mark a value permanently shared, so no later `x[i] <- v` writes into it.
    /// The compiler emits this for the intermediate of a *nested* assignment
    /// target (`x$a[i] <- v`), whose object is an element of a container rather
    /// than the value of a binding: R reaches the same conclusion by having
    /// `EnsureLocal`'s shallow duplicate increment every element it shares.
    pub fn mark_shared(&mut self, v: &Value) {
        if let Value::Obj(i) = v {
            if let Some(o) = self.heap.get_mut(*i as usize) {
                o.refcnt = REFCNT_MAX;
            }
        }
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
        match unboxed(v) {
            Some(Ub::I(n)) => return RData::Int(vec![Some(n)]),
            Some(Ub::F(f)) => return RData::Dbl(vec![Some(f)]),
            Some(Ub::B(b)) => return RData::Lgl(vec![Some(b)]),
            None => {}
        }
        self.get(v).map(|o| o.data.clone()).unwrap_or(RData::Null)
    }

    /// Which variant this value's data is, without copying the data.
    pub fn kind_of(&self, v: &Value) -> RKind {
        match unboxed(v) {
            Some(Ub::I(_)) => return RKind::Int,
            Some(Ub::F(_)) => return RKind::Dbl,
            Some(Ub::B(_)) => return RKind::Lgl,
            None => {}
        }
        self.get(v)
            .map(|o| RKind::of(&o.data))
            .unwrap_or(RKind::Null)
    }

    /// A length-1, unattributed, non-NA numeric/logical value as
    /// `(value, is_integer_typed)` — the scalar fast path for arithmetic and
    /// comparison. Returns `None` (forcing the full vector path) for anything
    /// with attributes, length != 1, an NA element, or a non-numeric type. Reads
    /// by borrow: no `data_of` clone of the backing vector.
    pub fn scalar_real(&self, v: &Value) -> Option<(f64, bool)> {
        match unboxed(v) {
            Some(Ub::I(n)) => return Some((n as f64, true)),
            Some(Ub::F(f)) => return Some((f, false)),
            Some(Ub::B(b)) => return Some((b as i64 as f64, true)),
            None => {}
        }
        let o = self.get(v)?;
        if !o.attrs.is_empty() {
            return None;
        }
        match &o.data {
            RData::Int(x) if x.len() == 1 => x[0].map(|n| (n as f64, true)),
            RData::Lgl(x) if x.len() == 1 => x[0].map(|b| (b as i64 as f64, true)),
            RData::Dbl(x) if x.len() == 1 => x[0].map(|n| (n, false)),
            _ => None,
        }
    }

    /// The attributes of a value.
    pub fn attrs_of(&self, v: &Value) -> NameMap {
        if unboxed(v).is_some() {
            return NameMap::default();
        }
        self.get(v).map(|o| o.attrs.clone()).unwrap_or_default()
    }

    /// Read one attribute.
    pub fn attr(&self, v: &Value, name: &str) -> Option<Value> {
        if unboxed(v).is_some() {
            return None;
        }
        self.get(v).and_then(|o| o.attrs.get(name).cloned())
    }

    /// Set (or, with `NULL`, remove) one attribute in place.
    pub fn set_attr(&mut self, v: &Value, name: &str, val: Value) {
        let is_null = matches!(self.data_of(&val), RData::Null);
        let Some(o) = self.get_mut(v) else { return };
        let old = if is_null {
            o.attrs.shift_remove(name)
        } else {
            o.attrs.insert(name.to_string(), val.clone())
        };
        // An attribute slot is a durable reference, like a binding: the value it
        // held loses one and the value replacing it gains one.
        if old.as_ref() == Some(&val) {
            return;
        }
        if let Some(o) = old {
            release(&mut self.heap, &o);
        }
        if !is_null {
            retain(&mut self.heap, &val);
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
        if unboxed(v).is_some() {
            return 1;
        }
        match self.get(v).map(|o| &o.data) {
            Some(RData::Null) | None => 0,
            Some(RData::Lgl(x)) => x.len(),
            Some(RData::Int(x)) => x.len(),
            Some(RData::Dbl(x)) => x.len(),
            Some(RData::Str(x)) => x.len(),
            Some(RData::List(x)) => x.len(),
            Some(RData::Args(x)) => x.len(),
            Some(RData::Environment(e)) => e.borrow().vars.len(),
            // A call's length is its parts: the function and its arguments.
            // A name is one thing.
            Some(RData::Lang(e)) => crate::builtins::lang_len(e),
            _ => 1,
        }
    }

    /// Coerce to logicals (`as.logical`); non-convertible strings become NA.
    pub fn as_lgl(&self, v: &Value) -> Vec<Option<bool>> {
        match unboxed(v) {
            Some(Ub::I(n)) => return vec![Some(n != 0)],
            Some(Ub::F(f)) => return vec![Some(f != 0.0)],
            Some(Ub::B(b)) => return vec![Some(b)],
            None => {}
        }
        match self.get(v).map(|o| &o.data) {
            Some(RData::Lgl(x)) => x.clone(),
            Some(RData::Int(x)) => x.iter().map(|e| e.map(|n| n != 0)).collect(),
            Some(RData::Dbl(x)) => x.iter().map(|e| e.map(|n| n != 0.0)).collect(),
            Some(RData::Str(x)) => x
                .iter()
                // R accepts exactly four spellings per truth value — the initial,
                // the all-caps word, the all-lower word, and the capitalised word
                // (`StringTrue` in R's coerceVector). Anything else, including
                // "Tr" or "yes", is NA.
                .map(|e| match e.as_deref() {
                    Some("T") | Some("TRUE") | Some("true") | Some("True") => Some(true),
                    Some("F") | Some("FALSE") | Some("false") | Some("False") => Some(false),
                    _ => None,
                })
                .collect(),
            Some(RData::List(x)) => x.iter().flat_map(|e| self.as_lgl(e)).collect(),
            _ => Vec::new(),
        }
    }

    /// Coerce to integers (`as.integer`); doubles truncate toward zero.
    pub fn as_int(&self, v: &Value) -> Vec<Option<i64>> {
        match unboxed(v) {
            Some(Ub::I(n)) => return vec![Some(n)],
            Some(Ub::F(f)) => return vec![f.is_finite().then_some(f.trunc() as i64)],
            Some(Ub::B(b)) => return vec![Some(b as i64)],
            None => {}
        }
        match self.get(v).map(|o| &o.data) {
            Some(RData::Lgl(x)) => x.iter().map(|e| e.map(|b| b as i64)).collect(),
            Some(RData::Int(x)) => x.clone(),
            Some(RData::Dbl(x)) => x
                .iter()
                .map(|e| e.and_then(|n| n.is_finite().then_some(n.trunc() as i64)))
                .collect(),
            Some(RData::Str(x)) => x
                .iter()
                .map(|e| e.as_ref().and_then(|s| parse_num(s.trim())))
                .map(|o| o.and_then(|n| n.is_finite().then_some(n.trunc() as i64)))
                .collect(),
            Some(RData::List(x)) => x.iter().flat_map(|e| self.as_int(e)).collect(),
            _ => Vec::new(),
        }
    }

    /// Coerce to doubles (`as.numeric`).
    pub fn as_dbl(&self, v: &Value) -> Vec<Option<f64>> {
        match unboxed(v) {
            Some(Ub::I(n)) => return vec![Some(n as f64)],
            Some(Ub::F(f)) => return vec![Some(f)],
            Some(Ub::B(b)) => return vec![Some(b as i64 as f64)],
            None => {}
        }
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
        match unboxed(v) {
            Some(Ub::I(n)) => return vec![Some(n.to_string())],
            Some(Ub::F(f)) => return vec![Some(format_dbl(f))],
            Some(Ub::B(b)) => return vec![Some(if b { "TRUE" } else { "FALSE" }.to_string())],
            None => {}
        }
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
        // An unboxed scalar is length-1: element 0 is itself.
        if unboxed(v).is_some() {
            return if i == 0 { v.clone() } else { self.null() };
        }
        // Read ONE element by borrowing the backing store — never clone the
        // whole vector. `data_of` clones all of `o.data`, so the old
        // `match self.data_of(v)` here was O(n) per call, making
        // `for (x in seq)` O(n^2) over the sequence length.
        enum Elem {
            Lgl(Option<bool>),
            Int(Option<i64>),
            Dbl(Option<f64>),
            Str(Option<String>),
            Val(Value),
            Null,
        }
        let e = match self.get(v).map(|o| &o.data) {
            Some(RData::Lgl(x)) => Elem::Lgl(x.get(i).cloned().flatten()),
            Some(RData::Int(x)) => Elem::Int(x.get(i).cloned().flatten()),
            Some(RData::Dbl(x)) => Elem::Dbl(x.get(i).cloned().flatten()),
            Some(RData::Str(x)) => Elem::Str(x.get(i).cloned().flatten()),
            Some(RData::List(x)) => x.get(i).cloned().map(Elem::Val).unwrap_or(Elem::Null),
            _ => Elem::Null,
        };
        // A non-NA numeric/logical element rides unboxed (allocation-free) — this
        // is what makes `for (i in seq)` allocate nothing per iteration. NA has
        // no unboxed form, and strings are not yet unboxed, so those still box.
        match e {
            Elem::Lgl(Some(b)) => Value::Bool(b),
            Elem::Int(Some(n)) => Value::Int(n),
            Elem::Dbl(Some(f)) => Value::Float(f),
            Elem::Lgl(o) => self.lgl(vec![o]),
            Elem::Int(o) => self.int(vec![o]),
            Elem::Dbl(o) => self.dbl(vec![o]),
            Elem::Str(o) => self.str_vec(vec![o]),
            Elem::Val(v) => v,
            Elem::Null => self.null(),
        }
    }

    /// `names(x)`, as a plain vector of `Option<String>` (`None` where unnamed).
    pub fn names(&self, v: &Value) -> Vec<Option<String>> {
        if let Some(n) = self.attr(v, "names") {
            return self.as_str(&n);
        }
        // A 1-D array carries no `names` attribute: R's `names.default` reads
        // `dimnames[[1]]` instead, which is where a `table`'s labels live.
        let one_d = self.attr(v, "dim").is_some_and(|d| self.length(&d) == 1);
        if one_d {
            if let Some(dn) = self.attr(v, "dimnames") {
                if let Some(RObj {
                    data: RData::List(xs),
                    ..
                }) = self.get(&dn)
                {
                    if let Some(first) = xs.first() {
                        return self.as_str(first);
                    }
                }
            }
        }
        Vec::new()
    }

    /// Whether the value is `NULL`.
    pub fn is_null(&self, v: &Value) -> bool {
        if unboxed(v).is_some() {
            return false;
        }
        matches!(self.get(v).map(|o| &o.data), Some(RData::Null) | None)
    }

    /// Whether the value is callable.
    pub fn is_function(&self, v: &Value) -> bool {
        matches!(
            self.get(v).map(|o| &o.data),
            Some(RData::Closure { .. }) | Some(RData::Builtin(_)) | Some(RData::Combinator { .. })
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
        // R's implicit class for a dimensioned object: rank 2 is a matrix (and
        // an array), any other rank is an array.
        match self.attr(v, "dim").map(|d| self.length(&d)) {
            Some(2) => return vec!["matrix".into(), "array".into()],
            Some(n) if n > 0 => return vec!["array".into()],
            _ => {}
        }
        match unboxed(v) {
            Some(Ub::I(_)) => return vec!["integer".into()],
            Some(Ub::F(_)) => return vec!["numeric".into()],
            Some(Ub::B(_)) => return vec!["logical".into()],
            None => {}
        }
        vec![match self.get(v).map(|o| &o.data) {
            Some(RData::Null) | None => "NULL",
            Some(RData::Lgl(_)) => "logical",
            Some(RData::Int(_)) => "integer",
            Some(RData::Dbl(_)) => "numeric",
            Some(RData::Str(_)) => "character",
            Some(RData::List(_)) => "list",
            Some(RData::Closure { .. })
            | Some(RData::Builtin(_))
            | Some(RData::Combinator { .. }) => "function",
            Some(RData::Environment(_)) => "environment",
            Some(RData::Args(_)) => "list",
            // An unevaluated expression's implicit class is what it *is*: a
            // call, or the name of one thing.
            Some(RData::Lang(_)) => "call",
            Some(RData::Sym(_)) => "name",
            // A foreign R object's real class is only known to R.
            Some(RData::RForeign(_)) => "R_object",
        }
        .to_string()]
    }

    /// `typeof(x)`.
    pub fn type_of(&self, v: &Value) -> &'static str {
        match unboxed(v) {
            Some(Ub::I(_)) => return "integer",
            Some(Ub::F(_)) => return "double",
            Some(Ub::B(_)) => return "logical",
            None => {}
        }
        match self.get(v).map(|o| &o.data) {
            Some(RData::Null) | None => "NULL",
            Some(RData::Lgl(_)) => "logical",
            Some(RData::Int(_)) => "integer",
            Some(RData::Dbl(_)) => "double",
            Some(RData::Str(_)) => "character",
            Some(RData::List(_)) | Some(RData::Args(_)) => "list",
            Some(RData::RForeign(_)) => "externalptr",
            Some(RData::Closure { .. }) | Some(RData::Combinator { .. }) => "closure",
            Some(RData::Builtin(_)) => "builtin",
            Some(RData::Environment(_)) => "environment",
            Some(RData::Lang(_)) => "language",
            Some(RData::Sym(_)) => "symbol",
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

    /// The call now being evaluated, whatever kind it is. A warning raised
    /// inside a primitive reports this — `as.integer("x")` warns as
    /// `In as.integer("x") : NAs introduced by coercion`.
    pub fn current_call(&self) -> Option<String> {
        self.calls.last().map(|c| ctx_text(&c.text))
    }

    /// The same call, as whole R source. A *diagnostic* shows one line of it; a
    /// condition object carries the call itself, so it needs all of it.
    pub fn current_call_source(&self) -> Option<String> {
        self.calls.last().map(|c| ctx_source(&c.text))
    }

    /// The deparsed source of each argument in the call now being entered —
    /// what a diagnostic that quotes an argument back at the caller reads.
    /// Empty when the context is not a call, which leaves the caller to say
    /// nothing rather than something wrong.
    pub fn current_call_arg_sources(&self) -> Vec<String> {
        let Some(src) = self.current_call_source() else {
            return Vec::new();
        };
        let Ok(mut exprs) = crate::parser::parse(&src) else {
            return Vec::new();
        };
        if exprs.len() != 1 {
            return Vec::new();
        }
        let crate::ast::Expr::Call { args, .. } = exprs.remove(0) else {
            return Vec::new();
        };
        args.iter()
            .map(|a| {
                a.value
                    .as_ref()
                    .map(crate::deparse::deparse_first_line)
                    .unwrap_or_default()
            })
            .collect()
    }

    /// The call [`RHost::enclosing_call`] names, as whole R source.
    pub fn enclosing_call_source(&self) -> Option<String> {
        self.calls
            .iter()
            .rev()
            .skip(1)
            .find(|c| c.closure)
            .map(|c| ctx_source(&c.text))
    }

    /// The innermost *function* context — what R's C `warning()` reports when
    /// it is called from inside a primitive that has no call of its own, as
    /// `CoercionWarning` is. A primitive makes no context, so the warning lands
    /// on the closure containing it: `print(as.integer("x"))`, and nothing at
    /// all when that closure is the top level.
    pub fn context_call(&self) -> Option<String> {
        self.innermost_context(0)
    }

    /// R's `findCall`, which starts at `R_GlobalContext->nextcontext`: the
    /// innermost function context *outside* the call now running. `warning()`
    /// is itself a closure in R, so its own context is the one being skipped,
    /// and what is left is the function that called it.
    pub fn enclosing_call(&self) -> Option<String> {
        self.innermost_context(1)
    }

    fn innermost_context(&self, skip: usize) -> Option<String> {
        self.enclosing_ctx(skip).map(|c| ctx_text(&c.text))
    }

    /// The innermost enclosing closure call as whole R source — what
    /// `sys.call()` parses back into a language object. The same search
    /// `enclosing_call` runs for the diagnostics, and it skips one frame for
    /// the same reason: `sys.call()` is itself a call.
    pub fn enclosing_source(&self) -> Option<String> {
        self.calls
            .iter()
            .rev()
            .skip(1)
            .find(|c| c.closure && c.entered)
            .map(|c| ctx_source(&c.text))
    }

    fn enclosing_ctx(&self, skip: usize) -> Option<&CallCtx> {
        self.calls.iter().rev().skip(skip).find(|c| c.closure)
    }

    /// The formals of the closure now executing, for the builtins that have to
    /// know what a call's arguments were matched *against*.
    pub fn current_formals(&self) -> Option<Vec<String>> {
        let (id, _) = self.frames.last()?.fun.as_ref()?;
        Some(self.closures.get(*id)?.params.clone())
    }

    /// Bind `name` in the current environment.
    pub fn set_var(&mut self, name: &str, val: Value) {
        let env = self.env();
        bind_in(&mut self.heap, &env, name, val);
    }

    /// Bind `name` in `env`, maintaining reference counts. Used by the call
    /// machinery, which binds into a frame environment that is not yet current.
    pub fn bind(&mut self, env: &Env, name: &str, val: Value) {
        bind_in(&mut self.heap, env, name, val);
    }

    /// Bind `name` in a binding environment (`<<-`): the first enclosing frame
    /// that already has it, else the global environment.
    pub fn set_super(&mut self, name: &str, val: Value) {
        let mut e = self.env().borrow().parent.clone();
        while let Some(cur) = e {
            let has = cur.borrow().vars.contains_key(name);
            if has {
                bind_in(&mut self.heap, &cur, name, val);
                return;
            }
            e = cur.borrow().parent.clone();
        }
        let g = self.global.clone();
        bind_in(&mut self.heap, &g, name, val);
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
            let call = self.current_call_source();
            self.set_error_call(call);
            self.error = Some(msg.into());
        }
        None
    }

    /// Decide which call the pending error reports. The first site to decide
    /// wins — `stop()` names its caller, a primitive that failed by returning
    /// `Err` names itself, and neither may be overwritten by a frame the unwind
    /// merely passes through.
    pub fn set_error_call(&mut self, call: Option<String>) {
        if self.error_call_known {
            return;
        }
        self.error_trace = self
            .calls
            .iter()
            .filter(|c| c.closure)
            .map(|c| ctx_source(&c.text))
            .collect();
        self.error_call = call;
        self.error_call_known = true;
    }

    /// Give the decision back: a handler took the error, so the next one starts
    /// over.
    pub fn clear_error_call(&mut self) {
        self.error_call = None;
        self.error_trace.clear();
        self.error_call_known = false;
    }

    /// Take and clear a pending error.
    pub fn take_error(&mut self) -> Option<String> {
        self.error.take()
    }
}

/// Parse a numeric literal the way `as.numeric` does, including `Inf`/`NaN`.
/// An unboxed length-1 atomic carried directly on the fusevm stack (no RHost
/// heap object), used for scalar-heavy code like `for`/`while` loop bodies.
/// `Value::Int/Float/Bool` are R's length-1 integer/double/logical with no
/// attributes; every host accessor maps them to that vector view so any builtin
/// consumes them transparently.
enum Ub {
    I(i64),
    F(f64),
    B(bool),
}

fn unboxed(v: &Value) -> Option<Ub> {
    match v {
        Value::Int(n) => Some(Ub::I(*n)),
        Value::Float(f) => Some(Ub::F(*f)),
        Value::Bool(b) => Some(Ub::B(*b)),
        _ => None,
    }
}

fn parse_num(s: &str) -> Option<f64> {
    match s {
        "Inf" => Some(f64::INFINITY),
        "-Inf" => Some(f64::NEG_INFINITY),
        "NaN" => Some(f64::NAN),
        _ => {
            // R accepts C-style hexadecimal (`0x1F`, `-0xff`) in numeric coercion.
            if let Some(hex) = s
                .strip_prefix("0x")
                .or_else(|| s.strip_prefix("0X"))
                .or_else(|| s.strip_prefix("-0x").or_else(|| s.strip_prefix("-0X")))
            {
                let neg = s.starts_with('-');
                return i64::from_str_radix(hex, 16)
                    .ok()
                    .map(|n| if neg { -n } else { n } as f64);
            }
            s.parse().ok()
        }
    }
}

thread_local! {
    /// Significant digits for numeric printing — R's `getOption("digits")`,
    /// default 7. `print(x, digits = n)` overrides it for one call.
    static PRINT_DIGITS: std::cell::Cell<usize> = const { std::cell::Cell::new(7) };
    /// R's `getOption("scipen")`, default 0: the penalty added to the width of
    /// a scientific rendering before it is compared with the fixed one, so a
    /// positive value biases towards fixed and a negative one towards
    /// scientific. R clamps it below at -9.
    static PRINT_SCIPEN: std::cell::Cell<i32> = const { std::cell::Cell::new(0) };
}

/// Significant digits `as.character`, `paste` and `toString` render doubles at.
/// Unlike `print`/`format`, these do NOT follow `getOption("digits")`: R fixes
/// them at 15, which is why `as.character(pi)` is "3.14159265358979" whatever
/// `digits` is set to. They do still follow `scipen`.
pub const AS_CHARACTER_DIGITS: usize = 15;

/// R's lower clamp on `scipen`; `options(scipen = -10)` warns and uses -9.
pub const SCIPEN_MIN: i32 = -9;

/// The current significant-digit setting for numeric printing.
pub fn print_digits() -> usize {
    PRINT_DIGITS.with(|c| c.get())
}

/// Set the significant-digit setting; returns the previous value so a caller can
/// restore it after a one-off `print(x, digits = n)`.
pub fn set_print_digits(d: usize) -> usize {
    PRINT_DIGITS.with(|c| c.replace(d.max(1)))
}

/// The current fixed-versus-scientific bias.
pub fn print_scipen() -> i32 {
    PRINT_SCIPEN.with(|c| c.get())
}

/// Set the fixed-versus-scientific bias, clamped the way R clamps it; returns
/// the previous value so a caller can restore it.
pub fn set_print_scipen(s: i32) -> i32 {
    PRINT_SCIPEN.with(|c| c.replace(s.max(SCIPEN_MIN)))
}

/// Run `f` with `digits` temporarily set, restoring the previous value after.
/// Every one-off digit override (`print(x, digits =)`, `format(x, digits =)`,
/// the fixed-15 `as.character` family) goes through here so none of them can
/// leak into `getOption("digits")`.
pub fn with_print_digits<T>(digits: usize, f: impl FnOnce() -> T) -> T {
    let prev = set_print_digits(digits);
    let out = f();
    set_print_digits(prev);
    out
}

/// A value held in R's global options list. `options` accepts any name, so the
/// store has to hold whatever an unrecognised one was set to as well as the
/// settings rlang acts on.
#[derive(Clone, Debug, PartialEq)]
pub enum OptionValue {
    Int(i64),
    Dbl(f64),
    Str(String),
    Lgl(bool),
}

thread_local! {
    /// Options other than `digits` and `scipen`. Those two are NOT kept here:
    /// they live in `PRINT_DIGITS` / `PRINT_SCIPEN`, which the formatting code
    /// reads once per value, and `option_get`/`option_set` route their names to
    /// those cells. Keeping one store per setting is what stops the two from
    /// drifting apart.
    static OTHER_OPTIONS: std::cell::RefCell<std::collections::HashMap<String, OptionValue>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// R's legal range for `options(digits = )`; outside it R raises
/// "invalid 'digits' parameter, allowed 1...22".
pub const DIGITS_MIN: i64 = 1;
pub const DIGITS_MAX: i64 = 22;

/// The current value of option `name`, or `None` if it has never been set.
pub fn option_get(name: &str) -> Option<OptionValue> {
    match name {
        "digits" => Some(OptionValue::Int(print_digits() as i64)),
        "scipen" => Some(OptionValue::Int(print_scipen() as i64)),
        // `warn` reads as 0 until something sets it, the way R's does.
        "warn" => Some(
            OTHER_OPTIONS
                .with(|o| o.borrow().get(name).cloned())
                .unwrap_or(OptionValue::Int(0)),
        ),
        _ => OTHER_OPTIONS.with(|o| o.borrow().get(name).cloned()),
    }
}

/// Set option `name`, returning its previous value and any warning text the
/// caller should emit. `digits` and `scipen` are validated the way R validates
/// them: `digits` outside 1..22 is an error, `scipen` below -9 warns and clamps.
/// The warning is handed back rather than printed here so that emitting it stays
/// with the builtin layer that owns R's warning format.
pub fn option_set(
    name: &str,
    value: OptionValue,
) -> Result<(Option<OptionValue>, Option<String>), String> {
    let prev = option_get(name);
    let mut warning = None;
    match name {
        "digits" => {
            let d = match value {
                OptionValue::Int(n) => n,
                OptionValue::Dbl(x) if x.is_finite() => x.trunc() as i64,
                _ => return Err("invalid 'digits' parameter, allowed 1...22".into()),
            };
            if !(DIGITS_MIN..=DIGITS_MAX).contains(&d) {
                return Err("invalid 'digits' parameter, allowed 1...22".into());
            }
            set_print_digits(d as usize);
        }
        "scipen" => {
            let s = match value {
                OptionValue::Int(n) => n,
                OptionValue::Dbl(x) if x.is_finite() => x.trunc() as i64,
                _ => return Err("invalid 'scipen' parameter".into()),
            };
            let s = s.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
            if s < SCIPEN_MIN {
                warning = Some(format!("invalid 'scipen' {s}, used {SCIPEN_MIN}"));
            }
            set_print_scipen(s);
        }
        _ => {
            OTHER_OPTIONS.with(|o| o.borrow_mut().insert(name.to_string(), value));
        }
    }
    Ok((prev, warning))
}

/// R's fixed-versus-scientific choice, given the two candidate widths.
///
/// R picks fixed exactly when `width(fixed) <= width(scientific) + scipen`, so
/// ties go to fixed and `scipen` shifts the threshold. Verified against R 4.6.1
/// over 57354 comparisons — every `digits` in 1..22 crossed with `scipen` in
/// {-9,-5,-2,-1,0,1,2,3,5,10,50} over 237 values spanning 1e-300 .. 1e300 and
/// both signs — with zero mismatches, taking R's own `format(x, scientific =
/// FALSE)` and `format(x, scientific = TRUE)` as the two widths so the test
/// assumed nothing about how either rendering is built.
///
/// This is the single place the choice is made; callers must not compare the
/// two widths themselves, or `scipen` silently stops applying at that site.
pub fn prefers_fixed(fixed_width: usize, sci_width: usize) -> bool {
    (fixed_width as i64) <= sci_width as i64 + print_scipen() as i64
}

/// Decimal places a fixed-notation rendering of `x` needs at the current
/// significant-digit setting, with trailing zeros dropped.
pub fn fixed_decimals(x: f64) -> usize {
    if !x.is_finite() || (x == x.trunc() && x.abs() < 1e15) {
        return 0;
    }
    let mag = x.abs().log10().floor() as i32;
    // Enough places to keep `print_digits()` significant digits even for tiny
    // magnitudes: a 15-place cap rounded values like `1e-17` down to `0`, so
    // `format_dbl` then judged fixed "0" narrower than scientific and printed
    // the wrong, lossy form. The double exponent range bounds the real need.
    let sig = print_digits() as i32;
    let d = ((sig - 1) - mag).clamp(0, 340) as usize;
    let s = format!("{x:.d$}");
    let trimmed = s.trim_end_matches('0');
    match trimmed.split_once('.') {
        Some((_, frac)) => frac.len(),
        None => 0,
    }
}

/// `x` in R's scientific spelling at `decimals` mantissa places: mantissa, `e`,
/// an explicit sign, and at least two exponent digits (`1e+05`, `1.5e-05`).
///
/// The mantissa comes from Rust's `{:.*e}`, which rounds correctly from the
/// exact binary value. The previous approach — dividing by `10f64.powi(mag)`
/// and formatting the quotient — was lossy, and the loss surfaced as soon as
/// `digits` could be set above the default: at `digits = 16`, `6.022e23`
/// divided down to `6.022000000000001`, so no trailing zeros trimmed and R's
/// `6.022e+23` printed as `6.022000000000001e+23`. Correct rounding also agrees
/// with the platform's `snprintf` (checked directly against a C `%.20e`), which
/// is the renderer R's own `formatReal` reaches for.
fn r_sci(x: f64, decimals: usize) -> String {
    let s = format!("{x:.decimals$e}");
    // Rust writes the exponent bare (`1.5e-5`); R pads it to two digits and
    // always signs it. `{:e}` already normalises the mantissa into [1, 10) and
    // carries `9.9999999` up to `1e1`, so nothing else needs adjusting.
    let (mant, exp) = s.split_once('e').unwrap_or((s.as_str(), "0"));
    let e: i32 = exp.parse().unwrap_or(0);
    format!("{mant}e{}{:02}", if e < 0 { '-' } else { '+' }, e.abs())
}

/// Mantissa decimal places a scientific rendering of `x` needs at the current
/// significant-digit setting, with trailing zeros dropped (`1e+05` needs none,
/// `1.5e-05` needs one).
pub fn sci_decimals(x: f64) -> usize {
    if !x.is_finite() || x == 0.0 {
        return 0;
    }
    let prec = print_digits().saturating_sub(1);
    let s = r_sci(x, prec);
    let mant = s.split_once('e').map_or(s.as_str(), |(m, _)| m);
    match mant.trim_end_matches('0').split_once('.') {
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
    // R never prints a negative zero: `0 * -2` is `0`, not `-0`. `-0.0 == 0.0`,
    // so this collapses the sign without touching any nonzero value.
    let x = if x == 0.0 { 0.0 } else { x };
    format!("{x:.decimals$}")
}

/// Scientific rendering with a given mantissa width, in R's shape: at least two
/// exponent digits and an explicit sign (`1e+05`, `1.5e-05`).
pub fn render_sci(x: f64, decimals: usize) -> String {
    if !x.is_finite() {
        return render_fixed(x, 0);
    }
    // `%e` already normalises the mantissa into [1, 10), carries `9.9999999`
    // up to `1e+01` rather than leaving `10e+00`, and pads the exponent to two
    // digits — so there is nothing left to adjust here.
    r_sci(if x == 0.0 { 0.0 } else { x }, decimals)
}

/// Format one double the way R prints it: at `getOption("digits")` significant
/// digits, taking fixed or scientific notation by [`prefers_fixed`] — which is
/// why `1e5` prints as `1e+05` but `123456789` stays fixed at the default
/// `scipen = 0`, and why raising `scipen` turns the first into `100000`.
pub fn format_dbl(x: f64) -> String {
    let fixed = render_fixed(x, fixed_decimals(x));
    if !x.is_finite() {
        return fixed;
    }
    let sci = render_sci(x, sci_decimals(x));
    if prefers_fixed(fixed.chars().count(), sci.chars().count()) {
        fixed
    } else {
        sci
    }
}

// ===========================================================================
// Running chunks: the program, closure bodies, loop bodies.
// ===========================================================================

/// Register every rlang builtin on a fresh VM and run `chunk` on it.
pub fn run_chunk(chunk: Chunk) -> Result<Value, String> {
    let mut vm = fresh_vm(chunk);
    let r = drive(&mut vm);
    // A top-level or one-shot chunk has no owner to be cached against, so the
    // VM is dropped; its cost was the allocation, which the closure cache is
    // what avoids.
    r
}

thread_local! {
    /// One parked VM per closure id, each still holding that closure's chunk.
    ///
    /// Every R call runs its body on a nested VM, and building one is not cheap:
    /// `VM::new` allocates the stack, frame and globals vectors and a JIT
    /// compiler, [`crate::builtins::install`] registers the whole builtin table
    /// on top, and the body chunk has to be *cloned* in because the VM owns it.
    /// A closure called in a loop paid all of that per iteration. Parking the VM
    /// under its closure id means the next call to the same closure reuses the
    /// allocations, the builtin table and the chunk already in place —
    /// `VM::reset` keeps the table and the hooks, and the chunk is moved out and
    /// straight back rather than cloned.
    ///
    /// A recursive (or otherwise re-entrant) call finds the slot empty, because
    /// the outer call took the VM out for the duration, and builds a fresh one —
    /// which is what keeps each activation's chunk state its own.
    /// Indexed by closure id — the ids are dense and small, so a vector beats a
    /// map, and the `Box` means parking and taking a VM moves a pointer rather
    /// than the whole `VM` struct.
    static CLOSURE_VMS: RefCell<Vec<Option<Box<VM>>>> = const { RefCell::new(Vec::new()) };
}

/// A VM ready to run `chunk`: allocations, builtin table and hooks all in place.
fn fresh_vm(chunk: Chunk) -> VM {
    let mut vm = VM::new(chunk);
    crate::builtins::install(&mut vm);
    vm
}

/// Run closure `id`'s body on its parked VM, parking it again afterwards.
fn run_closure_chunk(id: usize) -> Result<Value, String> {
    let parked = CLOSURE_VMS.with(|c| c.borrow_mut().get_mut(id).and_then(Option::take));
    let mut vm = match parked {
        Some(mut vm) => {
            // The chunk is already this closure's: move it out and hand it back
            // to `reset`, which clears the stack, frames, globals and ip while
            // keeping the builtin table and the numeric hook.
            let chunk = std::mem::take(&mut vm.chunk);
            vm.reset(chunk);
            vm
        }
        // First call, or a re-entrant one whose VM is checked out: this is the
        // only path that copies the body.
        None => {
            let chunk = with_host(|h| h.closures.get(id).map(|d| d.chunk.clone()))
                .ok_or_else(|| format!("internal: unknown closure #{id}"))?;
            Box::new(fresh_vm(chunk))
        }
    };
    let r = drive(&mut vm);
    CLOSURE_VMS.with(|c| {
        let mut slots = c.borrow_mut();
        if slots.len() <= id {
            slots.resize_with(id + 1, || None);
        }
        slots[id] = Some(vm);
    });
    r
}

/// Run a prepared VM to completion and read its outcome.
fn drive(vm: &mut VM) -> Result<Value, String> {
    // An `OPEN_CALL` in this chunk whose `CALL_OPENED` never ran — an argument
    // errored, or a `return`/`break` jumped past it — would leave its context
    // behind. Unwinding lands here, so the stack is cut back to the depth this
    // chunk started at however it ends.
    let depth = with_host(|h| h.calls.len());
    let out = drive_inner(vm);
    with_host(|h| h.calls.truncate(depth));
    out
}

fn drive_inner(vm: &mut VM) -> Result<Value, String> {
    // The tracing JIT is deliberately NOT enabled. It cannot compile R: fusevm's
    // tracer rejects any trace containing `Op::CallBuiltin` (builtin bodies are
    // arbitrary Rust it can't lower to Cranelift IR), and R lowers nearly every
    // operation — element fetch, comparisons, `%%`, indexing — to a builtin. So
    // for R the tracer never produces native code, yet its per-backward-branch
    // trace-cache lookup (a hashed probe) runs every loop iteration. Profiling a
    // scalar loop showed ~12% of time in that bookkeeping; turning the tracer off
    // is a measured ~12% speedup with zero loss of compilation (there was none).
    // AOT (`--aot`) is a separate Cranelift path and is unaffected.
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
    // A program that ended normally has already flushed after its last
    // statement; one that unwound has not, and R prints that leftover batch
    // *after* the error line — so leave it to the caller (see `main.rs`).
    if r.is_ok() {
        flush_warnings(false);
    }
    r
}

/// R's `nwarnings`: the most warnings one top-level statement keeps.
pub const MAX_WARNINGS: usize = 50;

/// Queue `msg` for the end of the current top-level statement.
///
/// `options(warn = 0)` (the default) defers; a positive `warn` prints at once;
/// a negative one drops the warning entirely.
pub fn queue_warning(msg: &str, call: Option<String>) {
    match warn_level() {
        w if w < 0 => {}
        0 => with_host(|h| {
            h.warn_count += 1;
            if h.warnings.len() < MAX_WARNINGS {
                h.warnings.push(Warning {
                    msg: msg.to_string(),
                    call,
                });
            }
        }),
        // `warn = 1` prints where the warning happens, under its own banner.
        // R's `vwarningcall_dflt` allows 18 columns for the `Warning in ` and
        // ` : ` around the call before deciding whether the message still fits.
        _ => {
            let text = match &call {
                None => format!("Warning: {msg}\n"),
                Some(c) => format!("Warning in {c} :{}", fold(c, msg, 18)),
            };
            emit_stderr(&text);
        }
    }
}

/// R's `LONGWARN`: the column past which a warning's first line is folded onto
/// the next, indented.
const LONGWARN: usize = 75;

/// The tail of a warning line, from just after `<call> :` — either ` msg` on
/// the same line, or a fold onto the next line indented two spaces. R keeps the
/// message on the line when `overhead + width(call) + width(msg's first line)`
/// fits in [`LONGWARN`]; `overhead` is R's allowance for the decoration around
/// the call, which differs per banner.
fn fold(call: &str, msg: &str, overhead: usize) -> String {
    let first = msg.split('\n').next().unwrap_or(msg);
    let width = |s: &str| crate::strwidth::display_width(s);
    if overhead + width(call) + width(first) > LONGWARN {
        format!("\n  {msg}\n")
    } else {
        format!(" {msg}\n")
    }
}

/// Print and clear the warnings the finished statement queued, in R's three
/// shapes: one warning under a singular banner, up to ten numbered under a
/// plural one, and more than that as a count. `after_error` adds the
/// `In addition: ` R prefixes the batch with when the statement also failed.
pub fn flush_warnings(after_error: bool) {
    let (msgs, count) = with_host(|h| {
        (
            std::mem::take(&mut h.warnings),
            std::mem::replace(&mut h.warn_count, 0),
        )
    });
    if count == 0 {
        return;
    }
    let lead = if after_error { "In addition: " } else { "" };
    // R's `PrintWarnings` allows 6 columns of decoration around the call for a
    // lone warning and 10 for a numbered one — the extra 4 being the `N: `
    // prefix — before deciding whether the message still fits on the line.
    let text = if count == 1 {
        format!("{lead}Warning message:\n{}", warning_line(&msgs[0], 6))
    } else if count <= 10 {
        let body: String = msgs
            .iter()
            .enumerate()
            .map(|(i, w)| format!("{}: {}", i + 1, warning_line(w, 10)))
            .collect();
        format!("{lead}Warning messages:\n{body}")
    } else if count < MAX_WARNINGS {
        format!("{lead}There were {count} warnings (use warnings() to see them)\n")
    } else {
        format!(
            "{lead}There were {MAX_WARNINGS} or more warnings (use warnings() to see the first {MAX_WARNINGS})\n"
        )
    };
    emit_stderr(&text);
}

/// An uncaught error as R's `verrorcall_dflt` writes it: `Error in <call> : `
/// then the message, folded onto the next line and indented two spaces when the
/// line would run past `LONGWARN`. R allows 14 columns for the decoration
/// around the call. Without a call it is a bare `Error: `, which is what a
/// `stop()` at top level produces.
pub fn error_report(msg: &str, call: Option<&str>) -> String {
    // One line, like every other diagnostic: the call is held as whole source
    // for the condition object's sake.
    match call.and_then(|c| c.lines().next()) {
        None => format!("Error: {msg}\n"),
        // The trailing space after the colon is R's, and survives the fold.
        Some(c) => format!("Error in {c} : {}", fold_error(c, msg)),
    }
}

/// R's `R_NShowCalls`: how much of the traceback is shown before its middle is
/// elided.
const N_SHOW_CALLS: usize = 50;

/// R's `R_ConciseTraceback` — the `Calls:` line under an uncaught error.
///
/// `frames` is the deparsed call of every function context the error was raised
/// under, innermost last; `call` is the one the error itself reports. The chain
/// reads outermost first and names functions, not calls: a callee that is not a
/// plain symbol shows as `<Anonymous>`.
///
/// `stop`, `warning` and the suppress wrappers clear everything collected
/// *inside* them, which is what keeps `stop()`'s own frame and the machinery
/// under it out of the chain. A chain naming nothing but the call the error
/// already reported is dropped — R does not repeat itself — and a long one is
/// elided in the middle, keeping the outermost frame and the innermost few.
pub fn traceback(frames: &[String], call: Option<&str>) -> Option<String> {
    // Collected innermost-first and reversed at the end, so a reset drops
    // exactly what was seen inside the frame that triggered it.
    let mut names: Vec<String> = Vec::new();
    let mut ncalls = 0usize;
    let mut top: Option<String> = None;
    let mut elided = false;
    for text in frames.iter().rev() {
        let this = call_name(text);
        if matches!(
            this.as_str(),
            "stop" | "warning" | "suppressWarnings" | ".signalSimpleWarning"
        ) {
            names.clear();
            ncalls = 0;
            top = None;
            elided = false;
            continue;
        }
        ncalls += 1;
        if elided || names.join(" -> ").len() > N_SHOW_CALLS {
            elided = true;
            top = Some(this);
        } else {
            names.push(this);
        }
    }
    let mut out = names
        .iter()
        .rev()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(" -> ");
    if elided {
        out = match top.filter(|t| t.len() < 50) {
            Some(t) => format!("{t} ... {out}"),
            None => format!("... {out}"),
        };
    }
    if ncalls == 1 && call.map(call_name).as_deref() == Some(out.as_str()) {
        return None;
    }
    (!out.is_empty()).then_some(out)
}

/// The function name a traceback shows for a call: the callee when it is a
/// plain symbol, `<Anonymous>` for anything else — a call in function position,
/// a function literal. Read off the deparsed text, where the callee is
/// everything before the argument list.
fn call_name(text: &str) -> String {
    let head = text.lines().next().unwrap_or_default();
    let head = head.split('(').next().unwrap_or_default();
    let head = head
        .strip_prefix('`')
        .and_then(|h| h.strip_suffix('`'))
        .unwrap_or(head);
    match crate::deparse::is_syntactic_name(head) {
        true => head.to_string(),
        false => "<Anonymous>".into(),
    }
}

/// The message half of an `Error in <call> : ` line — R's `tail` is `"\n  "`,
/// appended before the message when the first line would not fit.
fn fold_error(call: &str, msg: &str) -> String {
    let first = msg.split('\n').next().unwrap_or(msg);
    let width = |s: &str| crate::strwidth::display_width(s);
    if 14 + width(call) + width(first) > LONGWARN {
        format!("\n  {msg}\n")
    } else {
        format!("{msg}\n")
    }
}

/// One queued warning as `PrintWarnings` writes it: the message followed by a
/// space when it carries no call, and `In <call> : <message>` when it does.
fn warning_line(w: &Warning, overhead: usize) -> String {
    match &w.call {
        None => format!("{} \n", w.msg),
        Some(c) => format!("In {c} :{}", fold(c, &w.msg, overhead)),
    }
}

/// The current `options(warn = )`, which defaults to 0 as it does in R.
fn warn_level() -> i64 {
    match option_get("warn") {
        Some(OptionValue::Int(n)) => n,
        Some(OptionValue::Dbl(x)) if x.is_finite() => x.trunc() as i64,
        _ => 0,
    }
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
        RData::Combinator { kind, inner } => crate::builtins::call_combinator(kind, &inner, args),
        // A foreign R function handle (R6 method, package closure) is invoked in
        // embedded R with marshalled arguments.
        #[cfg(not(target_arch = "wasm32"))]
        RData::RForeign(ptr) => crate::rembed::call_handle(ptr, &args),
        _ => Err("attempt to apply non-function".into()),
    }
}

/// The call-depth ceiling. R stops at `options(expressions = 5000)` with
/// "evaluation nested too deeply"; rlang stops at the same place so runaway
/// recursion reports an R error instead of exhausting the native stack (each R
/// call runs its body on a nested VM, so depth costs real stack).
pub const MAX_DEPTH: usize = 5000;

/// Call a closure: match the arguments to the formals, push a frame whose
/// environment encloses the closure's captured environment, and run the body.
pub fn call_closure(
    id: usize,
    env: Env,
    args: Vec<(Option<String>, Value)>,
    fun_name: Option<String>,
) -> Result<Value, String> {
    // Only the formals are needed to enter the call; the body chunk stays in the
    // closure table (cloning the whole `ClosureDef` copied the bytecode on every
    // call, which for a closure called in a loop was the largest single cost of
    // calling it).
    let params = with_host(|h| h.closures.get(id).map(|d| d.params.clone()))
        .ok_or_else(|| format!("internal: unknown closure #{id}"))?;
    if with_host(|h| h.frames.len()) >= MAX_DEPTH {
        return Err("evaluation nested too deeply: infinite recursion?".into());
    }
    let frame_env = new_env(Some(env.clone()));
    let bindings = match_args(&params, &args)?;
    with_host(|h| {
        for (k, v) in bindings {
            h.bind(&frame_env, &k, v);
        }
    });
    with_host(|h| {
        // The frame keeps the evaluated arguments for `UseMethod`/`NextMethod`
        // to re-dispatch on, so they are durable references for as long as the
        // call runs — R's promise holds the same reference.
        for (_, v) in &args {
            retain(&mut h.heap, v);
        }
        // The dispatch that selected this method, if it was one — set just
        // before the call so `NextMethod` in the body can continue it.
        let dispatch = h.pending_dispatch.take();
        h.frames.push(Frame {
            env: frame_env,
            args,
            fun_name,
            fun: Some((id, env)),
            dispatch,
            on_exit: Vec::new(),
        })
    });
    let out = run_closure_chunk(id);
    // `on.exit` runs on the way out however the frame is left — normal return,
    // `return()`, or an error unwinding through it — so it is taken off the
    // frame before the result is propagated.
    let exits = with_host(|h| {
        let e = h.frames.last_mut().map(|f| std::mem::take(&mut f.on_exit));
        // Leaving the frame gives back every reference it held: its arguments
        // always, and its bindings when the environment dies with it — which it
        // does unless a closure defined inside captured it, or `environment()`
        // handed it out, in which case another `Rc` is still alive.
        if let Some(f) = h.frames.pop() {
            for (_, v) in &f.args {
                release(&mut h.heap, v);
            }
            if Rc::strong_count(&f.env) == 1 {
                let held: Vec<Value> = f.env.borrow().vars.values().cloned().collect();
                for v in &held {
                    release(&mut h.heap, v);
                }
            }
        }
        e.unwrap_or_default()
    });
    // `run_chunk` has already moved any error out of the host and into `out`,
    // so the cleanups run on a clean slate. An error raised by a cleanup
    // propagates only when the body itself succeeded — otherwise the body's
    // error is the one that reaches the caller, as in R.
    let mut out = out;
    for thunk in exits {
        // A `return()` signal from the body must not halt the cleanup chunk, and
        // whether the *call* prints is decided by its own value — a `cat` in the
        // cleanup must not make the result invisible.
        let (sig, vis) = with_host(|h| (h.signal.take(), h.visible));
        let cleanup = call_value(&thunk, Vec::new(), None);
        with_host(|h| {
            h.signal = sig;
            h.visible = vis;
        });
        if out.is_ok() {
            if let Err(e) = cleanup {
                out = Err(e);
            }
        }
    }
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

/// Which formal each supplied argument binds to — R's argument matching, in
/// R's own order: an exact tag first, then a partial tag against the formals
/// before `...`, then position. `None` for an argument that matched no formal
/// and so belongs to `...`.
///
/// The rule lives here once because two callers need the same answer about
/// different things: [`match_args`] binds *values* to formals, and
/// `match.call()` re-tags the *expressions* the caller wrote.
pub fn match_positions(
    params: &[String],
    tags: &[Option<String>],
) -> Result<Vec<Option<usize>>, String> {
    let mut to = vec![None; tags.len()];
    let mut taken = vec![false; params.len()];
    let dots_at = params.iter().position(|p| p == "...");

    // 1. exact tag matches
    for (ai, tag) in tags.iter().enumerate() {
        let Some(tag) = tag else { continue };
        if let Some(pi) = params.iter().position(|p| p == tag) {
            if taken[pi] {
                return Err(format!(
                    "formal argument \"{tag}\" matched by multiple actual arguments"
                ));
            }
            taken[pi] = true;
            to[ai] = Some(pi);
        }
    }
    // 2. partial tag matches, only against formals before `...`
    let partial_limit = dots_at.unwrap_or(params.len());
    for (ai, tag) in tags.iter().enumerate() {
        if to[ai].is_some() {
            continue;
        }
        let Some(tag) = tag else { continue };
        let hits: Vec<usize> = (0..partial_limit)
            .filter(|&pi| !taken[pi] && params[pi].starts_with(tag.as_str()))
            .collect();
        match hits.len() {
            0 => {}
            1 => {
                taken[hits[0]] = true;
                to[ai] = Some(hits[0]);
            }
            _ => return Err(format!("argument {tag} matches multiple formal arguments")),
        }
    }
    // 3. positional fill, stopping at `...` — everything past it is variadic
    let mut pi = 0usize;
    for (ai, tag) in tags.iter().enumerate() {
        if to[ai].is_some() || tag.is_some() {
            continue;
        }
        while pi < params.len() && taken[pi] && params[pi] != "..." {
            pi += 1;
        }
        if pi >= params.len() || params[pi] == "..." {
            break;
        }
        taken[pi] = true;
        to[ai] = Some(pi);
        pi += 1;
    }
    Ok(to)
}

/// Bind a call's arguments to a closure's formals, R's way. Unmatched
/// arguments go to `...` when the closure has one and are an error when it does
/// not.
pub fn match_args(
    params: &[String],
    args: &[(Option<String>, Value)],
) -> Result<Vec<(String, Value)>, String> {
    let tags: Vec<Option<String>> = args.iter().map(|(t, _)| t.clone()).collect();
    let to = match_positions(params, &tags)?;
    let mut bound: Vec<Option<Value>> = vec![None; params.len()];
    for (ai, slot) in to.iter().enumerate() {
        if let Some(pi) = slot {
            bound[*pi] = Some(args[ai].1.clone());
        }
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
    // leftovers → `...`, or an error
    let rest: Vec<(Option<String>, Value)> = args
        .iter()
        .zip(&to)
        .filter(|(_, slot)| slot.is_none())
        .map(|(a, _)| a.clone())
        .collect();
    if params.iter().any(|p| p == "...") {
        let dots = with_host(|h| h.alloc(RData::Args(rest)));
        out.push(("...".to_string(), dots));
    } else if !rest.is_empty() {
        // R names the unused arguments as they were *written* — `f(1, x + y)`
        // reports `unused argument (x + y)` — so the text comes from the call on
        // the context stack, and the tag is put back in front of it.
        let written = with_host(|h| h.current_call_arg_sources());
        let named: Vec<String> = to
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.is_none())
            .map(|(ai, _)| {
                let src = written.get(ai).cloned().unwrap_or_default();
                match &args[ai].0 {
                    Some(t) => format!("{t} = {src}"),
                    None => src,
                }
            })
            .collect();
        let plural = if named.len() > 1 { "s" } else { "" };
        return Err(format!("unused argument{plural} ({})", named.join(", ")));
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
