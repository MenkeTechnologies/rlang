//! The rkyv/bincode bytecode cache round-trips a compiled program: store then
//! load must reproduce a program that runs identically. Uses an isolated HOME so
//! a developer's real `~/.rlang` shard is untouched.

use rlang::{builtins, cache, host};

#[test]
fn store_then_load_reproduces_the_program() {
    let tmp = tempfile::tempdir().unwrap();
    // Point the cache at an isolated home for the duration of this test.
    let prev = std::env::var_os("HOME");
    std::env::set_var("HOME", tmp.path());

    let src = "double <- function(x) x * 2\ndouble(21)";
    let prog = rlang::compile(src).expect("compile");
    cache::store(src, &prog).expect("store");

    let loaded = cache::load(src).expect("cached program present");
    // A different source must miss.
    assert!(cache::load("print(1)").is_none());

    // The loaded program runs to the same value as a fresh compile.
    host::reset_host();
    host::with_host(|h| h.echo = false);
    let v = rlang::run_compiled(loaded).expect("run cached");
    assert_eq!(builtins::format_value(&v).join("\n"), "[1] 42");

    match prev {
        Some(p) => std::env::set_var("HOME", p),
        None => std::env::remove_var("HOME"),
    }
}
