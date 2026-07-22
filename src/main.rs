//! The `Rscript` binary entry point.
//!
//! Dispatch: `--lsp` speaks its protocol over stdio; `--repl` (or no file on a
//! TTY) starts the interactive loop; `--build` AOT-compiles into the cache;
//! `--dump-bytecode` prints the lowered fusevm chunk; otherwise a file, a `-e`
//! one-liner, or stdin is run. Errors go to stderr in the terse
//! `Rscript: <reason>` form; nothing else is printed.

use std::process::ExitCode;

/// Native stack for the interpreter thread. Every R call runs its body on a
/// nested VM, so R recursion consumes Rust stack; the default 8 MB main-thread
/// stack runs out around a hundred frames, well short of R's own limits.
const STACK_SIZE: usize = 512 * 1024 * 1024;

fn main() -> ExitCode {
    std::thread::Builder::new()
        .stack_size(STACK_SIZE)
        .spawn(run)
        .and_then(|h| {
            h.join()
                .map_err(|_| std::io::Error::other("interpreter thread panicked"))
        })
        .unwrap_or_else(|e| fail(&e.to_string()))
}

fn run() -> ExitCode {
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
