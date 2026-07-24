//! Ahead-of-time compilation.
//!
//! Two modes:
//!
//!   - [`build`] (`Rscript --build FILE`) precompiles to fusevm bytecode and
//!     warms the on-disk cache (`cache.rs`), so subsequent interpreted runs skip
//!     lex/parse/lower entirely.
//!   - [`compile_executable`] (`Rscript --aot FILE`) lowers the script to a
//!     fusevm `Chunk`, emits it as a relocatable native object via
//!     `fusevm::aot::compile_object`, and links it against the rlang runtime
//!     staticlib (`librlang.a`, which carries fusevm's AOT runtime plus
//!     `fusevm_aot_register_builtins` from `aot_runtime.rs`) into a standalone
//!     native `<name>.fvm` executable.
//!
//! R closures are not part of the fusevm chunk — the interpreter keeps them in
//! the thread-local host — so the executable path embeds the compiled closure
//! bodies in the chunk's `names` table under [`AOT_CLOSURES_TAG`]; the register
//! hook rebuilds them into the fresh AOT host before the driver runs.

use crate::compiler::Program;
use fusevm::Chunk;
use std::path::{Path, PathBuf};

/// Marker for the base64 closure image smuggled through `Chunk::names`. The
/// leading NUL keeps it from colliding with any real R name. `aot_runtime.rs`
/// strips this prefix to recover the image.
pub const AOT_CLOSURES_TAG: &str = "\u{0}rlang-aot-closures:";

/// Marker for the base64-encoded original script source, embedded alongside the
/// closures so the AOT entry can re-run the whole script in embedded R when the
/// native path hits non-standard evaluation (`dplyr`, `data.table`) — the same
/// whole-script CRAN fallback the interpreter has.
pub const AOT_SOURCE_TAG: &str = "\u{0}rlang-aot-source:";

/// The serde-flat closure form embedded in the AOT object (formals + body),
/// matching `cache::CClosure`.
type CClosure = (Vec<String>, Chunk);

/// Precompile `file` and store its bytecode in the cache. Returns a one-line
/// report of what was built. The report is explicit user-requested output.
pub fn build(file: &str) -> Result<String, String> {
    let src = std::fs::read_to_string(file).map_err(|e| format!("cannot read {file}: {e}"))?;
    let prog = crate::compile(&src)?;
    let (nclosures, nops) = (prog.closures.len(), prog.main.ops.len());
    crate::cache::store(&src, &prog)?;
    Ok(format!(
        "built {file}: {nops} top-level ops, {nclosures} closures -> ~/.rlang/scripts.rkyv"
    ))
}

/// Serialize a program's closures into the base64 image embedded in the AOT
/// object (see [`AOT_CLOSURES_TAG`]).
pub(crate) fn encode_closures(prog: &Program) -> Result<String, String> {
    use base64::Engine as _;
    let closures: Vec<CClosure> = prog
        .closures
        .iter()
        .map(|c| (c.params.clone(), c.chunk.clone()))
        .collect();
    let blob =
        bincode::serialize(&closures).map_err(|e| format!("aot: serialize closures: {e}"))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(blob))
}

/// AOT-compile `file` to a standalone native executable at `out`.
///
/// Emits the fusevm object, writes a tiny C entry that calls
/// `fusevm_aot_run_embedded`, and links both against the rlang runtime
/// staticlib. Returns the output path on success.
pub fn compile_executable(file: &str, out: &Path) -> Result<PathBuf, String> {
    let src = std::fs::read_to_string(file).map_err(|e| format!("cannot read {file}: {e}"))?;
    let prog = crate::compile(&src)?;

    // The object embeds only the fusevm chunk; carry the R closures alongside it
    // in the chunk's name table so the register hook can rebuild them, plus the
    // original source so the AOT entry can fall back to embedded R for NSE.
    let mut main = prog.main.clone();
    main.names
        .push(format!("{AOT_CLOSURES_TAG}{}", encode_closures(&prog)?));
    {
        use base64::Engine as _;
        let src_b64 = base64::engine::general_purpose::STANDARD.encode(src.as_bytes());
        main.names.push(format!("{AOT_SOURCE_TAG}{src_b64}"));
    }

    let runtime_lib = runtime_staticlib()?;

    let obj = out.with_extension("o");
    fusevm::aot::compile_object(&main, &obj).map_err(|e| format!("Rscript --aot: {e}"))?;

    // Call rlang's AOT entry (aot_runtime.rs), not fusevm's directly: it adds
    // the error-surfacing and whole-script CRAN fallback the interpreter has.
    let stub = out.with_extension("aot_main.c");
    std::fs::write(
        &stub,
        b"extern long rlang_aot_main(void);\nint main(void){return (int)rlang_aot_main();}\n" as &[u8],
    )
    .map_err(|e| format!("Rscript --aot: write entry stub: {e}"))?;

    let mut cmd = std::process::Command::new("cc");
    cmd.arg(&stub).arg(&obj).arg(&runtime_lib);
    // Platform libraries the Rust staticlib pulls in.
    if cfg!(target_os = "macos") {
        cmd.args([
            "-framework",
            "CoreFoundation",
            "-framework",
            "Security",
            "-liconv",
            "-lc++",
        ]);
    } else {
        cmd.args(["-lpthread", "-ldl", "-lm", "-lrt"]);
    }
    cmd.arg("-o").arg(out);

    let status = cmd
        .status()
        .map_err(|e| format!("Rscript --aot: invoking cc: {e}"))?;
    let _ = std::fs::remove_file(&stub);
    let _ = std::fs::remove_file(&obj);
    if !status.success() {
        return Err(format!(
            "Rscript --aot: link failed (cc exit {:?})",
            status.code()
        ));
    }
    Ok(out.to_path_buf())
}

/// Locate the rlang runtime staticlib. `RLANG_STATICLIB` overrides; otherwise
/// look for `librlang.a` beside the running `Rscript` (cargo emits it into the
/// same target directory as the binary).
fn runtime_staticlib() -> Result<PathBuf, String> {
    if let Ok(p) = std::env::var("RLANG_STATICLIB") {
        return Ok(PathBuf::from(p));
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    if let Some(dir) = exe.parent() {
        let cand = dir.join("librlang.a");
        if cand.exists() {
            return Ok(cand);
        }
    }
    Err("Rscript --aot: could not locate librlang.a (set RLANG_STATICLIB)".to_string())
}
