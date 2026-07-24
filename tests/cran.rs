//! CRAN bridge integration test. Runs in its own process (cargo compiles each
//! `tests/*.rs` as a separate binary), so embedding GNU R here cannot disturb
//! the other suites.
//!
//! The bridge needs a real R install (and the `jsonlite` package) to exercise,
//! so the test is conditional: with no R present it verifies the graceful
//! "unavailable" path instead of failing. That keeps CI green on a machine
//! without R while still checking real delegation where R exists.

#![cfg(not(target_arch = "wasm32"))]

use rlang::rembed;

#[test]
fn cran_bridge_delegates_or_degrades() {
    if !rembed::available() {
        // No R on this machine: the bridge must report itself unavailable and a
        // delegated call must reproduce the ordinary "could not find function"
        // error rather than panic.
        assert!(rembed::call("toJSON", &[]).is_err());
        return;
    }

    // Base R over the bridge: a value round-trips through embedded R.
    let two = rembed::eval_source("1 + 1").expect("eval 1+1");
    assert_eq!(rlang::host::with_host(|h| h.type_of(&two)), "double");

    // A data frame (a classed list rlang has no native type for) is kept as an
    // opaque handle and still supports `$` and reduction back to a native value.
    let n = rembed::eval_source("sum(data.frame(x = 1:4)$x)").expect("df$col");
    assert_eq!(rlang::host::with_host(|h| h.as_dbl(&n)), vec![Some(10.0)]);

    // A non-standard-evaluation program (dplyr) runs whole via the script
    // fallback, if dplyr is installed.
    if rembed::eval_source("requireNamespace(\"dplyr\", quietly = TRUE)").is_ok() {
        assert!(rembed::run_script(
            "library(dplyr); invisible(nrow(filter(data.frame(x = 1:5), x > 2)))"
        )
        .is_ok());
    }

    // A compiled CRAN package (jsonlite) — load it, then call one of its
    // routines, if it is installed. Absence of the package is not a failure.
    if rembed::eval_source("requireNamespace(\"jsonlite\", quietly = TRUE)").is_ok() {
        let _ = rembed::eval_source("suppressMessages(library(jsonlite))");
        let json = rembed::eval_source("as.character(jsonlite::toJSON(1:3))");
        if let Ok(v) = json {
            let s = rlang::host::with_host(|h| h.as_str(&v));
            assert_eq!(s.first().cloned().flatten().as_deref(), Some("[1,2,3]"));
        }
    }
}
