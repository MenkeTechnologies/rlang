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

    if cli.dap {
        return match rlang::dap::run() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => fail(&e),
        };
    }

    if let Some(src) = cli.eval {
        return run_source(&src);
    }

    if let Some(file) = cli.file.clone() {
        if cli.dump_tokens || cli.dump_ast || cli.disasm {
            return match dump(&file, &cli) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => fail(&e),
            };
        }
        if cli.aot {
            let out = cli
                .output
                .clone()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::path::Path::new(&file).with_extension("fvm"));
            return match rlang::aot::compile_executable(&file, &out) {
                // The output path is explicit user-requested output.
                Ok(p) => {
                    println!("{}", p.display());
                    ExitCode::SUCCESS
                }
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
        return match std::fs::read_to_string(&file) {
            Ok(src) => eval_with_cran_fallback(&src, || rlang::eval_str(&src)),
            Err(e) => fail(&format!("cannot read {file}: {e}")),
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
    eval_with_cran_fallback(src, || rlang::eval_str(src))
}

/// Evaluate a program on rlang's compiled path, but if it cannot (most often
/// non-standard evaluation — `dplyr::filter(df, x > 2)`, `data.table` `[`) and an
/// embedded R is available, run the whole script in R instead. rlang's own
/// stdout is captured during the attempt and discarded on fallback, so nothing
/// prints twice.
fn eval_with_cran_fallback(src: &str, run: impl FnOnce() -> Result<rlang::Value, String>) -> ExitCode {
    let (result, captured) = capture_stdout(run);
    match result {
        Ok(_) => {
            let _ = std::io::Write::write_all(&mut std::io::stdout(), &captured);
            ExitCode::SUCCESS
        }
        Err(e) => {
            #[cfg(not(target_arch = "wasm32"))]
            if rlang::rembed::available() {
                return match rlang::rembed::run_script(src) {
                    Ok(()) => ExitCode::SUCCESS,
                    Err(re) => fail(&re),
                };
            }
            let _ = std::io::Write::write_all(&mut std::io::stdout(), &captured);
            fail(&e)
        }
    }
}

/// Run `f` with fd 1 redirected into a temp file, returning what it wrote. A
/// temp file (not a pipe) avoids blocking when the program out-writes the pipe
/// buffer before anyone drains it.
fn capture_stdout<R>(f: impl FnOnce() -> R) -> (R, Vec<u8>) {
    use std::io::{Read, Seek, Write};
    use std::os::unix::io::AsRawFd;
    let path = std::env::temp_dir().join(format!("rlang-cap-{}.out", std::process::id()));
    let Ok(file) = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)
    else {
        return (f(), Vec::new());
    };
    // SAFETY: dup/dup2 on the process's own stdout fd; restored below.
    let (r, mut file) = unsafe {
        let saved = libc::dup(1);
        libc::dup2(file.as_raw_fd(), 1);
        let r = f();
        let _ = std::io::stdout().flush();
        libc::dup2(saved, 1);
        libc::close(saved);
        (r, file)
    };
    let mut buf = Vec::new();
    let _ = file.seek(std::io::SeekFrom::Start(0));
    let _ = file.read_to_end(&mut buf);
    let _ = std::fs::remove_file(&path);
    (r, buf)
}

/// The introspection dumps: token stream, AST, and bytecode disassembly. All
/// three are explicit user-requested output.
fn dump(file: &str, cli: &rlang::cli::Cli) -> Result<(), String> {
    let src = std::fs::read_to_string(file).map_err(|e| format!("cannot read {file}: {e}"))?;
    if cli.dump_tokens {
        for t in rlang::lexer::lex(&src)? {
            println!("{:>5}  {:?}", t.line, t.tok);
        }
    }
    if cli.dump_ast {
        for e in rlang::parser::parse(&src)? {
            println!("{e:#?}");
        }
    }
    if cli.disasm {
        let prog = rlang::compile(&src)?;
        print!("{}", rlang::disasm(&prog));
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
