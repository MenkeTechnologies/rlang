//! Element equality in `match`, `%in%`, `unique`, `duplicated` and the set
//! operations, and the `dim` predicates.
//!
//! R's hash tables (`src/main/unique.c`) compare doubles by value and strings by
//! text, after `match`'s coercion to a common type, so two numbers that merely
//! print alike are different elements. Each expected string was captured from
//! the reference `Rscript` (R 4.6.1) running the same program.

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
fn doubles_that_print_alike_are_distinct_elements() {
    // requal compares doubles by value, so 0.1 + 0.2 and 0.3 neither match nor deduplicate.
    assert_eq!(
        merged("print(duplicated(c(0.1 + 0.2, 0.3))); print(unique(c(0.1 + 0.2, 0.3))); print(match(0.1 + 0.2, 0.3)); print(0.3 %in% seq(0.1, 0.5, 0.1)); print(is.element(0.3, 0.1 + 0.2)); print(union(0.1 + 0.2, 0.3)); print(duplicated(c(-0, 0)))"),
        "[1] FALSE FALSE\n[1] 0.3 0.3\n[1] NA\n[1] FALSE\n[1] FALSE\n[1] 0.3 0.3\n[1] FALSE  TRUE\n"
    );
}

#[test]
fn match_coerces_to_the_common_type() {
    // match5 compares as character when either side is text, else as numbers; NA and NaN only match themselves.
    assert_eq!(
        merged("print(match(c(\"1\", \"2\"), 1:2)); print(match(TRUE, 1)); print(match(c(1, NA, NaN), c(NaN, NA, 1))); print(match(3, 1:2, nomatch = 0L)); print(match(c(1, 2), c(2, 1), incomparables = 2)); print(c(\"a\", NA) %in% c(NA, \"b\")); print(match(factor(c(\"x\", \"y\")), c(\"y\", \"x\"))); print(match(list(1, \"a\"), list(\"a\", 1))); print(union(1:3, c(\"3\", \"x\")))"),
        "[1] 1 2\n[1] 1\n[1] 3 2 1\n[1] 0\n[1]  2 NA\n[1] FALSE  TRUE\n[1] 2 1\n[1] 2 1\n[1] \"1\" \"2\" \"3\" \"x\"\n"
    );
}

#[test]
fn duplicated_honours_from_last_and_incomparables() {
    // fromLast scans from the end; incomparables are never duplicates; list elements compare with identical.
    assert_eq!(
        merged("print(duplicated(c(1, 2, 1), fromLast = TRUE)); print(duplicated(c(1, 2, 1, 3), incomparables = 1)); print(unique(c(1, 2, 1), fromLast = TRUE)); x <- c(5, 3, 5, NA, NA, 3); print(anyDuplicated(x)); print(anyDuplicated(x, fromLast = TRUE)); print(duplicated(list(1, \"1\", 1))); print(unique(list(1, 1L, \"1\", 1))); print(duplicated(c(1, NA, NA, NaN, NaN)))"),
        "[1]  TRUE FALSE FALSE\n[1] FALSE FALSE FALSE FALSE\n[1] 2 1\n[1] 3\n[1] 4\n[1] FALSE FALSE  TRUE\n[[1]]\n[1] 1\n\n[[2]]\n[1] 1\n\n[[3]]\n[1] \"1\"\n\n[1] FALSE FALSE  TRUE FALSE  TRUE\n"
    );
}

#[test]
fn matrix_methods_work_on_rows() {
    // duplicated.matrix and unique.matrix compare whole rows.
    assert_eq!(
        merged("m <- matrix(c(1, 1, 3, 2, 2, 4), 3); print(duplicated(m)); print(unique(m)); print(anyDuplicated(m)); print(duplicated(m, fromLast = TRUE)); dm <- matrix(c(\"a\", \"a\", \"b\", \"b\"), 2, dimnames = list(c(\"r1\", \"r2\"), c(\"x\", \"y\"))); print(unique(dm))"),
        "[1] FALSE  TRUE FALSE\n     [,1] [,2]\n[1,]    1    2\n[2,]    3    4\n[1] 2\n[1]  TRUE FALSE FALSE\n   x   y  \nr1 \"a\" \"b\"\n"
    );
}

#[test]
fn factor_levels_are_the_distinct_labels() {
    // factor() levels are unique(as.character(sorted unique values)), so numbers that print alike share a level.
    assert_eq!(
        merged("x <- c(0.1 + 0.2, 0.3); f <- factor(x); print(levels(f)); print(as.integer(f)); print(table(x)); print(tapply(1:2, x, sum))"),
        "[1] \"0.3\"\n[1] 1 1\nx\n0.3 \n  2 \n0.3 \n  3 \n"
    );
}

#[test]
fn dim_predicates() {
    // is.matrix wants two extents, is.array any dim attribute.
    assert_eq!(
        merged("print(is.matrix(matrix(1))); print(is.array(1:3)); print(is.array(matrix(1))); print(is.matrix(array(1:8, c(2, 2, 2)))); print(is.array(array(1:8, c(2, 2, 2)))); print(is.matrix(factor(\"a\")))"),
        "[1] TRUE\n[1] FALSE\n[1] TRUE\n[1] FALSE\n[1] TRUE\n[1] FALSE\n"
    );
}
