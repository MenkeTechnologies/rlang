//! `local` and evaluation in a supplied environment. Each expected string was
//! captured from the reference `Rscript` (R 4.6.1) running the same program.

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
fn local_with_an_environment_evaluates_there() {
    // local(expr, envir) is eval(quote(expr), envir): bindings land in envir.
    assert_eq!(
        merged("e <- new.env(); local({ y <- 2 }, envir = e); print(get(\"y\", e)); local(envir = e, z <- 3); print(e$z); print(exists(\"y\")); print(local({ a <- 2; a * 3 }))"),
        "[1] 2\n[1] 3\n[1] FALSE\n[1] 6\n"
    );
}
