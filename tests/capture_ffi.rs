//! CI-safe tests for the output-capture entry point (the wasm build's bridge)
//! and the inline-Rust FFI error path. Neither needs R, `rustc`, or a linker —
//! the full FFI compile+call and the AOT `.fvm` link are exercised manually and
//! by `aot_runtime`'s unit tests; here we lock the parts that must hold in any
//! headless environment.

/// `eval_capture` must collect everything R writes to stdout — top-level
/// autoprint, `print`, and `cat` — into the returned string, in order. This is
/// exactly what the wasm host reads back, so a regression that let output escape
/// to the process stdout (bypassing `host::emit`) would surface here.
#[test]
fn eval_capture_collects_all_r_output() {
    let out = rlang::eval_capture("print(1 + 1)\ncat(\"x\", 42, \"\\n\")\n7 * 6");
    assert_eq!(out, "[1] 2\nx 42 \n[1] 42\n");
}

/// A run error is appended as a trailing `Error:` line rather than lost, so a
/// single wasm call always yields a complete transcript.
#[test]
fn eval_capture_reports_errors_inline() {
    let out = rlang::eval_capture("cat(\"before\\n\")\nstop(\"boom\")");
    assert!(out.starts_with("before\n"), "prior output kept: {out:?}");
    assert!(out.contains("boom"), "error surfaced: {out:?}");
}

/// `.Call` on a name that was never registered by `.rust()` must fail with a
/// clear message — not silently return NULL or panic.
#[test]
fn dot_call_on_unregistered_routine_errors() {
    let err = rlang::eval_str("invisible(.Call(\"never_registered\", 1L))")
        .expect_err("unregistered .Call must error");
    assert!(
        err.contains("not a registered native routine"),
        "unexpected error: {err}"
    );
}
