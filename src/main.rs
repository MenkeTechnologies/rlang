//! The `Rscript` binary entry point.
//!
//! Dispatch: `--lsp` speaks its protocol over stdio; `--repl` (or no file on a
//! TTY) starts the interactive loop; `--build` AOT-compiles into the cache;
//! `--dump-bytecode` prints the lowered fusevm chunk; otherwise a file, a `-e`
//! one-liner, or stdin is run. Errors go to stderr in the terse
//! `Rscript: <reason>` form; nothing else is printed.

use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = rlang::cli::parse();

    if cli.lsp {
        return match rlang::lsp::run() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => fail(&e),
        };
    }

    if let Some(src) = cli.eval {
        return run_source(&src);
    }

    if let Some(file) = cli.file {
        if cli.dump_bytecode {
            return match dump(&file) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => fail(&e),
            };
        }
        if cli.build {
            return match rlang::aot::build(&file) {
                // A build report is explicit user-requested output.
                Ok(msg) => {
                    println!("{msg}");
                    ExitCode::SUCCESS
                }
                Err(e) => fail(&e),
            };
        }
        return match rlang::eval_file(&file) {
            Ok(_) => ExitCode::SUCCESS,
            Err(e) => fail(&e),
        };
    }

    if cli.repl || atty_stdin() {
        rlang::repl::run();
        return ExitCode::SUCCESS;
    }

    let src = std::io::read_to_string(std::io::stdin()).unwrap_or_default();
    run_source(&src)
}

fn run_source(src: &str) -> ExitCode {
    match rlang::eval_str(src) {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => fail(&e),
    }
}

fn dump(file: &str) -> Result<(), String> {
    let src = std::fs::read_to_string(file).map_err(|e| format!("cannot read {file}: {e}"))?;
    let prog = rlang::compile(&src)?;
    println!("== main ==\n{:#?}", prog.main.ops);
    for (i, c) in prog.closures.iter().enumerate() {
        println!(
            "== closure #{i} ({}) ==\n{:#?}",
            c.params.join(", "),
            c.chunk.ops
        );
    }
    Ok(())
}

fn atty_stdin() -> bool {
    // SAFETY: isatty is a pure query on the stdin fd.
    unsafe { libc::isatty(libc::STDIN_FILENO) == 1 }
}

fn fail(msg: &str) -> ExitCode {
    eprintln!("Rscript: {msg}");
    ExitCode::FAILURE
}
