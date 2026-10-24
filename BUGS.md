# Known gaps

The honest list of what rlang does **not** do yet. Nothing here is faked as
working: calling an unimplemented primitive raises `could not find function`,
and two harnesses diff against the reference `Rscript` rather than against a
self-recorded baseline — `cargo run --bin parity` on a hand-authored corpus, and
`cargo run --bin parity-fuzz` on thousands of generated snippets across the
surfaces. The fuzzer reports one divergence class, recorded under *Evaluation
model* below (the baseline in `tests/data/parity_fuzz_baseline.txt` is
deliberately still empty, so the run keeps failing on it), and a
run that compared nothing — no cases generated, or an oracle that never answered
— now exits 2 rather than reporting that zero. What remains below is structural
— whole subsystems, not per-primitive gaps.

## Evaluation model

- **Which of `NA` and `NaN` survives an operation on both is the hardware's,
  and rlang reproduces x86-64's answer only.** `NaN + NA`, `sum(c(1, NaN,
  NA))` and `mean(c(NaN, NA))` are computed by R as plain C double
  arithmetic, and R documents the outcome as platform-dependent. An x86-64 R
  keeps the first operand's payload (`NaN + NA` is NaN), which is what rlang
  implements and what the corpus is frozen against (the `Rscript` on `PATH`).
  An arm64 R propagates the signalling `NA` payload R stores for a literal
  `NA` over a quiet NaN in either order (`NaN + NA` is NA), but a quieted `NA`
  — one already through an operation — does not win, so `sum(c(NaN, 1),
  c(NA, 2))` is still NaN there. rlang holds a missing double as `None`, not as
  a payload with a signalling bit, so it cannot tell those two `NA`s apart.
  This is the one divergence class `parity-fuzz` reports against the
  Homebrew arm64 `Rscript` (`sum(c(NaN, NA))`, mode `missing`).
- **`force` and `withVisible` lose their argument's visibility** — fixed. The
  diagnosis recorded here was wrong: neither name is in `compiler::R_PRIMITIVES`
  (R implements both as closures, so neither belongs there), and `call_op`'s
  entry reset was not the cause. Neither had an arm in `call_primitive` at all,
  so both fell through `other => cran_call(…)` to the embedded GNU R — which
  receives arguments rlang has ALREADY evaluated and therefore cannot be told
  what the flag was. `force(invisible(1))` printed `[1] 1` where R is silent,
  and `withVisible` answered `TRUE` for every argument. Both now have native
  arms: `identity | force` returns the argument (R's `function(x) x`), and
  `withVisible` reads `h.visible` and builds `list(value=, visible=)` the way
  `.Internal(withVisible)` reads `R_Visible`. Both names joined
  `VISIBILITY_TRANSPARENT` so the post-forcing restore leaves the flag alone.
  Regression: `tests/visibility.rs::force_and_with_visible_report_the_arguments_flag`,
  six assertions read off R 4.6.1.
  Residue (pre-existing, unchanged by the fix): printing the function itself
  still deparses as `function (...) .Primitive("force")`, where R shows the
  closure body `function (x)` / `x` with its bytecode and namespace lines.
  Every name in the primitive table deparses that way; matching R here means a
  base-R prelude rlang does not have.
- **Expressions are first-class**, so `quote()`, `sys.call()`, `match.call()`,
  `sys.function()`, `eval()` and `deparse()` of an unevaluated expression all
  work on rlang's own evaluator. `quote(x)` is compiled the way a formula is —
  the argument is never compiled, its deparse rides across as a constant, and
  the primitive parses it back — and the result is a real `LANGSXP`/`SYMSXP`
  that prints as source, deparses, indexes (`quote(f(1))[[1]]`), decomposes with
  `as.list`, and answers `class`/`typeof`/`mode`/`is.call`/`is.name`. `eval`
  runs one in the caller's environment, so it reads and binds there.
- **`with()`, `evalq()` and `delayedAssign()` take their expression
  unevaluated.** The compiler lowers `with(data, expr)` and `evalq(expr, envir)`
  to `eval(quote(expr), …)` and `delayedAssign(x, value)` to an assignment of a
  promise, which is forced at first read in the environment it was written in.
  `sys.call(which)`, `sys.function(which)` and `sys.calls()` index the function
  frames outermost first. A frame that R makes for a base closure rlang runs as
  a builtin (the four `tryCatch` frames, `print` around an argument) is not
  there, so a frame number taken *inside* such a call differs from R's.
- **Arguments and defaults are promises.** A closure's argument is wrapped in a
  thunk at the call and forced on first read, in the environment it was written
  in: `f <- function(a, b) a; f(1, stop("no"))` returns 1, an unused argument
  is rejected before it is evaluated (`unused argument (x + y)`), and an
  argument's side effects happen where the body first reads it. A default is
  bound the same way when the caller omits the formal, so it is evaluated at
  first use in the callee's frame — `f <- function(a, b = a * 2) { a <- 10; b }`
  gives 20, and a default that is never read never runs. A literal argument or
  default is bound as itself, since forcing it could not differ. A forced
  promise runs in its writer's environment, so `environment()`, `nargs()`,
  `missing()`, `Recall()`, `sys.function()` and `parent.frame(n)` written inside
  an argument answer for the closure that wrote it. Primitives take their
  arguments forced, as R's builtins do.

  Where the distinction is observable through the context stack it is
  reproduced: a call is opened before its arguments so a condition raised in one
  names the enclosing call as R's forced promise does, and a `sys.call()` written
  as an argument still reports the frame whose body wrote it. Non-standard-
  evaluation *programs* (`dplyr::filter(df, x > 2)`, `data.table` `[`, `subset`)
  run by re-running the whole script in the embedded GNU R (needs R installed)
  when rlang cannot evaluate it. Set `RLANG_NO_CRAN=1` to force the native path
  only.
- **`match.arg()`, `formals()` and `formalArgs()`** — fixed. A default is still
  compiled into the body prologue, so `formals()` parses the closure's deparsed
  source back and returns each default as the expression written (the empty
  symbol for a formal without one, printed as a blank line the way R does).
  `match.arg` follows base R's closure: one-argument form reads the formal's
  default out of the calling function and evaluates it in that frame, then
  `pmatch`-style exact-then-unique-prefix matching, `several.ok`, and R's
  `'arg' should be one of “a”, “bb”` error. Regression:
  `tests/eval.rs::formals_and_match_arg_read_the_written_defaults`.
  `nargs()` and `missing()`, the neighbouring pieces of argument
  introspection, both work — `missing()` for a formal with a default included.
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

  **An arithmetic or comparison operator is the exception: its warning reports
  no call where R names one.** `1:5 + 1:2` warns `In 1:5 + 1:2 : longer object
  length …` in R and bare in rlang, and so does `2147483647L + 1L`. A subscript
  assignment is the same: `x[5:6] <- 1:3` and `f[1] <- "z"` (R: ``In `[<-.factor`(`*tmp*`, 1, value = "z")``) warn without the call. The message,
  the queueing and the batch position are right; only the `In <call> :` line is
  absent. `+ - * /` lower to native fusevm ops that carry no call text, and
  opening a context on every one would cost the hot path the design exists to
  keep native — so the no-call shape R itself uses is reported rather than the
  enclosing call, which would name the wrong one. Closing it without that cost
  means a side table from code position to deparsed text, read only when a
  warning actually fires; the eager form is what is ruled out, not the idea.

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
  `"Error in f() : msg\n"`. A condition object signalled with `stop(cond)`,
  `warning(cond)`, `message(cond)` or `signalCondition(cond)` reaches a
  `tryCatch` handler as itself, extra fields and own call included; one raised
  from a message is rebuilt from what the raise recorded, because the unwind
  has already cut the context stack back past the frame that raised it. A
  condition raised directly in `tryCatch`'s `expr` names
  `doTryCatch(return(expr), name, parentenv, handler)`, as in R, since rlang
  stands up the frames R's own `tryCatch` code makes.

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
- **Environments are manipulable**: `new.env(parent =)` (enclosing, by
  default, the environment it is called from, as R's `parent.frame()` default
  does), `environment()` and `environment<-` (a closure rebuilt around the new
  environment), `topenv()` (the global environment is rlang's one top level),
  `local()`, `globalenv()`, `environmentName()`, `parent.frame()`, `ls`/`objects` with
  `all.names`, `assign`/`get`/`exists` with `envir`, `eval(expr, envir)`, `$`
  and `[[` on an environment. `sys.nframe()` counts the call contexts the
  compiler opens for every function R implements as a closure — `print`,
  `sapply`, the frames `tryCatch` makes — so its depth is R's, except under
  `local()`, which compiles to one closure call where R's goes through
  `eval.parent` and `eval` (R answers 7 for `print(local(sys.nframe()))`,
  rlang 2). `sys.frame(n)` is not implemented. `baseenv()`/`emptyenv()` have no rlang-side
  representation and go to the CRAN bridge, and `environmentName` knows only
  the global environment's name — every other frame is anonymous, which is the
  empty string R gives one too.

## Types

- **`dim<-` checks the new dimensions** — fixed. It is R's `dimgets` now: an
  empty, missing or negative extent, or a product that is not the object's
  length, raises R's error, and the error names the assignment
  (`Error in dim(m)[1] <- 4 :`), which every failing replacement function now
  does. Setting `dim` drops the names and old dimnames, and `NULL` drops both.
  Regression: `tests/linalg.rs::dim_assignment_checks_the_length`.


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
  a margin onto its result) work, and so do the array-specific helpers
  `slice.index` and `arrayInd`.
- **`apply` with a named margin answered silently wrong** — fixed. R's `apply`
  accepts a character `MARGIN` and matches it against `names(dimnames(X))`;
  rlang read `MARGIN` through `as_int`, so a name became `NA`, was dropped, and
  the margin list was empty — `FUN` was called once on the whole array, with no
  error or warning (`apply(m, "rows", sum)` gave `21` where R gives `a 9, b
  12`). The arm now follows base R's closure: a character margin is `match`ed
  against the dimnames' names, an unnamed dimension answering to `""`, with
  R's `'X' must have named dimnames` when there are none and `not all elements
  of 'MARGIN' are names of dimensions` when one does not resolve. A numeric
  margin is range-checked the way `d[MARGIN]` reports `NA` in R — `apply(m, 3,
  sum)` raises `'MARGIN' does not match dim(X)` where it used to index past
  `dim(X)` and abort the interpreter. Regression:
  `tests/eval.rs::apply_resolves_a_character_margin_against_the_dimnames_names`,
  nine assertions read off R 4.6.1.
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
- **Linear algebra without `eigen`.** `%*%`, `t`, `diag`, `apply` over margins,
  `rowSums`/`colSums`/`rowMeans`/`colMeans`, `outer`/`%o%`, `crossprod`/
  `tcrossprod`, `cbind`/`rbind`, and `solve`/`det`/`determinant` work;
  `eigen`, `qr`, `chol` and `svd` are not native. `solve` and `det` port
  LAPACK 3.12's `dgetrf`/`dgetrf2`/`dgetrs`/`dgecon` (`src/linalg.rs`), and
  the BLAS calls those make — and `%*%` itself — follow the reference R's
  OpenBLAS kernels, so results match R to the last bit (diffed with
  `sprintf("%a")` for n = 2…130, `tests/linalg.rs`). Two limits: the bit
  pattern is the reference build's (R 4.6.1 over OpenBLAS 0.3.34 on arm64),
  and another BLAS sums in another order; and `dlatrs`'s careful rescaling
  path is not ported, so the reciprocal condition number of a matrix with
  entries near the overflow or underflow threshold can differ from R's.
- **Integer overflow produces `NA` with a warning**, as R does, and the result
  keeps class `"integer"` — `2147483647L + 1L`, `* 2L` and `-2147483647L - 2L`
  all give `NA`. Only the warning's call differs, under the operator entry
  above: R names `In 2147483647L + 1L :`, rlang reports no call.
- **`%%`/`%/%` and `var` differ from R by ULPs at the edge of f64 precision.**
  R accumulates them in C `long double`; Rust has no equivalent, so a modulus of
  a value past `2^53` (where R warns of "complete loss of accuracy") or a
  variance landing on a 7th-significant-digit rounding tie can differ in the
  last place. `round` and `signif` are no longer in this list: they are ports of
  R's `fround` and `fprec`, which work in plain double arithmetic, so
  `round(0.05, 1)` is `0` and `round(0.45, 1)` is `0.4` exactly as in R.

## Printing and formatting

- **`[[` follows `do_subset2_dflt`.** One subscript per dimension for a matrix
  or array (`m[[i, j]]`, character subscripts against the `dimnames`), recursive
  indexing through nested lists (`l[[c("a", "b")]]`), `exact =`, and R's wording
  for what it refuses (`attempt to select less than one element in
  integerOneIndex`, `invalid negative subscript in get1index <real>`,
  `subscript out of bounds`, `no such index at level 2`, `recursive indexing
  failed at level 2`). An unmatched name is `NULL` on a list and out of bounds
  on an atomic vector. `m[[i, j]] <- v` checks the same, with `[[ ]] subscript
  out of bounds` and `more elements supplied than there are to replace`.
- **Result types follow R's.** `min`, `max`, `range`, `pmin`, `pmax`, `cummax`,
  `cummin`, `abs` and `median` of integer or logical input stay integer (a
  double only when the answer is an average); `-TRUE` and `+TRUE` are integers;
  integer overflow, `cumsum` overflow and an out-of-range `as.integer` raise
  warnings a `tryCatch(warning = )` sees, with R's text.
- **`table()` always names its margins**, with `""` when nothing names them,
  which is what heads a one-way table with a blank line, and a one-dimensional
  array whose `dimnames` are named prints that name above its labels. `dnn`,
  `exclude` and `useNA` are honoured; a 3-D table prints `, , z = p` headers and
  names the slice's margins. `margin.table`/`marginSums` keep the class and the
  margin names; `apply` carries `names(dimnames(X))` onto its result and hands
  extra arguments to `FUN`.
- **`unlist` and `c` name elements as `NewExtractNames` does**: elements without
  a name under a tag are numbered among themselves, a lone one is the bare tag
  (`unlist(list(a = list(x = 1, 2)))` is `a.x`, `a`), and an atomic vector under
  a tag numbers every element (`c(a = c(x = 1, 2))` is `a.x`, `a2`). `recursive`
  and `use.names` are honoured, and `sapply` over equal-length list results
  returns a list matrix.

- **`table(v)` lost the line naming `v` after a replacement function** — fixed.
  The diagnosis recorded here (a name carried on the value) was wrong: a
  replacement target makes a program ineligible for slot compilation, and on
  the general path the compiler had already wrapped `v` in a promise thunk
  before it looked for a bare symbol to pass as `.dnn`. The names are now read
  before the promise rewrite, one per unnamed argument, which also gives
  `table(a, b)` both margin names. `library(pkg)` had the same ordering bug and
  read `pkg` as a variable. Regression: the `table` snippets in the corpus.


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
- **`format()` handles `trim`, `nsmall`, `digits`, `big.mark`, `width`,
  `scientific`, `justify`, lists, common decimals, and common-width
  justification** (and `formatC`/`prettyNum`/`deparse` exist). The
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
  `{ }` split across lines, and the 60-column wrap. A closure whose environment
  is not the global one omits R's trailing `<environment: 0x…>` line, which
  carries a process address that could not match anyway. An R *primitive*
  prints as R's `PrintSpecial` prints it — the formals R keeps for it in
  `.ArgsEnv` / `.GenericArgsEnv` (`src/primargs.rs`, read off R 4.6) then the
  `.Primitive` call (`function (..., na.rm = FALSE)  .Primitive("sum")`), or the
  bare call for one with none (`.Primitive("[")`) — and `args()` returns those
  formals as a closure. A base function that is a *closure* in R but a Rust
  builtin here (`paste`, `force`) still prints as `function (...)
  .Primitive("name")`, since rlang has no R source for its formals.
  `deparse(sum)` is exact.
- **`str()` is a port of `str.default`.** The per-type `vec.len` scaling
  (logical 1.5x, integer and tidy numeric 2.5x, other numeric 1.25x, call 0.5x),
  the width-filling character rule at the top level against four per element
  once nested, joint `format(digits = 3)` of a numeric vector with
  `drop0trailing`, the `Factor w/ n levels` abbreviation, `function (args)`,
  `language` / `symbol` headers, `chr(0)` / `int(0)` / `list()`, the padded
  `$ name :` labels, and the `vec.len`, `give.head`, `give.length`,
  `max.level`, `list.len`, `give.attr`, `digits.d` and `nchar.max` arguments.
  The header of a function that is a closure in R but a builtin here shows
  `function ()`, since rlang has no formals for it.
- **`dput()` and `summary()` are native.** `dput` and
  `deparse` of a value port `deparse.c`'s value cases: inline names, `structure()`
  for other attributes, typed `NA`s, 15-digit doubles and the `width.cutoff`
  wrap. `summary()` ports `summary.default` and `summary.factor` with the
  `summaryDefault` print method for vectors and factors; a matrix, list or
  data frame still goes to the embedded GNU R.
- **`hexmode` is native except for its bitwise methods.** `as.hexmode`, and
  `format`, `print`, `as.character` and `[` of a `hexmode`, port R's methods;
  `!`, `&` and `|` on one (R's bitwise `!.hexmode`, `&.hexmode`, `|.hexmode`)
  are not: they act logically, so `as.hexmode(12) & as.hexmode(10)` is `TRUE`
  where R gives the hexmode `"8"`.

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

- **Back-references and look-around work, on a second engine.** The `regex`
  crate is linear-time and has neither, so a pattern it rejects *for those
  features* is retried on `fancy-regex` (`src/re.rs`); every other pattern stays
  on the first engine. That covers `(a)\\1` in the default dialect and
  `(?<=a)b`, `a(?=b)` under `perl = TRUE`. The two engines are not PCRE: a
  pattern using atomic groups, possessive quantifiers, recursion or `\\K` is
  still rejected. `strsplit` applies R's end-of-match rule (`rm_eo > 0`), so a
  zero-width look-behind match splits where it stands.
- **`regexpr`, `gregexpr` and `regexec` report what R's do.** An `NA`
  subject is `NA` rather than a `-1` miss; a `regexec` group that did not take
  part is position 0, width 0 (TRE's -1/-1 through `rm_so + 1`), which
  `regmatches` cuts to `""`; and `perl = TRUE` adds the `capture.start`,
  `capture.length` and `capture.names` attributes, with `regexec(perl = TRUE)`
  naming its positions after named groups as R's R-level implementation does.
  `regmatches<-` is a port of R's, for `regexpr` and `gregexpr` results and
  `invert = TRUE`; a failure names the assignment (`regmatches(x, m) <- v`)
  where R names its `` `regmatches<-`(`*tmp*`, …) `` closure frame.
- **`sprintf` checks the argument against the conversion** the way
  `do_sprintf` does: `%d` takes an integer, a logical, or a double whose every
  element is whole; `%f`/`%e`/`%g` take any number; a character argument is only
  for `%s`; an unknown conversion is `unrecognised format specification`. `formatC`
  accepts `d` for integers and `f e E g G fg` for reals (not `x`/`o`), and its
  `mode` re-types the argument first.

## Syntax

- **`else` may start a new line at top level.** R only allows that inside `{ }`;
  rlang accepts both, so a program R rejects can run here. The parity corpus
  treats "both reject" as parity, so this leniency is visible only for that one
  construct.
- **The keywords are function values, but loops and assignments run only as
  calls.** `` `if`(a, b, c) ``, `` `for`(i, s, b) ``, `` `<-`(x, v) `` and every other
  keyword or operator written as a call is folded by the parser into the node
  its syntax builds, exactly as R's call and syntax are one object. Through a
  value (`` f <- `if` ``, `` sapply(x, `(`) ``, `` Reduce(`&&`, …) ``) `if`, `(`, `{`,
  `&&` and `||` force their arguments only where R would evaluate them; `for`,
  `while`, `repeat`, `<-`, `<<-` and `=` raise an error there instead, because
  they read an argument as an *expression* (the loop variable, the target) and a
  promise reached through a value carries only its thunk.
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
- **The subset and subassign operators dispatch** — fixed. `[`, `[[`, `$`,
  `[<-`, `[[<-` and `$<-` used to run their defaults on every object, so a
  `$.rec` / `[[<-.rec` was never called (`x$a` on a classed list read the
  element; `x$a <- v` wrote it). Each now does R's `DispatchOrEval`: an object
  with a `class` attribute and a matching `<op>.<class>` (or `<op>.default`)
  method gets the call, `$` / `$<-` hand the method the name as a string, the
  replacement forms tag the new value `value`, and the getters force
  visibility on as R's builtins do. The same holds when the operator is called
  as a function value (`` sapply(xs, `[[`, "k") ``), and `[<-`, `[[<-`, `$<-`
  are now callable that way too. Two prerequisites were ported with it: an
  empty argument (`m[1, ]`, `f(1, )`) is R's `R_MissingArg`, so `missing()` is
  TRUE and the default fills it, and `x[]` parses as one empty subscript, so
  `x[] <- v` fills every element (it used to leave `x` unchanged).
  `NextMethod()` now passes each matched formal's *current* value, as
  `do_nextmethod` does, so a method that changes `x` before `NextMethod()`
  forwards the changed `x`. Regressions: `tests/eval.rs`
  `subset_operators_dispatch_to_s3_methods`,
  `subassign_operators_dispatch_to_replacement_methods`,
  `next_method_from_a_subset_method_reaches_the_default`,
  `an_empty_argument_is_missing`, all read off R 4.6.1.
  Residue: a method reached by internal dispatch (this one, `length.cls`,
  `print.cls`) runs with no call context of its own, so `sys.call()` inside it
  is `NULL` where R gives `` `$.sc`(s, hello) ``, and `conditionCall()` of an
  error raised inside it is not the method call. `@` still compiles to `$`, so
  it would dispatch to `$.cls`.
- **No S4 (`setClass`, `setGeneric`, `isVirtualClass`), no Reference Classes,
  no R6.** `@` parses but is compiled as `$` (see the 2026-10-14 audit section
  below), which is not S4 slot semantics.
- **S3 group generics dispatch.** An operator on an object with a class calls
  `<op>.<class>` or `Ops.<class>` (left operand first, the right one's method
  when the left has none, and R's "Incompatible methods" warning and the
  internal operator when both differ); `Math` and `Summary` members reach
  `Math.<class>` / `Summary.<class>`. The method sees `.Generic`, and
  `NextMethod()` falls through to the internal operator. `.Class`, `.Method`
  and `.Group` are not bound. The factor methods (`Ops.factor`, `Ops.ordered`,
  `Summary.ordered`) are implemented natively.

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

## Open defects found by the 2026-10-14 book audit

Each was observed at 0.1.10+43 (the debug `Rscript` built from that source)
against GNU R 4.6.0 (2026-04-24), `/usr/local/bin/Rscript`.

- **`rbind`/`cbind` lost their deparsed labels in any file that defines a
  function** — fixed. The seam labels described under *Types* were computed
  from the argument expressions after the promise rewrite in
  `src/compiler.rs`; slot mode skips that rewrite, so a unit with no `function`
  kept the labels, but once any `function` appeared anywhere in the file every
  symbol argument arrived as `.rlang_promise(function() x)` and no longer
  matched as a bare symbol (`rbind(x, y)` printed `[1,]`, `[2,]` where R
  prints `x`, `y`; at `deparse.level = 2` the wrapper itself became the
  label). The labels are now read off the arguments as written, before the
  rewrite, the way `table`'s `.dnn` already was — so a call inside a closure
  body labels with the closure's own formals, as R does. Explicitly named
  arguments were unaffected. Regression:
  `tests/eval.rs::bind_labels_survive_the_promise_rewrite`, seven assertions
  read off R 4.6.1.

- **Doubles that print alike are treated as equal by the matching and set
  functions.** `0.1 + 0.2` and `0.3` differ in the last bit; R keeps them apart,
  rlang merges them in `match`, `%in%`, `duplicated`, `setdiff`, `union`,
  `intersect` and `unique`:

  ```r
  x <- c(0.1 + 0.2, 0.3)
  match(0.3, 0.1 + 0.2)              # R: NA                rlang: 1
  (0.1 + 0.2) %in% 0.3               # R: FALSE             rlang: TRUE
  duplicated(x)                      # R: FALSE FALSE       rlang: FALSE TRUE
  length(unique(x))                  # R: 2                 rlang: 1
  length(setdiff(x, 0.3))            # R: 1                 rlang: 0
  length(union(x, x))                # R: 2                 rlang: 1
  length(intersect(0.1 + 0.2, 0.3))  # R: 0                 rlang: 1
  ```

  `split` and `factor` go wrong differently: R groups through `as.character`
  (15 significant digits), but rlang builds the level set from the raw doubles
  (`factor_levels` in `src/builtins.rs`), so two distinct doubles that format
  alike become two levels with the same label, and `split` then puts every
  element whose label matches into each of them. rlang's labels also carry fewer
  digits than `as.character` (`1 + 1e-10` labels as `1`), so `y` below collides
  where R keeps it apart. In both cases each element lands in both groups:

  ```r
  split(1:2, x)        # R: one group `0.3` = 1 2
                       # rlang: two groups, both named `0.3`, each = 1 2
  levels(factor(x))    # R: "0.3"                   rlang: "0.3" "0.3"

  y <- c(1, 1 + 1e-10)
  split(1:2, y)        # R: `1` = 1, `1.0000000001` = 2
                       # rlang: two groups, both named `1`, each = 1 2
  levels(factor(y))    # R: "1" "1.0000000001"      rlang: "1" "1"
  ```

  Measured with rlang 0.1.10 (`RLANG_NO_CRAN=1 target/debug/Rscript`).

- **`.Call` silently passes only the first element of a longer vector.**
  `r_to_fusevm` in `src/builtins.rs` takes `as_int(v).first()` / `num1` /
  `str1`, so the rest of the vector is dropped with no error, while the error
  arm claims arguments "must be length-1":

  ```r
  .rust('#[no_mangle] pub extern "C" fn twice(x: i64) -> i64 { x * 2 }')
  .Call("twice", c(1L, 2L, 3L))      # rlang: [1] 2
  ```

  Expected: a length-1 marshaller rejects a length-3 argument rather than
  truncating it (R's `.Call` hands the routine the whole vector).

- **`x@name` is compiled as `x$name`.** The compiler lowers `IndexKind::At`
  through the same arm as `IndexKind::Dollar`, so `@` reads a list element;
  it does not read an attribute, and R 4.6's error for a non-S4 object is never
  raised:

  ```r
  x <- structure(list(a = 1), foo = 2)
  x@a     # R: Error in x@a : no applicable method for `@` applied to an
          #    object of class "list"          rlang: [1] 1
  x@foo   # R: the same error                  rlang: NULL
  ```

- **`NaN:3` gives `NA` where R stops.**

  ```r
  NaN:3   # R: Error in NaN:3 : NA/NaN argument      rlang: [1] NA
  ```

- **`print` writes a raw bell byte for `"\a"`.** R escapes it on print;
  rlang emits byte 0x07 between the quotes:

  ```r
  print("\a")   # R: [1] "\a"      rlang: [1] "<BEL>"  (od -c shows \a)
  ```

- **`cor` does not warn on a zero standard deviation.**

  ```r
  cor(c(1, 1, 1), c(1, 2, 3))
  # R: [1] NA, plus  Warning message: In cor(c(1, 1, 1), c(1, 2, 3)) :
  #    the standard deviation is zero          rlang: [1] NA, no warning
  ```

- **`exists()` with `envir=` does not find builtins** — fixed. An inheriting
  search from any environment now ends in the base bindings (the primitives
  and the base constants), because every enclosure chain in R ends in base:
  `exists("sum", envir = globalenv())` and `exists("pi", envir = new.env())`
  are TRUE, and `inherits = FALSE` still looks at the one frame only.
  Regression: `tests/eval.rs::exists_inherits_through_to_the_base_bindings`.
  `baseenv()` is still missing (`could not find function "baseenv"`): rlang
  has no base environment object to hand back.

- **`missing()` on a forwarded missing argument gives `FALSE`** — fixed. A
  formal bound to a promise of a bare symbol is followed into the environment
  that wrote it, R's `do_missing` → `R_isMissing`, through any number of
  hand-offs (`h <- function(z) g(z)`). A forwarded formal that its own default
  filled in reads as supplied (R's `MISSING == 2`), and a promise of any other
  expression is never missing. `k <- function(a, b) h(a, b); k(1)` with `h`
  testing `missing(b)` no longer dies with `object 'b' not found`.
  Regression: `tests/eval.rs::missing_follows_a_forwarded_bare_symbol`.

- **`T` and `F` were keywords** — fixed. The lexer read them as `TRUE`/`FALSE`
  literals, so `function(F)` was a parse error (`bad parameter False`) and
  `T <- 5` could not shadow. They are now ordinary names resolved to the base
  bindings. `isTRUE`/`isFALSE` coerced their argument (`isTRUE(5)` was TRUE);
  they now require a logical, as R's `is.logical(x) && …` does.
  Regression: `tests/eval.rs::t_and_f_are_bindings_not_keywords`.

- **`match.fun`, `getS3method` and `subset` were missing** — added. `match.fun`
  looks a string or symbol up as a function from `parent.frame(2)`, as R's
  does; `getS3method` requires a generic (an internal one, or a closure whose
  body calls `UseMethod`, per utils' `findGeneric`) and finds a bound
  `<f>.<class>`; `subset` ports `subset.default` and `subset.matrix`, reading
  `select` unevaluated with the column names bound to their indices, and
  dispatches to a user's `subset.<class>`. Residue: an error raised inside
  `match.fun` reports `Error in match.fun(…)` where R names its inner
  `get(as.character(FUN), mode = "function", envir = envir)` call; there is no
  S3 methods table, so `getS3method("print", "default")` finds nothing; and
  `subset.data.frame` is unavailable because data frames are not native.
  Regression: `tests/eval.rs::match_fun_resolves_from_the_callers_caller`,
  `get_s3_method_needs_a_generic_and_a_method`,
  `subset_ports_the_default_and_matrix_methods`.

- **No expression vectors: `parse(text = )` is missing.** `expression(…)`
  and `parse` are both `could not find function` under `RLANG_NO_CRAN=1`,
  because rlang has no `EXPRSXP` value type to return (a `typeof`
  `"expression"` list whose `eval` runs each element and returns the last).
  `eval(parse(text = "x <- 3; x * 2"))` therefore fails with `could not find
  function "parse"`. Adding the type is the prerequisite.

- **`pkg::name` drops the qualifier.** The parser reads `base::sum` as `sum`,
  so a user binding shadows it: `sum <- 3; base::sum` is `[1] 3` where R
  returns the primitive.

- **Also open:** `Sys.time()`/`Sys.Date()` are missing; `with()` is lowered to
  `eval(quote(expr), data)` and so never dispatches a user's `with.<class>`;
  `as.numeric` dispatches `as.numeric.<class>` where R dispatches the
  `as.double` methods; `@` compiles to `$`; `sys.call()` is `NULL` inside a
  method reached through operator dispatch.


- **Rejecting 8,000 lexically nested closure calls takes ~47 s and 5.7 GB on
  the debug build.** rlang does stop with `evaluation nested too deeply:
  infinite recursion?`, but only after that cost; R refuses the same program
  at parse time in under a second (`Error: contextstack overflow`, 0.7 s,
  78 MB RSS). Plain recursion to the same depth is fast (0.17 s), and 8,000
  nested calls of the builtin `identity` succeed in 24 s (R refuses those at parse time
  too), so the cost tracks lexical nesting rather than the depth check. Generated with:

  ```sh
  perl -e 'print "f <- function(x) x\nx <- ", "f(" x 8000, "1", ")" x 8000, "\nprint(x)\n"' > deep.R
  /usr/bin/time -l Rscript deep.R    # rlang: 47.35 real, 5708693504 max RSS
  ```

`options()$digits` being `NULL` (with `getOption("digits")` answering 7) is the
case already recorded under *Printing and formatting*: a bare `options()`
returns only what has been set.
