//! rlang — R as a fusevm frontend.
//!
//! Pipeline: `lexer` → `parser` builds an R AST → `compiler` lowers it to a
//! `fusevm::Chunk` (plus a chunk per closure body) → fusevm executes it, calling
//! back into the `host` through registered builtins for every R-specific
//! operation. There is no bespoke VM or JIT here — execution and codegen live in
//! fusevm.

pub mod aot;
pub mod ast;
pub mod banner;
pub mod builtins;
pub mod cache;
pub mod cli;
pub mod compiler;
pub mod host;
pub mod intercepts;
pub mod lexer;
pub mod lsp;
pub mod parser;
pub mod repl;

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

/// Evaluate `src` and return what `print` would show for the last value — the
/// convenience entry point for tests.
pub fn eval_to_string(src: &str) -> Result<String, String> {
    let v = eval_quiet(src)?;
    Ok(builtins::format_value(&v).join("\n").trim_end().to_string())
}
