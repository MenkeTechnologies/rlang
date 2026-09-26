//! `solve`, `det`, `determinant`, the matrix products, and the array index
//! helpers, against the reference `Rscript` (R 4.6.1, reference LAPACK 3.12.1
//! over OpenBLAS 0.3.34 on arm64).
//!
//! The numeric cases are asserted through `sprintf("%a")`, so they pin every
//! bit: R's answers depend on the order its BLAS kernels sum in, and a
//! textbook LU or dot product agrees with them only to a few ULPs — which
//! `print(solve(m) %*% m)` shows as a different pattern of `1e-16` residues.
//! Every expectation was read off the reference; the assertions are literal,
//! so no R install is needed to run them.

use std::process::Command;

/// Run a program through the built `Rscript` with stderr merged into stdout.
fn merged(program: &str) -> String {
    let rscript = env!("CARGO_BIN_EXE_Rscript");
    let out = Command::new("sh")
        .arg("-c")
        .arg(format!("{rscript:?} -e \"$0\" 2>&1",))
        .arg(program)
        .env("RLANG_NO_CRAN", "1")
        .output()
        .expect("run Rscript binary");
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// A 20×20 / 70×70 test matrix both implementations build by exact arithmetic
/// (a decimal literal would not do: R's own `strtod` misreads some 17-digit
/// literals on this platform, so the two would not even start from the same
/// matrix).
const BIG: &str = "a <- function(n) { i <- seq_len(n * n); matrix(((i * i * 31 + i * 17) %% 1009 - 504) / 7, n) }";

#[test]
fn solve_inverts_and_solves_with_rs_bits() {
    assert_eq!(
        merged("m <- matrix(c(2,1,1,3), 2); print(solve(m)); print(solve(m, c(1,2)))"),
        "     [,1] [,2]\n[1,]  0.6 -0.2\n[2,] -0.2  0.4\n[1] 0.2 0.6\n"
    );
    assert_eq!(
        merged("m <- matrix(c(1,2,3,4,5,6,7,8,10),3); cat(sprintf('%a', solve(m)))"),
        "-0x1.5555555555551p-1 -0x1.555555555555dp+0 0x1.0000000000004p+0 \
         -0x1.555555555555dp-1 0x1.d55555555555dp+1 -0x1.0000000000004p+1 \
         0x1.0000000000002p+0 -0x1.0000000000004p+1 0x1.0000000000004p+0"
    );
    // Past the 8-row register block of the triangular-solve kernel …
    assert_eq!(
        merged(&format!("{BIG}; cat(sprintf('%a', solve(a(20), seq_len(20))[c(1, 10, 20)]), sprintf('%a', det(a(20))))")),
        "-0x1.1f4707e9e16ep-4 0x1.213611959dcf5p-2 0x1.3789b428e453cp-2 -0x1.67b2a36cff252p+137"
    );
    // … and past LAPACK's 64-column block, where dgetrf stops being one
    // recursive dgetrf2 and updates the trailing matrix panel by panel.
    assert_eq!(
        merged(&format!(
            "{BIG}; cat(sprintf('%a', solve(a(70))[c(1, 2450, 4900)]))"
        )),
        "-0x1.fd19cb405f807p-9 -0x1.e855f8eb9fb05p-13 -0x1.fc64e80b95492p-10"
    );
}

#[test]
fn solve_labels_the_result_from_the_matrix() {
    // rownames(ans) = colnames(a); the identity right-hand side carries
    // rownames(a) as its colnames.
    assert_eq!(
        merged("x <- matrix(c(4,2,7,6),2,dimnames=list(c('a','b'),c('x','y'))); print(solve(x)); print(solve(x, c(1,2)))"),
        "     a    b\nx  0.6 -0.7\ny -0.2  0.4\n   x    y \n-0.8  0.6 \n"
    );
    assert_eq!(merged("print(solve(c(a = 4)))"), "        a\n[1,] 0.25\n");
}

#[test]
fn solve_raises_rs_errors_from_solve_default() {
    assert_eq!(
        merged("solve(matrix(c(1,2,2,4),2))"),
        "Error in solve.default(matrix(c(1, 2, 2, 4), 2)) : \n  \
         Lapack routine dgesv: system is exactly singular: U[2,2] = 0\n\
         Calls: solve -> solve.default\nExecution halted\n"
    );
    // The reciprocal condition number is dgecon's estimate, to R's digits.
    assert_eq!(
        merged("r <- try(solve(matrix(c(1,2,2,4+1e-15),2)), silent = TRUE); cat(r)"),
        "Error in solve.default(matrix(c(1, 2, 2, 4 + 1e-15), 2)) : \n  \
         system is computationally singular: reciprocal condition number = 2.46716e-17\n"
    );
    assert_eq!(
        merged("r <- try(solve(matrix(1:6,2)), silent = TRUE); cat(r)"),
        "Error in solve.default(matrix(1:6, 2)) : 'a' (2 x 3) must be square\n"
    );
    assert_eq!(
        merged("m <- diag(3); r <- try(solve(m, 1:2), silent = TRUE); cat(r)"),
        "Error in solve.default(m, 1:2) : \n  'b' (2 x 1) must be compatible with 'a' (3 x 3)\n"
    );
}

#[test]
fn det_and_determinant() {
    assert_eq!(
        merged("print(det(matrix(c(1,2,3,4,5,6,7,8,10),3)))"),
        "[1] -3\n"
    );
    assert_eq!(
        merged("print(determinant(matrix(1:4,2)))"),
        "$modulus\n[1] 0.6931472\nattr(,\"logarithm\")\n[1] TRUE\n\n$sign\n[1] -1\n\nattr(,\"class\")\n[1] \"det\"\n"
    );
    assert_eq!(
        merged("print(det(matrix(c(NA,1,2,3),2))); print(det(matrix(0,0,0)))"),
        "[1] NA\n[1] 1\n"
    );
    assert_eq!(
        merged("det(matrix(1:6,2))"),
        "Error in determinant.matrix(x, logarithm = TRUE, ...) : \n  'x' must be a square matrix\n\
         Calls: det -> determinant -> determinant.matrix\nExecution halted\n"
    );
    assert_eq!(
        merged("det(1:3)"),
        "Error in UseMethod(\"determinant\") : \n  no applicable method for 'determinant' applied to an object of class \"c('integer', 'numeric')\"\n\
         Calls: det -> determinant\nExecution halted\n"
    );
}

#[test]
fn matrix_products_sum_as_rs_blas_does() {
    // A row vector times a matrix is dgemv('T'), whose kernel sums in eight
    // lanes; crossprod with a one-column side goes the same way.
    let setup = "v <- ((seq_len(9) * 29) %% 31 - 15) / 3; i <- seq_len(81); a <- matrix(((i * i * 31 + i * 17) %% 1009 - 504) / 7, 9)";
    let want = "-0x1.02e79e79e79e8p+9 -0x1.e0c30c30c30bfp+5 0x1.9186186186188p+5";
    assert_eq!(
        merged(&format!("{setup}; cat(sprintf('%a', v %*% a)[1:3])")),
        want
    );
    assert_eq!(
        merged(&format!(
            "{setup}; cat(sprintf('%a', crossprod(a, v))[1:3])"
        )),
        want
    );
    // An NA is propagated, not read as zero.
    assert_eq!(
        merged("print(matrix(c(NA,1,2,3),2) %*% c(1,1))"),
        "     [,1]\n[1,]   NA\n[2,]    4\n"
    );
    // outer's default "*" is a matrix product: double, whatever the inputs.
    assert_eq!(merged("print(typeof(outer(1:3, 1:2)))"), "[1] \"double\"\n");
    // A vector is one column to tcrossprod.
    assert_eq!(
        merged("print(tcrossprod(1:2))"),
        "     [,1] [,2]\n[1,]    1    2\n[2,]    2    4\n"
    );
}

#[test]
fn array_index_helpers() {
    assert_eq!(
        merged("print(arrayInd(5, c(2,3)))"),
        "     [,1] [,2]\n[1,]    1    3\n"
    );
    assert_eq!(
        merged("print(arrayInd(c(2,5), c(2,3), list(c('a','b'), NULL), useNames=TRUE))"),
        "  row col\nb   2   1\na   1   3\n"
    );
    assert_eq!(
        merged("print(slice.index(matrix(1:6, 2), 2))"),
        "     [,1] [,2] [,3]\n[1,]    1    2    3\n[2,]    1    2    3\n"
    );
    assert_eq!(
        merged("r <- try(slice.index(matrix(1:4,2), 3), silent = TRUE); cat(r)"),
        "Error in slice.index(matrix(1:4, 2), 3) : incorrect value for 'MARGIN'\n"
    );
}

#[test]
fn dim_assignment_checks_the_length() {
    assert_eq!(
        merged("z <- 1:6; dim(z) <- c(2,3); dim(z)[1] <- 3"),
        "Error in dim(z)[1] <- 3 : \n  dims [product 9] do not match the length of object [6]\nExecution halted\n"
    );
    // Setting dim drops the names; NULL drops dim and dimnames.
    assert_eq!(
        merged("x <- c(a=1,b=2,c=3,d=4); dim(x) <- c(2,2); print(names(attributes(x)))"),
        "[1] \"dim\"\n"
    );
    assert_eq!(
        merged("m <- matrix(1:4,2,dimnames=list(c('r','s'),NULL)); dim(m) <- NULL; print(attributes(m))"),
        "NULL\n"
    );
}
