//! R's bridge to fusevm's inline-Rust FFI (`fusevm::ffi`).
//!
//! R has no `rust { }` block syntax, so rlang surfaces the shared substrate
//! through two R-idiomatic builtins (wired in `builtins.rs`):
//!
//!   - `.rust(code)` compiles a self-contained Rust block — its
//!     `pub extern "C" fn` exports — to a `cdylib` on first use, caches it under
//!     `~/.cache/fusevm/ffi`, and `dlopen`s it.
//!   - `.Call(name, ...)` — R's own native-call verb — invokes a registered
//!     export, marshalling length-1 R vectors to fusevm scalars and the scalar
//!     result back to a length-1 R vector.
//!
//! Both need `rustc` on `PATH` plus `dlopen`, so they run only on native
//! targets; on `wasm32` they return a clear error rather than pretending.

use fusevm::Value;

/// Compile and register the exports of a self-contained inline Rust block.
#[cfg(not(target_arch = "wasm32"))]
pub fn register(code: &str) -> Result<(), String> {
    use base64::Engine as _;
    let b64 = base64::engine::general_purpose::STANDARD.encode(code.as_bytes());
    fusevm::ffi::compile_and_register(&b64)
}

/// Call a previously `.rust()`-registered FFI export by name.
#[cfg(not(target_arch = "wasm32"))]
pub fn call(name: &str, args: &[Value]) -> Result<Value, String> {
    match fusevm::ffi::try_call(name, args) {
        Some(r) => r,
        None => Err(format!(
            "\"{name}\" is not a registered native routine (call .rust() first)"
        )),
    }
}

#[cfg(target_arch = "wasm32")]
pub fn register(_code: &str) -> Result<(), String> {
    Err("rust FFI is unavailable on wasm (it needs rustc + dlopen)".into())
}

#[cfg(target_arch = "wasm32")]
pub fn call(_name: &str, _args: &[Value]) -> Result<Value, String> {
    Err("rust FFI is unavailable on wasm (it needs rustc + dlopen)".into())
}
