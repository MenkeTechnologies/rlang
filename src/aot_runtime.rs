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
    crate::host::with_host(|h| h.load_closures(closures));
    crate::builtins::install(vm);
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
}
