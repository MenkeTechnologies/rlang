//! Command-line interface for the `Rscript` binary.

use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "Rscript",
    version,
    about = "R on fusevm — a compiled R runtime (bytecode VM + Cranelift JIT)",
    long_about = None,
)]
pub struct Cli {
    /// Execute a one-liner instead of a file (`Rscript -e 'print(1 + 1)'`).
    #[arg(short = 'e', long = "eval", value_name = "SRC")]
    pub eval: Option<String>,

    /// Start the interactive REPL.
    #[arg(long = "repl")]
    pub repl: bool,

    /// Speak the Language Server Protocol over stdio.
    #[arg(long = "lsp")]
    pub lsp: bool,

    /// Ahead-of-time compile the script's bytecode into the on-disk cache.
    #[arg(long = "build")]
    pub build: bool,

    /// Print the compiled fusevm bytecode for the script and exit.
    #[arg(long = "dump-bytecode")]
    pub dump_bytecode: bool,

    /// The `.R` script to run (omit with --repl / --lsp / -e).
    #[arg(value_name = "FILE")]
    pub file: Option<String>,

    /// Arguments passed through to the R program as `commandArgs()`.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub argv: Vec<String>,
}

/// Parse the process arguments.
pub fn parse() -> Cli {
    Cli::parse()
}
