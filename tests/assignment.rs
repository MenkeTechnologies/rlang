//! The value and evaluation order of a complex assignment, and the subscript
//! checks and errors of `[`, `[[`, `$` and their replacement forms.
//!
//! `x[i] <- v`, `x$a <- v` and `f(x) <- v` are R's `applydefine`: the
//! right-hand side is evaluated first, the target is rebuilt and assigned back,
//! and the value of the whole expression is the right-hand side — not the
//! rebuilt object. Each expected string was captured from the reference
//! `Rscript` (R 4.6.1) running the same program.

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
fn replacement_assignment_yields_its_right_hand_side() {
    // The value of f(x) <- v and x[i] <- v is v, not the rebuilt object.
    assert_eq!(
        merged("z <- 1:10; print(length(z) <- 3); print(z); x <- c(1, 2); y <- (names(x) <- c(\"a\", \"b\")); print(y); l <- list(); print(l$a <- 5); print(l[[\"b\"]] <- \"q\"); m <- 1:4; k <- (m[2] <- 99L); print(k); print(m[[3]] <- 7L); q <- 1:3; print(q[5] <- 1)"),
        "[1] 3\n[1] 1 2 3\n[1] \"a\" \"b\"\n[1] 5\n[1] \"q\"\n[1] 99\n[1] 7\n[1] 1\n"
    );
}

#[test]
fn a_function_ending_in_a_replacement_returns_the_value() {
    // A closure whose last statement is a complex assignment returns the right-hand side.
    assert_eq!(
        merged("f <- function(x) { attr(x, \"foo\") <- \"bar\" }; print(f(1)); g <- function(x) { x[2] <- 10 }; print(g(1:3)); h <- function(v) { v$q <- 1 }; print(h(list())); x <- 1; k <- function() { names(x) <- \"a\" }; print(k()); print(x)"),
        "[1] \"bar\"\n[1] 10\n[1] 1\n[1] \"a\"\n[1] 1\n"
    );
}

#[test]
fn the_right_hand_side_is_evaluated_before_the_subscript() {
    // applydefine evaluates the right-hand side first, then the target's subscripts.
    assert_eq!(
        merged("x <- 1:3; x[{cat(\"i\\n\"); 1}] <- {cat(\"v\\n\"); 9L}; print(x); n <- c(a = 1); names(n)[{cat(\"idx\\n\"); 1}] <- {cat(\"val\\n\"); \"A\"}; print(n)"),
        "v\ni\n[1] 9 2 3\nval\nidx\nA \n1 \n"
    );
}

#[test]
fn the_returned_value_is_not_aliased_to_the_container() {
    // The value handed back and the element stored are independent copies under modification.
    assert_eq!(
        merged("l <- list(); v <- (l$a <- c(1, 2)); v[1] <- 100; print(l$a); print(v); y <- c(1, 2, 3); z <- (y[2] <- 50); z[1] <- -1; print(y); print(z)"),
        "[1] 1 2\n[1] 100   2\n[1]  1 50  3\n[1] -1\n"
    );
}

#[test]
fn nested_and_super_replacement_targets() {
    // Nested targets and <<- still write back through every level.
    assert_eq!(
        merged("f <- function() { g <- function() x[1] <<- 77L; g() }; x <- 1:3; print(f()); print(x); n <- c(a = 1, b = 2); names(n)[2] <- \"B\"; print(n); e <- list(a = list(b = 1)); print(e$a$c <- 3); str(e); ff <- factor(c(\"a\", \"b\")); levels(ff)[1] <- \"z\"; print(ff)"),
        "[1] 77\n[1] 77  2  3\na B \n1 2 \n[1] 3\nList of 1\n $ a:List of 2\n  ..$ b: num 1\n  ..$ c: num 3\n[1] z b\nLevels: z b\n"
    );
}

#[test]
fn double_bracket_value_length_is_checked_before_the_subscript() {
    // do_subassign2_dflt rejects an empty or long value for an atomic target before resolving the index.
    assert_eq!(
        merged("x <- 1:6; for (s in list(quote(x[[0]] <- NULL), quote(x[[NA]] <- NULL), quote(x[[NA]] <- 1L), quote(x[[2]] <- integer(0)), quote(x[[10]] <- 1:2))) print(tryCatch(eval(s), error = function(e) conditionMessage(e))); m <- matrix(1:4, 2); print(tryCatch(m[[1, 5]] <- integer(0), error = function(e) conditionMessage(e)))"),
        "[1] \"replacement has length zero\"\n[1] \"replacement has length zero\"\n[1] \"attempt to select more than one element in integerOneIndex\"\n[1] \"replacement has length zero\"\n[1] \"more elements supplied than there are to replace\"\n[1] \"replacement has length zero\"\n"
    );
}

#[test]
fn subassignment_warnings_name_the_assignment() {
    // [<- and [[<- warn and fail under the whole assignment call, as applydefine evaluates them.
    assert_eq!(
        merged("x <- 1:10; x[5:6] <- 1:3; l <- list(a = 1:5); l$a[2:3] <- 1:3; f <- function() { y <- 1:4; y[1:3] <- 1:2; y }; z <- f(); r <- tryCatch(x[5:6] <- 1:3, warning = function(w) conditionCall(w)); print(r); x[[2]] <- 1:2"),
        "Warning message:\nIn x[5:6] <- 1:3 :\n  number of items to replace is not a multiple of replacement length\nWarning message:\nIn l$a[2:3] <- 1:3 :\n  number of items to replace is not a multiple of replacement length\nWarning message:\nIn y[1:3] <- 1:2 :\n  number of items to replace is not a multiple of replacement length\nx[5:6] <- 1:3\nError in x[[2]] <- 1:2 : more elements supplied than there are to replace\nExecution halted\n"
    );
}

#[test]
fn a_matrix_subassignment_error_names_the_assignment() {
    // An array that does not tile its selection is an error under the assignment call.
    assert_eq!(
        merged("m <- matrix(1:6, 2); m[1, ] <- 1:2"),
        "Error in m[1, ] <- 1:2 : \n  number of items to replace is not a multiple of replacement length\nExecution halted\n"
    );
}

#[test]
fn na_subscripts_in_a_matrix_read_select_na_cells() {
    // An NA row or column subscript selects NA cells, labelled NA, rather than nothing.
    assert_eq!(
        merged("m <- matrix(1:6, 2, dimnames = list(c(\"a\", \"b\"), c(\"x\", \"y\", \"z\"))); print(m[NA, ]); print(m[, c(NA, 1)]); print(m[NA, NA]); m2 <- matrix(1:4, 2); print(m2[c(TRUE, NA), ]); print(m2[NA_integer_, ]); a <- array(1:8, c(2, 2, 2)); print(a[NA, 1, ]); print(a[1, , NA])"),
        "      x  y  z\n<NA> NA NA NA\n<NA> NA NA NA\n  <NA> x\na   NA 1\nb   NA 2\n     <NA> <NA> <NA>\n<NA>   NA   NA   NA\n<NA>   NA   NA   NA\n     [,1] [,2]\n[1,]    1    3\n[2,]   NA   NA\n[1] NA NA\n     [,1] [,2]\n[1,]   NA   NA\n[2,]   NA   NA\n     [,1] [,2]\n[1,]   NA   NA\n[2,]   NA   NA\n"
    );
}

#[test]
fn na_subscripts_and_empty_values_in_subassignment() {
    // VectorAssign counts NA subscripts toward n; MatrixAssign and ArrayAssign refuse NA under a longer value, and a NULL value fails as not a multiple.
    assert_eq!(
        merged("x <- matrix(1:6, 2); for (s in list(quote(x[, c(1, 3)] <- NULL), quote(x[1, 1] <- NULL), quote(x[, 1] <- integer(0)), quote(x[NA, ] <- NULL), quote(x[NA] <- NULL), quote(x[NA] <- 1:2), quote(x[NA, ] <- c(10, 100, 2)), quote(x[0, ] <- NULL), quote(x[NA, 1] <- 5L))) print(tryCatch({ eval(s); \"ok\" }, error = function(e) conditionMessage(e))); a <- array(1:8, c(2, 2, 2)); for (s in list(quote(a[1, 1, 1] <- NULL), quote(a[NA, 1, 1] <- NULL), quote(a[NA, 1, 1] <- 1:2), quote(a[1, 1, ] <- integer(0)))) print(tryCatch({ eval(s); \"ok\" }, error = function(e) conditionMessage(e))); v <- 1:6; for (s in list(quote(v[NA] <- NULL), quote(v[c(NA, 2, 3)] <- 7:8))) print(tryCatch({ eval(s); v }, error = function(e) conditionMessage(e), warning = function(w) conditionMessage(w)))"),
        "[1] \"number of items to replace is not a multiple of replacement length\"\n[1] \"number of items to replace is not a multiple of replacement length\"\n[1] \"replacement has length zero\"\n[1] \"NAs are not allowed in subscripted assignments\"\n[1] \"replacement has length zero\"\n[1] \"NAs are not allowed in subscripted assignments\"\n[1] \"NAs are not allowed in subscripted assignments\"\n[1] \"ok\"\n[1] \"ok\"\n[1] \"number of items to replace is not a multiple of replacement length\"\n[1] \"number of items to replace is not a multiple of replacement length\"\n[1] \"NAs are not allowed in subscripted assignments\"\n[1] \"replacement has length zero\"\n[1] \"replacement has length zero\"\n[1] \"NAs are not allowed in subscripted assignments\"\n"
    );
}

#[test]
fn a_missing_name_in_a_matrix_subscript_is_out_of_bounds() {
    // A character NA matches no dimname.
    assert_eq!(
        merged("m <- matrix(1:6, 2, dimnames = list(c(\"a\", \"b\"), c(\"x\", \"y\", \"z\"))); m[c(\"a\", NA), \"y\"]"),
        "Error in m[c(\"a\", NA), \"y\"] : subscript out of bounds\nExecution halted\n"
    );
}

#[test]
fn subscript_errors_name_the_subscript_call() {
    // errorcall from [, [[ and $ names the subscript expression, inside closures too.
    assert_eq!(
        merged("x <- 1:3; f <- function() x[[5]]; print(tryCatch(x[[\"a\"]], error = function(e) conditionCall(e))); print(tryCatch(x$a, error = function(e) deparse(conditionCall(e)))); f()"),
        "x[[\"a\"]]\n[1] \"x$a\"\nError in x[[5]] : subscript out of bounds\nCalls: f\nExecution halted\n"
    );
}

#[test]
fn length_assignment_keeps_names_even_when_emptied() {
    // xlengthgets carries names to the new length, character(0) included, and pads with "".
    assert_eq!(
        merged("x <- c(a = 1, b = 2); length(x) <- 0; print(x); print(names(x)); l <- list(a = 1); length(l) <- 0; print(l); z <- c(a = 1); length(z) <- 3; print(z)"),
        "named numeric(0)\ncharacter(0)\nnamed list()\n a       \n 1 NA NA \n"
    );
}
