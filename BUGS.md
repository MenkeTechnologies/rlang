# Known gaps

The honest list of what rlang does **not** do yet. Nothing here is faked as
working: calling an unimplemented primitive raises `could not find function`,
and two harnesses diff against the reference `Rscript` rather than against a
self-recorded baseline — `cargo run --bin parity` on a hand-authored corpus, and
`cargo run --bin parity-fuzz` on thousands of generated snippets across 57
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

- **`NextMethod()` is missing** — dispatch finds the first matching method and
  stops. `UseMethod` works, and the primitives R treats as generic (`print`,
  `format`, `as.character`, `length`, `c`, `sort`, …) hand off to a user's
  `<generic>.<class>` before running their own implementation, so a
  `print.myclass` takes over both `print(x)` and top-level autoprint.
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
