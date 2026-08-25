//! Where R's warnings appear, and in what shape.
//!
//! Under the default `options(warn = 0)` R does *not* print a warning where it
//! is raised: it queues it and prints the whole batch once the top-level
//! statement that raised it finishes, flushing stdout first so a reader with
//! both streams joined sees the statement's output before its warnings. Getting
//! that wrong is invisible to a stdout-only diff — the parity corpus cannot see
//! it — so these run the built `Rscript` with the two streams merged and assert
//! the interleaving directly.
//!
//! Every expectation here was read off the reference `Rscript` (R 4.6.1), but
//! the assertions are literal, so no R install is needed to run them.
//!
//! The `In <call> :` prefix is asserted here too, including the two rules that
//! are easy to get subtly wrong: *which* call a warning names — R's context
//! stack, not the innermost call — and when the message folds onto its own
//! line. The one case still without a prefix is a warning raised by an
//! arithmetic operator, whose call rlang's native binary ops do not carry (see
//! BUGS.md); it is asserted in the call-less form rlang actually produces.

use std::process::Command;

/// Run a one-liner through the built `Rscript` with stderr merged into stdout,
/// which is what makes the ordering between the two observable.
fn merged(program: &str) -> String {
    let rscript = env!("CARGO_BIN_EXE_Rscript");
    let out = Command::new("sh")
        .arg("-c")
        .arg(format!("{rscript:?} -e \"$0\" 2>&1",))
        .arg(program)
        // The whole-script fallback into an embedded GNU R would answer for
        // rlang and hide whatever rlang itself did.
        .env("RLANG_NO_CRAN", "1")
        .output()
        .expect("run Rscript binary");
    String::from_utf8_lossy(&out.stdout).to_string()
}

#[test]
fn a_warning_prints_after_the_statement_that_raised_it() {
    // The value comes first even though the warning was raised while computing
    // it: rlang printed the warning immediately before, which put it above the
    // `[1] NaN` that R shows above the warning.
    assert_eq!(
        merged("print(sqrt(-1))"),
        "[1] NaN\nWarning message:\nIn sqrt(-1) : NaNs produced\n"
    );

    // …and each statement flushes its own batch, so the two interleave rather
    // than all the warnings landing ahead of all the output.
    assert_eq!(
        merged(r#"warning("a"); print(1); warning("b"); print(2)"#),
        "Warning message:\na \n[1] 1\nWarning message:\nb \n[1] 2\n"
    );
}

#[test]
fn a_batch_of_warnings_takes_r_s_three_shapes() {
    // Two to ten: a plural banner, numbered.
    assert_eq!(
        merged(r#"for (i in 1:3) warning(paste("w", i)); print("done")"#),
        "Warning messages:\n1: w 1 \n2: w 2 \n3: w 3 \n[1] \"done\"\n"
    );
    // More than ten: a count instead of the messages.
    assert_eq!(
        merged(r#"invisible(sapply(1:12, function(i) warning("w"))); print("done")"#),
        "There were 12 warnings (use warnings() to see them)\n[1] \"done\"\n"
    );
    // At R's `nwarnings` ceiling the count stops being exact.
    assert_eq!(
        merged(r#"invisible(sapply(1:60, function(i) warning("w"))); print("done")"#),
        "There were 50 or more warnings (use warnings() to see the first 50)\n[1] \"done\"\n"
    );
}

#[test]
fn options_warn_selects_deferred_immediate_or_silent() {
    // A positive `warn` prints at the point of the warning, under a different
    // banner — so it lands *before* the value, unlike the deferred form.
    assert_eq!(
        merged(r#"options(warn = 1); warning("a"); print(1)"#),
        "Warning: a\n[1] 1\n"
    );
    // With a call it takes the `Warning in <call> :` form instead.
    assert_eq!(
        merged(r#"options(warn = 1); f <- function() warning("a"); f(); print(1)"#),
        "Warning in f() : a\n[1] 1\n"
    );
    // A negative one drops warnings entirely.
    assert_eq!(
        merged(r#"options(warn = -1); warning("a"); print(1)"#),
        "[1] 1\n"
    );
    // And it reads back as R's default before anything sets it.
    assert_eq!(merged(r#"print(getOption("warn"))"#), "[1] 0\n");
}

#[test]
fn the_internal_warnings_r_raises_are_raised_too() {
    // Recycling that does not tile exactly. The result is still produced.
    // R names the arithmetic call here — `In 1:3 + 1:2 :` — which rlang cannot
    // (BUGS.md): `+` lowers to a native fusevm op carrying no call text. The
    // call-less form is what it produces, and a wrong call would be worse.
    assert_eq!(
        merged("print(1:3 + 1:2)"),
        "[1] 2 4 4\nWarning message:\nlonger object length is not a multiple of shorter object length \n"
    );
    // An exact multiple is silent — this is the half that a naive "warn on any
    // unequal lengths" check gets wrong.
    assert_eq!(merged("print(1:6 + 1:3)"), "[1] 2 4 6 5 7 9\n");

    // Text that will not parse as a number.
    assert_eq!(
        merged(r#"print(as.integer("x"))"#),
        "[1] NA\nWarning message:\nIn print(as.integer(\"x\")) : NAs introduced by coercion\n"
    );
    // An NA that was already NA in the source did not come *from* the
    // coercion, so a source with no unparseable element stays silent.
    assert_eq!(merged(r#"print(as.numeric(c("1", NA)))"#), "[1]  1 NA\n");

    // Both ends of an empty range warn, one per extreme.
    assert_eq!(
        merged("print(range(numeric(0)))"),
        "[1]  Inf -Inf\nWarning messages:\n1: In min(x) : no non-missing arguments to min; returning Inf\n2: In max(x) : no non-missing arguments to max; returning -Inf\n"
    );
}

/// Which call a warning names. R does not report the innermost call: it reports
/// a *context*, and only a closure makes one. So a warning raised inside a
/// primitive lands on the closure containing it, while a warning a primitive
/// raises about itself names that primitive — and the two can be the same
/// expression away from each other.
#[test]
fn a_warning_names_the_context_it_was_raised_in() {
    // `warning()` names its caller: R's `findCall` starts one context out, so
    // `warning()`'s own frame is skipped.
    assert_eq!(
        merged(r#"f <- function(x) { warning("inner"); x }; print(f(1))"#),
        "[1] 1\nWarning message:\nIn f(1) : inner\n"
    );
    // …and at top level there is no context to find, which is the call-less
    // form — message, trailing space.
    assert_eq!(merged(r#"warning("top")"#), "Warning message:\ntop \n");
    // The innermost one wins, not the outermost.
    assert_eq!(
        merged(r#"g <- function() warning("in g"); f <- function() g(); f()"#),
        "Warning message:\nIn g() : in g\n"
    );

    // A coercion warning has no call of its own, so it lands on the enclosing
    // *closure* — `print` is one in R, so it is named…
    assert_eq!(
        merged(r#"x <- rev(as.integer("x"))"#),
        "Warning message:\nIn rev(as.integer(\"x\")) : NAs introduced by coercion\n"
    );
    // …while `sum` is a primitive, makes no context, and leaves the same
    // warning at the same depth with no call at all. This is the half a
    // "report the enclosing call" rule gets wrong.
    assert_eq!(
        merged(r#"x <- sum(as.integer("x"))"#),
        "Warning message:\nNAs introduced by coercion \n"
    );

    // A primitive that raises a warning *about itself* names itself, however
    // deep it sits — and names the call as it was *written*, not as it was
    // evaluated: `sqrt(v)`, with the formal, not the -1 bound to it.
    assert_eq!(
        merged(r#"f <- function(v) sqrt(v); print(f(-1))"#),
        "[1] NaN\nWarning message:\nIn sqrt(v) : NaNs produced\n"
    );

    // The apply family builds its own call, so a warning from `FUN` reports
    // that rather than the `lapply(…)` the caller wrote.
    assert_eq!(
        merged(r#"invisible(lapply(1:2, function(i) warning("w")))"#),
        "Warning messages:\n1: In FUN(X[[i]], ...) : w\n2: In FUN(X[[i]], ...) : w\n"
    );
}

/// An error takes the same treatment: `Error in <call> :` with R's fold, a
/// bare `Error:` when it carries none, and — because both streams are merged
/// here — the order R puts the pieces in when a statement both warns and stops.
#[test]
fn an_error_reports_its_call_and_the_warnings_that_preceded_it() {
    assert_eq!(
        merged(r#"f <- function() stop("boom"); f()"#),
        "Error in f() : boom\nExecution halted\n"
    );
    // `stop()` skips its own frame like `warning()` does, so at top level there
    // is no call and R drops the `in` clause entirely.
    assert_eq!(
        merged(r#"stop("plain")"#),
        "Error: plain\nExecution halted\n"
    );
    // A primitive that fails names itself, not whatever encloses it.
    assert_eq!(
        merged(r#"f <- function(v) sqrt(v); f("x")"#),
        "Error in sqrt(v) : non-numeric argument to mathematical function\nCalls: f\nExecution halted\n"
    );
    // Long enough to fold, under the 14 columns R allows an error's decoration.
    assert_eq!(
        merged(r#"cat(list(5), "\n")"#),
        "Error in cat(list(5), \"\\n\") : \n  argument 1 (type 'list') cannot be handled by 'cat'\nExecution halted\n"
    );
    // R holds the statement's warnings until *after* the error line and prints
    // them under `In addition:`. rlang used to lose them here outright: they
    // were written into a diagnostics sink that had already been replayed.
    assert_eq!(
        merged(r#"f <- function() { warning("w"); stop("e") }; f()"#),
        "Error in f() : e\nIn addition: Warning message:\nIn f() : w\nExecution halted\n"
    );
}

/// R's `Calls:` line under an uncaught error — the chain of function contexts
/// it came through, outermost first, naming functions rather than calls.
#[test]
fn an_uncaught_error_shows_the_chain_it_came_through() {
    // `stop`'s own frame, and everything inside it, is dropped from the chain.
    assert_eq!(
        merged(
            r#"f <- function() stop("s"); g <- function() f(); h <- function() g(); h()"#
        ),
        "Error in f() : s\nCalls: h -> g -> f\nExecution halted\n"
    );
    // A chain that names only the call the error already reported is dropped:
    // `Error in f() : boom` says everything `Calls: f` would.
    assert_eq!(
        merged(r#"f <- function() stop("boom"); f()"#),
        "Error in f() : boom\nExecution halted\n"
    );
    // A callee that is not a plain symbol has no name to show.
    assert_eq!(
        merged(r#"f <- function() (function() stop("anon"))(); f()"#),
        "Error in (function() stop(\"anon\"))() : anon\nCalls: f -> <Anonymous>\nExecution halted\n"
    );
    // The apply family's own call is in the chain, under the name R's
    // implementation calls it by.
    assert_eq!(
        merged(r#"invisible(sapply(1:2, function(i) stop("z")))"#),
        "Error in FUN(X[[i]], ...) : z\nCalls: sapply -> lapply -> FUN\nExecution halted\n"
    );
    // Past `R_NShowCalls` the middle is elided, keeping the outermost frame.
    assert_eq!(
        merged(
            r#"aaaaaaaaaa <- function() stop("x"); bbbbbbbbbb <- function() aaaaaaaaaa(); cccccccccc <- function() bbbbbbbbbb(); dddddddddd <- function() cccccccccc(); eeeeeeeeee <- function() dddddddddd(); ffffffffff <- function() eeeeeeeeee(); ffffffffff()"#
        ),
        "Error in aaaaaaaaaa() : x\nCalls: ffffffffff ... dddddddddd -> cccccccccc -> bbbbbbbbbb -> aaaaaaaaaa\nExecution halted\n"
    );
    // The chain comes before the held warnings, which come before the exit.
    assert_eq!(
        merged(
            r#"f <- function() { warning("w"); stop("e") }; g <- function() f(); h <- function() g(); h()"#
        ),
        "Error in f() : e\nCalls: h -> g -> f\nIn addition: Warning message:\nIn f() : w\nExecution halted\n"
    );
}

/// R keeps a warning's message on the line with its call only while the whole
/// line fits `LONGWARN`; past that the message folds onto the next line,
/// indented two spaces. The allowance for the decoration differs per banner, so
/// the same message can fit under one and fold under another.
#[test]
fn a_long_warning_folds_onto_its_own_line() {
    // 6 + 13 + 45 = 64: fits.
    assert_eq!(
        merged(
            r#"f <- function(a, b) { warning("a warning message that is quite long indeed here"); 1 }; invisible(f(100, 200))"#
        ),
        "Warning message:\nIn f(100, 200) : a warning message that is quite long indeed here\n"
    );
    // 10 + 4 + 62 = 76: folds, and the numbered banner is what pushed it over.
    assert_eq!(
        merged(
            r#"f <- function(a) { warning("this message is long enough that it must fold onto its own line"); 1 }; g <- function() { f(1); f(2) }; invisible(g())"#
        ),
        "Warning messages:\n1: In f(1) :\n  this message is long enough that it must fold onto its own line\n2: In f(2) :\n  this message is long enough that it must fold onto its own line\n"
    );
    // `warn = 1` prints immediately and allows 18 columns, so it folds sooner.
    assert_eq!(
        merged(
            r#"options(warn=1); f <- function(aaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbb) warning("a message long enough to fold onto its own line"); f(1,2)"#
        ),
        "Warning in f(1, 2) : a message long enough to fold onto its own line\n"
    );
}

#[test]
fn a_muffled_warning_is_not_queued() {
    // `suppressWarnings` establishes the muffle restart, so nothing reaches the
    // batch — a deferred queue must not resurrect what a handler swallowed.
    assert_eq!(
        merged(r#"suppressWarnings(print(as.numeric("x"))); print(1)"#),
        "[1] NA\n[1] 1\n"
    );
    // A `tryCatch` that takes the warning unwinds before the default action.
    assert_eq!(
        merged(r#"print(tryCatch(as.numeric("x"), warning = function(w) "caught"))"#),
        "[1] \"caught\"\n"
    );
}
