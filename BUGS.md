# Known gaps

The honest list of what rlang does **not** do yet. Nothing here is faked as
working: calling an unimplemented primitive raises `could not find function`,
and the parity harness (`cargo run --bin parity`) diffs every corpus snippet
against the reference `Rscript` rather than against a self-recorded baseline.

## Evaluation model

- **Arguments are evaluated eagerly, not as promises.** R passes unevaluated
  promises, which is what makes `substitute()`, `quote()`, `deparse()`,
  `match.call()`, `sys.call()`, and every non-standard-evaluation idiom
  (`subset(df, x > 1)`, formulas, `~`) work. rlang evaluates each argument at the
  call site, so those are absent. Defaults still behave lazily — they compile
  into a body prologue (`if (missing(p)) p <- <default>`), so a default may refer
  to another argument.
- **No condition system.** `tryCatch`, `withCallingHandlers`, `simpleError`,
  `on.exit`, `signalCondition`, restarts. `stop()` aborts the program and
  `warning()`/`message()` write to stderr, but nothing can catch them.
- **No `~` formulas.** The token lexes; nothing consumes it.
- **No environments as first-class manipulation targets** beyond `new.env()`,
  `environment()`, `$`, and `[[` on an environment: `local()`, `sys.function()`,
  `parent.frame()`, `eval(expr, envir)` are missing.

## Types

- **No data frames**, and therefore none of `data.frame`, `subset`, `merge`,
  `aggregate`, `read.csv`, `write.csv`.
- **No factors** (`factor`, `levels`, `table`), no complex numbers, no raw
  vectors, no `Date`/`POSIXct`.
- **No arrays past 2 dimensions.** `dim` of length 2 prints and indexes as a
  matrix; longer `dim` vectors are carried but not honored by indexing or print.
- **No `apply` over matrix margins**, `outer`, `%*%`, `solve`, `crossprod`, or
  any of the linear-algebra surface. `cbind`/`rbind` are not implemented.
- **Integer overflow wraps to a double** rather than producing `NA` with a
  warning, because arithmetic is computed in `f64` and narrowed back.

## Printing and formatting

- **`options(digits=, scipen=)` is not implemented.** The 7-significant-digit
  default and the `scipen = 0` fixed-vs-scientific rule are, and are checked
  against R by the parity corpus, but neither is configurable.
- **`format()` is a thin `as.character`** — no `nsmall`, `width`, `justify`,
  `big.mark`, or scientific control.
- **No `str()`, `summary()`, or `dput()`.**

## Syntax

- **`else` may start a new line at top level.** R only allows that inside `{ }`;
  rlang accepts both, so a program R rejects can run here. The parity corpus
  treats "both reject" as parity, so this leniency is visible only for that one
  construct.
- **`?help`, `::` namespaces** — `pkg::name` parses and the qualifier is dropped
  (rlang has one namespace); `?` is lexed and unused.
- **No `library()`/`require()`/packages.** There is no package system, so any
  CRAN-dependent program is out of scope by construction.

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
- **No DAP adapter.** The LSP server is in the tree; debugging is not.
- **`Recall()` and `Negate()` are stubs** that raise rather than pretending to
  work.
