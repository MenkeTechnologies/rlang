# Known gaps

The honest list of what rlang does **not** do yet. Nothing here is faked as
working: calling an unimplemented primitive raises `could not find function`,
and two harnesses diff against the reference `Rscript` rather than against a
self-recorded baseline — `cargo run --bin parity` on a hand-authored corpus, and
`cargo run --bin parity-fuzz` on thousands of generated snippets across 58
surfaces. The fuzzer reports one known gap class, listed with its reasoning in
`tests/data/parity_fuzz_baseline.txt` (R's `R_Visible` rules — a bare numeric
literal in statement position lowers to a native VM op that never enters an
rlang builtin, so there is no hook on which to re-set the flag); everything else
is at parity. What remains below is structural — whole subsystems, not
per-primitive gaps.

## Evaluation model

- **Arguments are evaluated eagerly, not as promises**, so `substitute()`,
  `quote()`, `match.call()`, `sys.call()`, and `deparse()` of an unevaluated
  expression are absent from rlang's own evaluator. Non-standard-evaluation
  *programs* (`dplyr::filter(df, x > 2)`, `data.table` `[`, `subset`) still run:
  when rlang cannot evaluate a script, the whole thing is re-run in the embedded
  GNU R (needs R installed), so the answer is correct even though rlang's JIT
  didn't produce it. Set `RLANG_NO_CRAN=1` to force the native path only.
  Defaults behave lazily — they compile into a body prologue
  (`if (missing(p)) p <- <default>`), so a default may refer to another argument.
- **The condition system catches, but records no call and has no restarts.**
  `tryCatch` selects a handler by condition class (`error`, `warning`,
  `message`, `condition`), `finally` runs either way, and `try` returns a
  `"try-error"` string. `on.exit` runs when a frame is left, however it is left.
  `stop`, `warning`, `message` and `signalCondition` raise real condition
  objects, and `conditionMessage` / `simpleError` / `simpleCondition` build and
  read them. `warning()` and `message()` still print and continue when nothing
  is waiting to catch them, which is R's default action. Two gaps: a condition
  carries **no `call`**, because the body reaching a builtin is a value rather
  than an expression — so `conditionCall` is always `NULL`, `print(cond)` is
  `<simpleError: msg>` rather than `<simpleError in f(): msg>`, and `try`'s
  string is R's call-less `"Error : msg\n"` rather than
  `"Error in f() : msg\n"`. And there are **no restarts**
  (`withRestarts`, `invokeRestart`, `muffleWarning`), so
  `withCallingHandlers` unwinds like `tryCatch` instead of resuming.
- **`local()` works; the rest of the environment surface does not.**
  `local(expr)` compiles to `(function() expr)()`, which is R's own definition,
  so it gets a fresh environment enclosing the caller's. `sys.function()`,
  `parent.frame()` and `eval(expr, envir)` are still missing.
- **Formulas (`~`) parse and become real formula objects** — `lhs ~ rhs` is
  deparsed to R source and built in the CRAN bridge, so `lm(y ~ x, data = df)`,
  `aggregate(v ~ g, df, sum)`, and one-sided `~ x` work. A formula referencing a
  bare rlang variable (`lm(y ~ x)` with `x` defined only in rlang) can't see it —
  pass the data explicitly, or use literal vectors.
- **No environments as first-class manipulation targets** beyond `new.env()`,
  `environment()`, `local()`, `$`, and `[[` on an environment: `sys.function()`,
  `parent.frame()`, `eval(expr, envir)` are missing.

## Types

- **No *native* data frames / raw vectors / dates / S4 objects — they live in
  the CRAN bridge instead.** rlang has no rlang-side type for these, so a value
  of one is held as an opaque handle to the embedded GNU R (see below), and any
  operation on it (`df$col`, `df[i, ]`, `nrow`, `print`, `toJSON(df)`) is
  delegated there. This needs R installed; the values are correct but not
  inspectable from rlang's own primitives.
- **No complex numbers, no `Date`/`POSIXct` native type.** **Factors are a
  complete subsystem**, including ordered ones. A factor survives being subset
  or reordered — `f[i]` (with `drop =`), `f[[i]]`, `head`/`tail`, `rev`, `sort`,
  `unique`, `rep`, `c`, `split` and the set operators all rebuild the level
  table and class the way R's `[.factor` / `rep.factor` do, rather than handing
  back the bare integer codes. Operators go through R's group generics: `==` and
  `!=` compare *labels*, so `f == "a"` selects the right elements; `<`/`>` on an
  *ordered* factor compare level positions, and on an unordered one answer `NA`
  with R's "not meaningful for factors" warning instead of silently comparing
  codes. Label coercion (`as.vector`, `paste`, `toString`, `match`, `%in%`,
  `split`/`tapply` grouping) reads the labels, `min`/`max`/`range` follow
  `Summary.ordered`, and the type predicates exclude factors the way R's do
  (`is.numeric(f)` and `is.integer(f)` are FALSE). One cosmetic gap remains: the
  `Ops.factor` warning is emitted without R's `In Ops.factor(f, "b") :` call
  prefix, because a builtin receives values rather than expressions and `+`
  lowers to a native fusevm op that never sees the argument text. The message
  body and the returned `NA`s match.
- **N-D arrays** (`array`, N-D `a[i, j, k]` read/write, slice-drop, `, , k`
  printing, `aperm`, `apply` over any margin, and the labels `apply` carries from
  a margin onto its result) work; the array-specific helpers (`slice.index`,
  `arrayInd`) do not.
- **`dimnames` work at any rank**: `matrix(dimnames=)` and `array(dimnames=)`,
  `rbind`/`cbind` carrying an input vector's names onto the cross dimension,
  the `dimnames`/`rownames`/`colnames` accessors, dimname-aware matrix and
  `, , <label>` array printing, character subscripts (`m["r1", "c2"]`, read and
  write) resolved per margin, labels carried onto a subset (as `dimnames` when a
  rank ≥ 2 survives, as `names` when it drops to a vector), and reductions that
  keep a dimension's labels as names (`colSums`/`rowSums`/`colMeans`/`rowMeans`).
  `dimnames(x) <-`, `rownames(x) <-` and `colnames(x) <-` assign them, and
  `apply` carries the margin's labels onto its result. `rbind`/`cbind` synthesise
  R's deparse-derived seam labels (`rbind(x, x)` gives rownames `"x"`, `"x"`) at
  every `deparse.level`: a builtin receives values rather than expressions, so
  the compiler passes the deparsed argument text alongside them. It cannot do
  that through `...` — `rbind(...)` inside a function gets no deparsed labels,
  because the forwarded arguments only exist at run time.
- **Partial linear algebra.** `%*%`, `t`, `diag`, `apply` over margins,
  `rowSums`/`colSums`/`rowMeans`/`colMeans`, `outer`/`%o%`, `crossprod`/
  `tcrossprod`, and `cbind`/`rbind` work; `solve`, `det`, and `eigen` are not
  implemented.
- **Integer overflow wraps to a double** rather than producing `NA` with a
  warning, because arithmetic is computed in `f64` and narrowed back.
- **`%%`/`%/%`, `var`, and `round` differ from R by ULPs at the edge of f64
  precision.** R accumulates them in C `long double`; Rust has no equivalent, so
  a modulus of a value past `2^53` (where R warns of "complete loss of
  accuracy"), a variance landing on a 7th-significant-digit rounding tie, or a
  `round` of an exact `N.NN5` half (`round(0.05, 1)`) can differ in the last
  place. The common cases — including `round(0.15, 1)`, `round(2.675, 2)` — match.

## Printing and formatting

- **`options(digits=, scipen=)` is not implemented**, but `print(x, digits = n)`
  is: it overrides the significant-digit count for that one call and restores the
  default afterwards. The 7-significant-digit default and the `scipen = 0`
  fixed-vs-scientific rule are checked against R by the parity corpus; the global
  `options()` toggles are not configurable.
- **`format()` handles `nsmall`, `digits`, `big.mark`, `width`, `scientific`,
  common decimals, and common-width justification** (and
  `formatC`/`prettyNum`/`deparse` exist), but not the `justify` argument. The
  fixed-versus-scientific choice is the same width rule `print` uses, so
  `format(1e6)` is `"1e+06"` and `big.mark` does not apply to it.
- **A closure prints and deparses its own source.** `print(f)`, `deparse(f)` and
  `format(f)` render it through a port of R's `deparse.c` (`src/deparse.rs`):
  `Rscript` runs with `keep.source = FALSE`, so R re-renders the parse tree
  rather than echoing the original text, and rlang reproduces those layout rules
  — the header on its own line, four-space block indentation, an `if` inside
  `{ }` split across lines, and the 60-column wrap. Two gaps remain. A closure
  whose environment is not the global one omits R's trailing
  `<environment: 0x…>` line, which carries a process address that could not match
  anyway. And a *primitive* prints as `function (...) .Primitive("name")` instead
  of with R's real formals (`function (..., na.rm = FALSE)  .Primitive("sum")`),
  because rlang has no per-builtin formals table — the reference corpus's
  signatures describe what rlang reads, not what R declares. `deparse(sum)` is
  exact.
- **No `str()`, `summary()`, or `dput()`.**

## Syntax

- **`else` may start a new line at top level.** R only allows that inside `{ }`;
  rlang accepts both, so a program R rejects can run here. The parity corpus
  treats "both reject" as parity, so this leniency is visible only for that one
  construct.
- **`?help`, `::` namespaces** — `pkg::name` parses and the qualifier is dropped
  (rlang has one namespace); `?` is lexed and unused.
- **CRAN packages run through an embedded-R bridge, not natively.**
  `library(pkg)` and any package routine (including compiled C/C++/Fortran) are
  delegated to a `dlopen`'d GNU R via FFI (`src/rembed.rs`) — rlang does not
  re-implement the package system or R's C API. This needs a real R install at
  run time; without one, `library` and unknown functions report "could not find
  function" as before. Current marshalling limits: named-list *names* are not yet
  carried into R, and a return value with no rlang representation (S4 object,
  environment, data frame) surfaces as an error rather than a value.

## S3 / S4 / R5

- **`UseMethod` and `NextMethod` both work.** Dispatch records the classes it has
  not tried yet on the method's frame, so `NextMethod()` continues down the class
  vector and ends at `<generic>.default` — or, for the primitives R treats as
  generic (`print`, `format`, `as.character`, `length`, `c`, `sort`, …), at the
  primitive's own implementation. Those primitives hand off to a user's
  `<generic>.<class>` first, so a `print.myclass` takes over both `print(x)` and
  top-level autoprint.
- **No S4 (`setClass`, `setGeneric`, `isVirtualClass`), no Reference Classes,
  no R6.** `@` parses and reads an attribute, which is not S4 slot semantics.
- **No *user-definable* group generics** (`Ops`, `Math`, `Summary`), so a class
  of your own cannot overload `+` through S3. The built-in factor ones
  (`Ops.factor`, `Ops.ordered`, `Summary.ordered`) are implemented natively.

## Runtime

- **No garbage collection.** The `RHost` heap only grows within a run; a
  long-running loop that allocates many vectors will hold all of them until the
  process exits.
- **Closure bodies are cloned per call.** `Chunk` is cloned on entry to every
  call, which costs on deeply recursive workloads.
- **AOP intercepts are a registry, not a weave.** `intercepts::matches()` is live
  and tested; the dispatcher does not consult it yet.
- **The DAP adapter does not step.** The handshake, launch, and
  run-to-completion path with stdout forwarded as `output` events are real;
  breakpoints and stepping are not wired to the fusevm line table yet.
- **Runtime-constructed functions are limited to `Negate()` and `Vectorize()`.**
  Both work — a `Combinator` value wraps the inner function — but there is no
  general first-class function synthesis (`as.function`, `body<-`, `Compose`,
  building a closure from a body expression). `Recall()` re-invokes the executing
  closure.
