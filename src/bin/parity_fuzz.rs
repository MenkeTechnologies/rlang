//! Differential parity fuzzer: reference `Rscript -e <s>` vs rlang `Rscript -e <s>`.
//!
//! Generates thousands of grammar-driven, deterministic-output R snippets, runs
//! each through both interpreters, and reports every case where stdout OR exit
//! code diverge. Each case is produced from a per-index seed so any divergence
//! replays exactly: `parity-fuzz --seed <N> --once`.
//!
//! Ported from the rubylang harness (`rubylang/src/bin/parity_fuzz.rs`), itself
//! ported from zshrs: same RunOut / render / differs / run_with_timeout infra,
//! same seed→deterministic Mode dispatch, same parallel workers, delta-debug
//! `minimize`, `--verify` K-consecutive re-check, `--baseline` allowlist + gap
//! `signature`, `--once` replay, and report file under
//! `target/parity-fuzz/divergences.txt`. Only the generators and the invocation
//! (R, not Ruby) differ.
//!
//! Both sides share the binary name `Rscript` (rlang's exe and the reference R
//! shell), so the SUT is always resolved by ABSOLUTE path from this harness's
//! own directory and the oracle from an absolute system path — neither can
//! resolve to the other. See `ours_bin` / `oracle_path`.
//!
//! The generators are biased toward the historically weak areas of an R
//! frontend: vector print-width/alignment, float shortest-repr and `digits`,
//! `seq`/`rep` with fractional `by`, `sprintf`/`formatC` specs, named-vector and
//! matrix layout, `%/%`/`%%` sign, `factor`/`table` printing, and the apply
//! family. Pure random bytes only produce mutual syntax errors that agree on
//! both sides and teach nothing.
//!
//! Determinism invariant: the generator NEVER emits a construct whose output is
//! nondeterministic for reasons unrelated to parity — no `Sys.time`/`date`, no
//! `runif`/`rnorm`/`sample` (RNG stream), no `tempfile`, no environment/closure
//! prints (`<environment: 0x..>`), no `proc.time`, no `.Machine`-dependent
//! widths. Every program prints something deterministic so an empty-vs-empty run
//! can never hide a gap. A program NEVER begins with `-`: a leading dash is
//! misparsed by BOTH arg parsers (R: "option '-e' requires a non-empty
//! argument"; clap: "unexpected argument") in DIFFERENT ways, a false gap.
//!
//! Build:  cargo build --bin parity-fuzz
//! Run:    ./target/debug/parity-fuzz --count 5000

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Also compare stderr (normalized) when set via `--stderr`.
static CMP_STDERR: AtomicBool = AtomicBool::new(false);

// ---------------------------------------------------------------------------
// PRNG — inline splitmix64, no `rand` dependency.
// ---------------------------------------------------------------------------

struct Rng(u64);

impl Rng {
    fn seed(s: u64) -> Rng {
        // Avoid a zero state degenerating; splitmix64 tolerates any seed but a
        // nonzero start keeps the first draw well-mixed.
        Rng(s ^ 0x9E37_79B9_7F4A_7C15)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `0..n` (n >= 1).
    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    /// Inclusive range `lo..=hi`.
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next_u64() % (hi - lo + 1) as u64) as i64
    }

    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len())]
    }
}

// ---------------------------------------------------------------------------
// Interpreter locations / invocation.
// ---------------------------------------------------------------------------

/// The rlang binary under test — a sibling of this harness exe. Always an
/// absolute path so it can never be confused with the reference `Rscript` on
/// PATH (they share the name `Rscript`).
fn ours_bin() -> PathBuf {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_Rscript") {
        return PathBuf::from(p);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let cand = dir.join("Rscript");
            if cand.exists() {
                return cand;
            }
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("debug")
        .join("Rscript")
}

/// The ORACLE: the reference `Rscript` (GNU R). Every divergence is "rlang
/// disagrees with THIS R", so which R it is, is part of the result.
///
/// `RLANG_FUZZ_RSCRIPT` names it explicitly; if set but unusable this is a HARD
/// ERROR (falling back to a different R would silently answer a different
/// question). Otherwise the first existing system path wins. Candidates are
/// absolute system paths, never `target/`, so the oracle can never resolve to
/// our own binary.
fn oracle_path() -> &'static str {
    static ORACLE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    ORACLE.get_or_init(|| {
        if let Ok(p) = std::env::var("RLANG_FUZZ_RSCRIPT") {
            if !Path::new(&p).exists() {
                eprintln!("parity-fuzz: RLANG_FUZZ_RSCRIPT={p}: no such file");
                std::process::exit(2);
            }
            return p;
        }
        for p in [
            "/opt/homebrew/bin/Rscript",
            "/usr/local/bin/Rscript",
            "/usr/bin/Rscript",
        ] {
            if Path::new(p).exists() {
                return p.to_string();
            }
        }
        "Rscript".to_string()
    })
}

/// `<path> (<R --version line>)`, for the run header and the report file so a
/// divergence record is attributable to the exact oracle that produced it.
fn oracle_id() -> String {
    let path = oracle_path();
    let ver = Command::new(path)
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        // `Rscript --version` prints to stdout on R 4.x; older builds used
        // stderr. Try stdout first, fall back to stderr, take the first line.
        .map(|o| {
            let s = if o.stdout.is_empty() { &o.stderr } else { &o.stdout };
            String::from_utf8_lossy(s)
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .to_string()
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    format!("{path} ({ver})")
}

/// A private HOME for the SUT so its rkyv bytecode cache
/// (`$HOME/.rlang/scripts.rkyv`) never pollutes the user's real `~/.rlang`. The
/// cache is content+schema addressed, so a miss recompiles fresh and a benign
/// read-modify-write race between parallel workers can only cost a recompile,
/// never a wrong answer.
fn ours_home() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("parity-fuzz")
        .join("home")
}

/// Raw bytes, never `String`: R can emit output that is not valid UTF-8 (an
/// 8-bit locale, `intToUtf8`, `rawToChar`). Comparing bytes keeps the surface
/// honest; lossy rendering is for the human-facing report only.
struct RunOut {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    exit: i32,
    timed_out: bool,
}

/// Render captured bytes for a report. Invalid UTF-8 is shown lossily AND
/// followed by a hex line, so two different invalid byte strings do not both
/// render to U+FFFD and hide a divergence.
fn render(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim_end_matches('\n');
    if std::str::from_utf8(bytes).is_err() {
        let hex: Vec<String> = bytes.iter().map(|b| format!("{b:02x}")).collect();
        return format!("{text}\n  (hex) {}", hex.join(" "));
    }
    text.to_string()
}

/// Normalize a diagnostic so wording can be compared without the interpreter
/// name or source location. Drops R's `Error in <call> :` / `Error:` and
/// `Execution halted` framing, rlang's `Rscript:` prefix, and any trailing
/// `Warning message:` block, leaving the human-readable reason.
fn norm_stderr(s: &[u8]) -> Vec<u8> {
    let text = String::from_utf8_lossy(s);
    let mut out: Vec<String> = Vec::new();
    let mut skip_warning = false;
    for line in text.split('\n') {
        let l = line.trim_end();
        if l == "Execution halted" || l.is_empty() {
            continue;
        }
        // A `Warning message:` header and its indented continuation are R-only
        // chatter (rlang does not emit warnings yet); drop the whole block.
        if l.starts_with("Warning message") || l.starts_with("Warning messages") {
            skip_warning = true;
            continue;
        }
        if skip_warning {
            if l.starts_with(' ') || l.starts_with('\t') {
                continue;
            }
            skip_warning = false;
        }
        // Strip `Error in foo(x) : msg` / `Error: msg` down to `msg`.
        let l = if let Some(rest) = l.strip_prefix("Error") {
            match rest.find(':') {
                Some(idx) => rest[idx + 1..].trim(),
                None => rest.trim(),
            }
        } else {
            l
        };
        let l = l.strip_prefix("Rscript: ").unwrap_or(l);
        out.push(l.trim().to_string());
    }
    out.join("\n").into_bytes()
}

/// The divergence predicate. stdout + exit always; stderr only under `--stderr`.
fn differs(a: &RunOut, b: &RunOut) -> bool {
    if a.stdout != b.stdout || a.exit != b.exit {
        return true;
    }
    if CMP_STDERR.load(Ordering::Relaxed) {
        return norm_stderr(&a.stderr) != norm_stderr(&b.stderr);
    }
    false
}

/// Spawn `cmd` and wait up to `timeout`, killing it if it overruns.
fn run_with_timeout(mut cmd: Command, timeout: Duration) -> RunOut {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(_) => {
            return RunOut {
                stdout: Vec::new(),
                stderr: Vec::new(),
                exit: -999,
                timed_out: false,
            }
        }
    };
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                use std::io::Read;
                let mut buf = Vec::new();
                if let Some(mut out) = child.stdout.take() {
                    let _ = out.read_to_end(&mut buf);
                }
                let mut ebuf = Vec::new();
                if let Some(mut err) = child.stderr.take() {
                    let _ = err.read_to_end(&mut ebuf);
                }
                return RunOut {
                    stdout: buf,
                    stderr: ebuf,
                    exit: status.code().unwrap_or(-1),
                    timed_out: false,
                };
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return RunOut {
                        stdout: Vec::new(),
                        stderr: Vec::new(),
                        exit: -1,
                        timed_out: true,
                    };
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            Err(_) => {
                return RunOut {
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                    exit: -998,
                    timed_out: false,
                }
            }
        }
    }
}

/// Run the reference R with `--vanilla` so a user `~/.Rprofile`/`Renviron`
/// cannot perturb output, matching the minimal environment rlang runs in.
fn run_oracle(script: &str, timeout: Duration) -> RunOut {
    let mut cmd = Command::new(oracle_path());
    cmd.args(["--vanilla", "-e", script]);
    run_with_timeout(cmd, timeout)
}

fn run_ours(script: &str, bin: &Path, timeout: Duration) -> RunOut {
    let mut cmd = Command::new(bin);
    cmd.args(["-e", script]);
    // Test rlang's own compiled path, never the embedded-R fallback — a
    // divergence must come from rlang, not from R answering for it.
    cmd.env("RLANG_NO_CRAN", "1");
    // Redirect the bytecode cache into a private HOME so the fuzzer never
    // pollutes the user's ~/.rlang (see `ours_home`).
    cmd.env("HOME", ours_home());
    run_with_timeout(cmd, timeout)
}

// ---------------------------------------------------------------------------
// Literal pools + builders shared by the generators.
// ---------------------------------------------------------------------------

const INTS: &[&str] = &["0", "1", "2", "3", "5", "7", "10", "42", "100"];
const NEG_INTS: &[&str] = &["-1", "-2", "-3", "-7", "-10"];
const DBLS: &[&str] = &["0.5", "1.5", "2.25", "3.14", "10.0", "0.1", "0.333", "100.25"];
const WORDS: &[&str] = &["foo", "bar", "baz", "hello", "world", "abc", "xyz", "quux"];

/// A non-negative integer literal — safe as the first token of a program.
fn ii<'a>(r: &mut Rng) -> &'a str {
    r.pick(INTS)
}
/// A signed integer literal — only for non-leading positions.
fn si<'a>(r: &mut Rng) -> &'a str {
    if r.below(2) == 0 {
        r.pick(INTS)
    } else {
        r.pick(NEG_INTS)
    }
}
fn ff<'a>(r: &mut Rng) -> &'a str {
    r.pick(DBLS)
}
fn ww<'a>(r: &mut Rng) -> &'a str {
    r.pick(WORDS)
}

fn one(s: String) -> Vec<String> {
    vec![s]
}

/// `c(a, b, c, …)` of 3–5 signed ints.
fn vec_int(r: &mut Rng) -> String {
    let n = r.range(3, 5) as usize;
    let items: Vec<&str> = (0..n).map(|_| si(r)).collect();
    format!("c({})", items.join(", "))
}

/// `c(a, b, c, …)` of 3–5 doubles.
fn vec_dbl(r: &mut Rng) -> String {
    let n = r.range(3, 5) as usize;
    let items: Vec<&str> = (0..n).map(|_| ff(r)).collect();
    format!("c({})", items.join(", "))
}

/// `c("w1", "w2", …)` of 3–4 words.
fn vec_str(r: &mut Rng) -> String {
    let n = r.range(3, 4) as usize;
    let items: Vec<String> = (0..n).map(|_| format!("\"{}\"", ww(r))).collect();
    format!("c({})", items.join(", "))
}

// ---------------------------------------------------------------------------
// Generators — one per Mode. Each returns a statement list joined by newlines.
// Every program's first token is a letter/paren/digit (never `-`).
// ---------------------------------------------------------------------------

fn gen_arith(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    // `^` is kept OUT of the chained pool: `3 ^ 100` overflows f64's exact
    // integer range, and a following `%% / %/%` then lands in R's documented
    // "complete loss of accuracy" regime, where R uses extended-precision
    // (long double) intermediates that Rust's f64 cannot reproduce. Power is
    // exercised separately below with a small, bounded exponent.
    let ops = ["+", "-", "*", "/", "%%", "%/%"];
    let a = ii(r);
    let b = si(r);
    let c = si(r);
    let op1 = r.pick(&ops);
    let op2 = r.pick(&ops);
    one(match r.below(6) {
        0 => format!("{a} {op1} {b} {op2} {c}"),
        1 => format!("({a} {op1} {b}) {op2} {c}"),
        2 => format!("{a}L {op1} {b}L"),
        3 => format!("abs({b} {op1} {c})"),
        4 => format!("{a} ^ {} {op1} {b}", r.range(0, 5)),
        _ => format!("({a} + 0.0) {op1} {b}"),
    })
}

fn gen_numfmt(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let a = ff(r);
    let b = ff(r);
    one(match r.below(8) {
        0 => format!("{a} / {b}"),
        // Round an irrational product, not a divide that can land on an exact
        // N.NN5 tie — R rounds ties in C `long double`, unreproducible in f64
        // (see gen_rounding).
        1 => format!("round({a} * pi, {})", r.range(0, 6)),
        2 => format!("signif({a} * {b}, {})", r.range(1, 6)),
        3 => format!("format({a} / {b}, nsmall = {})", r.range(0, 5)),
        4 => format!("c({a}, {b}, {a} * {b})"),
        5 => format!("formatC({a} / {b}, digits = {}, format = \"f\")", r.range(0, 5)),
        6 => format!("prettyNum({}, big.mark = \",\")", r.range(1000, 9_999_999)),
        _ => format!("sqrt({a}) + {b}"),
    })
}

fn gen_vectors(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let v = vec_int(r);
    let i = r.range(1, 4);
    one(match r.below(9) {
        0 => format!("{v}[{i}]"),
        1 => format!("({v})[-{i}]"),
        2 => format!("({v})[c({}, {})]", r.range(1, 3), r.range(1, 3)),
        3 => format!("({v})[{v} > 0]"),
        4 => format!("head({v}, {})", r.range(1, 3)),
        5 => format!("tail({v}, {})", r.range(1, 3)),
        6 => format!("length({v})"),
        7 => format!("({v})[c(TRUE, FALSE)]"),
        _ => format!("rev({v})"),
    })
}

fn gen_seqrep(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let a = r.range(0, 4);
    let b = r.range(a + 1, a + 9);
    one(match r.below(9) {
        0 => format!("seq({a}, {b})"),
        1 => format!("seq({a}, {b}, by = {})", *r.pick(&["0.5", "0.25", "2", "1.5"])),
        2 => format!("seq_len({})", r.range(0, 6)),
        3 => format!("seq_along(c({}, {}, {}))", si(r), si(r), si(r)),
        4 => format!("rep({}, {})", si(r), r.range(1, 5)),
        5 => format!("rep(c({}, {}), times = {})", si(r), si(r), r.range(1, 4)),
        6 => format!("rep(c({}, {}), each = {})", si(r), si(r), r.range(1, 4)),
        7 => format!("seq({a}, {b}, length.out = {})", r.range(2, 6)),
        _ => format!("{a}:{b}"),
    })
}

fn gen_vecmath(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    // Half the draws use a double vector so float-vector print alignment
    // (`digits`, decimal padding) is exercised alongside the integer path.
    let v = if r.below(2) == 0 { vec_int(r) } else { vec_dbl(r) };
    // `var`/`sd` run only on integer vectors: on fractional inputs R accumulates
    // the sum of squares in C `long double`, so a result landing on a 7th-sig
    // rounding tie prints one ULP off from Rust's f64 — a precision artifact,
    // not an algorithm gap (the two-pass formula matches R). Integer inputs sum
    // exactly, so `var`/`sd` still get real coverage without the false gap.
    let vi = vec_int(r);
    one(match r.below(12) {
        0 => format!("sum({v})"),
        1 => format!("prod({v})"),
        2 => format!("mean({v})"),
        3 => format!("max({v})"),
        4 => format!("min({v})"),
        5 => format!("range({v})"),
        6 => format!("cumsum({v})"),
        7 => format!("cumprod({v})"),
        8 => format!("diff({v})"),
        9 => format!("median({v})"),
        10 => format!("var({vi})"),
        _ => format!("sd({vi})"),
    })
}

fn gen_sortops(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let v = vec_int(r);
    one(match r.below(9) {
        0 => format!("sort({v})"),
        1 => format!("sort({v}, decreasing = TRUE)"),
        2 => format!("order({v})"),
        3 => format!("rank({v})"),
        4 => format!("rev(sort({v}))"),
        5 => format!("unique({v})"),
        6 => format!("duplicated({v})"),
        7 => format!("which.max({v})"),
        _ => format!("which.min({v})"),
    })
}

fn gen_strings(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let w = ww(r);
    one(match r.below(11) {
        0 => format!("paste(\"{w}\", \"{}\")", ww(r)),
        1 => format!("paste0(\"{w}\", {})", r.range(1, 9)),
        2 => format!("paste(\"{w}\", \"{}\", sep = \"-\")", ww(r)),
        3 => format!("paste(c(\"{w}\", \"{}\"), collapse = \"+\")", ww(r)),
        4 => format!("nchar(\"{w}\")"),
        5 => format!("substr(\"{w}\", {}, {})", r.range(1, 3), r.range(3, 5)),
        6 => format!("toupper(\"{w}\")"),
        7 => format!("tolower(\"ABC{w}\")"),
        8 => format!("substring(\"{w}\", {})", r.range(1, 4)),
        9 => format!("trimws(\"  {w}  \")"),
        _ => format!("rev(strsplit(\"{w}\", \"\")[[1]])"),
    })
}

fn gen_strproc(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let s = format!("{}{}", ww(r), ww(r));
    let pats = ["[a-c]+", "o+", "[aeiou]", "l+", "^.", ".$", "z", "[a-z]{2}"];
    let p = r.pick(&pats);
    one(match r.below(10) {
        0 => format!("grepl(\"{p}\", \"{s}\")"),
        1 => format!("sub(\"{p}\", \"X\", \"{s}\")"),
        2 => format!("gsub(\"{p}\", \"X\", \"{s}\")"),
        3 => format!("grep(\"{p}\", c(\"{s}\", \"{}\"))", ww(r)),
        4 => format!("regmatches(\"{s}\", regexpr(\"{p}\", \"{s}\"))"),
        5 => format!("startsWith(\"{s}\", \"{}\")", &s[..1.min(s.len())]),
        6 => format!("endsWith(\"{s}\", \"{}\")", ww(r)),
        7 => format!("strsplit(\"{s}\", \"{p}\")"),
        8 => format!("nchar(gsub(\"{p}\", \"\", \"{s}\"))"),
        _ => format!("length(gregexpr(\"{p}\", \"{s}\")[[1]])"),
    })
}

fn gen_sprintf(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let n = si(r);
    let f = ff(r);
    let w = ww(r);
    one(match r.below(10) {
        0 => format!("sprintf(\"%.3f\", {f})"),
        1 => format!("sprintf(\"%05d\", {})", r.range(0, 999)),
        2 => format!("sprintf(\"%x\", {})", r.range(0, 999)),
        3 => format!("sprintf(\"%e\", {f})"),
        4 => format!("sprintf(\"%-8s|\", \"{w}\")"),
        5 => format!("sprintf(\"%+d\", {n})"),
        6 => format!("sprintf(\"%8.2f\", {f})"),
        7 => format!("sprintf(\"%d-%s\", {}, \"{w}\")", r.range(0, 99)),
        8 => format!("sprintf(\"%g\", {f} * {})", r.range(1, 1000)),
        _ => format!("formatC({n}, width = 6, flag = \"0\")"),
    })
}

fn gen_logical(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let v = vec_int(r);
    // `a` leads case 0 at column 0, so it must be non-negative — a leading `-`
    // is misparsed by both arg parsers and is a false gap, not a language one.
    let a = ii(r);
    let b = si(r);
    one(match r.below(11) {
        0 => format!("{a} > {b}"),
        1 => format!("({v}) > 0"),
        2 => format!("any(({v}) > 0)"),
        3 => format!("all(({v}) > 0)"),
        4 => format!("which(({v}) %% 2 == 0)"),
        5 => format!("xor({a} > 0, {b} > 0)"),
        6 => format!("({v}) >= {a} & ({v}) <= {b}"),
        7 => format!("sum(({v}) > 0)"),
        8 => format!("isTRUE({a} == {b})"),
        9 => format!("!c(TRUE, FALSE, {})", if r.below(2) == 0 { "TRUE" } else { "NA" }),
        _ => format!("({a} > {b}) || ({a} < {b})"),
    })
}

fn gen_ifelse(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let v = vec_int(r);
    let n = r.range(0, 10);
    one(match r.below(6) {
        0 => format!("ifelse(({v}) > 0, \"pos\", \"nonpos\")"),
        1 => format!("if ({n} > 5) \"hi\" else \"lo\""),
        2 => format!("ifelse(({v}) %% 2 == 0, ({v}), 0L)"),
        3 => format!("if ({n} %% 2 == 0) \"even\" else \"odd\""),
        4 => format!("ifelse(is.na(c(1, NA, {})), -1, 0)", si(r)),
        _ => format!("max(0, {})", si(r)),
    })
}

fn gen_control(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let n = r.range(3, 7);
    one(match r.below(6) {
        0 => format!("s <- 0; for (i in 1:{n}) s <- s + i; s"),
        1 => format!("v <- c(); for (i in 1:{n}) v <- c(v, i * i); v"),
        2 => format!("i <- 1; s <- 0; while (i <= {n}) {{ s <- s + i; i <- i + 1 }}; s"),
        3 => format!("acc <- 1; for (i in 1:{n}) acc <- acc * i; acc"),
        4 => format!("out <- c(); for (w in c(\"{}\", \"{}\")) out <- c(out, nchar(w)); out", ww(r), ww(r)),
        _ => format!("i <- 0; repeat {{ i <- i + 1; if (i >= {n}) break }}; i"),
    })
}

fn gen_funcs(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let n = r.range(2, 8);
    one(match r.below(6) {
        0 => format!("f <- function(x) x * x + 1; f({})", si(r)),
        1 => format!("fact <- function(n) if (n <= 1) 1 else n * fact(n - 1); fact({n})"),
        2 => format!(
            "fib <- function(n) if (n < 2) n else fib(n - 1) + fib(n - 2); fib({})",
            r.range(2, 12)
        ),
        3 => format!("adder <- function(a) function(b) a + b; adder({})({})", si(r), si(r)),
        4 => format!("f <- function(x, y = {}) x + y; f({})", si(r), si(r)),
        _ => format!("g <- function(...) sum(...); g({}, {}, {})", si(r), si(r), si(r)),
    })
}

fn gen_apply(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let v = vec_int(r);
    let n = r.range(2, 5);
    one(match r.below(9) {
        0 => format!("sapply(1:{n}, function(x) x ^ 2)"),
        1 => format!("vapply(1:{n}, function(x) x * 2L, integer(1))"),
        2 => format!("unlist(lapply({v}, function(x) x + 1))"),
        3 => format!("mapply(function(a, b) a + b, 1:{n}, {n}:1)"),
        4 => format!("Reduce(`+`, {v})"),
        5 => format!("Reduce(function(a, b) a * b, 1:{n}, accumulate = TRUE)"),
        6 => format!("Filter(function(x) x > 0, {v})"),
        7 => format!("unlist(Map(function(a, b) a - b, 1:{n}, {n}:1))"),
        _ => format!("do.call(paste, as.list(c(\"{}\", \"{}\")))", ww(r), ww(r)),
    })
}

fn gen_lists(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let (a, b) = (si(r), si(r));
    one(match r.below(8) {
        0 => format!("l <- list(a = {a}, b = {b}); l$a + l$b"),
        1 => format!("l <- list({a}, {b}, {}); l[[2]]", si(r)),
        2 => format!("names(list(x = {a}, y = {b}))"),
        3 => format!("unlist(list({a}, {b}, {}))", si(r)),
        4 => format!("setNames(c({a}, {b}), c(\"{}\", \"{}\"))", ww(r), ww(r)),
        5 => format!("l <- list(a = {a}); l$b <- {b}; unlist(l)"),
        6 => format!("length(list({a}, {b}, list({}, {})))", si(r), si(r)),
        _ => format!("lengths(list(1:{}, 1:{}))", r.range(1, 4), r.range(1, 4)),
    })
}

fn gen_matrix(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let (nr, nc) = (r.range(2, 3), r.range(2, 3));
    let n = nr * nc;
    one(match r.below(9) {
        0 => format!("matrix(1:{n}, nrow = {nr})"),
        1 => format!("matrix(1:{n}, nrow = {nr}, byrow = TRUE)"),
        2 => format!("t(matrix(1:{n}, nrow = {nr}))"),
        3 => format!("dim(matrix(1:{n}, nrow = {nr}))"),
        4 => format!("rowSums(matrix(1:{n}, nrow = {nr}))"),
        5 => format!("colSums(matrix(1:{n}, nrow = {nr}))"),
        6 => format!("apply(matrix(1:{n}, nrow = {nr}), 1, sum)"),
        7 => format!("diag(matrix(1:{}, nrow = {n2}))", n * n, n2 = n),
        _ => format!("matrix(1:{n}, nrow = {nr}) %*% diag({nc})"),
    })
}

fn gen_types(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let n = si(r);
    let f = ff(r);
    one(match r.below(11) {
        0 => format!("as.integer({f})"),
        1 => format!("as.numeric(\"{f}\")"),
        2 => format!("as.character({n})"),
        3 => format!("as.logical({})", *r.pick(&["0", "1", "2"])),
        4 => format!("class({})", if r.below(2) == 0 { format!("{n}L") } else { f.to_string() }),
        5 => format!("typeof({n}L)"),
        6 => format!("is.na(c({n}, NA, {f}))"),
        7 => format!("as.integer(c(\"{}\", \"{}\"))", r.range(0, 99), r.range(0, 99)),
        8 => format!("storage.mode({n}L)"),
        9 => format!("as.numeric(TRUE) + {f}"),
        _ => format!("round(as.numeric(\"{f}\") * {})", r.range(1, 9)),
    })
}

fn gen_setops(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let a = vec_int(r);
    let b = vec_int(r);
    one(match r.below(9) {
        0 => format!("union({a}, {b})"),
        1 => format!("intersect({a}, {b})"),
        2 => format!("setdiff({a}, {b})"),
        3 => format!("{a} %in% {b}"),
        4 => format!("match({a}, {b})"),
        5 => format!("unique(c({a}, {b}))"),
        6 => format!("sort(unique(c({a}, {b})))"),
        7 => format!("is.element({}, {b})", si(r)),
        _ => format!("as.vector(table(c({a}, {b})))"),
    })
}

fn gen_rounding(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let f = ff(r);
    let g = ff(r);
    one(match r.below(9) {
        // Round an irrational product, never an exact N.NN5 tie: at a decimal
        // tie R rounds in C `long double` (its `fround`) while Rust rounds the
        // f64, so the two can pick opposite even neighbours — a precision
        // artifact, not an algorithm gap. Non-tie inputs exercise the same path.
        0 => format!("round({f} * pi, {})", r.range(0, 4)),
        1 => format!("ceiling({f} * {g})"),
        2 => format!("floor({f} * {g})"),
        3 => format!("trunc({f} * {g})"),
        4 => format!("signif({f} * {g}, {})", r.range(1, 5)),
        5 => format!("round(c(0.5, 1.5, 2.5, 3.5))"),
        6 => format!("round({f} * 100) / 100"),
        7 => format!("ceiling(sqrt({}))", r.range(1, 200)),
        _ => format!("floor(log2({}))", r.range(1, 1000)),
    })
}

fn gen_bitops(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let a = r.range(0, 255);
    let b = r.range(0, 255);
    one(match r.below(6) {
        0 => format!("bitwAnd({a}L, {b}L)"),
        1 => format!("bitwOr({a}L, {b}L)"),
        2 => format!("bitwXor({a}L, {b}L)"),
        3 => format!("bitwShiftL({a}L, {})", r.range(0, 4)),
        4 => format!("bitwShiftR({a}L, {})", r.range(0, 4)),
        _ => format!("bitwNot({a}L)"),
    })
}

fn gen_factor(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let s = vec_str(r);
    one(match r.below(6) {
        0 => format!("as.integer(factor({s}))"),
        1 => format!("levels(factor({s}))"),
        2 => format!("nlevels(factor({s}))"),
        3 => format!("as.character(factor({s}))"),
        4 => format!("table(factor({s}))"),
        _ => format!("as.vector(table(factor({s})))"),
    })
}

fn gen_trig(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let f = ff(r);
    // asin/acos want [-1,1]; a value outside gives NaN on both sides (parity).
    let unit = *r.pick(&["0.5", "0.25", "1.0", "0.75", "0.1"]);
    one(match r.below(12) {
        0 => format!("sin({f})"),
        1 => format!("cos({f})"),
        2 => format!("tan({f})"),
        3 => format!("asin({unit})"),
        4 => format!("acos({unit})"),
        5 => format!("atan({f})"),
        6 => format!("atan2({f}, {})", ff(r)),
        7 => format!("sinh({unit})"),
        8 => format!("cosh({unit})"),
        9 => format!("tanh({f})"),
        10 => format!("expm1({unit})"),
        _ => format!("log1p({unit})"),
    })
}

fn gen_mathfn(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let n = r.range(0, 10);
    let k = r.range(0, 6);
    one(match r.below(11) {
        0 => format!("factorial({n})"),
        1 => format!("choose({}, {k})", r.range(0, 12)),
        2 => format!("gamma({})", r.range(1, 9)),
        3 => format!("lgamma({})", r.range(1, 40)),
        4 => format!("beta({}, {})", r.range(1, 6), r.range(1, 6)),
        5 => format!("lbeta({}, {})", r.range(1, 9), r.range(1, 9)),
        6 => format!("sign({})", si(r)),
        7 => format!("cumsum({})", vec_int(r)),
        8 => format!("cumprod(1:{})", r.range(1, 6)),
        9 => format!("lfactorial({})", r.range(1, 20)),
        _ => format!("factorial({}) / factorial({})", r.range(3, 8), r.range(1, 3)),
    })
}

fn gen_pmaxmin(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let a = vec_int(r);
    let b = vec_int(r);
    one(match r.below(9) {
        0 => format!("pmax({a}, {b})"),
        1 => format!("pmin({a}, {b})"),
        2 => format!("pmax({a}, 0)"),
        3 => format!("cummax({a})"),
        4 => format!("cummin({a})"),
        5 => format!("tabulate(c({}, {}, {}, {}), {})", r.range(1,4), r.range(1,4), r.range(1,4), r.range(1,4), r.range(3,5)),
        6 => format!("findInterval(c({}, {}), c(1, 2, 3, 4))", r.range(0,5), r.range(0,5)),
        7 => format!("pmin(pmax({a}, 0), 3)"),
        _ => format!("range({a})"),
    })
}

fn gen_linalg(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let n = r.range(2, 3);
    let m = r.range(2, 3);
    one(match r.below(9) {
        0 => format!("outer(1:{n}, 1:{m})"),
        1 => format!("outer(1:{n}, 1:{m}, \"+\")"),
        2 => format!("cbind(1:{n}, {}:{})", n + 1, n + n),
        3 => format!("rbind(1:{m}, {}:{})", m + 1, m + m),
        4 => format!("crossprod(matrix(1:{}, nrow = {n}))", n * m),
        5 => format!("tcrossprod(matrix(1:{}, nrow = {n}))", n * m),
        6 => format!("t(outer(1:{n}, 1:{m}))"),
        7 => format!("matrix(1:{}, nrow = {n}) %*% 1:{m}", n * m),
        _ => format!("diag(outer(1:{n}, 1:{n}))"),
    })
}

fn gen_stringx(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let w = ww(r);
    one(match r.below(9) {
        0 => format!("chartr(\"abc\", \"ABC\", \"{w}\")"),
        1 => format!("strtoi(\"{}\", {})", r.range(10, 999), *r.pick(&["10", "8", "16"])),
        2 => format!("sprintf(\"%d:%s\", 1:3, \"{w}\")"),
        3 => format!("toupper(chartr(\"aeiou\", \"AEIOU\", \"{w}\"))"),
        4 => format!("paste(rev(strsplit(\"{w}\", \"\")[[1]]), collapse = \"\")"),
        5 => format!("strtoi(\"{}\")", r.range(0, 9999)),
        6 => format!("nchar(chartr(\"{}\", \"X\", \"{w}\"))", &w[..1.min(w.len())]),
        7 => format!("sprintf(\"[%s]\", c(\"{w}\", \"{}\"))", ww(r)),
        _ => format!("chartr(\"{w}\", \"{}\", \"{w}{w}\")", ww(r)),
    })
}

fn gen_listx(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let n = r.range(3, 6);
    one(match r.below(8) {
        0 => format!("Position(function(x) x > {}, c({}, {}, {}))", r.range(1,3), si(r), si(r), si(r)),
        1 => format!("Find(function(x) x %% 2 == 0, 1:{n})"),
        2 => format!("Filter(function(x) x > 0, {})", vec_int(r)),
        3 => format!("Reduce(function(a, b) a + b, 1:{n}, accumulate = TRUE)"),
        4 => format!("mapply(function(a, b) a * b, 1:{n}, {n}:1)"),
        5 => format!("lengths(list(1:{}, 1:{}, 1:{}))", r.range(1,4), r.range(1,4), r.range(1,4)),
        6 => format!("do.call(pmax, list(c(1, 5), c(3, 2)))"),
        _ => format!("unlist(Map(`+`, 1:{n}, {n}:1))"),
    })
}

/// A vector literal mixing finite values with the special markers R prints
/// deterministically (`NA`, `NaN`, `Inf`, `-Inf`).
fn special_vec(r: &mut Rng) -> String {
    let atoms = ["1", "2", "-3", "0", "NA", "NaN", "Inf", "-Inf", "5.5"];
    let n = r.range(3, 5) as usize;
    let items: Vec<&str> = (0..n).map(|_| *r.pick(&atoms)).collect();
    format!("c({})", items.join(", "))
}

fn gen_predicates(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let v = special_vec(r);
    one(match r.below(9) {
        0 => format!("is.na({v})"),
        1 => format!("is.nan({v})"),
        2 => format!("is.finite({v})"),
        3 => format!("is.infinite({v})"),
        4 => format!("anyNA({v})"),
        5 => format!("complete.cases({v})"),
        6 => format!("sum(is.na({v}))"),
        7 => format!("which(is.finite({v}))"),
        _ => format!("{v}[is.finite({v})]"),
    })
}

fn gen_numedge(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let v = special_vec(r);
    let empty = *r.pick(&["numeric(0)", "integer(0)"]);
    one(match r.below(9) {
        0 => format!("max({empty})"),
        1 => format!("min({empty})"),
        2 => format!("range({empty})"),
        3 => format!("sum({v}, na.rm = TRUE)"),
        4 => format!("max({v}, na.rm = TRUE)"),
        5 => format!("mean({v}, na.rm = TRUE)"),
        6 => format!("prod({empty})"),
        7 => format!("cumsum(1:{})", r.range(1, 6)),
        _ => format!("range({v}, na.rm = TRUE)"),
    })
}

fn gen_strx2(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let w = ww(r);
    one(match r.below(9) {
        0 => format!("strrep(\"{w}\", {})", r.range(0, 4)),
        1 => format!("trimws(\"  {w}  \", which = \"{}\")", *r.pick(&["left", "right", "both"])),
        2 => format!("substring(\"{w}\", 1:{})", r.range(2, 4)),
        3 => format!("encodeString(\"{w}\\t{w}\")"),
        4 => format!("x <- \"{w}\"; substr(x, {}, {}) <- \"XY\"; x", r.range(1, 3), r.range(3, 5)),
        5 => format!("strrep(c(\"{w}\", \"{}\"), 2)", ww(r)),
        6 => format!("nchar(strrep(\"{w}\", {}))", r.range(1, 5)),
        7 => format!("toupper(substring(\"{w}{w}\", {}))", r.range(1, 4)),
        _ => format!("encodeString(c(\"{w}\", \"a\\nb\"))"),
    })
}

fn gen_listx2(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let n = r.range(4, 6);
    let keys = "c(\"a\", \"b\", \"a\", \"b\", \"c\")";
    one(match r.below(9) {
        0 => format!("split(1:5, {keys})"),
        1 => format!("tapply(c({}, {}, {}, {}, {}), {keys}, sum)", si(r), si(r), si(r), si(r), si(r)),
        2 => format!("modifyList(list(a = {}, b = {}), list(b = {}))", si(r), si(r), si(r)),
        3 => format!("Reduce(`-`, 1:{n}, right = TRUE)"),
        4 => format!("Reduce(`+`, 1:{n}, accumulate = TRUE, right = TRUE)"),
        5 => format!("rapply(list({}, {}), function(x) x * 2, how = \"unlist\")", si(r), si(r)),
        6 => format!("vapply(1:{n}, function(x) c(x, x * x), numeric(2))"),
        7 => format!("sapply(1:{n}, function(x) c(x, -x))"),
        _ => format!("tapply(1:5, {keys}, length)"),
    })
}

fn gen_indexing(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let v = vec_int(r);
    let i = r.range(1, 4);
    one(match r.below(10) {
        0 => format!("m <- matrix(1:6, nrow = 2); m[{}, ]", r.range(1, 2)),
        1 => format!("m <- matrix(1:6, nrow = 2); m[, {}]", r.range(1, 3)),
        2 => format!("m <- matrix(1:6, nrow = 2); m[{}, {}]", r.range(1, 2), r.range(1, 3)),
        3 => format!("({v})[-{i}]"),
        4 => format!("({v})[c(TRUE, FALSE)]"),
        5 => format!("x <- c(a = 1, b = 2, c = 3); x[\"{}\"]", *r.pick(&["a", "b", "c"])),
        6 => format!("({v})[{i}:{}]", r.range(1, 4)),
        7 => format!("l <- list({}, {}, {}); l[[{}]]", si(r), si(r), si(r), r.range(1, 3)),
        8 => format!("({v})[({v}) > 0]"),
        _ => format!("({v})[c(-1, -2)]"),
    })
}

fn gen_replace(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    one(match r.below(9) {
        0 => format!("x <- 1:5; x[{}] <- {}; x", r.range(1, 5), si(r)),
        1 => format!("x <- 1:5; x[x > {}] <- 0; x", r.range(1, 3)),
        2 => format!("x <- 1:3; names(x) <- c(\"a\", \"b\", \"c\"); x"),
        3 => format!("x <- 1:6; dim(x) <- c(2, 3); x"),
        4 => format!("x <- c(1, 2); length(x) <- {}; x", r.range(3, 5)),
        5 => format!("m <- matrix(1:4, 2); m[{}, {}] <- 9; m", r.range(1, 2), r.range(1, 2)),
        6 => format!("l <- list(a = 1, b = 2); l$c <- {}; l", si(r)),
        7 => format!("x <- 1:5; x[[{}]] <- {}; x", r.range(1, 5), si(r)),
        _ => format!("x <- 1:5; x[-1] <- 0; x"),
    })
}

fn gen_switch(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let key = *r.pick(&["a", "b", "c", "z"]);
    let n = r.range(1, 4);
    one(match r.below(8) {
        0 => format!("switch(\"{key}\", a = 1, b = 2, c = 3)"),
        1 => format!("switch(\"{key}\", a = 1, b = 2, 99)"),
        2 => format!("switch({n}, \"x\", \"y\", \"z\")"),
        3 => format!("switch(\"{key}\", a = , b = 2, c = 3)"),
        4 => format!(
            "f <- function(t) switch(t, a = \"A\", b = \"B\", \"?\"); f(\"{key}\")"
        ),
        5 => format!("x <- switch(\"{key}\", a = 10); is.null(x)"),
        6 => format!("sapply(c(\"a\", \"b\"), function(x) switch(x, a = 1, b = 2))"),
        _ => format!("switch({n} + 1, {}, {}, {})", si(r), si(r), si(r)),
    })
}

fn gen_strx3(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let w = ww(r);
    one(match r.below(7) {
        0 => format!("casefold(\"{}\")", w.to_uppercase()),
        1 => format!("casefold(\"{w}\", upper = TRUE)"),
        2 => format!("chartr(\"a-e\", \"A-E\", \"{w}\")"),
        3 => format!("chartr(\"a-z\", \"A-Z\", \"{w}{w}\")"),
        4 => format!(
            "f <- function(n) if (n <= 1) 1 else n * Recall(n - 1); f({})",
            r.range(1, 8)
        ),
        5 => format!(
            "fib <- function(n) if (n < 2) n else Recall(n - 1) + Recall(n - 2); fib({})",
            r.range(2, 12)
        ),
        _ => format!("casefold(chartr(\"a-c\", \"A-C\", \"{w}\"))"),
    })
}

fn gen_regexflags(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let s = format!("{}{}", ww(r), ww(r));
    let pats = ["[A-C]+", "O+", "[AEIOU]", "L+", "[A-Z]{2}"];
    let p = r.pick(&pats);
    one(match r.below(6) {
        0 => format!("grepl(\"{p}\", \"{s}\", ignore.case = TRUE)"),
        1 => format!("gsub(\"{p}\", \"X\", \"{s}\", ignore.case = TRUE)"),
        2 => format!("sub(\"{p}\", \"X\", \"{s}\", ignore.case = TRUE)"),
        3 => format!("grepl(\"{}\", \"{s}\", fixed = TRUE)", &s[..1.min(s.len())]),
        4 => format!("grep(\"{p}\", c(\"{s}\", \"{}\"), ignore.case = TRUE)", ww(r)),
        _ => format!("gsub(\"[aeiou]\", \"_\", \"{s}\", ignore.case = TRUE)"),
    })
}

fn gen_factorx(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let s = vec_str(r);
    one(match r.below(8) {
        0 => format!("as.integer(cut(1:{}, c(0, 2, 4, 6, 8)))", r.range(3, 8)),
        1 => format!("nlevels(cut(1:{}, c(0, 5, 10)))", r.range(2, 10)),
        2 => format!("cut(c({}, {}, {}), c(0, 3, 6, 9))", r.range(1, 8), r.range(1, 8), r.range(1, 8)),
        3 => format!("levels(cut(1:9, c(0, 3, 6, 9)))"),
        4 => format!("as.integer(droplevels(factor({s}, levels = c(\"a\", \"b\", \"c\", \"d\"))))"),
        5 => format!("droplevels(factor({s}, levels = c(\"a\", \"b\", \"c\", \"d\", \"e\")))"),
        6 => format!("factor({s}, levels = c(\"a\", \"b\", \"c\"), ordered = TRUE)"),
        _ => format!("nlevels(droplevels(factor({s}, levels = c(\"a\", \"b\", \"c\", \"d\"))))"),
    })
}

fn gen_deparsex(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    one(match r.below(8) {
        0 => format!("deparse({}:{})", r.range(1, 3), r.range(4, 9)),
        1 => format!("deparse(c({}, {}, {}))", ff(r), ff(r), ff(r)),
        2 => format!("deparse(c(\"{}\", \"{}\"))", ww(r), ww(r)),
        3 => format!("deparse({}L)", si(r)),
        4 => format!("deparse(c(TRUE, FALSE, NA))"),
        5 => format!("deparse({})", si(r)),
        6 => format!("diff(c({}, {}, {}, {}), differences = 2)", si(r), si(r), si(r), si(r)),
        _ => format!("diff(1:{}, lag = {})", r.range(5, 10), r.range(1, 3)),
    })
}

fn gen_fmtx(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    one(match r.below(8) {
        0 => format!("format(c({}, {}, {}))", si(r), si(r), si(r)),
        1 => format!("format(c({}, {}, {}))", ff(r), ff(r), ff(r)),
        2 => format!("format(c(\"{}\", \"{}\", \"{}\"))", ww(r), ww(r), ww(r)),
        3 => format!("sprintf(\"%o\", {})", r.range(0, 999)),
        4 => format!("sprintf(\"%o %x %X\", {}, {}, {})", r.range(0, 500), r.range(0, 500), r.range(0, 500)),
        5 => format!("format(c({}, {}), nsmall = {})", ff(r), ff(r), r.range(1, 4)),
        6 => format!("format(c({}, {}, {}), big.mark = \",\")", r.range(1000, 99999), r.range(1000, 99999), r.range(1000, 99999)),
        _ => format!("format(seq({}, {}, {}))", r.range(0, 3), r.range(8, 20), ff(r)),
    })
}

fn gen_seqx2(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    one(match r.below(9) {
        0 => format!("rep_len(1:{}, {})", r.range(2, 4), r.range(1, 9)),
        1 => format!("seq.int({}, {}, {})", r.range(0, 3), r.range(8, 16), r.range(2, 4)),
        2 => format!("rev(c(a = {}, b = {}, c = {}))", si(r), si(r), si(r)),
        3 => format!("unname(c(x = {}, y = {}))", si(r), si(r)),
        4 => format!("isTRUE(all.equal({}, {} + 1e-10))", r.range(1, 9), r.range(1, 9)),
        5 => format!("isTRUE(all.equal({}, {}))", si(r), si(r)),
        6 => format!("all.equal(c({}, {}), c({}, {}))", ff(r), ff(r), ff(r), ff(r)),
        7 => format!("rep_len(c(\"{}\", \"{}\"), {})", ww(r), ww(r), r.range(1, 7)),
        _ => format!("rev(setNames(1:3, c(\"a\", \"b\", \"c\")))"),
    })
}

fn gen_combinator(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let v = vec_int(r);
    one(match r.below(8) {
        0 => format!("Negate(is.na)(c({}, NA, {}))", si(r), si(r)),
        1 => format!("Filter(Negate(is.na), c({}, NA, {}, NA, {}))", si(r), si(r), si(r)),
        2 => format!("Negate(function(x) x > 0)({v})"),
        3 => format!("Vectorize(function(x) x ^ 2)(1:{})", r.range(2, 6)),
        4 => format!("Vectorize(function(x, y) x + y)(1:{n}, {n}:1)", n = r.range(2, 5)),
        5 => format!("sapply({v}, Negate(function(x) x > 0))"),
        6 => format!("is.function(Negate(is.null))"),
        _ => format!("Filter(Negate(function(x) x %% 2 == 0), 1:{})", r.range(3, 9)),
    })
}

fn gen_arrays(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    // Dimensions that multiply to <= 24 so the data 1:N fills exactly.
    let (d0, d1, d2) = (r.range(2, 3), r.range(2, 3), r.range(2, 4));
    let n = d0 * d1 * d2;
    one(match r.below(9) {
        0 => format!("array(1:{n}, c({d0}, {d1}, {d2}))[{}, {}, {}]", r.range(1, d0), r.range(1, d1), r.range(1, d2)),
        1 => format!("dim(array(1:{n}, c({d0}, {d1}, {d2})))"),
        2 => format!("apply(array(1:{n}, c({d0}, {d1}, {d2})), 3, sum)"),
        3 => format!("apply(array(1:{n}, c({d0}, {d1}, {d2})), 1, max)"),
        4 => format!("a <- array(1:{n}, c({d0}, {d1}, {d2})); a[, , {}]", r.range(1, d2)),
        5 => format!("a <- array(1:{n}, c({d0}, {d1}, {d2})); a[{}, , ]", r.range(1, d0)),
        6 => format!("array(1:{n}, c({d0}, {d1}, {d2}))"),
        7 => format!("aperm(matrix(1:{}, {d0}))", d0 * d1),
        _ => format!("length(array(0, c({d0}, {d1}, {d2})))"),
    })
}

fn gen_stat2(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let v = vec_int(r);
    one(match r.below(9) {
        0 => format!("quantile(1:{}, {})", r.range(4, 20), *r.pick(&["0.25", "0.5", "0.75", "0.1"])),
        1 => format!("quantile(1:{})", r.range(4, 20)),
        2 => format!("cor(1:{n}, (1:{n}) * {})", r.range(2, 5), n = r.range(3, 8)),
        3 => format!("rle(c({}, {}, {}, {}, {}))$lengths", r.range(1,3), r.range(1,3), r.range(1,3), r.range(1,3), r.range(1,3)),
        4 => format!("rle(c({}, {}, {}, {}, {}))", r.range(1,2), r.range(1,2), r.range(1,2), r.range(1,2), r.range(1,2)),
        5 => format!("inverse.rle(rle(c({}, {}, {}, {})))", r.range(1,3), r.range(1,3), r.range(1,3), r.range(1,3)),
        6 => format!("sort({v}, index.return = TRUE)$ix"),
        7 => format!("quantile({v})"),
        _ => format!("cor({v}, rev({v}))"),
    })
}

// ---------------------------------------------------------------------------
// Mode plumbing.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum Mode {
    Arith,
    Numfmt,
    Vectors,
    Seqrep,
    Vecmath,
    Sortops,
    Strings,
    Strproc,
    Sprintf,
    Logical,
    Ifelse,
    Control,
    Funcs,
    Apply,
    Lists,
    Matrix,
    Types,
    Setops,
    Rounding,
    Bitops,
    Factor,
    Trig,
    Mathfn,
    Pmaxmin,
    Linalg,
    Stringx,
    Listx,
    Predicates,
    Numedge,
    Strx2,
    Listx2,
    Indexing,
    Replace,
    Switch,
    Strx3,
    Regexflags,
    Factorx,
    Deparsex,
    Fmtx,
    Seqx2,
    Combinator,
    Arrays,
    Stat2,
}

const ALL_MODES: &[Mode] = &[
    Mode::Arith,
    Mode::Numfmt,
    Mode::Vectors,
    Mode::Seqrep,
    Mode::Vecmath,
    Mode::Sortops,
    Mode::Strings,
    Mode::Strproc,
    Mode::Sprintf,
    Mode::Logical,
    Mode::Ifelse,
    Mode::Control,
    Mode::Funcs,
    Mode::Apply,
    Mode::Lists,
    Mode::Matrix,
    Mode::Types,
    Mode::Setops,
    Mode::Rounding,
    Mode::Bitops,
    Mode::Factor,
    Mode::Trig,
    Mode::Mathfn,
    Mode::Pmaxmin,
    Mode::Linalg,
    Mode::Stringx,
    Mode::Listx,
    Mode::Predicates,
    Mode::Numedge,
    Mode::Strx2,
    Mode::Listx2,
    Mode::Indexing,
    Mode::Replace,
    Mode::Switch,
    Mode::Strx3,
    Mode::Regexflags,
    Mode::Factorx,
    Mode::Deparsex,
    Mode::Fmtx,
    Mode::Seqx2,
    Mode::Combinator,
    Mode::Arrays,
    Mode::Stat2,
];

fn gen_case(seed: u64, mode: Mode) -> Vec<String> {
    match mode {
        Mode::Arith => gen_arith(seed),
        Mode::Numfmt => gen_numfmt(seed),
        Mode::Vectors => gen_vectors(seed),
        Mode::Seqrep => gen_seqrep(seed),
        Mode::Vecmath => gen_vecmath(seed),
        Mode::Sortops => gen_sortops(seed),
        Mode::Strings => gen_strings(seed),
        Mode::Strproc => gen_strproc(seed),
        Mode::Sprintf => gen_sprintf(seed),
        Mode::Logical => gen_logical(seed),
        Mode::Ifelse => gen_ifelse(seed),
        Mode::Control => gen_control(seed),
        Mode::Funcs => gen_funcs(seed),
        Mode::Apply => gen_apply(seed),
        Mode::Lists => gen_lists(seed),
        Mode::Matrix => gen_matrix(seed),
        Mode::Types => gen_types(seed),
        Mode::Setops => gen_setops(seed),
        Mode::Rounding => gen_rounding(seed),
        Mode::Bitops => gen_bitops(seed),
        Mode::Factor => gen_factor(seed),
        Mode::Trig => gen_trig(seed),
        Mode::Mathfn => gen_mathfn(seed),
        Mode::Pmaxmin => gen_pmaxmin(seed),
        Mode::Linalg => gen_linalg(seed),
        Mode::Stringx => gen_stringx(seed),
        Mode::Listx => gen_listx(seed),
        Mode::Predicates => gen_predicates(seed),
        Mode::Numedge => gen_numedge(seed),
        Mode::Strx2 => gen_strx2(seed),
        Mode::Listx2 => gen_listx2(seed),
        Mode::Indexing => gen_indexing(seed),
        Mode::Replace => gen_replace(seed),
        Mode::Switch => gen_switch(seed),
        Mode::Strx3 => gen_strx3(seed),
        Mode::Regexflags => gen_regexflags(seed),
        Mode::Factorx => gen_factorx(seed),
        Mode::Deparsex => gen_deparsex(seed),
        Mode::Fmtx => gen_fmtx(seed),
        Mode::Seqx2 => gen_seqx2(seed),
        Mode::Combinator => gen_combinator(seed),
        Mode::Arrays => gen_arrays(seed),
        Mode::Stat2 => gen_stat2(seed),
    }
}

fn mode_name(m: Mode) -> &'static str {
    match m {
        Mode::Arith => "arith",
        Mode::Numfmt => "numfmt",
        Mode::Vectors => "vectors",
        Mode::Seqrep => "seqrep",
        Mode::Vecmath => "vecmath",
        Mode::Sortops => "sortops",
        Mode::Strings => "strings",
        Mode::Strproc => "strproc",
        Mode::Sprintf => "sprintf",
        Mode::Logical => "logical",
        Mode::Ifelse => "ifelse",
        Mode::Control => "control",
        Mode::Funcs => "funcs",
        Mode::Apply => "apply",
        Mode::Lists => "lists",
        Mode::Matrix => "matrix",
        Mode::Types => "types",
        Mode::Setops => "setops",
        Mode::Rounding => "rounding",
        Mode::Bitops => "bitops",
        Mode::Factor => "factor",
        Mode::Trig => "trig",
        Mode::Mathfn => "mathfn",
        Mode::Pmaxmin => "pmaxmin",
        Mode::Linalg => "linalg",
        Mode::Stringx => "stringx",
        Mode::Listx => "listx",
        Mode::Predicates => "predicates",
        Mode::Numedge => "numedge",
        Mode::Strx2 => "strx2",
        Mode::Listx2 => "listx2",
        Mode::Indexing => "indexing",
        Mode::Replace => "replace",
        Mode::Switch => "switch",
        Mode::Strx3 => "strx3",
        Mode::Regexflags => "regexflags",
        Mode::Factorx => "factorx",
        Mode::Deparsex => "deparsex",
        Mode::Fmtx => "fmtx",
        Mode::Seqx2 => "seqx2",
        Mode::Combinator => "combinator",
        Mode::Arrays => "arrays",
        Mode::Stat2 => "stat2",
    }
}

fn mode_from_name(s: &str) -> Option<Mode> {
    ALL_MODES.iter().copied().find(|&m| mode_name(m) == s)
}

fn build_program(stmts: &[String]) -> String {
    stmts.join("\n")
}

/// True iff oracle and rlang disagree on stdout or exit for `script`. Infra
/// failures (spawn/wait errors, timeouts) are NOT parity gaps.
fn diverges(script: &str, bin: &Path, timeout: Duration) -> bool {
    let o = run_oracle(script, timeout);
    if o.timed_out {
        return false;
    }
    let r = run_ours(script, bin, timeout);
    if r.exit == -999 || r.exit == -998 || r.timed_out || o.exit == -999 || o.exit == -998 {
        return false;
    }
    differs(&o, &r)
}

/// Delta-debug a diverging statement list to a locally-minimal one: repeatedly
/// drop any single statement whose removal preserves the divergence, to a
/// fixpoint.
fn minimize(stmts: Vec<String>, bin: &Path, timeout: Duration) -> Vec<String> {
    let mut cur = stmts;
    loop {
        let mut removed = false;
        let mut i = 0;
        while i < cur.len() {
            let mut cand = cur.clone();
            cand.remove(i);
            if !cand.is_empty() && diverges(&build_program(&cand), bin, timeout) {
                cur = cand;
                removed = true;
            } else {
                i += 1;
            }
        }
        if !removed {
            break;
        }
    }
    cur
}

/// Normalize a reproducer to a stable gap-class signature: keep the last
/// non-empty line (the probe), mask numeric literals and quoted words so many
/// instances of the same gap collapse to one signature.
fn signature(program: &str) -> String {
    let body = program
        .lines()
        .map(|l| l.trim())
        .rfind(|l| !l.is_empty())
        .unwrap_or("")
        .to_string();
    let mut s = body;
    for (pat, rep) in [
        (r"[0-9]+\.[0-9]+([eE][-+]?[0-9]+)?", "F"),
        (r"[0-9]+[eE][-+]?[0-9]+", "F"),
        (r"-?[0-9]+", "N"),
        ("\"[^\"]*\"", "W"),
        ("'[^']*'", "W"),
    ] {
        s = regex_lite_replace(&s, pat, rep);
    }
    s
}

fn regex_lite_replace(s: &str, pat: &str, rep: &str) -> String {
    match regex::Regex::new(pat) {
        Ok(re) => re.replace_all(s, rep).into_owned(),
        Err(_) => s.to_string(),
    }
}

// ---------------------------------------------------------------------------
// CLI.
// ---------------------------------------------------------------------------

struct Args {
    count: u64,
    base_seed: u64,
    once: bool,
    timeout_ms: u64,
    out_path: PathBuf,
    max_report: usize,
    jobs: usize,
    mode: Option<Mode>,
    verify: usize,
    baseline: Option<PathBuf>,
}

fn parse_args() -> Args {
    let mut count = 2000u64;
    let mut base_seed = 1u64;
    let mut once = false;
    let mut timeout_ms = 10000u64;
    let mut max_report = 200usize;
    let mut mode: Option<Mode> = None;
    let mut verify = 1usize;
    let mut baseline: Option<PathBuf> = None;
    let mut jobs = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let mut out_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("parity-fuzz")
        .join("divergences.txt");

    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "--count" | "-c" => {
                i += 1;
                count = argv.get(i).and_then(|s| s.parse().ok()).unwrap_or(count);
            }
            "--seed" | "-s" => {
                i += 1;
                base_seed = argv
                    .get(i)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(base_seed);
            }
            "--once" => once = true,
            "--timeout-ms" => {
                i += 1;
                timeout_ms = argv
                    .get(i)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(timeout_ms);
            }
            "--out" | "-o" => {
                i += 1;
                if let Some(p) = argv.get(i) {
                    out_path = PathBuf::from(p);
                }
            }
            "--max-report" => {
                i += 1;
                max_report = argv
                    .get(i)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(max_report);
            }
            "--jobs" | "-j" => {
                i += 1;
                jobs = argv
                    .get(i)
                    .and_then(|s| s.parse().ok())
                    .filter(|&j| j >= 1)
                    .unwrap_or(jobs);
            }
            "--mode" | "-m" => {
                i += 1;
                match argv.get(i).and_then(|s| mode_from_name(s)) {
                    Some(m) => mode = Some(m),
                    None => {
                        eprintln!(
                            "unknown --mode '{}'",
                            argv.get(i).map(|s| s.as_str()).unwrap_or("")
                        );
                        std::process::exit(2);
                    }
                }
            }
            a if a.starts_with("--") && mode_from_name(&a[2..]).is_some() => {
                mode = Some(mode_from_name(&a[2..]).unwrap());
            }
            "--verify" => {
                i += 1;
                verify = argv
                    .get(i)
                    .and_then(|s| s.parse().ok())
                    .filter(|&k| k >= 1)
                    .unwrap_or(verify);
            }
            "--baseline" => {
                i += 1;
                baseline = argv.get(i).map(PathBuf::from);
            }
            "--stderr" => {
                CMP_STDERR.store(true, Ordering::Relaxed);
            }
            "--help" | "-h" => {
                let modes: Vec<&str> = ALL_MODES.iter().copied().map(mode_name).collect();
                eprintln!(
                    "parity-fuzz — differential R/rlang parity fuzzer\n\
                     \n\
                     --count N        number of cases (default 2000)\n\
                     --seed N         base seed; case i uses seed+i (default 1)\n\
                     --mode M         one of: {}\n\
                     (each also accepted as a `--<mode>` shorthand; default: all\n\
                     modes, round-robin by case index)\n\
                     --stderr         also require the diagnostics to match\n\
                     --once           run a single case (seed) and print both outputs\n\
                     --timeout-ms N   per-interpreter wall-clock timeout (default 10000)\n\
                     --out PATH       divergence corpus file\n\
                     --max-report N   stop after N divergences (default 200)\n\
                     --jobs N         parallel workers (default = CPU count)\n\
                     --verify K       require K consecutive divergences to report (default 1)\n\
                     --baseline FILE  allowlist of known-gap signatures; only a NEW\n\
                                      divergence fails the run (exit 1)\n\
                     \n\
                     env  RLANG_FUZZ_RSCRIPT=PATH  the reference Rscript to compare against.\n\
                                      The oracle is part of the result; every run prints it.",
                    modes.join(", ")
                );
                std::process::exit(0);
            }
            _ => {}
        }
        i += 1;
    }
    Args {
        count,
        base_seed,
        once,
        timeout_ms,
        out_path,
        max_report,
        jobs,
        mode,
        verify,
        baseline,
    }
}

/// The mode for case `idx`: the pinned `--mode` if given, else round-robin over
/// every mode so a default run spreads coverage across all surfaces.
fn mode_for(idx: u64, pinned: Option<Mode>) -> Mode {
    match pinned {
        Some(m) => m,
        None => ALL_MODES[(idx as usize) % ALL_MODES.len()],
    }
}

fn main() {
    let args = parse_args();
    let bin = ours_bin();
    let timeout = Duration::from_millis(args.timeout_ms);
    let _ = std::fs::create_dir_all(ours_home());

    if !bin.exists() {
        eprintln!(
            "rlang binary not found at {}; run `cargo build` first",
            bin.display()
        );
        std::process::exit(2);
    }

    // --once: replay a single seed, minimize if it diverges, dump both sides.
    if args.once {
        let mode = mode_for(args.base_seed, args.mode);
        let stmts = gen_case(args.base_seed, mode);
        let script = build_program(&stmts);
        let o = run_oracle(&script, timeout);
        let r = run_ours(&script, &bin, timeout);
        let diverged = !o.timed_out && differs(&o, &r);
        println!("seed   : {}", args.base_seed);
        println!("mode   : {}", mode_name(mode));
        let (show, o, r) = if diverged && stmts.len() > 1 {
            let m = minimize(stmts, &bin, timeout);
            let ms = build_program(&m);
            let mo = run_oracle(&ms, timeout);
            let mr = run_ours(&ms, &bin, timeout);
            (ms, mo, mr)
        } else {
            (script, o, r)
        };
        println!("program:\n  {}", show.replace('\n', "\n  "));
        println!("--- R      exit={} timeout={} ---", o.exit, o.timed_out);
        let _ = std::io::stdout().write_all(&o.stdout);
        println!("--- rlang  exit={} timeout={} ---", r.exit, r.timed_out);
        let _ = std::io::stdout().write_all(&r.stdout);
        println!("--- {} ---", if diverged { "DIVERGE" } else { "match" });
        std::process::exit(if diverged { 1 } else { 0 });
    }

    use std::sync::atomic::AtomicU64;
    use std::sync::Mutex;

    let next = AtomicU64::new(0);
    let checked = AtomicU64::new(0);
    let timeouts = AtomicU64::new(0);
    let stop = AtomicBool::new(false);
    let divergences: Mutex<Vec<(u64, String)>> = Mutex::new(Vec::new());
    let start = Instant::now();

    eprintln!(
        "fuzzing {} cases across {} workers (mode {})…",
        args.count,
        args.jobs,
        args.mode.map(mode_name).unwrap_or("all"),
    );

    std::thread::scope(|scope| {
        for _ in 0..args.jobs {
            scope.spawn(|| loop {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                let idx = next.fetch_add(1, Ordering::Relaxed);
                if idx >= args.count {
                    break;
                }
                let seed = args.base_seed.wrapping_add(idx);
                let mode = mode_for(idx, args.mode);
                let stmts = gen_case(seed, mode);
                let script = build_program(&stmts);
                let o = run_oracle(&script, timeout);
                let r = run_ours(&script, &bin, timeout);
                let done = checked.fetch_add(1, Ordering::Relaxed) + 1;
                if o.timed_out || r.timed_out {
                    timeouts.fetch_add(1, Ordering::Relaxed);
                }
                // oracle-side timeout ⇒ pathological case; not a parity gap.
                if !o.timed_out && differs(&o, &r) {
                    let minimal = minimize(stmts, &bin, timeout);
                    let mscript = build_program(&minimal);
                    let mo = run_oracle(&mscript, timeout);
                    let mr = run_ours(&mscript, &bin, timeout);
                    // Re-verify: a real gap diverges every time; a transient
                    // won't reproduce. Require `verify` consecutive divergences.
                    let mut confirmed = differs(&mo, &mr);
                    for _ in 1..args.verify.max(1) {
                        if !confirmed {
                            break;
                        }
                        confirmed = diverges(&mscript, &bin, timeout);
                    }
                    if !confirmed {
                        return; // continue loop iteration
                    }
                    let err_of = |o: &RunOut| -> String {
                        if CMP_STDERR.load(Ordering::Relaxed) {
                            format!(
                                "\n  stderr: {}",
                                render(&norm_stderr(&o.stderr)).replace('\n', "\n  ")
                            )
                        } else {
                            String::new()
                        }
                    };
                    let rec = format!(
                        "==== seed {seed} (mode {}) ====\n\
                         program:\n  {}\n\
                         R     : exit={} timeout={}{}\n{}\n\
                         rlang : exit={} timeout={}{}\n{}\n",
                        mode_name(mode),
                        mscript.replace('\n', "\n  "),
                        mo.exit,
                        mo.timed_out,
                        err_of(&mo),
                        render(&mo.stdout),
                        mr.exit,
                        mr.timed_out,
                        err_of(&mr),
                        render(&mr.stdout),
                    );
                    let mut d = divergences.lock().unwrap();
                    d.push((seed, rec));
                    if d.len() >= args.max_report {
                        stop.store(true, Ordering::Relaxed);
                    }
                }
                if done % 500 == 0 {
                    let n = divergences.lock().unwrap().len();
                    eprintln!(
                        "  {done}/{} checked, {n} divergences, {:.0}/s",
                        args.count,
                        done as f64 / start.elapsed().as_secs_f64().max(0.001)
                    );
                }
            });
        }
    });

    let checked = checked.load(Ordering::Relaxed);
    let timeouts = timeouts.load(Ordering::Relaxed);
    let mut divergences: Vec<(u64, String)> = divergences.into_inner().unwrap();
    divergences.sort_by_key(|(seed, _)| *seed);
    let divergences: Vec<String> = divergences.into_iter().map(|(_, r)| r).collect();
    let elapsed = start.elapsed();

    let sig_of = |rec: &str| -> String {
        let prog = rec
            .split("program:\n")
            .nth(1)
            .and_then(|s| s.split("\nR     :").next())
            .unwrap_or(rec);
        signature(prog)
    };

    let allowed: std::collections::HashSet<String> = match &args.baseline {
        Some(bp) => std::fs::read_to_string(bp)
            .unwrap_or_default()
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect(),
        None => std::collections::HashSet::new(),
    };
    let mut new_records: Vec<&String> = Vec::new();
    let mut new_sigs: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut known = 0usize;
    for rec in &divergences {
        let sig = sig_of(rec);
        if args.baseline.is_some() && allowed.contains(&sig) {
            known += 1;
        } else {
            new_records.push(rec);
            new_sigs.insert(sig);
        }
    }

    let oracle = oracle_id();
    println!(
        "\nfuzzed {checked} cases in {:.1}s ({:.0}/s)\n\
         oracle      : {}\n\
         divergences : {} ({} known / {} new)\n\
         timeouts    : {}",
        elapsed.as_secs_f64(),
        checked as f64 / elapsed.as_secs_f64().max(0.001),
        oracle,
        divergences.len(),
        known,
        new_records.len(),
        timeouts,
    );

    if !divergences.is_empty() {
        if let Some(parent) = args.out_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(mut f) = std::fs::File::create(&args.out_path) {
            let _ = writeln!(f, "# oracle: {oracle}");
            for d in &divergences {
                let _ = writeln!(f, "{d}");
            }
            println!(
                "wrote {} divergences to {}",
                divergences.len(),
                args.out_path.display()
            );
        }
    }

    if !new_records.is_empty() {
        println!(
            "\n--- {} NEW gap signature(s) (add to baseline once triaged) ---",
            new_sigs.len()
        );
        for s in &new_sigs {
            println!("{s}");
        }
        println!(
            "\n--- first {} new divergence record(s) ---",
            new_records.len().min(5)
        );
        for d in new_records.iter().take(5) {
            println!("{d}");
        }
        std::process::exit(1);
    }
    if known > 0 {
        println!("all {known} divergences are known (in baseline) — OK");
    }
}
