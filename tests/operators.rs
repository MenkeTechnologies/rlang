//! The call an operator names in its warnings and errors, and `:`.
//!
//! R's arithmetic, comparison, logic and `:` raise through `warningcall` and
//! `errorcall` with the operator call itself, so `1:3 + 1:2` warns
//! `In 1:3 + 1:2 :` and `1 + "a"` fails `Error in 1 + "a" :`. Each expected
//! string was captured from the reference `Rscript` (R 4.6.1) running the same
//! program.

use std::process::Command;

/// Run a one-liner through the built `Rscript` with stderr merged into stdout.
fn merged(program: &str) -> String {
    let rscript = env!("CARGO_BIN_EXE_Rscript");
    let out = Command::new("sh")
        .arg("-c")
        .arg(format!("{rscript:?} -e \"$0\" 2>&1",))
        .arg(program)
        // Keep the whole-script fallback into an embedded GNU R out of it.
        .env("RLANG_NO_CRAN", "1")
        .output()
        .expect("run Rscript binary");
    String::from_utf8_lossy(&out.stdout).to_string()
}

#[test]
fn recycling_warnings_name_the_operator_call() {
    // Native + - * and the builtin operators alike name the operator call, inside closures too.
    assert_eq!(
        merged("x <- c(a = 1, b = 2); print(x * c(10, 100, 1000)); print(1:6 / 1:4); print(1:3 == 1:2); print(c(TRUE, FALSE, TRUE) & c(TRUE, FALSE)); print(1:3 %% 1:2); f <- function(v) v - 1:2; print(f(1:3)); g <- function(a, b) { s <- a + b; s * 2 }; print(g(1:3, 1:2))"),
        "[1]   10  200 1000\nWarning message:\nIn x * c(10, 100, 1000) :\n  longer object length is not a multiple of shorter object length\n[1] 1 1 1 1 5 3\nWarning message:\nIn 1:6/1:4 :\n  longer object length is not a multiple of shorter object length\n[1]  TRUE  TRUE FALSE\nWarning message:\nIn 1:3 == 1:2 :\n  longer object length is not a multiple of shorter object length\n[1]  TRUE FALSE  TRUE\nWarning message:\nIn c(TRUE, FALSE, TRUE) & c(TRUE, FALSE) :\n  longer object length is not a multiple of shorter object length\n[1] 0 0 0\nWarning message:\nIn 1:3%%1:2 :\n  longer object length is not a multiple of shorter object length\n[1] 0 0 2\nWarning message:\nIn v - 1:2 :\n  longer object length is not a multiple of shorter object length\n[1] 4 8 8\nWarning message:\nIn a + b : longer object length is not a multiple of shorter object length\n"
    );
}

#[test]
fn integer_overflow_names_the_operator_call() {
    // NAs produced by integer overflow is raised by warningcall on the operator.
    assert_eq!(
        merged("print(.Machine$integer.max + 1L); print(2147483647L * 2L); print(-2147483647L - 2L)"),
        "[1] NA\nWarning message:\nIn .Machine$integer.max + 1L : NAs produced by integer overflow\n[1] NA\nWarning message:\nIn 2147483647L * 2L : NAs produced by integer overflow\n[1] NA\nWarning message:\nIn -2147483647L - 2L : NAs produced by integer overflow\n"
    );
}

#[test]
fn operator_warnings_batch_and_catch_with_their_call() {
    // Each warning in a loop names its own operator; a caught one carries the call.
    assert_eq!(
        merged("for (i in 1:2) z <- i + 1:3 + 1:2; w <- tryCatch(1:3 + 1:2, warning = function(w) conditionCall(w)); print(w)"),
        "Warning messages:\n1: In i + 1:3 + 1:2 :\n  longer object length is not a multiple of shorter object length\n2: In i + 1:3 + 1:2 :\n  longer object length is not a multiple of shorter object length\n1:3 + 1:2\n"
    );
}

#[test]
fn a_long_operator_call_shows_its_first_line() {
    // The call is deparsed, and only its first line is shown.
    assert_eq!(
        merged("print(c(1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30) + c(1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29))"),
        " [1]  2  4  6  8 10 12 14 16 18 20 22 24 26 28 30 32 34 36 38 40 42 44 46 48 50\n[26] 52 54 56 58 31\nWarning message:\nIn c(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17,  :\n  longer object length is not a multiple of shorter object length\n"
    );
}

#[test]
fn an_operator_error_names_the_operator_call() {
    // errorcall(call, ...) from arithmetic names the operator, with the closure trace.
    assert_eq!(
        merged("f <- function() list(1) * 2; f()"),
        "Error in list(1) * 2 : non-numeric argument to binary operator\nCalls: f\nExecution halted\n"
    );
}

#[test]
fn an_operator_error_at_top_level() {
    // A top-level operator error names the operator.
    assert_eq!(
        merged("x <- 1 + \"a\""),
        "Error in 1 + \"a\" : non-numeric argument to binary operator\nExecution halted\n"
    );
}

#[test]
fn colon_uses_the_first_element_with_a_warning() {
    // do_colon warns once, using whichever end has more than one element.
    assert_eq!(
        merged("x <- c(3, 4); print(x:5); print(1:c(2, 9)); f <- function(n) 1:n; print(f(c(2, 3))); print(1:5^1:2)"),
        "[1] 3 4 5\nWarning message:\nIn x:5 : numerical expression has 2 elements: only the first used\n[1] 1 2\nWarning message:\nIn 1:c(2, 9) : numerical expression has 2 elements: only the first used\n[1] 1 2\nWarning message:\nIn 1:n : numerical expression has 2 elements: only the first used\n[1] 1 2\nWarning message:\nIn 1:5^1:2 : numerical expression has 5 elements: only the first used\n"
    );
}

#[test]
fn colon_types_follow_seq_colon() {
    // Whole ends inside int give integers; otherwise doubles.
    assert_eq!(
        merged("print(1.5:4); print(-1:-3); print(2147483646:2147483647); print(2147483647:2147483648); print(\"2\":\"4\"); print(TRUE:3); print(0.5:-2); print(typeof(1e10:1e10))"),
        "[1] 1.5 2.5 3.5\n[1] -1 -2 -3\n[1] 2147483646 2147483647\n[1] 2147483647 2147483648\n[1] 2 3 4\n[1] 1 2 3\n[1]  0.5 -0.5 -1.5\n[1] \"double\"\n"
    );
}

#[test]
fn colon_with_a_missing_end_is_an_error() {
    // An NA end is errorcall(call, NA/NaN argument).
    assert_eq!(
        merged("print(1:NA)"),
        "Error in 1:NA : NA/NaN argument\nCalls: print\nExecution halted\n"
    );
}

#[test]
fn colon_with_an_empty_end_is_an_error() {
    // A zero-length end is errorcall(call, argument of length 0).
    assert_eq!(
        merged("g <- function() integer(0):3; g()"),
        "Error in integer(0):3 : argument of length 0\nCalls: g\nExecution halted\n"
    );
}
