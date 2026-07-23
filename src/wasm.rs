//! `wasm32-unknown-unknown` entry point — the web-worker host bridge.
//!
//! wasm has no real stdout, so R's output is captured with `eval_capture` and
//! handed back to the JS host over linear memory. The ABI is deliberately tiny
//! and dependency-free (no wasm-bindgen):
//!
//!   1. `rlang_alloc(len)` → a `len`-byte buffer; JS writes the UTF-8 R source.
//!   2. `rlang_eval(ptr, len)` runs it and returns a packed `u64`:
//!      `(result_ptr << 32) | result_len`, pointing at a fresh UTF-8 transcript.
//!   3. JS reads `result_len` bytes at `result_ptr`, then frees both buffers
//!      with `rlang_free(ptr, len)`.
//!
//! Every buffer is a boxed slice (capacity == length), so `rlang_free`'s
//! `Vec::from_raw_parts(ptr, len, len)` reconstruction is always sound.

use crate::eval_capture;

/// Allocate a `len`-byte buffer in wasm linear memory for the JS host to fill.
#[no_mangle]
pub extern "C" fn rlang_alloc(len: usize) -> *mut u8 {
    let mut boxed = vec![0u8; len].into_boxed_slice();
    let ptr = boxed.as_mut_ptr();
    std::mem::forget(boxed);
    ptr
}

/// Free a buffer previously returned by [`rlang_alloc`] or [`rlang_eval`].
///
/// # Safety
/// `ptr`/`len` must name a live buffer from one of those two functions, freed
/// exactly once.
#[no_mangle]
pub unsafe extern "C" fn rlang_free(ptr: *mut u8, len: usize) {
    if !ptr.is_null() && len != 0 {
        // Buffers are boxed slices, so capacity == length.
        drop(unsafe { Vec::from_raw_parts(ptr, len, len) });
    }
}

/// Evaluate UTF-8 R source at `(ptr, len)` and return a packed pointer
/// `(result_ptr << 32) | result_len` to a freshly allocated transcript.
///
/// # Safety
/// `(ptr, len)` must name a readable buffer of `len` bytes (typically from
/// [`rlang_alloc`]).
#[no_mangle]
pub unsafe extern "C" fn rlang_eval(ptr: *const u8, len: usize) -> u64 {
    let src = if ptr.is_null() {
        String::new()
    } else {
        let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
        String::from_utf8_lossy(bytes).into_owned()
    };
    let mut out = eval_capture(&src).into_bytes().into_boxed_slice();
    let out_ptr = out.as_mut_ptr();
    let out_len = out.len();
    std::mem::forget(out);
    ((out_ptr as u64) << 32) | (out_len as u64)
}
