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

#[test]
fn integer_arithmetic_overflows_to_na_at_32_bits() {
    // R's `integer` is a C `int` and `INT_MIN` is reserved for `NA_integer_`, so
    // the representable range is ±`INT_MAX`. rlang holds integers in `i64` and
    // lowers scalar `+ - *` to native fusevm ops, so without an explicit bound
    // these widened silently and answered `2147483648` where R answers `NA`.
    assert_eq!(r("2147483647L + 1L"), "[1] NA");
    assert_eq!(r("2147483647L * 2L"), "[1] NA");
    assert_eq!(r("-2147483647L - 2L"), "[1] NA");
    // `-2147483648` is `NA_integer_`'s own bit pattern, so it is out of range too.
    assert_eq!(r("-2147483647L - 1L"), "[1] NA");
    // The overflow is NA-per-element, not NA-for-the-whole-vector.
    assert_eq!(r("c(2147483647L, 1L) + c(1L, 1L)"), "[1] NA  2");
    // Overflow does not promote: the result is still an integer vector.
    assert_eq!(r("typeof(2147483647L + 1L)"), "[1] \"integer\"");
    // The bound applies to integers only — doubles carry on past 2^31.
    assert_eq!(r("2147483647 + 1"), "[1] 2147483648");
    assert_eq!(r("2147483647L + 0L"), "[1] 2147483647");
    assert_eq!(r("-2147483647L - 0L"), "[1] -2147483647");
}

#[test]
fn transpose_treats_a_dimensionless_vector_as_a_column() {
    // `t(1:3)` is the 1x3 *row* matrix: a vector with no `dim` transposes as an
    // n x 1 column. Reading it as 1 x n instead returned the transpose of R's
    // answer, and `t(t(x))` then failed to round-trip.
    assert_eq!(r("dim(t(1:3))"), "[1] 1 3");
    assert_eq!(r("dim(t(t(1:3)))"), "[1] 3 1");
    // A named vector's names label the single row's columns.
    assert_eq!(r("dimnames(t(c(a = 1, b = 2)))[[2]]"), "[1] \"a\" \"b\"");
    assert_eq!(r("is.null(dimnames(t(c(a = 1, b = 2)))[[1]])"), "[1] TRUE");
    // Transposing a matrix swaps its dimnames along with its margins.
    assert_eq!(
        r("dimnames(t(matrix(1:4, 2, dimnames = list(c(\"r1\", \"r2\"), c(\"c1\", \"c2\")))))[[1]]"),
        "[1] \"c1\" \"c2\""
    );
}

#[test]
fn append_splices_at_after_rather_than_at_the_tail() {
    // `after` is the position `values` is spliced in *after*; it was ignored, so
    // every insert landed at the tail regardless of the argument.
    assert_eq!(r("append(1:5, 0, after = 0)"), "[1] 0 1 2 3 4 5");
    assert_eq!(r("append(1:3, 99, after = 1)"), "[1]  1 99  2  3");
    // The default is `length(x)`, and an `after` past the end also appends.
    assert_eq!(r("append(1:3, 4:5)"), "[1] 1 2 3 4 5");
    assert_eq!(r("append(1:3, 99, after = 10)"), "[1]  1  2  3 99")
    ;
    // Both slices keep the names that travelled with them.
    assert_eq!(
        r("names(append(c(a = 1, b = 2, c = 3), c(z = 9), after = 1))"),
        "[1] \"a\" \"z\" \"b\" \"c\""
    );
}

#[test]
fn cut_spaces_interior_breaks_over_the_unwidened_range() {
    // R lays the breaks evenly across the data range and only then nudges the
    // first and last outward, so `cut(1:10, 3)` cuts at 4 and 7 and 4 falls in
    // the *first* bin. Spreading them across the widened range instead shifted
    // every interior break and mis-binned the boundary values.
    assert_eq!(
        r("levels(cut(1:10, 3))"),
        "[1] \"(0.991,4]\" \"(4,7]\"     \"(7,10]\""
    );
    // Ten elements widen the index prefix, so R leads the row with a space.
    assert_eq!(r("as.integer(cut(1:10, 3))"), " [1] 1 1 1 1 2 2 2 3 3 3");
    // `labels = FALSE` asks for the bin numbers, not a factor.
    assert_eq!(r("cut(c(1, 5, 9), 3, labels = FALSE)"), "[1] 1 2 3");
    // A constant `x` has no range to divide: R substitutes `abs(x)/1000` as the
    // half-width, and then has to raise `dig.lab` past 3 because 4.995, 5 and
    // 5.005 all render as "5" at three significant digits.
    assert_eq!(
        r("levels(cut(c(5, 5, 5), 2))"),
        "[1] \"(4.995,5]\" \"(5,5.005]\""
    );
}

#[test]
fn vapply_takes_its_result_type_from_fun_value_when_x_is_empty() {
    // With nothing to map over there is no result to infer a type from, which is
    // exactly what `FUN.VALUE` is for; the simplifier used to answer `list()`.
    assert_eq!(r("vapply(integer(0), function(x) \"a\", character(1))"), "character(0)");
    assert_eq!(r("typeof(vapply(list(), function(x) x, numeric(1)))"), "[1] \"double\"");
    // A character `X` still contributes its (empty) names, which is what makes R
    // print the `named` prefix.
    assert_eq!(r("vapply(character(0), nchar, integer(1))"), "named integer(0)");
    assert_eq!(r("names(vapply(character(0), nchar, integer(1)))"), "character(0)");
}

#[test]
fn sapply_simplifies_length_one_list_results_one_level() {
    // R simplifies uniformly length-1 results with `unlist(recursive = FALSE)`,
    // which flattens exactly one level — so a length-1 *list* result simplifies
    // to a flat list rather than staying doubly nested.
    assert_eq!(r("length(sapply(1:2, function(x) list(x)))"), "[1] 2");
    assert_eq!(r("sapply(1:2, function(x) list(x))[[2]]"), "[1] 2");
    // Longer list results have no matrix form, so those still stay a list.
    assert_eq!(r("length(sapply(1:2, function(x) list(x, x)))"), "[1] 2");
    assert_eq!(r("length(sapply(1:2, function(x) list(x, x))[[1]])"), "[1] 2");
}

#[test]
fn subsetting_a_named_vector_always_yields_names() {
    // An out-of-bounds index or an unmatched label selects `NA`, and R labels
    // that slot `NA_character_` — printed `<NA>`. The names attribute used to be
    // dropped whenever *every* selected name was missing.
    assert_eq!(r("c(a = 1, b = 2)[3]"), "<NA> \n  NA");
    assert_eq!(r("is.na(names(c(a = 1, b = 2)[3]))"), "[1] TRUE");
    assert_eq!(r("c(a = 1, b = 2, c = 3)[c(\"a\", \"zz\")]"), "   a <NA> \n   1   NA");
    // The same applies to lists, where an NA name heads its element `$<NA>`.
    assert_eq!(r("list(a = 1, b = 2)[\"zz\"]"), "$<NA>\nNULL");
    // An *unnamed* vector still has no names, so it prints without a header.
    assert_eq!(r("(1:2)[5]"), "[1] NA");
    // R pads the untagged slots of a partially named list with the empty string,
    // not NA — and an empty name numbers its element rather than heading it.
    assert_eq!(r("names(list(a = 1, 2))"), "[1] \"a\" \"\"");
    assert_eq!(r("list(a = 1, 2)[[2]]"), "[1] 2");
}

#[test]
fn zero_extent_matrices_and_null_keep_their_own_print_forms() {
    // A matrix with no rows still prints its column header over a row-label
    // gutter fixed at the width of `[1,]`, whatever the dimnames would have been.
    assert_eq!(r("matrix(1:4, 2)[0, ]"), "     [,1] [,2]");
    assert_eq!(
        r("matrix(1:4, 2, dimnames = list(c(\"rrrr1\", \"r2\"), c(\"a\", \"b\")))[0, ]"),
        "     a b"
    );
    // With no columns there is nothing after the gutter, and the separating
    // space is not left dangling on the header or on any row.
    assert_eq!(r("matrix(1:4, 2)[, 0]"), "    \n[1,]\n[2,]");
    // No rows *and* no columns: R names the shape rather than printing a header.
    assert_eq!(r("matrix(character(0), 0, 0)"), "<0 x 0 matrix>");
    // `format(NULL)` is the word, not the empty character vector that formatting
    // NULL's zero elements would give — `format(character(0))` is that.
    assert_eq!(r("format(NULL)"), "[1] \"NULL\"");
    assert_eq!(r("format(character(0))"), "character(0)");
}
