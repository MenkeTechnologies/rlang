# Known gaps

The honest list of what rlang does **not** do yet. Nothing here is faked as
working: calling an unimplemented primitive raises `could not find function`,
and two harnesses diff against the reference `Rscript` rather than against a
self-recorded baseline — `cargo run --bin parity` on a hand-authored corpus, and
`cargo run --bin parity-fuzz` on thousands of generated snippets across 63
surfaces. The fuzzer reports one divergence class, recorded under *Evaluation
model* below (the baseline in `tests/data/parity_fuzz_baseline.txt` is
deliberately still empty, so the run keeps failing on it), and a
run that compared nothing — no cases generated, or an oracle that never answered
— now exits 2 rather than reporting that zero. What remains below is structural
— whole subsystems, not per-primitive gaps.

## Evaluation model

- **An argument that turns itself invisible makes the whole call invisible.**
  `inherits(invisible(1), "numeric")` prints nothing where R prints `[1] TRUE`,
  so the value is computed correctly and then never auto-printed. R sets
  `R_Visible = TRUE` on entry to a call and lets only the CALLEE's own return
  clear it; here an argument clears it and nothing sets it back.

  The split is exact, and it is not about the function's meaning: it is whether
  the name is in `compiler::R_PRIMITIVES`. A name in that list makes no R
  context, so its arguments are evaluated EAGERLY, before `call_op` resets the
  flag — and the reset stands. A name outside it is called the way R calls a
  closure, its arguments become promises, and forcing one during the call runs
  `invisible()` AFTER that reset. Measured:

  ```text
  leaks (not in R_PRIMITIVES)   ok (in R_PRIMITIVES)
  inherits(invisible(1), …)     length(invisible(1))
  nchar(invisible("ab"))        class(invisible(1))
  paste(invisible("a"))         sum(invisible(1))
  toupper(invisible("a"))       sqrt(invisible(4))
  rev(invisible(c(1,2)))        as.character(invisible(1))
  substr(invisible("abc"),1,2)  is.na(invisible(1))
  ```

  Only an argument that clears the flag WHILE BEING FORCED does it, which is why
  `x <- invisible(1); inherits(x, "numeric")` is right — the promise is already
  forced — and why `inherits(suppressWarnings(1), "numeric")` is right too.

  Found by `parity-fuzz` (seed 31646, mode `conditions`) as
  `inherits(try(stop("foo"), silent = TRUE), "try-error")`: `try` returns its
  error object invisibly, so the enclosing `inherits` inherited that.

  Not fixed, because the fix is a visibility MODEL and not a patch: seventeen
  sites clear the flag today (assignment, `invisible`, `library`, …), and a
  blanket reset after the callee returns would have to exempt every one of them
  and still let a closure whose body ends in `invisible(x)` propagate outward.
  R's own rule — set on entry, cleared only by the callee's own return — is what
  this needs to grow, and half of it is worse than none.

- **Expressions are first-class**, so `quote()`, `sys.call()`, `match.call()`,
  `sys.function()`, `eval()` and `deparse()` of an unevaluated expression all
  work on rlang's own evaluator. `quote(x)` is compiled the way a formula is —
  the argument is never compiled, its deparse rides across as a constant, and
  the primitive parses it back — and the result is a real `LANGSXP`/`SYMSXP`
  that prints as source, deparses, indexes (`quote(f(1))[[1]]`), decomposes with
  `as.list`, and answers `class`/`typeof`/`mode`/`is.call`/`is.name`. `eval`
  runs one in the caller's environment, so it reads and binds there.
- **Arguments are evaluated eagerly, not as promises**, and this is a decision
  rather than an omission. `substitute()` does *not* need promises — the
  caller's call is on the context stack, so the expression a formal stands for
  is recoverable, and it works. What is left is one root cause with four
  visible shapes: an argument is evaluated even when the callee never uses it,
  and it is evaluated *before* the call rather than at first use. So
  `f <- function(a, b) a; f(1, stop("no"))` stops where R returns 1;
  `f <- function(a) a; f(1, x + y)` with `x` unbound reports
  `object 'x' not found` where R reports `unused argument (x + y)`, because the
  argument is evaluated before the matching that would have rejected it; an
  argument's side effects happen at the call rather than where the body first
  reads it; and an argument the body never reads still runs.

  The cost of closing it, measured on this tree rather than estimated: a
  closure call costs about 7.1µs, a loop iteration 1.4µs, and forcing one
  deferred expression 1.4–4.2µs (measured through `local(expr)`, which already
  compiles to a thunk built and called — the same machinery a promise would
  force through). Every argument would allocate and force one of those, so a
  one-argument call gains 20–60% and a three-argument call runs roughly 1.6–2.8x
  slower. Worse, an argument in arithmetic position (`f(i * 2)`) would stop
  being two native fusevm ops and become a chunk run. That is the whole of
  rlang's arithmetic and call performance spent to close four shapes, none of
  which appears in the parity corpus or in 8000 fuzz cases. Defaults already
  behave lazily by another route — they compile into a body prologue
  (`if (missing(p)) p <- <default>`), so a default may refer to another
  argument.

  Where the distinction is observable through the context stack it *is*
  reproduced: a call is opened before its arguments so a condition raised in one
  names the enclosing call as R's forced promise does, and a `sys.call()` written
  as an argument still reports the frame whose body wrote it. Non-standard-
  evaluation *programs* (`dplyr::filter(df, x > 2)`, `data.table` `[`, `subset`)
  run by re-running the whole script in the embedded GNU R (needs R installed)
  when rlang cannot evaluate it. Set `RLANG_NO_CRAN=1` to force the native path
  only.
- **The condition system, including the call a condition carries.**
  `tryCatch` selects a handler by condition class (`error`, `warning`,
  `message`, `condition`), `finally` runs either way, and `try` returns a
  `"try-error"` string. `on.exit` runs when a frame is left, however it is left.
  `stop`, `warning`, `message` and `signalCondition` raise real condition
  objects, and so do rlang's own internal warnings (`NaNs produced`,
  `Ops.factor`'s "not meaningful for factors"), so those are catchable and
  muffleable too. `conditionMessage` / `simpleError` / `simpleCondition` build
  and read condition objects. `warning()` and `message()` print and continue
  when nothing is waiting to catch them, which is R's default action.
  **Restarts work**: `withRestarts` / `invokeRestart` / `computeRestarts` /
  `restartDescription` / `isRestart`, and the built-in `muffleWarning` and
  `muffleMessage` that `warning()` and `message()` establish around their own
  signal. `withCallingHandlers` therefore *resumes* — its handler runs at the
  signalling point with the stack intact, and evaluation carries on from there
  unless the handler transfers to a restart — and `suppressWarnings` /
  `suppressMessages` muffle for real rather than passing the value through.
  **A warning reports the call it was raised in**, the way R does: the compiler
  fixes each call's deparsed text at compile time and the runtime keeps a
  context stack, so a batch prints `In f(1) : msg` under R's own rules for
  *which* call that is. Those rules are not "the innermost call": R makes a
  context for a closure and none for a primitive, so `print(as.integer("x"))`
  names `print(...)` while `sum(as.integer("x"))` names nothing, and `warning()`
  skips its own frame to land on its caller. Where R's own definition of a
  function makes the call itself, that call is what is reported — `lapply`'s
  `FUN(X[[i]], ...)`, `apply`'s `FUN(newX[, i], ...)`, `Reduce`'s
  `f(init, x[[i]])`, `range`'s `min(x)` / `max(x)`. R's fold rule is reproduced
  too: the message stays on the line with the call while it fits `LONGWARN`, and
  folds onto the next line indented two spaces when it does not, under the
  different allowances R gives the singular, numbered and `warn = 1` banners.
  *Where* the batch prints is R's as well: an uncaught warning is queued under
  the default `options(warn = 0)` and the whole batch is written once the
  top-level statement finishes, after that statement's own stdout.

  **An error reports its call too**, off the same context stack:
  `Error in f() : boom`, a bare `Error: boom` for a `stop()` at top level, R's
  14-column fold for a long one, the statement's held warnings after it under
  `In addition:`, and the `Execution halted` line R's front end closes with. A
  script that stops inside the CRAN bridge reports R's own `geterrmessage()`
  verbatim rather than a message about the delegation — which is the whole
  message but not the `Calls:` line, since `geterrmessage()` does not carry one.
  So a script that both errors *and* falls back shows the error without its
  chain; run with `RLANG_NO_CRAN=1` and rlang prints both from its own stack.

  **The condition object carries the call as well**, now that there is a type
  for one: `conditionCall(e)` hands back the language object, `print(cond)`
  shows `<simpleError in f(): msg>`, and `try`'s string is
  `"Error in f() : msg\n"`. A condition that unwinds to a `tryCatch` is rebuilt
  from what the raise recorded, because the unwind has already cut the context
  stack back past the frame that raised it.

  One gap remains. A condition raised by an **operator or an index** reports no
  call — R names them (`In 1:3 + 1:2 : longer object length …`,
  `Error in x[[5]] : subscript out of bounds`, `In Ops.factor(f, "b") : …`), but
  `+ - * /` and `[[` lower to native fusevm ops and index builtins carrying no
  call text, and pushing one on every arithmetic op would cost the hot path the
  design keeps native; the call-less form is printed rather than the enclosing
  call, which would name the wrong one. The language-object work does not
  change that: a call could only be recovered at raise time from a map keyed by
  the bytecode position, and the ops that raise these — `+ - * /` lowered to
  native fusevm ops, `[[` to an index builtin — carry neither a constant to
  hang one on nor a way to reach such a map. R's `Calls: f -> g` traceback *is*
  printed — the chain of function
  contexts, outermost first, with `stop`'s own frame dropped and R's mid-chain
  elision past `R_NShowCalls` — but it shows what rlang's own call graph looks
  like. Where R's own definition of a function dispatches to a method, rlang
  pushes the context that dispatch would have made, so `seq(7, 5, by = 3)`
  reports `seq.default(7, 5, by = 3)` and `Calls: seq -> seq.default` — but only
  for the generics rlang has been taught, not for every S3 layer in base R.
- **A restart object does not `format()` the way R's does.** `print` gives R's
  `<restart: name >` and `$name` / `restartDescription` / `computeRestarts`
  ordering all match, but the `handler`, `test` and `interactive` slots hold
  `NULL` rather than live functions and `exit` holds rlang's frame id rather
  than an environment. `format(restartObject)` therefore differs — though R's
  own output there embeds a heap address (`<environment: 0x…>`) that changes
  between two runs of R itself, so it is not a parity target for anyone.
- **`local()` works; part of the environment surface does not.**
  `local(expr)` compiles to `(function() expr)()`, which is R's own definition,
  so it gets a fresh environment enclosing the caller's. `sys.function()`,
  `parent.frame()` and `eval(expr, envir)` all work.
- **Formulas (`~`) parse and become real formula objects** — `lhs ~ rhs` is
  deparsed to R source and built in the CRAN bridge, so `lm(y ~ x, data = df)`,
  `aggregate(v ~ g, df, sum)`, and one-sided `~ x` work. A formula referencing a
  bare rlang variable (`lm(y ~ x)` with `x` defined only in rlang) can't see it —
  pass the data explicitly, or use literal vectors.
- **Environments are manipulable**: `new.env()`, `environment()`, `local()`,
  `globalenv()`, `environmentName()`, `parent.frame()`, `ls`/`objects` with
  `all.names`, `assign`/`get`/`exists` with `envir`, `eval(expr, envir)`, `$`
  and `[[` on an environment. `sys.nframe()` and `sys.frame(n)` are not: a
  builtin pushes no frame in rlang where R's closures do, so the numbering they
  report would not be R's. `baseenv()`/`emptyenv()` have no rlang-side
  representation and go to the CRAN bridge, and `environmentName` knows only
  the global environment's name — every other frame is anonymous, which is the
  empty string R gives one too.

## Types

- **An rlang closure does not cross the bridge as an R function.** `setRefClass`
  works — fields, methods and `<<-` into a field all behave — but R's own
  machinery calls `formals()` and `body()` on the methods it was handed, and
  those are rlang closures marshalled as opaque values, so three
  "argument is not a function" warnings ride along with a correct answer.
  Marshalling a closure as a real R function is what that needs.
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
  prefix, because `+` lowers to a native fusevm op that never sees the argument
  text — the same reason arithmetic's recycling warning carries no call. The
  message body and the returned `NA`s match.
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

- **Numeric literals with a decimal exponent past about ±100 parse to a
  different double than R's**, one ULP away. R's own `R_strtod` scales the
  mantissa by `10^expn` in double arithmetic instead of rounding correctly, so R
  reads the literal `1e100` as `0x1.249ad2594c37ep+332` where C's `strtod`, Rust
  and rlang all read `0x1.249ad2594c37dp+332` — and in R itself
  `1e100 == 10^100` is `FALSE` while the computed `10^100` matches everyone
  else. Below that exponent range R agrees with correct rounding exactly. This
  is a lexer gap, not a formatting one: rendering the *same* double agrees with
  R at every `digits` from 1 to 22. It shows up only as a difference in the last
  displayed digits of such a literal at high `digits`, and the fuzzer's
  `optsfmt` surface writes `10^100` rather than `1e100` so that it measures
  formatting rather than this.
- **`options()` does not enumerate a default set.** `options(digits=, scipen=)`
  and `getOption` are implemented, including the invisible named list of prior
  values that makes `old <- options(...)`; `options(old)` restore, the
  untagged-string query form, R's 1..22 range check on `digits` and its -9 clamp
  on `scipen`. Any other option name is stored and read back but has no effect,
  and a bare `options()` returns only what has been set rather than R's ~73
  defaults, so `getOption("width")` is `NULL` where R says 80.
- **`format()` handles `nsmall`, `digits`, `big.mark`, `width`, `scientific`,
  common decimals, and common-width justification** (and
  `formatC`/`prettyNum`/`deparse` exist), but not the `justify` argument. The
  fixed-versus-scientific choice is the same width rule `print` uses — fixed
  when its width is no wider than the scientific one plus `getOption("scipen")`
  — so `format(1e6)` is `"1e+06"` at the default `scipen` of 0, and `big.mark`
  does not apply to it.
- **`as.character`, `paste` and `toString` render doubles at a fixed 15
  significant digits**, following `scipen` but deliberately *not* `digits`, so
  `paste(pi)` stays `"3.14159265358979"` under `options(digits = 3)` while
  `cat(pi)` and `format(pi)` follow the setting. This matches R.
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

## Text

- **Strings are measured in R's three units, and each site uses the right one.**
  `nchar(type=)` answers in code points, UTF-8 bytes or terminal columns; the
  `sprintf` field width is a byte count, as in C; and `print`, `format`,
  `formatC` and `strtrim` lay out in columns, so a CJK character claims two and
  a combining mark none. The column table is R's own answer for every assigned
  code point, swept out of the reference `Rscript` (`src/strwidth.rs`).
  `toupper`/`tolower` map one character to one character the way `towupper` does,
  so `toupper("straße")` is `"STRAßE"` and the character count never changes.
  Three limits remain, all of them cases R answers with bytes that are not a
  valid Rust string:
  - **A string literal that is not valid UTF-8 is rejected.** `"\xff"` is a raw
    byte in R and a parse error here; rlang's string type is `String`.
  - **A surrogate code point is rejected.** R takes `"\uD800"` with a warning.
  - **`sprintf("%.Ns", x)` cuts on a character boundary,** where R cuts at
    exactly N bytes and can emit half a sequence.
- **Character data orders through the collation locale, as R's does.** `sort`,
  `order`, `rank`, `xtfrm`, `sort.list`, `min`/`max`/`range`, `<`/`>`/`<=`/`>=`
  and the default `factor` levels all collate, so `sort(c("B", "a"))` is `a B`
  and `"a" < "B"` is `TRUE`. This was previously recorded here as needing "a
  collation table rlang does not carry" and as an accented-character corner
  (`a é z` vs `a z é`); both were wrong. The gap covered every mixed-case
  character vector — plain ASCII — and `icu_collator`'s root ordering matches
  the reference R exactly, diffed over 500 generated groups mixing Latin,
  accents, Greek, Cyrillic, CJK, Hangul, digits and punctuation. Only the `C`
  locale orders by code point, and rlang follows R there too. Two limits remain:
  - **Collation is ICU's *root* ordering, not a per-locale tailoring.** `en_US`,
    `de_DE` and `fr_FR` were measured to agree with root; a locale that really
    tailors (Swedish, where `ä` sorts after `z`) would diverge.
  - **`LC_ALL=POSIX` is treated as the `C` locale**, which is what POSIX
    specifies and what glibc does. The reference R on Darwin instead falls back
    to the system `strcoll` there and answers `é a b B z`.
- **190 code points case-map differently**, measured by sweeping every code point
  against the reference `Rscript`: they are characters R's own case table
  predates (Vithkuqi `U+10570…`, the `U+A7C0…U+A7DC` Latin additions, `U+1C89`,
  `U+2C2F`) and therefore leaves unmapped, while Rust's newer tables map them.
  Everything R maps, rlang maps identically.

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
- **Subscript assignment copies the whole vector.** `x[i] <- v` builds a new
  vector every time, because rlang has no equivalent of R's `NAMED`/reference
  count and so cannot tell whether the target is shared with another binding.
  A loop that fills a vector is therefore quadratic — 0.07s / 0.21s / 0.75s /
  2.63s for n = 5000 / 10000 / 20000 / 40000 on a debug build, against R's flat
  ~0.1s, since R mutates in place when the count allows. Reads are not affected
  (`x[i]` takes its selection through a borrow). Fixing this needs a real
  reference count maintained at every point a handle is stored into an
  environment, a list or an attribute; a partial version would silently corrupt
  an aliased vector, so it is not worth half-doing.
- **A closure body is copied on the first call to it.** Later calls reuse a VM
  parked under the closure id, which still holds the chunk, so the copy is once
  per closure rather than once per call — but a *re-entrant* call (recursion,
  or a closure calling itself through `Map`) finds the parked VM checked out and
  builds a fresh one, chunk copy included. Deep recursion still pays per level.
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
