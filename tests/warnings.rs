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
//! Not asserted: the `In <call> :` prefix R puts on a warning raised inside a
//! call. rlang's conditions carry no call (see BUGS.md), so its batches carry
//! the call-less form R uses at top level — message plus a trailing space.

use std::process::Command;

/// Run a one-liner through the built `Rscript` with stderr merged into stdout,
/// which is what makes the ordering between the two observable.
fn merged(program: &str) -> String {
    let rscript = env!("CARGO_BIN_EXE_Rscript");
    let out = Command::new("sh")
        .arg("-c")
        .arg(format!("{rscript:?} -e \"$0\" 2>&1", ))
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
    assert_eq!(merged("print(sqrt(-1))"), "[1] NaN\nWarning message:\nNaNs produced \n");

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
        "[1] NA\nWarning message:\nNAs introduced by coercion \n"
    );
    // An NA that was already NA in the source did not come *from* the
    // coercion, so a source with no unparseable element stays silent.
    assert_eq!(merged(r#"print(as.numeric(c("1", NA)))"#), "[1]  1 NA\n");

    // Both ends of an empty range warn, one per extreme.
    assert_eq!(
        merged("print(range(numeric(0)))"),
        "[1]  Inf -Inf\nWarning messages:\n1: no non-missing arguments to min; returning Inf \n2: no non-missing arguments to max; returning -Inf \n"
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
