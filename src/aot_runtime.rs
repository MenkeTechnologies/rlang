//! AOT runtime hook for a standalone `.fvm` executable.
//!
//! fusevm's AOT model embeds the bincode-serialized `Chunk` in the object and,
//! at load, deserializes it and runs it on a `VM` (`fusevm_aot_run_embedded`).
//! Before running, fusevm calls back into the frontend through the C symbol
//! `fusevm_aot_register_builtins` to install the frontend's builtins on the run
//! VM. A standalone rlang binary is that AOT object + the rlang runtime
//! staticlib (this hook + `crate::builtins`) + a `main` calling
//! `fusevm::aot::fusevm_aot_run_embedded()`.
//!
//! rlang keeps R closures in the thread-local host, not in the fusevm chunk, so
//! `aot::compile_executable` embeds them in `Chunk::names` under
//! [`crate::aot::AOT_CLOSURES_TAG`]. Here we recover and load them before the
//! driver runs, so a compiled script that defines and calls functions behaves
//! exactly as the interpreter would.

use crate::host::ClosureDef;
use fusevm::{Chunk, VM};

thread_local! {
    /// The original script source, recovered from the embedded chunk by the
    /// register hook so `rlang_aot_main` can re-run it in embedded R on the NSE
    /// fallback path.
    static AOT_SOURCE: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// Rebuild the closure table embedded by `aot::compile_executable`.
fn embedded_closures(chunk: &Chunk) -> Vec<ClosureDef> {
    use base64::Engine as _;
    for name in &chunk.names {
        let Some(b64) = name.strip_prefix(crate::aot::AOT_CLOSURES_TAG) else {
            continue;
        };
        let Ok(blob) = base64::engine::general_purpose::STANDARD.decode(b64) else {
            continue;
        };
        let Ok(closures) = bincode::deserialize::<Vec<(Vec<String>, Chunk)>>(&blob) else {
            continue;
        };
        return closures
            .into_iter()
            .map(|(params, chunk)| ClosureDef { params, chunk })
            .collect();
    }
    Vec::new()
}

/// Recover the base64-encoded script source embedded by `compile_executable`.
fn embedded_source(chunk: &Chunk) -> Option<String> {
    use base64::Engine as _;
    for name in &chunk.names {
        if let Some(b64) = name.strip_prefix(crate::aot::AOT_SOURCE_TAG) {
            if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64) {
                if let Ok(s) = String::from_utf8(bytes) {
                    return Some(s);
                }
            }
        }
    }
    None
}

/// Register rlang's builtins and load the embedded R closures onto the AOT run
/// VM. Required link symbol for a standalone rlang AOT binary.
///
/// # Safety
/// `vm` must be a valid, exclusively-borrowable `*mut VM`; fusevm's AOT entry
/// (`fusevm_aot_run_embedded`) passes the live run VM.
#[no_mangle]
pub unsafe extern "C" fn fusevm_aot_register_builtins(vm: *mut VM) {
    // SAFETY: the fusevm AOT entry hands us the live run VM for this call.
    let vm = unsafe { &mut *vm };
    let closures = embedded_closures(&vm.chunk);
    let source = embedded_source(&vm.chunk);
    AOT_SOURCE.with(|s| *s.borrow_mut() = source);
    crate::host::with_host(|h| h.load_closures(closures));
    crate::builtins::install(vm);
}

/// Run `f` with fd 1 redirected into a temp file, returning what it wrote — the
/// same technique `main.rs` uses so a discarded native attempt does not
/// double-print alongside the embedded-R fallback. A temp file (not a pipe)
/// avoids blocking when the program out-writes the pipe buffer.
#[cfg(not(target_arch = "wasm32"))]
fn capture_stdout<R>(f: impl FnOnce() -> R) -> (R, Vec<u8>) {
    use std::io::{Read, Seek, Write};
    use std::os::unix::io::AsRawFd;
    let path = std::env::temp_dir().join(format!("rlang-aot-cap-{}.out", std::process::id()));
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

/// Entry point for a standalone rlang `.fvm` executable (called by the C `main`
/// stub `compile_executable` writes). It wraps fusevm's AOT driver with the two
/// things the interpreter's run path has and the bare fusevm entry lacks:
///
///   1. **Error surfacing.** An rlang builtin abort records the error in the
///      thread-local host and halts the VM; fusevm sees a clean halt and would
///      exit 0. We check `host::take_error()` and report it on stderr.
///   2. **Whole-script CRAN fallback.** On such an error, if an embedded R is
///      available, re-run the original source in R (non-standard evaluation:
///      `dplyr`, `data.table`). The native attempt's stdout is captured and
///      discarded so nothing prints twice — exactly like `main.rs`.
///
/// # Safety
/// Calls the fusevm AOT driver, whose `extern` blob/entry symbols the linked
/// object defines; valid only inside a linked `.fvm` executable.
#[no_mangle]
pub unsafe extern "C" fn rlang_aot_main() -> i64 {
    let (code, captured) = capture_stdout(|| fusevm::aot::fusevm_aot_run_embedded());
    let err = crate::host::with_host(|h| h.take_error());
    match err {
        None => {
            use std::io::Write as _;
            let _ = std::io::stdout().write_all(&captured);
            code
        }
        Some(e) => {
            #[cfg(not(target_arch = "wasm32"))]
            if crate::rembed::available() {
                if let Some(src) = AOT_SOURCE.with(|s| s.borrow().clone()) {
                    // Discard the captured native output; let R produce the real one.
                    return match crate::rembed::run_script(&src) {
                        Ok(()) => 0,
                        Err(re) => {
                            eprintln!("Rscript: {re}");
                            1
                        }
                    };
                }
            }
            use std::io::Write as _;
            let _ = std::io::stdout().write_all(&captured);
            eprintln!("Rscript: {e}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The closure image `aot::compile_executable` bakes into `Chunk::names`
    /// must round-trip back to the same formals and body count the interpreter
    /// loaded — this is what lets an AOT `.fvm` binary run scripts that define
    /// and call functions.
    #[test]
    fn embedded_closures_round_trip() {
        // A script whose compiled program has two closures with distinct arity.
        let prog = crate::compile("f <- function(x) x + 1\ng <- function(a, b) a * b\nf(1)")
            .expect("compile");
        assert_eq!(prog.closures.len(), 2, "expected two user closures");

        // Bake the image into a chunk's name table exactly as the emitter does.
        let tag = crate::aot::AOT_CLOSURES_TAG;
        let mut chunk = prog.main.clone();
        chunk.names.push(format!(
            "{tag}{}",
            crate::aot::encode_closures(&prog).unwrap()
        ));

        let recovered = embedded_closures(&chunk);
        assert_eq!(recovered.len(), prog.closures.len());
        for (a, b) in recovered.iter().zip(prog.closures.iter()) {
            assert_eq!(a.params, b.params, "formals must survive the round trip");
            assert_eq!(a.chunk.ops.len(), b.chunk.ops.len(), "body must survive");
        }
    }

    /// A chunk with no embedded image yields no closures (a straight-line AOT
    /// program), not a panic.
    #[test]
    fn no_image_yields_no_closures() {
        let chunk = crate::compile("cat(1 + 1)").unwrap().main;
        assert!(embedded_closures(&chunk).is_empty());
    }

    /// The source `compile_executable` bakes into `Chunk::names` must round-trip
    /// verbatim — this is what the AOT entry re-runs in embedded R on the NSE
    /// fallback path. A chunk without the tag yields `None`.
    #[test]
    fn embedded_source_round_trips() {
        use base64::Engine as _;
        let src = "suppressMessages(library(dplyr))\nfilter(mtcars, cyl == 4)\n";
        let mut chunk = crate::compile("1 + 1").unwrap().main;
        assert_eq!(embedded_source(&chunk), None);
        let b64 = base64::engine::general_purpose::STANDARD.encode(src.as_bytes());
        chunk.names.push(format!("{}{b64}", crate::aot::AOT_SOURCE_TAG));
        assert_eq!(embedded_source(&chunk).as_deref(), Some(src));
    }
}
