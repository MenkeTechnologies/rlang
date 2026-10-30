//! Seeded random streams and the normal distribution, against R.
//!
//! `set.seed` followed by `runif`, `rnorm` or `sample` must reproduce R's own
//! numbers exactly — scripts are written against those values — so each case
//! runs the built `Rscript` and compares its merged stdout/stderr with what the
//! reference `Rscript` (R 4.6.1) printed for the same program. Every expected
//! string was captured from that interpreter, not written by hand.

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
fn seeded_streams_match_r() {
    // set.seed's scramble, Mersenne-Twister, inversion normals and the rejection sampler, one stream.
    assert_eq!(
        merged("set.seed(42); print(runif(5)); print(rnorm(5)); print(sample(10)); print(sample(5, 3, replace = TRUE))"),
        "[1] 0.9148060 0.9370754 0.2861395 0.8304476 0.6417455\n[1]  0.04788474 -1.10459944  0.53902380  0.58020632 -0.65750284\n [1]  5  4  2  8  3  1 10  6  7  9\n[1] 5 5 4\n"
    );
}

#[test]
fn normal_parameters_recycle_over_the_draws() {
    // rnorm/runif recycle mean/sd over n; a length-n vector n asks for n draws.
    assert_eq!(
        merged("set.seed(3); print(rnorm(4, c(0, 100))); print(runif(c(9, 9, 9), 1, 2)); print(rnorm(0)); print(round(runif(3, 10, 20), 2))"),
        "[1] -0.9619334 99.7074743  0.2587882 98.8478681\n[1] 1.577610 1.630979 1.512016\nnumeric(0)\n[1] 15.05 15.34 15.57\n"
    );
}

#[test]
fn probability_sampling_takes_each_of_rs_three_paths() {
    // ProbSampleReplace, Walker's alias (more than 200 likely cells) and ProbSampleNoReplace.
    assert_eq!(
        merged("set.seed(7); print(sample(5, 10, replace = TRUE, prob = c(0.1, 0.2, 0.3, 0.2, 0.2))); print(sample(300, 5, replace = TRUE, prob = rep(1, 300))); print(sample(1:5, 3, prob = c(5, 1, 1, 1, 1))); print(sample(5, 3, prob = c(0, 0, 1, 1, 1)))"),
        " [1] 1 2 3 3 3 5 2 1 3 2\n[1] 168  59 268 296 287\n[1] 3 1 2\n[1] 3 4 5\n"
    );
}

#[test]
fn sample_draws_elements_through_subsetting() {
    // sample(x) on a non-count vector indexes x, keeping names and factor levels.
    assert_eq!(
        merged("set.seed(5); print(sample(c(a = 1, b = 2, c = 3))); print(sample(factor(c(\"x\", \"y\", \"z\")))); print(sample(letters, 4)); print(sample(2.5))"),
        "b a c \n2 1 3 \n[1] z x y\nLevels: x y z\n[1] \"i\" \"u\" \"g\" \"w\"\n[1] 1 2\n"
    );
}

#[test]
fn rounding_sampler_is_the_pre_3_6_stream() {
    // sample.kind = "Rounding" warns and switches R_unif_index to floor(n * u).
    assert_eq!(
        merged("set.seed(123, sample.kind = \"Rounding\"); print(sample(10)); set.seed(123, sample.kind = \"Rejection\"); print(sample(10))"),
        "Warning message:\nIn set.seed(123, sample.kind = \"Rounding\") :\n  non-uniform 'Rounding' sampler used\n [1]  3  8  4  7  6  1 10  9  2  5\n [1]  3 10  2  8  6  9  1  7  5  4\n"
    );
}

#[test]
fn sample_larger_than_population_names_the_inner_call() {
    // The error is raised under sample's own sample.int call, as R's closure makes it.
    assert_eq!(
        merged("sample(1:3, 5)"),
        "Error in sample.int(length(x), size, replace, prob) : \n  cannot take a sample larger than the population when 'replace = FALSE'\nCalls: sample -> sample.int\nExecution halted\n"
    );
}

#[test]
fn invalid_parameters_warn_nas_produced() {
    // An invalid parameter is NaN in the result with one NAs produced warning.
    assert_eq!(
        merged("set.seed(1); print(rnorm(2, sd = -1)); print(runif(3, 1, c(2, NA)))"),
        "[1] NaN NaN\nWarning message:\nIn rnorm(2, sd = -1) : NAs produced\n[1] 1.265509      NaN 1.372124\nWarning message:\nIn runif(3, 1, c(2, NA)) : NAs produced\n"
    );
}

#[test]
fn bad_seed_is_an_error() {
    // asInteger of the seed: a string warns then fails, NA fails.
    assert_eq!(
        merged("set.seed(\"x\")"),
        "Error in set.seed(\"x\") : supplied seed is not a valid integer\nIn addition: Warning message:\nIn set.seed(\"x\") : NAs introduced by coercion\nExecution halted\n"
    );
}

#[test]
fn normal_density_distribution_and_quantile() {
    // dnorm/pnorm/qnorm across Cody's and Wichura's branches, tails, log scale.
    assert_eq!(
        merged("print(dnorm(c(-1, 0, 1.5, 10, 40))); print(pnorm(c(-40, -3, -1.96, 0, 0.5, 1.96, 7, 40))); print(qnorm(c(1e-300, 0.001, 0.025, 0.5, 0.975))); print(pnorm(1, lower.tail = FALSE, log.p = TRUE)); print(qnorm(-1000, log.p = TRUE)); print(qnorm(-1e5, log.p = TRUE)); print(dnorm(1:3, mean = c(0, 1), sd = 2, log = TRUE))"),
        "[1] 2.419707e-01 3.989423e-01 1.295176e-01 7.694599e-23 0.000000e+00\n[1] 0.000000000 0.001349898 0.024997895 0.500000000 0.691462461 0.975002105\n[7] 1.000000000 1.000000000\n[1] -37.047096  -3.090232  -1.959964   0.000000   1.959964\n[1] -1.841022\n[1] -44.61575\n[1] -447.1979\n[1] -1.737086 -1.737086 -2.737086\n"
    );
}

#[test]
fn normal_functions_keep_the_longest_arguments_attributes() {
    // math3 copies attributes from the longest argument; NaN from valid input warns.
    assert_eq!(
        merged("print(dnorm(c(a = 1, b = 2))); print(pnorm(1, mean = matrix(1:4, 2))); print(qnorm(c(0.5, NA, NaN))); print(qnorm(2))"),
        "         a          b \n0.24197072 0.05399097 \n          [,1]        [,2]\n[1,] 0.5000000 0.022750132\n[2,] 0.1586553 0.001349898\n[1]   0  NA NaN\n[1] NaN\nWarning message:\nIn qnorm(2) : NaNs produced\n"
    );
}

#[test]
fn sample_int_hashes_beyond_ten_million() {
    // sample.int takes do_sample2 for n > 1e7 without replacement and do_sample otherwise; n past INT_MAX gives doubles.
    assert_eq!(
        merged("set.seed(10); print(sample(1e8, 5)); print(sample(3e9, 2)); print(sample(2e7, 3, replace = TRUE)); print(class(sample(3e9, 2)))"),
        "[1] 32132745 88715632 97204663 94793215 84842639\n[1] 1841696074 1712838157\n[1]  7166727 10108246 18618325\n[1] \"numeric\"\n"
    );
}
