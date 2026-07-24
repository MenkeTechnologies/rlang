//! rlang — R as a fusevm frontend.
//!
//! Pipeline: `lexer` → `parser` builds an R AST → `compiler` lowers it to a
//! `fusevm::Chunk` (plus a chunk per closure body) → fusevm executes it, calling
//! back into the `host` through registered builtins for every R-specific
//! operation. There is no bespoke VM or JIT here — execution and codegen live in
//! fusevm.

// Cross-target: the R front-end (lex → parse → lower) and the vector heap run
// identically on native and on `wasm32-unknown-unknown`.
pub mod ast;
pub mod builtins;
pub mod compiler;
pub mod ffi;
pub mod host;
pub mod intercepts;
pub mod lexer;
pub mod parser;

// Native-only: Cranelift AOT, the on-disk cache, and the LSP/DAP/REPL/CLI
// frontends all need a real OS and are excluded from the wasm build.
#[cfg(not(target_arch = "wasm32"))]
pub mod aot;
#[cfg(not(target_arch = "wasm32"))]
pub mod aot_runtime;
// The CRAN bridge embeds GNU R over FFI; native-only (no libR on wasm).
#[cfg(not(target_arch = "wasm32"))]
pub mod rembed;
#[cfg(not(target_arch = "wasm32"))]
pub mod banner;
#[cfg(not(target_arch = "wasm32"))]
pub mod cache;
#[cfg(not(target_arch = "wasm32"))]
pub mod cli;
#[cfg(not(target_arch = "wasm32"))]
pub mod dap;
#[cfg(not(target_arch = "wasm32"))]
pub mod lsp;
#[cfg(not(target_arch = "wasm32"))]
pub mod repl;

// wasm-only: the `rlang_eval` C-ABI export for the web-worker host.
#[cfg(target_arch = "wasm32")]
pub mod wasm;

pub use fusevm::Value;

/// Compile a source string to a runnable program.
pub fn compile(src: &str) -> Result<compiler::Program, String> {
    compiler::compile(&parser::parse(src)?)
}

/// Parse, compile, and run R source on a fresh host, echoing top-level values
/// the way `Rscript` does. Returns the last expression's value.
pub fn eval_str(src: &str) -> Result<Value, String> {
    host::reset_host();
    run_compiled(compile(src)?)
}

/// Like [`eval_str`], but with the top-level echo off — for embedding and for
/// tests that want the value, not the transcript.
pub fn eval_quiet(src: &str) -> Result<Value, String> {
    host::reset_host();
    host::with_host(|h| h.echo = false);
    run_compiled(compile(src)?)
}

/// Run an already-compiled program on the current (freshly reset) host.
pub fn run_compiled(prog: compiler::Program) -> Result<Value, String> {
    host::with_host(|h| h.load_closures(prog.closures));
    host::run_main(prog.main)
}

/// Read and run a `.R` file.
pub fn eval_file(path: &str) -> Result<Value, String> {
    let src = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    eval_str(&src)
}

/// Disassemble a compiled program: the top-level chunk and every closure body,
/// each op numbered by its index so jump targets read directly.
pub fn disasm(prog: &compiler::Program) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let mut chunk = |title: String, c: &fusevm::Chunk| {
        let _ = writeln!(out, "== {title} ==");
        for (i, op) in c.ops.iter().enumerate() {
            let _ = writeln!(out, "{i:>5}  {op:?}");
        }
        if !c.constants.is_empty() {
            let _ = writeln!(out, "  constants: {:?}", c.constants);
        }
    };
    chunk("main".to_string(), &prog.main);
    for (i, c) in prog.closures.iter().enumerate() {
        chunk(format!("closure #{i} ({})", c.params.join(", ")), &c.chunk);
    }
    out
}

/// Evaluate `src` and return what `print` would show for the last value — the
/// convenience entry point for tests.
pub fn eval_to_string(src: &str) -> Result<String, String> {
    let v = eval_quiet(src)?;
    Ok(builtins::format_value(&v).join("\n").trim_end().to_string())
}

/// Evaluate `src` with top-level echo on, capturing everything R would write to
/// stdout (autoprint, `print`, `cat`) into a returned string instead of the
/// process stdout. This is the entry point the wasm build hands to its JS host
/// (`src/wasm.rs`) — wasm has no real stdout — and it is unit-testable on native
/// targets. A compile or run error is appended as a trailing `Error:` line so a
/// single call always yields a complete transcript.
pub fn eval_capture(src: &str) -> String {
    host::reset_host();
    host::start_capture();
    let result = compile(src).and_then(run_compiled);
    let mut out = host::take_capture();
    if let Err(e) = result {
        out.push_str(&format!("Error: {e}\n"));
    }
    out
}
