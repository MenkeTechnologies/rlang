//! Semantics tests against the library API.
//!
//! Each case asserts a behavior that R defines and a naive implementation gets
//! wrong — vector recycling, copy-on-modify, function-position lookup, lazy
//! defaults, `<<-` scoping, and R's index forms. `eval_to_string` returns what
//! `print` would show, so the assertions read like a transcript.

use rlang::eval_to_string;

fn r(src: &str) -> String {
    eval_to_string(src).unwrap_or_else(|e| panic!("{src}\n  failed: {e}"))
}

#[test]
fn a_scalar_is_a_length_one_vector() {
    assert_eq!(r("length(1)"), "[1] 1");
    assert_eq!(r("1"), "[1] 1");
    assert_eq!(r("c(1, 2) + 1"), "[1] 2 3");
}

#[test]
fn arithmetic_recycles_the_shorter_operand() {
    assert_eq!(r("1:6 + c(10, 20)"), "[1] 11 22 13 24 15 26");
}

#[test]
fn integer_stays_integer_but_division_does_not() {
    assert_eq!(r("typeof(1L + 1L)"), "[1] \"integer\"");
    assert_eq!(r("typeof(1L / 1L)"), "[1] \"double\"");
    assert_eq!(r("typeof(1)"), "[1] \"double\"");
}

#[test]
fn the_four_index_forms() {
    assert_eq!(r("(1:5)[2]"), "[1] 2");
    assert_eq!(r("(1:5)[-2]"), "[1] 1 3 4 5");
    assert_eq!(r("(1:5)[c(TRUE, FALSE)]"), "[1] 1 3 5");
    assert_eq!(r("c(a = 1, b = 2)[\"b\"]"), "b \n2");
    // A positive subscript past the end is NA, not an error.
    assert_eq!(r("(1:2)[5]"), "[1] NA");
}

#[test]
fn assignment_copies_rather_than_aliases() {
    assert_eq!(r("x <- c(1, 2); y <- x; y[1] <- 99; x"), "[1] 1 2");
    assert_eq!(r("l <- list(a = 1); m <- l; m$a <- 2; l$a"), "[1] 1");
}

#[test]
fn assigning_past_the_end_grows_with_na() {
    assert_eq!(r("x <- c(1, 2); x[4] <- 4; x"), "[1]  1  2 NA  4");
}

#[test]
fn nested_replacement_rebuilds_the_whole_target() {
    assert_eq!(r("l <- list(v = 1:3); l$v[2] <- 99; l$v"), "[1]  1 99  3");
    assert_eq!(
        r("x <- 1:3; names(x) <- c(\"a\", \"b\", \"c\"); names(x)[2]"),
        "[1] \"b\""
    );
}

#[test]
fn function_position_skips_non_function_bindings() {
    // R finds the `c` function even when `c` is also bound to a value.
    assert_eq!(r("c <- 1; c(1, 2)"), "[1] 1 2");
}

#[test]
fn defaults_may_refer_to_other_arguments() {
    assert_eq!(r("f <- function(x, y = x * 2) x + y; f(3)"), "[1] 9");
}

#[test]
fn superassignment_writes_to_the_enclosing_frame() {
    assert_eq!(
        r("counter <- function() { n <- 0; function() { n <<- n + 1; n } }; s <- counter(); s(); s(); s()"),
        "[1] 3"
    );
}

#[test]
fn dots_forward_arguments_including_tags() {
    assert_eq!(r("f <- function(...) sum(...); f(1, 2, 3)"), "[1] 6");
    assert_eq!(
        r("f <- function(...) paste(..., sep = \"-\"); f(\"a\", \"b\")"),
        "[1] \"a-b\""
    );
}

#[test]
fn return_exits_the_function_early() {
    assert_eq!(
        r("f <- function(x) { if (x < 0) return(\"neg\"); \"pos\" }; f(-1)"),
        "[1] \"neg\""
    );
}

#[test]
fn s3_dispatch_walks_the_class_vector_then_default() {
    assert_eq!(
        r("area <- function(s) UseMethod(\"area\")
           area.square <- function(s) s$side^2
           area.default <- function(s) 0
           sq <- structure(list(side = 3), class = c(\"square\", \"shape\"))
           area(sq)"),
        "[1] 9"
    );
    assert_eq!(
        r("f <- function(x) UseMethod(\"f\"); f.default <- function(x) \"fallback\"; f(1)"),
        "[1] \"fallback\""
    );
}

#[test]
fn conditions_reject_na_and_empty_vectors() {
    assert!(eval_to_string("if (NA) 1").is_err());
    assert!(eval_to_string("if (logical(0)) 1").is_err());
    assert!(eval_to_string("undefined_name").is_err());
    assert!(eval_to_string("stop(\"boom\")").is_err());
}

#[test]
fn short_circuit_operators_do_not_evaluate_the_right_side() {
    // `nonexistent` would error if `||` evaluated it.
    assert_eq!(r("TRUE || nonexistent"), "[1] TRUE");
    assert_eq!(r("FALSE && nonexistent"), "[1] FALSE");
}

#[test]
fn user_defined_infix_operators_dispatch_by_name() {
    assert_eq!(
        r("`%+%` <- function(a, b) paste0(a, b); \"x\" %+% \"y\""),
        "[1] \"xy\""
    );
}

#[test]
fn the_native_pipe_inserts_the_left_side_first() {
    assert_eq!(r("c(3, 1, 2) |> sort() |> rev()"), "[1] 3 2 1");
}

#[test]
fn matrices_are_column_major_vectors_with_dim() {
    assert_eq!(r("m <- matrix(1:6, nrow = 2); m[2, 3]"), "[1] 6");
    assert_eq!(r("m <- matrix(1:6, nrow = 2); m[, 2]"), "[1] 3 4");
    assert_eq!(r("dim(matrix(1:6, ncol = 2))"), "[1] 3 2");
}

#[test]
fn empty_vectors_print_their_type() {
    assert_eq!(r("character(0)"), "character(0)");
    assert_eq!(r("integer(0)"), "integer(0)");
    assert_eq!(r("NULL"), "NULL");
}

#[test]
fn wide_vectors_wrap_with_index_prefixes() {
    let out = r("1:30");
    let lines: Vec<&str> = out.lines().collect();
    assert!(lines.len() > 1, "expected wrapping, got {out:?}");
    assert!(lines[0].starts_with(" [1]"));
    assert!(lines.iter().all(|l| l.len() <= 80));
}

/// The transcript a program produces, including `cat` output, so a test can
/// assert on *where* evaluation went rather than only on its final value.
fn transcript(src: &str) -> String {
    rlang::eval_capture(src).trim_end().to_string()
}

#[test]
fn a_calling_handler_resumes_at_the_signalling_point() {
    // The distinction from `tryCatch`: the handler runs, and then the statement
    // *after* `warning()` still runs. Verified against Rscript 4.6.1.
    assert_eq!(
        transcript(
            "print(withCallingHandlers({ warning(\"w\"); cat(\"resumed\\n\"); 7 }, \
             warning = function(x) { cat(\"H\\n\"); invokeRestart(\"muffleWarning\") }))"
        ),
        "H\nresumed\n[1] 7"
    );
}

#[test]
fn a_calling_handler_that_returns_normally_still_resumes() {
    // No restart invoked: R runs the handler, resumes, and then applies the
    // warning's default action, which goes to stderr and not to this transcript.
    assert_eq!(
        transcript(
            "print(withCallingHandlers({ warning(\"w\"); cat(\"resumed\\n\"); 7 }, \
             warning = function(x) cat(\"H\\n\")))"
        ),
        "H\nresumed\n[1] 7"
    );
}

#[test]
fn calling_handlers_run_inner_to_outer_until_one_muffles() {
    // Both handlers see it, innermost first, and the outer one muffles.
    assert_eq!(
        transcript(
            "withCallingHandlers(withCallingHandlers({ warning(\"w\"); cat(\"resumed\\n\") }, \
             warning = function(x) cat(\"inner\\n\")), \
             warning = function(x) { cat(\"outer\\n\"); invokeRestart(\"muffleWarning\") })"
        ),
        "inner\nouter\nresumed"
    );
    // Muffling in the inner handler ends the search: the outer never runs.
    assert_eq!(
        transcript(
            "withCallingHandlers(withCallingHandlers({ warning(\"w\"); cat(\"resumed\\n\") }, \
             warning = function(x) { cat(\"inner\\n\"); invokeRestart(\"muffleWarning\") }), \
             warning = function(x) cat(\"outer\\n\"))"
        ),
        "inner\nresumed"
    );
}

#[test]
fn an_exiting_handler_stops_the_search_and_unwinds() {
    // Calling handler first, then the enclosing `tryCatch` tears the stack down,
    // so the marker after the signal never runs.
    assert_eq!(
        transcript(
            "print(tryCatch(withCallingHandlers({ warning(\"w\"); cat(\"NOT\\n\"); 1 }, \
             warning = function(x) cat(\"calling\\n\")), \
             warning = function(x) paste(\"exiting\", conditionMessage(x))))"
        ),
        "calling\n[1] \"exiting w\""
    );
    // The other nesting: the innermost handler is the exiting one, so the outer
    // calling handler is never reached at all.
    assert_eq!(
        transcript(
            "print(withCallingHandlers(tryCatch({ warning(\"w\"); 1 }, \
             warning = function(x) \"exiting\"), warning = function(x) cat(\"NOT\\n\")))"
        ),
        "[1] \"exiting\""
    );
}

#[test]
fn a_handler_does_not_re_enter_its_own_frame() {
    // The inner `warning` must not reach the handler that is running, or the
    // handler recurses forever.
    assert_eq!(
        transcript(
            "withCallingHandlers({ warning(\"w\"); cat(\"resumed\\n\") }, \
             warning = function(x) { cat(\"H\\n\"); suppressWarnings(warning(\"again\")); \
             invokeRestart(\"muffleWarning\") })"
        ),
        "H\nresumed"
    );
}

#[test]
fn a_restart_transfers_control_to_the_frame_that_established_it() {
    assert_eq!(
        transcript("print(withRestarts(invokeRestart(\"r1\", 5), r1 = function(v) v * 2))"),
        "[1] 10"
    );
    // Everything after `invokeRestart` is skipped.
    assert_eq!(
        transcript(
            "print(withRestarts({ cat(\"body\\n\"); invokeRestart(\"r1\"); cat(\"NOT\\n\") }, \
             r1 = function() \"done\"))"
        ),
        "body\n[1] \"done\""
    );
    // Established but not invoked: the body's own value comes out.
    assert_eq!(
        transcript("print(withRestarts(9, r1 = function() 0))"),
        "[1] 9"
    );
}

#[test]
fn a_restart_transfer_is_not_an_error_and_runs_cleanups_on_the_way() {
    // `tryCatch(error =)` must not absorb the transfer, but `finally` still runs.
    assert_eq!(
        transcript(
            "print(withRestarts(tryCatch(invokeRestart(\"r1\", 3), \
             error = function(e) \"WRONG\", finally = cat(\"fin\\n\")), \
             r1 = function(v) paste(\"restart\", v)))"
        ),
        "fin\n[1] \"restart 3\""
    );
    assert_eq!(
        transcript(
            "print(withRestarts((function() { on.exit(cat(\"exit\\n\")); \
             invokeRestart(\"r1\", 4) })(), r1 = function(v) paste(\"restart\", v)))"
        ),
        "exit\n[1] \"restart 4\""
    );
}

#[test]
fn nested_restarts_of_the_same_name_resolve_innermost_first() {
    assert_eq!(
        transcript(
            "print(withRestarts(withRestarts(invokeRestart(\"r1\", 2), \
             r1 = function(v) paste(\"inner\", v)), r1 = function(v) paste(\"outer\", v)))"
        ),
        "[1] \"inner 2\""
    );
    // A restart *object* names one exact frame, so it reaches the outer one even
    // though an inner restart shares its name.
    assert_eq!(
        transcript(
            "print(withRestarts(withRestarts({ x <- computeRestarts()[[2]]; \
             invokeRestart(x, 2) }, r1 = function(v) paste(\"inner\", v)), \
             r1 = function(v) paste(\"outer\", v)))"
        ),
        "[1] \"outer 2\""
    );
}

#[test]
fn the_signalling_builtins_establish_the_muffle_restarts() {
    // R puts `muffleWarning` around `warning()` and `muffleMessage` around
    // `message()`, plus the evaluator's own `abort`.
    // Untrimmed: the second line is the `abort` restart, whose `$name` is NULL,
    // so `cat` writes only the separator and the newline.
    assert_eq!(
        rlang::eval_capture(
            "withCallingHandlers(warning(\"w\"), warning = function(x) { \
             for (y in computeRestarts()) cat(y$name, \"\\n\"); \
             invokeRestart(\"muffleWarning\") })"
        ),
        "muffleWarning \n \n"
    );
    // Outside a signal only `abort` is established.
    assert_eq!(transcript("print(length(computeRestarts()))"), "[1] 1");
    assert_eq!(
        transcript("print(computeRestarts()[[1]])"),
        "<restart: abort >"
    );
}

#[test]
fn invoking_a_restart_that_is_not_established_is_an_error() {
    assert_eq!(
        transcript(
            "print(tryCatch(invokeRestart(\"nope\"), error = function(e) conditionMessage(e)))"
        ),
        "[1] \"no 'restart' 'nope' found\""
    );
}

#[test]
fn suppress_wrappers_muffle_without_stopping_the_body() {
    assert_eq!(
        transcript("print(suppressWarnings({ warning(\"s\"); cat(\"resumed\\n\"); 3 }))"),
        "resumed\n[1] 3"
    );
    assert_eq!(
        transcript("print(suppressMessages({ message(\"s\"); cat(\"resumed\\n\"); 4 }))"),
        "resumed\n[1] 4"
    );
    assert_eq!(
        transcript("print(suppressWarnings(as.numeric(\"zz\")))"),
        "[1] NA"
    );
}
