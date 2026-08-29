//! What auto-prints at a script's top level, and what does not.
//!
//! R sets `R_Visible = TRUE` on entry to a call and lets only the CALLEE's own
//! return clear it. An ARGUMENT that turns itself invisible therefore does not
//! hide the call's own value: `inherits(invisible(1), "numeric")` prints
//! `[1] TRUE`. Here the argument cleared the flag and nothing set it back, so
//! the value was computed correctly and then never printed — silence where R
//! prints, which a stdout diff of a `print()`-wrapped corpus cannot see.
//!
//! It only ever affected a name reached through the closure-context path (one
//! outside `compiler::R_PRIMITIVES`), whose arguments arrive as promises and are
//! forced inside `call_primitive` — AFTER `call_op`'s entry reset. The reset now
//! happens between forcing the arguments and running the primitive, which is R's
//! own order. Found by `parity-fuzz` (seed 31646) as
//! `inherits(try(stop("foo"), silent = TRUE), "try-error")`.
//!
//! Every expectation was read off the reference `Rscript` (R 4.6.1); the
//! assertions are literal, so no R install is needed to run them.

use std::process::Command;

/// Run a program through the built `Rscript` and return its stdout, which is
/// where auto-printing lands — the whole point of these cases.
fn out(program: &str) -> String {
    let rscript = env!("CARGO_BIN_EXE_Rscript");
    let out = Command::new(rscript)
        .arg("-e")
        .arg(program)
        // The whole-script fallback into an embedded GNU R would answer for
        // rlang and hide whatever rlang itself did.
        .env("RLANG_NO_CRAN", "1")
        .output()
        .expect("run Rscript binary");
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// The six primitives whose result an invisible argument used to hide, plus the
/// `try` shape the fuzzer reduced to.
#[test]
fn an_invisible_argument_does_not_hide_the_calls_value() {
    assert_eq!(out(r#"inherits(invisible(1), "numeric")"#), "[1] TRUE\n");
    assert_eq!(out(r#"nchar(invisible("ab"))"#), "[1] 2\n");
    assert_eq!(out(r#"paste(invisible("a"))"#), "[1] \"a\"\n");
    assert_eq!(out(r#"toupper(invisible("a"))"#), "[1] \"A\"\n");
    assert_eq!(out(r#"rev(invisible(c(1, 2)))"#), "[1] 2 1\n");
    assert_eq!(out(r#"substr(invisible("abc"), 1, 2)"#), "[1] \"ab\"\n");
    assert_eq!(
        out(r#"inherits(try(stop("foo"), silent = TRUE), "try-error")"#),
        "[1] TRUE\n"
    );
}

/// The eager path — a name IN `R_PRIMITIVES`, whose arguments are evaluated
/// before the entry reset — was always right and must stay so.
#[test]
fn the_eager_path_still_prints() {
    assert_eq!(out("length(invisible(1))"), "[1] 1\n");
    assert_eq!(out("sum(invisible(1))"), "[1] 1\n");
    assert_eq!(out("sqrt(invisible(4))"), "[1] 2\n");
    assert_eq!(out("class(invisible(1))"), "[1] \"numeric\"\n");
}

/// What must stay silent: the callee's own `invisible`, an assignment, and the
/// wrappers that carry their argument's visibility through. `identity` is R's
/// `function(x) x`, so its value is the promise's and so is its visibility.
#[test]
fn what_is_invisible_stays_invisible() {
    assert_eq!(out("invisible(5)"), "");
    assert_eq!(out("x <- 5"), "");
    assert_eq!(out("identity(invisible(1))"), "");
    assert_eq!(out("suppressWarnings(invisible(1))"), "");
    assert_eq!(out("try(stop(\"foo\"), silent = TRUE)"), "");
    // A closure decides its own visibility from its last evaluated expression:
    // a bare symbol carries the promise's, a constant does not.
    assert_eq!(out("f <- function(x) x\nf(invisible(1))"), "");
    assert_eq!(out("g <- function(x) { x; 5 }\ng(invisible(1))"), "[1] 5\n");
    assert_eq!(out("h <- function(x) 99\nh(invisible(1))"), "[1] 99\n");
}

/// `force` and `withVisible` are the two names whose whole contract is the
/// visibility flag, and neither had a native arm: both fell through to the
/// embedded GNU R, which receives arguments rlang has ALREADY evaluated and so
/// cannot be told what the flag was. `force(invisible(1))` printed where R is
/// silent, and `withVisible` answered `TRUE` for every argument. Expectations
/// read off the reference `Rscript` (R 4.6.1).
#[test]
fn force_and_with_visible_report_the_arguments_flag() {
    assert_eq!(out("force(invisible(1))"), "");
    assert_eq!(out("force(1)"), "[1] 1\n");
    assert_eq!(
        out("withVisible(1)"),
        "$value\n[1] 1\n\n$visible\n[1] TRUE\n\n"
    );
    assert_eq!(
        out("withVisible(invisible(2))"),
        "$value\n[1] 2\n\n$visible\n[1] FALSE\n\n"
    );
    // The list `withVisible` builds is itself visible, whatever it reports.
    assert_eq!(out("withVisible(invisible(2))$visible"), "[1] FALSE\n");
    assert_eq!(out("withVisible(invisible(2))$value"), "[1] 2\n");
}
