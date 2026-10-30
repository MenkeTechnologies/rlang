//! How a factor prints: its labels, then a `Levels:` line in `print.factor`'s
//! shape. Each expected string was captured from the reference `Rscript`
//! (R 4.6.1) running the same program.

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
fn a_levels_line_that_does_not_fit_is_elided() {
    // print.factor keeps the leading levels that fit, then ... and the last, led by the count.
    assert_eq!(
        merged("print(cut(c(2, 9, 1), c(10), right = FALSE)); print(factor(letters)); print(factor(LETTERS, ordered = TRUE)); f <- factor(1:30); print(f); print(factor(c(strrep(\"x\", 70), \"y\"))); print(factor(c(strrep(\"w\", 90))))"),
        "[1] [1.8,2.6)   [8.2,9.01)  [0.992,1.8)\n10 Levels: [0.992,1.8) [1.8,2.6) [2.6,3.4) [3.4,4.2) [4.2,5) ... [8.2,9.01)\n [1] a b c d e f g h i j k l m n o p q r s t u v w x y z\nLevels: a b c d e f g h i j k l m n o p q r s t u v w x y z\n [1] A B C D E F G H I J K L M N O P Q R S T U V W X Y Z\n26 Levels: A < B < C < D < E < F < G < H < I < J < K < L < M < N < O < ... < Z\n [1] 1  2  3  4  5  6  7  8  9  10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25\n[26] 26 27 28 29 30\n30 Levels: 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 ... 30\n[1] xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\n[2] y                                                                     \n2 Levels: xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx ...\n[1] wwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwww\nLevels: wwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwwww\n"
    );
}

#[test]
fn a_levels_line_that_fits_is_whole() {
    // Short level sets print in full, NA level included.
    assert_eq!(
        merged("print(factor(paste0(\"level_number_\", 1:6))); print(factor(c(\"a\", NA), exclude = NULL)); print(factor(character(0)))"),
        "[1] level_number_1 level_number_2 level_number_3 level_number_4 level_number_5\n[6] level_number_6\n6 Levels: level_number_1 level_number_2 level_number_3 ... level_number_6\n[1] a    <NA>\nLevels: a <NA>\nfactor()\nLevels: \n"
    );
}
