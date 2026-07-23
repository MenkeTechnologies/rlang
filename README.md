```
██████╗ ██╗      █████╗ ███╗   ██╗ ██████╗
██╔══██╗██║     ██╔══██╗████╗  ██║██╔════╝
██████╔╝██║     ███████║██╔██╗ ██║██║  ███╗
██╔══██╗██║     ██╔══██║██║╚██╗██║██║   ██║
██║  ██║███████╗██║  ██║██║ ╚████║╚██████╔╝
╚═╝  ╚═╝╚══════╝╚═╝  ╚═╝╚═╝  ╚═══╝ ╚═════╝
```

[![CI](https://github.com/MenkeTechnologies/rlang/actions/workflows/ci.yml/badge.svg)](https://github.com/MenkeTechnologies/rlang/actions/workflows/ci.yml)
![Rust](https://img.shields.io/badge/Rust-2021-05d9e8?style=flat-square)
[![Docs](https://img.shields.io/badge/docs-online-blue.svg)](https://menketechnologies.github.io/rlang/)
![license](https://img.shields.io/badge/license-MIT-ff2a6d?style=flat-square)
![status](https://img.shields.io/badge/status-active%20%C2%B7%20in%20development-9b5de5?style=flat-square)

### `[R, COMPILED TO BYTECODE — JIT-COMPILED, NOT TREE-WALKED]`

> *"GNU R walks the tree. rlang compiles it."*

**R in Rust** — a compiled R runtime, hosted on the
[`fusevm`](https://github.com/MenkeTechnologies/fusevm) bytecode VM with a
three-tier Cranelift JIT — the same engine behind `zshrs`, `stryke`, `awkrs`,
`elisp`, and `rubylang`.

### [`Read the Docs`](https://menketechnologies.github.io/rlang/) &middot; [`Engineering Report`](https://menketechnologies.github.io/rlang/report.html) &middot; [`Primitive Reference`](https://menketechnologies.github.io/rlang/reference.html)

---

## Table of Contents

- [\[0x00\] Overview](#0x00-overview)
- [\[0x01\] Install](#0x01-install)
- [\[0x02\] Usage](#0x02-usage)
- [\[0x03\] Language Features](#0x03-language-features)
- [\[0x04\] Command-Line Flags](#0x04-command-line-flags)
- [\[0x05\] Architecture](#0x05-architecture)
- [\[0x06\] Parity Harness](#0x06-parity-harness)
- [\[0x07\] Status & Roadmap](#0x07-status--roadmap)
- [\[0x08\] Documentation](#0x08-documentation)
- [\[0xFF\] License](#0xff-license)

---

## [0x00] OVERVIEW

GNU R evaluates R by walking a parse tree in C. `rlang` lexes and parses R to an
AST, lowers it to `fusevm` bytecode, and runs it on a compiled VM with a
Cranelift JIT. rlang carries no VM or JIT of its own. Highlights:

- **Compiled, not tree-walked** — `for`, `while`, `repeat`, `if`, `&&` and `||`
  lower to native fusevm jumps with native integer loop counters, so the tracing
  JIT sees ordinary loops.
- **fusevm-hosted** — no local `vm.rs` / `jit.rs`; the shared engine behind
  `zshrs`, `stryke`, `awkrs`, `elisp`, and `rubylang`. `jit-disk-cache` persists
  native code across runs.
- **Everything is a vector** — there are no scalars: `1` is a double vector of
  length one, every value carries attributes (`names`, `dim`, `class`), and every
  operator recycles and propagates `NA`.
- **Copy-on-modify** — `y <- x; y[1] <- 9` leaves `x` alone, and complex targets
  (`l$v[2] <- 9`, `names(x) <- v`) compile the way R defines them: rebuild the
  container, then re-bind it.
- **Three-valued logic** — `NA & FALSE` is FALSE and `NA | TRUE` is TRUE, because
  the answer is decided regardless of the missing value.
- **S3 dispatch** — `UseMethod` walks the class vector then `.default`, with
  implicit classes for the builtin types.
- **AOP intercepts** — a glob-matched before/after/around call-intercept
  registry, the same design as zshrs's function intercepts.
- **Native executables** — `Rscript --aot FILE` lowers the script to a fusevm
  object and links it against the rlang runtime into a standalone `.fvm` binary
  (user closures embedded, no interpreter startup).
- **Inline-Rust FFI** — `.rust("…Rust source…")` compiles a self-contained Rust
  block to a cached `cdylib` on first use; `.Call(name, …)` — R's own native-call
  verb — invokes its exports, marshalling length-1 vectors to `i64`/`f64`/string
  and back.
- **Runs on wasm** — the same crate builds for `wasm32-unknown-unknown` (pure
  interpreter, no Cranelift) and exports `rlang_eval` for a web-worker host.
- **Editor-ready** — an LSP server and a DAP adapter over stdio, introspection
  dumps (`--dump-tokens`, `--dump-ast`, `--disasm`), and a REPL on a persistent
  host where a function defined at one prompt completes at the next.
- **Differential parity** — a hand-authored snippet corpus plus a grammar-driven
  fuzzer, both diffed live against the reference `Rscript`; the corpus is frozen
  and replayed in CI with no R installed.

---

## [0x01] INSTALL

```sh
git clone https://github.com/MenkeTechnologies/rlang
cd rlang
cargo build

# run a file, a one-liner, or the REPL
./target/debug/Rscript script.R
./target/debug/Rscript -e 'print(sum(1:100))'
./target/debug/Rscript --repl
```

`rlang` is a standalone Rust crate (an explicit empty `[workspace]` keeps it
independent of the meta repo). On native targets `fusevm` is pulled from
crates.io with the `jit`, `jit-disk-cache`, `aot`, and `ffi` features; the
wasm build uses the bare interpreter. Run the tests with `cargo test`.

```sh
# AOT-compile to a standalone native executable
./target/debug/Rscript --aot script.R && ./script.fvm

# build the wasm engine (web-worker host; exports rlang_eval / rlang_alloc / rlang_free)
cargo rustc --lib --crate-type cdylib --target wasm32-unknown-unknown
```

#### Zsh tab completion

```sh
cp completions/_Rscript /usr/local/share/zsh/site-functions/_Rscript
# or: fpath=(/path/to/rlang/completions $fpath) in .zshrc
autoload -Uz compinit && compinit
```

---

## [0x02] USAGE

```r
fib <- function(n) if (n < 2) n else fib(n - 1) + fib(n - 2)
print(sapply(0:10, fib))
# [1]  0  1  1  2  3  5  8 13 21 34 55

x <- c(a = 1, b = 2, c = 3)
print(x[x > 1])
#  b  c
#  2  3

counter <- function() {
  n <- 0
  function() {
    n <<- n + 1
    n
  }
}
tick <- counter()
tick(); tick()
print(tick())        # [1] 3

m <- matrix(1:6, nrow = 2)
print(m[, 2])        # [1] 3 4

c(3, 1, 2) |> sort() |> rev()   # [1] 3 2 1
```

---

## [0x03] LANGUAGE FEATURES

Implemented and checked against the reference `Rscript`:

- **Vectors & types** — logical, integer, double, character, and list vectors
  with `NA` in every atomic type, recycling, type promotion in `c()`, and the
  `L` integer-literal suffix.
- **Attributes** — `names`, `dim`, `class`, and arbitrary `attr()`, preserved
  through arithmetic and subsetting.
- **All four index forms** — positive, negative (exclusion), logical (recycled),
  and character (by name), plus `[[`, `$`, and 2-D matrix indexing `m[i, j]`.
- **Assignment** — `<-`, `=`, `->`, `<<-`, growing assignment past the end,
  index/`$`/`[[` targets, nested targets, and replacement functions
  (`names(x) <-`, `dim(x) <-`, `class(x) <-`, user-defined `` `f<-` ``).
- **Functions** — defaults that may refer to other arguments, `...` forwarding,
  R's exact/partial/positional argument matching, lexical closures, `return()`,
  and function-position lookup that skips non-function bindings.
- **Control flow** — `if`/`else` as an expression, `for`, `while`, `repeat`,
  `break`, `next`, and short-circuiting `&&` / `||`.
- **Operators** — the full precedence ladder from `?Syntax`, `%%`/`%/%` with the
  sign of the divisor, `%in%`, user-defined `%op%`, and the native pipe `|>`.
- **S3** — `class()`, `inherits()`, `structure()`, `UseMethod` dispatch with
  implicit classes and `.default` fallback.
- **Primitive library** — the apply family (`lapply`/`sapply`/`Map`/`Filter`/
  `Reduce`/`do.call`), string and regex functions (`paste`, `sprintf`, `substr`,
  `strsplit`, `grepl`, `sub`, `gsub`), numeric summaries (`sum`, `mean`,
  `median`, `var`, `sd`, `cumsum`, `diff`), sequence and set functions, and
  matrix helpers.
- **R's printing** — `[n]` index prefixes with 80-column wrapping, shared decimal
  widths, quoted and left-justified character vectors, named-vector column pairs,
  `[i,]`/`[,j]` matrix layout, and `$name` / `[[n]]` list sections.

---

## [0x04] COMMAND-LINE FLAGS

| Flag | Effect |
| --- | --- |
| `FILE` | Run a `.R` script. |
| `-e SRC` | Run a one-liner. |
| `--repl` | Interactive REPL on a persistent host. |
| `--lsp` | Language Server Protocol over stdio. |
| `--dap` | Debug Adapter Protocol over stdio (handshake + run to completion). |
| `--build FILE` | AOT-compile the script's bytecode into the on-disk cache. |
| `--aot FILE` | AOT-compile the script to a standalone native `.fvm` executable (override the path with `-o OUT`). |
| `-o OUT` | Output path for `--aot` (default: the script's name with a `.fvm` extension). |
| `--dump-tokens FILE` | Print the lexer token stream. |
| `--dump-ast FILE` | Print the parsed AST. |
| `--disasm FILE` | Disassemble the lowered fusevm chunk. |

---

## [0x05] ARCHITECTURE

rlang contains no virtual machine or JIT of its own. The execution path mirrors
how `zshrs` hosts zsh and `rubylang` hosts Ruby:

```
R source → lexer → parser (AST) → lower to fusevm bytecode → fusevm VM + Cranelift JIT
                                          │
                              RHost heap (vectors, attributes, environments, closures)
```

| Piece | How |
| --- | --- |
| **fusevm-hosted** | No local `vm.rs` / `jit.rs`. R lowers to fusevm bytecode and runs on the shared three-tier Cranelift JIT; `jit-disk-cache` persists native code across runs. |
| **Native control flow** | Loops and branches lower to native fusevm jumps over native integer counters, so hot loops trace-compile. |
| **Vectors on the host heap** | Every R value is a `Value::Obj` handle into the `RHost` heap, because R has no scalars and any value can carry attributes. |
| **Environments by reference** | Frames are `Rc<RefCell<..>>` environments chained to their enclosure — R's lexical scoping, and what lets `<<-` reach the defining frame. |
| **R truthiness** | A condition must be a single non-`NA` logical, so conditions normalize through a `TRUTHY` op before a native branch. |
| **Complex assignment** | `f(x) <- v` compiles to `` x <- `f<-`(x, v) `` and `x[i] <- v` rebuilds and re-binds `x`, so nested targets unwind through the same two rules. |

---

## [0x06] PARITY HARNESS

Behaviour is checked against the reference `Rscript` by a **differential parity
harness** — `cargo run --bin parity` diffs the snippet corpus
(`tests/data/parity_corpus.R`) live against the system R, and `tests/parity.rs`
replays the frozen outputs in CI with no R installed. Nothing is faked as
working: an unimplemented primitive raises `could not find function`.

The `examples/` directory holds runnable programs that double as tests: the
scripts embed `stopifnot` assertions that abort on any divergence from R, and
`tests/examples.rs` runs every example through the binary in CI, asserting a
clean exit and stdout matching the frozen reference output
(`cargo run --bin parity -- --freeze-examples` regenerates it).

Where the fixed corpus is hand-authored, the **differential fuzzer** —
`cargo run --bin parity-fuzz` — generates thousands of grammar-driven R snippets
across 27 surfaces (vectors, `seq`/`rep`, apply family, `sprintf`/`formatC`,
matrices and linear algebra, `factor`/`table`, set/bit ops, trig, gamma/`choose`,
`pmax`/`pmin`, string translation, …) and runs each through the
reference `Rscript --vanilla -e` and rlang's own `Rscript -e`, reporting every
case where stdout or exit code diverges. Both binaries share the name `Rscript`,
so each is resolved by absolute path — the reference from a system path, rlang's
from this harness's own directory — and can never be confused. Generators emit
only deterministic-output programs (no `Sys.time`, RNG, or environment prints),
so any divergence is a genuine gap. A finding is delta-debugged to its minimal
reproducer and replays exactly with `--seed <N> --once`.

```sh
cargo build --bin parity-fuzz
./target/debug/parity-fuzz --count 5000                       # sweep all modes
./target/debug/parity-fuzz --sprintf --count 2000             # one surface
./target/debug/parity-fuzz --seed 52 --once                   # replay one case
./target/debug/parity-fuzz --count 5000 \
    --baseline tests/data/parity_fuzz_baseline.txt            # gate on NEW gaps only
```

The fuzzer currently reports **zero** divergences across its 27 surfaces over
repeated multi-seed sweeps, so `tests/data/parity_fuzz_baseline.txt` is empty;
with `--baseline` the run exits non-zero the moment any *new* divergence class
appears — a regression, or a surface that just started diverging. Like `parity`,
it needs R on `PATH` (or `RLANG_FUZZ_RSCRIPT`), so it is a development tool, not
a CI gate.

---

## [0x07] STATUS & ROADMAP

The standalone `Rscript` binary, the REPL, the rkyv bytecode cache, the `--aot`
native-executable emitter, the inline-Rust FFI bridge (`.rust` / `.Call`), the
`wasm32` build, the AOP call-intercept registry, the LSP server, and the DAP
adapter (handshake plus run-to-completion; stepping is a later wave) are all in
the tree. The parity corpus and every example match the reference R
byte-for-byte.

Arguments are evaluated eagerly rather than as promises, so `substitute()` /
`quote()` / non-standard evaluation are not available; `tryCatch` and the
condition system, data frames, complex numbers, and most of the linear-algebra
surface (`outer`, `solve`, `crossprod`, `cbind`/`rbind`) are not implemented yet.
Factors, `table`, `%*%`, and `apply` over matrix margins now work. See
[`BUGS.md`](BUGS.md) for the full known-gaps list.

---

## [0x08] DOCUMENTATION

- **[Read the Docs](https://menketechnologies.github.io/rlang/)** — the HUD
  documentation site.
- **[Engineering Report](https://menketechnologies.github.io/rlang/report.html)**
  — architecture, value model, roadmap, dependency posture.
- **[Primitive Reference](https://menketechnologies.github.io/rlang/reference.html)**
  — the primitive library, generated from the language-server corpus.
- **[`BUGS.md`](BUGS.md)** — the honest known-gaps list.

---

## [0xFF] LICENSE

MIT — free and open source. See [`LICENSE`](LICENSE).
