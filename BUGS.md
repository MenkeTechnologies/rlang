# Known gaps

The honest list of what rlang does **not** do yet. Nothing here is faked as
working: calling an unimplemented primitive raises `could not find function`,
and two harnesses diff against the reference `Rscript` rather than against a
self-recorded baseline — `cargo run --bin parity` on a hand-authored corpus, and
`cargo run --bin parity-fuzz` on thousands of generated snippets across 21
surfaces. The fuzzer currently reports **zero** divergences across those
surfaces (its baseline in `tests/data/parity_fuzz_baseline.txt` is empty); what
remains below is structural — whole subsystems, not per-primitive gaps.

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
- **No condition system.** `tryCatch`, `withCallingHandlers`, `simpleError`,
  `on.exit`, `signalCondition`, restarts. `stop()` aborts the program and
  `warning()`/`message()` write to stderr, but nothing can catch them.
- **Formulas (`~`) parse and become real formula objects** — `lhs ~ rhs` is
  deparsed to R source and built in the CRAN bridge, so `lm(y ~ x, data = df)`,
  `aggregate(v ~ g, df, sum)`, and one-sided `~ x` work. A formula referencing a
  bare rlang variable (`lm(y ~ x)` with `x` defined only in rlang) can't see it —
  pass the data explicitly, or use literal vectors.
- **No environments as first-class manipulation targets** beyond `new.env()`,
  `environment()`, `$`, and `[[` on an environment: `local()`, `sys.function()`,
  `parent.frame()`, `eval(expr, envir)` are missing.

## Types

- **No *native* data frames / raw vectors / dates / S4 objects — they live in
  the CRAN bridge instead.** rlang has no rlang-side type for these, so a value
  of one is held as an opaque handle to the embedded GNU R (see below), and any
  operation on it (`df$col`, `df[i, ]`, `nrow`, `print`, `toJSON(df)`) is
  delegated there. This needs R installed; the values are correct but not
  inspectable from rlang's own primitives.
- **No complex numbers, no `Date`/`POSIXct` native type.** Factors are
  supported (`factor`, `levels`, `nlevels`, `table`, and their printing), but
  only the default `sort`-ordered levels — no ordered factors.
- **N-D arrays** (`array`, N-D `a[i, j, k]` read/write, slice-drop, `, , k`
  printing, `aperm`, `apply` over any margin) work; named-margin `apply` and
  array-specific helpers (`slice.index`, `arrayInd`) do not.
- **Matrix `dimnames` work for the 2-D common cases**: `matrix(dimnames=)`,
  `rbind`/`cbind` carrying an input vector's names onto the cross dimension,
  the `dimnames`/`rownames`/`colnames` accessors, dimname-aware matrix
  printing, and reductions that keep a dimension's labels as names
  (`colSums`/`rowSums`/`colMeans`/`rowMeans`). Two gaps remain: `rbind(x, x)`
  does not synthesise deparse-derived seam labels (`"x"`, `"x"`) because
  builtins receive argument values, not expressions; and `dimnames` on N-D
  arrays (3-D+) is not stored.
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
- **`format()` handles `nsmall`, `digits`, `big.mark`, common decimals, and
  common-width justification** (and `formatC`/`prettyNum`/`deparse` exist), but
  not the `width`/`justify` arguments or per-call scientific control.
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

- **`NextMethod()` is missing** — dispatch finds the first matching method and
  stops.
- **No S4 (`setClass`, `setGeneric`, `isVirtualClass`), no Reference Classes,
  no R6.** `@` parses and reads an attribute, which is not S4 slot semantics.
- **No group generics** (`Ops`, `Math`, `Summary`), so a class cannot overload
  `+` through S3.

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
