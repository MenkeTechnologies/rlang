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
//! A clean run is only meaningful if it looked at something, so the run is
//! accounted in three buckets and every one of them reaches the exit status:
//! `compared` (the oracle answered and at least one side produced output),
//! `drained` (the oracle timed out or would not spawn, so the case was never
//! judged), and `no-signal` (both sides silent, which agrees trivially). If
//! `compared` is zero the run exits 2 instead of reporting "0 divergences" —
//! see the tail of [`main`].
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
            let s = if o.stdout.is_empty() {
                &o.stderr
            } else {
                &o.stdout
            };
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

/// Did this pair carry any *evidence*? Two runs that both printed nothing agree
/// trivially: the case exercised no observable behaviour, so counting it as a
/// match inflates the denominator without testing anything. A run made entirely
/// of these is a run that compared nothing, and [`main`] refuses to pass it.
fn no_signal(a: &RunOut, b: &RunOut) -> bool {
    if !a.stdout.is_empty() || !b.stdout.is_empty() {
        return false;
    }
    if CMP_STDERR.load(Ordering::Relaxed)
        && (!norm_stderr(&a.stderr).is_empty() || !norm_stderr(&b.stderr).is_empty())
    {
        return false;
    }
    true
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
const DBLS: &[&str] = &[
    "0.5", "1.5", "2.25", "3.14", "10.0", "0.1", "0.333", "100.25",
];
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
        5 => format!(
            "formatC({a} / {b}, digits = {}, format = \"f\")",
            r.range(0, 5)
        ),
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
        1 => format!(
            "seq({a}, {b}, by = {})",
            *r.pick(&["0.5", "0.25", "2", "1.5"])
        ),
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
    let v = if r.below(2) == 0 {
        vec_int(r)
    } else {
        vec_dbl(r)
    };
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
        9 => format!(
            "!c(TRUE, FALSE, {})",
            if r.below(2) == 0 { "TRUE" } else { "NA" }
        ),
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
        4 => format!(
            "out <- c(); for (w in c(\"{}\", \"{}\")) out <- c(out, nchar(w)); out",
            ww(r),
            ww(r)
        ),
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
        3 => format!(
            "adder <- function(a) function(b) a + b; adder({})({})",
            si(r),
            si(r)
        ),
        4 => format!("f <- function(x, y = {}) x + y; f({})", si(r), si(r)),
        _ => format!(
            "g <- function(...) sum(...); g({}, {}, {})",
            si(r),
            si(r),
            si(r)
        ),
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
        4 => format!(
            "class({})",
            if r.below(2) == 0 {
                format!("{n}L")
            } else {
                f.to_string()
            }
        ),
        5 => format!("typeof({n}L)"),
        6 => format!("is.na(c({n}, NA, {f}))"),
        7 => format!(
            "as.integer(c(\"{}\", \"{}\"))",
            r.range(0, 99),
            r.range(0, 99)
        ),
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
        5 => "round(c(0.5, 1.5, 2.5, 3.5))".to_string(),
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

/// A factor whose levels are a fixed alphabet, so two independently generated
/// factors share a level set — which is what `==` between factors requires.
fn some_factor(r: &mut Rng, ordered: bool) -> String {
    let n = r.range(3, 6) as usize;
    let items: Vec<String> = (0..n)
        .map(|_| format!("\"{}\"", r.pick(&["a", "b", "c", "d"])))
        .collect();
    format!(
        "factor(c({}), levels = c(\"a\", \"b\", \"c\", \"d\"){})",
        items.join(", "),
        if ordered { ", ordered = TRUE" } else { "" }
    )
}

/// Subsetting and reordering a factor. Every one of these rebuilds the codes,
/// so each is a chance to drop the `levels`/`class` attributes and silently
/// hand back bare integers — the failure this surface exists to catch.
fn gen_factorsub(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let ordered = r.below(4) == 0;
    let f = some_factor(r, ordered);
    let i = r.range(1, 3);
    let j = r.range(3, 5);
    one(match r.below(14) {
        0 => format!("({f})[{i}:{j}]"),
        1 => format!("({f})[-{i}]"),
        2 => format!("({f})[[{i}]]"),
        3 => format!("({f})[c(TRUE, FALSE)]"),
        4 => format!("({f})[{i}:{j}, drop = TRUE]"),
        5 => format!("head({f}, {i})"),
        6 => format!("tail({f}, {i})"),
        7 => format!("rev({f})"),
        8 => format!("sort({f})"),
        9 => format!("sort({f}, decreasing = TRUE)"),
        10 => format!("unique({f})"),
        11 => format!("rep({f}, {i})"),
        12 => format!("c({}, {})", some_factor(r, false), some_factor(r, false)),
        _ => format!("levels(({f})[{i}:{j}])"),
    })
}

/// Operators and label coercion on a factor. A factor compares by *label*, so
/// anything reading the codes here answers the wrong thing rather than erroring.
fn gen_factorops(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let f = some_factor(r, false);
    let o = some_factor(r, true);
    let w = *r.pick(&["a", "b", "c", "d", "z"]);
    one(match r.below(14) {
        0 => format!("({f}) == \"{w}\""),
        1 => format!("({f}) != \"{w}\""),
        2 => format!("(function(x) x[x == \"{w}\"])({f})"),
        3 => format!("which(({f}) == \"{w}\")"),
        4 => format!("({o}) < \"{w}\""),
        5 => format!("({o}) >= \"{w}\""),
        6 => format!("sum(({f}) == \"{w}\")"),
        7 => format!("({f}) %in% c(\"a\", \"{w}\")"),
        8 => format!("match({f}, c(\"a\", \"b\", \"c\", \"d\"))"),
        9 => format!("paste({f}, collapse = \"-\")"),
        10 => format!("as.vector({f})"),
        11 => format!("toString({f})"),
        12 => format!("droplevels(({f})[1:2])"),
        _ => format!("table(({f})[1:2])"),
    })
}

/// The condition system: `tryCatch` handler selection, `finally` ordering,
/// `on.exit` cleanup (including when the frame unwinds), `local` scoping and
/// `NextMethod` chaining. These are lazy special forms compiled to thunks, so a
/// mistake shows up as the body running at the wrong time — or not at all.
fn gen_conditions(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let w = ww(r);
    let n = r.range(1, 9);
    one(match r.below(16) {
        0 => format!("tryCatch({n}, error = function(e) \"caught\")"),
        1 => format!("tryCatch(stop(\"{w}\"), error = function(e) conditionMessage(e))"),
        2 => format!("tryCatch(stop(\"{w}\"), error = function(e) class(e))"),
        3 => format!("tryCatch(warning(\"{w}\"), warning = function(x) conditionMessage(x))"),
        4 => format!("tryCatch(message(\"{w}\"), message = function(x) conditionMessage(x))"),
        5 => format!("tryCatch(stop(\"{w}\"), condition = function(x) \"cond\")"),
        6 => format!("tryCatch({n}, finally = cat(\"fin\\n\"))"),
        7 => format!("tryCatch(stop(\"{w}\"), error = function(e) {n}, finally = cat(\"fin\\n\"))"),
        8 => format!(
            "tryCatch(tryCatch(stop(\"{w}\"), error = function(e) stop(\"outer\")), \
             error = function(e) conditionMessage(e))"
        ),
        9 => format!("(function() {{ on.exit(cat(\"x\\n\")); {n} }})()"),
        10 => format!(
            "(function() {{ on.exit(cat(\"a\\n\")); on.exit(cat(\"b\\n\"), add = TRUE); {n} }})()"
        ),
        11 => format!(
            "tryCatch((function() {{ on.exit(cat(\"cl\\n\")); stop(\"{w}\") }})(), \
             error = function(e) conditionMessage(e))"
        ),
        12 => format!("local({{ v <- {n}; v * 2 }})"),
        13 => format!("class(try(stop(\"{w}\"), silent = TRUE))"),
        14 => format!("inherits(try(stop(\"{w}\"), silent = TRUE), \"try-error\")"),
        _ => format!(
            "{{ f <- function(x) UseMethod(\"f\"); f.a <- function(x) c(\"a\", NextMethod()); \
             f.default <- function(x) \"{w}\"; f(structure({n}, class = \"a\")) }}"
        ),
    })
}

/// `withCallingHandlers` — the *resuming* half of the condition system.
///
/// Every case is written so the answer depends on control flow, not just on the
/// handler running: a `cat` marker after the signalling point only prints if
/// evaluation resumed there, and the value printed at the end is the body's
/// only when nothing unwound. A handler that unwinds instead of resuming — the
/// pre-restart behaviour — changes both.
fn gen_calling(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let w = ww(r);
    let n = r.range(1, 9);
    one(match r.below(18) {
        // Handler muffles: marker prints, body value survives.
        0 => format!(
            "print(withCallingHandlers({{ warning(\"{w}\"); cat(\"resumed\\n\"); {n} }}, \
             warning = function(x) {{ cat(\"H\", conditionMessage(x), \"\\n\"); \
             invokeRestart(\"muffleWarning\") }}))"
        ),
        // Handler returns normally: R still resumes, and the warning falls
        // through to its default action afterwards.
        1 => format!(
            "print(withCallingHandlers({{ warning(\"{w}\"); cat(\"resumed\\n\"); {n} }}, \
             warning = function(x) cat(\"H\\n\")))"
        ),
        2 => format!(
            "print(withCallingHandlers({{ message(\"{w}\"); cat(\"resumed\\n\"); {n} }}, \
             message = function(x) {{ cat(\"H\", conditionMessage(x)); \
             invokeRestart(\"muffleMessage\") }}))"
        ),
        3 => format!(
            "print(withCallingHandlers({{ message(\"{w}\"); cat(\"resumed\\n\"); {n} }}, \
             message = function(x) cat(\"H\\n\")))"
        ),
        // Inner declines, outer muffles: both run, inner first, then resume.
        4 => format!(
            "withCallingHandlers(withCallingHandlers({{ warning(\"{w}\"); cat(\"resumed\\n\") }}, \
             warning = function(x) cat(\"inner\\n\")), \
             warning = function(x) {{ cat(\"outer\\n\"); invokeRestart(\"muffleWarning\") }})"
        ),
        // Inner muffles: the outer handler never sees it at all.
        5 => format!(
            "withCallingHandlers(withCallingHandlers({{ warning(\"{w}\"); cat(\"resumed\\n\") }}, \
             warning = function(x) {{ cat(\"inner\\n\"); invokeRestart(\"muffleWarning\") }}), \
             warning = function(x) cat(\"outer\\n\"))"
        ),
        // Calling handler runs first, then the exiting one unwinds past the marker.
        6 => format!(
            "print(tryCatch(withCallingHandlers({{ warning(\"{w}\"); cat(\"resumed\\n\"); {n} }}, \
             warning = function(x) cat(\"calling\\n\")), \
             warning = function(x) paste(\"exiting\", conditionMessage(x))))"
        ),
        // The innermost handler is the exiting one, so the outer calling handler
        // is never reached.
        7 => format!(
            "print(withCallingHandlers(tryCatch({{ warning(\"{w}\"); {n} }}, \
             warning = function(x) \"exiting\"), warning = function(x) cat(\"calling\\n\")))"
        ),
        // Two handlers on one call: both run, in the order written.
        8 => format!(
            "withCallingHandlers({{ warning(\"{w}\"); cat(\"resumed\\n\") }}, \
             condition = function(x) cat(\"cond\\n\"), \
             warning = function(x) {{ cat(\"warn\\n\"); invokeRestart(\"muffleWarning\") }})"
        ),
        // An error has no restart: the handler runs, then the stack still comes down.
        9 => format!(
            "print(tryCatch(withCallingHandlers({{ stop(\"{w}\"); cat(\"NOT\\n\") }}, \
             error = function(e) cat(\"calling\", conditionMessage(e), \"\\n\")), \
             error = function(e) paste(\"exiting\", conditionMessage(e))))"
        ),
        10 => format!("print(suppressWarnings({{ warning(\"{w}\"); cat(\"resumed\\n\"); {n} }}))"),
        11 => format!("print(suppressMessages({{ message(\"{w}\"); cat(\"resumed\\n\"); {n} }}))"),
        // The signal comes from a nested frame, so resumption has to land back
        // inside that frame rather than at the handler's.
        12 => format!(
            "f <- function() {{ warning(\"{w}\"); cat(\"resumed\\n\"); {n} }}\n\
             print(withCallingHandlers(f(), \
             warning = function(x) invokeRestart(\"muffleWarning\")))"
        ),
        // A handler that signals the same class must not re-enter itself.
        13 => format!(
            "withCallingHandlers({{ warning(\"{w}\"); cat(\"resumed\\n\") }}, \
             warning = function(x) {{ cat(\"H\\n\"); suppressWarnings(warning(\"again\")); \
             invokeRestart(\"muffleWarning\") }})"
        ),
        // A custom condition class, signalled with no default action.
        14 => format!(
            "cnd <- structure(class = c(\"{w}c\", \"condition\"), \
             list(message = \"{w}\", call = NULL))\n\
             print(withCallingHandlers({{ signalCondition(cnd); cat(\"resumed\\n\"); {n} }}, \
             {w}c = function(x) cat(\"saw\", conditionMessage(x), \"\\n\")))"
        ),
        15 => format!(
            "withCallingHandlers({{ warning(\"{w}\"); cat(\"resumed\\n\") }}, \
             warning = function(x) {{ print(class(x)); print(conditionMessage(x)); \
             invokeRestart(\"muffleWarning\") }})"
        ),
        // Suppression nested either way round.
        16 => format!(
            "withCallingHandlers(suppressWarnings({{ warning(\"{w}\"); cat(\"resumed\\n\") }}), \
             warning = function(x) cat(\"NOT\\n\"))"
        ),
        _ => format!(
            "suppressWarnings(withCallingHandlers({{ warning(\"{w}\"); cat(\"resumed\\n\") }}, \
             warning = function(x) cat(\"inner\\n\")))"
        ),
    })
}

/// Restarts: establishing them, transferring to them, and enumerating them.
///
/// A restart is a non-local transfer, so what matters is *where* control lands:
/// the statement after `invokeRestart` must not run, `on.exit` and `finally`
/// along the way must, and `tryCatch(error =)` must not absorb the transfer.
fn gen_restarts(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let w = ww(r);
    let n = r.range(1, 9);
    one(match r.below(18) {
        0 => format!(
            "print(withRestarts(invokeRestart(\"r1\", {n}), r1 = function(v) v * 2))"
        ),
        1 => format!(
            "print(withRestarts({{ cat(\"body\\n\"); invokeRestart(\"r1\"); cat(\"NOT\\n\") }}, \
             r1 = function() \"{w}\"))"
        ),
        // Established but never invoked: the body's own value comes out.
        2 => format!("print(withRestarts({{ cat(\"body\\n\"); {n} }}, r1 = function() 0))"),
        3 => "withRestarts(print(length(computeRestarts())), r1 = function() 1, r2 = function() 2)"
            .to_string(),
        4 => "withRestarts(for (x in computeRestarts()) cat(x$name, \"\\n\"), \
              r1 = function() 1, r2 = function() 2)"
            .to_string(),
        5 => format!(
            "withRestarts(for (x in computeRestarts()) cat(\"[\", restartDescription(x), \"]\\n\"), \
             r1 = list(handler = function() 1, description = \"{w}\"), r2 = \"desc {w}\")"
        ),
        // Two frames share a name: the innermost wins.
        6 => format!(
            "print(withRestarts(withRestarts(invokeRestart(\"r1\", {n}), \
             r1 = function(v) paste(\"inner\", v)), r1 = function(v) paste(\"outer\", v)))"
        ),
        // A restart *object* names one exact frame, so it reaches the outer one.
        7 => format!(
            "print(withRestarts(withRestarts({{ x <- computeRestarts()[[2]]; \
             invokeRestart(x, {n}) }}, r1 = function(v) paste(\"inner\", v)), \
             r1 = function(v) paste(\"outer\", v)))"
        ),
        // The transfer passes through `tryCatch` untouched, but `finally` runs.
        8 => format!(
            "print(withRestarts(tryCatch(invokeRestart(\"r1\", {n}), \
             error = function(e) \"WRONG\", finally = cat(\"fin\\n\")), \
             r1 = function(v) paste(\"restart\", v)))"
        ),
        9 => format!(
            "print(withRestarts((function() {{ on.exit(cat(\"exit\\n\")); \
             invokeRestart(\"r1\", {n}) }})(), r1 = function(v) paste(\"restart\", v)))"
        ),
        // Transfer out of a deep call chain.
        10 => format!(
            "f <- function(k) if (k == 0) invokeRestart(\"r1\", \"{w}\") else f(k - 1)\n\
             print(withRestarts(f({}), r1 = function(v) paste(\"caught\", v)))",
            r.range(1, 12)
        ),
        11 => format!(
            "print(tryCatch(invokeRestart(\"{w}\"), error = function(e) conditionMessage(e)))"
        ),
        12 => "withRestarts(print(computeRestarts()), r1 = function() 1)".to_string(),
        13 => "withRestarts(print(sapply(computeRestarts(), class)), r1 = function() 1)".to_string(),
        // The muffle restarts a signalling builtin establishes around itself.
        14 => format!(
            "withCallingHandlers(warning(\"{w}\"), warning = function(x) {{ \
             for (y in computeRestarts()) cat(y$name, \"\\n\"); \
             invokeRestart(\"muffleWarning\") }})"
        ),
        15 => format!(
            "withCallingHandlers(message(\"{w}\"), message = function(x) {{ \
             print(length(computeRestarts())); invokeRestart(\"muffleMessage\") }})"
        ),
        // The restart handler's own visibility decides whether the call prints.
        16 => format!(
            "withRestarts(invokeRestart(\"r1\", {n}), r1 = function(v) invisible(v))\n\
             withRestarts(invokeRestart(\"r1\", {n}), r1 = function(v) v)"
        ),
        // Restart established outside, condition handled inside: the handler
        // transfers past the signalling frame entirely.
        _ => format!(
            "print(withRestarts(withCallingHandlers({{ warning(\"{w}\"); \"NOT\" }}, \
             warning = function(x) invokeRestart(\"r1\", {n})), \
             r1 = function(v) paste(\"jumped\", v)))"
        ),
    })
}

/// Every other generator draws its strings from `WORDS`, which is pure ASCII —
/// the one alphabet where R's three string units (code points, UTF-8 bytes,
/// terminal columns) all agree. So none of them could ever see a unit confusion.
/// This pool exists to break that: accented Latin (2 bytes, 1 column), CJK and
/// emoji (wide, 2 columns), a combining mark (0 columns), and the characters
/// whose full Unicode case mapping changes length while R's per-character one
/// does not (`ß`, `ﬁ`, `İ`, final sigma).
const UWORDS: &[&str] = &[
    "café",
    "naïve",
    "日本語",
    "한글",
    "Привет",
    "Ωμέγα",
    "straße",
    "ﬁx",
    "İstanbul",
    "ΣΑΣ",
    "e\\u0301",
    "😀ok",
    "a\\u3000b",
    "→←",
];

fn uw<'a>(r: &mut Rng) -> &'a str {
    r.pick(UWORDS)
}

/// A `c("…", "…")` of 2–3 non-ASCII words.
fn vec_uw(r: &mut Rng) -> String {
    let n = r.range(2, 3) as usize;
    let items: Vec<String> = (0..n).map(|_| format!("\"{}\"", uw(r))).collect();
    format!("c({})", items.join(", "))
}

/// Strings measured and laid out in every unit R uses: `nchar(type=)`, the
/// byte-counted `sprintf` field width, and the column-counted `print` / `format`
/// / `formatC` / `strtrim` layouts — plus the per-character case map and the
/// code-point round trip.
fn gen_strwidth(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let s = uw(r);
    let v = vec_uw(r);
    let w = r.range(1, 10);
    one(match r.below(20) {
        0 => format!("nchar(\"{s}\")"),
        1 => format!("nchar(\"{s}\", type = \"bytes\")"),
        2 => format!("nchar(\"{s}\", type = \"width\")"),
        3 => format!(
            "nchar({v}, type = \"{}\")",
            r.pick(&["chars", "bytes", "width"])
        ),
        4 => format!("sprintf(\"[%{w}s]\", \"{s}\")"),
        5 => format!("sprintf(\"[%-{w}s]\", \"{s}\")"),
        6 => format!("print({v})"),
        7 => format!("print(c(a = \"{s}\", bb = \"{}\"))", uw(r)),
        8 => format!("print(matrix({v}, 1))"),
        9 => format!("print(format({v}))"),
        10 => format!("print(format(\"{s}\", width = {w}))"),
        11 => format!("cat(\"[\", formatC(\"{s}\", width = {w}), \"]\\n\")"),
        12 => format!("cat(\"[\", formatC(\"{s}\", width = {w}, flag = \"-\"), \"]\\n\")"),
        13 => format!("cat(strtrim(\"{s}\", {w}), \"\\n\")"),
        14 => format!("print(utf8ToInt(\"{s}\"))"),
        15 => format!("print(intToUtf8(utf8ToInt(\"{s}\")))"),
        16 => format!("print(intToUtf8(utf8ToInt(\"{s}\"), multiple = TRUE))"),
        17 => format!("cat(toupper(\"{s}\"), tolower(\"{s}\"), \"\\n\")"),
        18 => format!("print(nchar(toupper({v}), type = \"bytes\"))"),
        _ => format!("print(substr(\"{s}\", 1, {}))", r.range(1, 4)),
    })
}

/// Character ordering runs through the `LC_COLLATE` locale, not code points, so
/// R sorts `c("B", "a")` as `a B`.
///
/// Nothing else here could see that. `gen_sortops` only ever sorts integers, so
/// no character vector reaches `sort`/`order`/`rank` at all; and `WORDS` — the
/// pool every string generator draws from — is uniformly lowercase, which is
/// exactly the case where collation and code-point order agree. The divergence
/// needs a *case* difference or an accent, so this pool carries both.
const CWORDS: &[&str] = &[
    "Apple", "apple", "banana", "Banana", "Cherry", "cherry", "aB", "Ab", "ab", "AB", "zoo", "Zoo",
    "café", "Café", "élan", "Élan", "straße", "Strasse", "ñu", "Ñu", "ΣΑΣ", "σας", "_x", "0a",
];

fn cw<'a>(r: &mut Rng) -> &'a str {
    r.pick(CWORDS)
}

/// A `c("…", "…")` of 3–5 words that differ in case or accent.
fn vec_cw(r: &mut Rng) -> String {
    let n = r.range(3, 5) as usize;
    let items: Vec<String> = (0..n).map(|_| format!("\"{}\"", cw(r))).collect();
    format!("c({})", items.join(", "))
}

/// Magnitudes that can actually reach scientific notation. The shared `DBLS`
/// pool spans only 0.1 .. 100.25, and at any `digits` in 1..22 every one of
/// those renders fixed — `any(grepl("e", format(DBLS)))` is `FALSE` in R — so
/// no generator drawing from `DBLS` can ever exercise the fixed-versus-
/// scientific choice, let alone `scipen`'s bias on it. These straddle the
/// switch in both directions and at both signs.
const WIDE_DBLS: &[&str] = &[
    "1e5",
    "1e-5",
    "123456789",
    "0.000012345",
    "1/3",
    "pi",
    "1e15",
    "1e-15",
    "-1e5",
    "-0.000012345",
    "99999",
    "1234.5678",
    "1e100",
    "6.022e23",
    "0.1",
    "1e6",
];

/// A `digits` value spanning R's whole legal 1..22 range, weighted to the ends
/// where the notation switch actually moves.
fn dig(r: &mut Rng) -> i64 {
    *r.pick(&[1, 2, 3, 5, 7, 10, 15, 16, 17, 21, 22])
}

/// A `scipen` value. R clamps below at -9 (with a warning) and has no upper
/// bound, so the pool straddles the clamp and both sides of zero.
fn spen(r: &mut Rng) -> i64 {
    *r.pick(&[-9, -5, -2, -1, 0, 1, 2, 5, 10, 30])
}

/// `options(digits=, scipen=)` and the rendering sites they govern. `digits`
/// sets significant digits; `scipen` biases the fixed-versus-scientific choice,
/// which R makes by `width(fixed) <= width(scientific) + scipen`. The two reach
/// every numeric rendering — `print`, top-level auto-print, `cat`, `format`,
/// vector/matrix/list printing — while `as.character`/`paste`/`toString` take
/// `scipen` but NOT `digits` (they are fixed at 15 significant digits), so both
/// families are generated here to keep that asymmetry under test.
fn gen_optsfmt(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let v = r.pick(WIDE_DBLS);
    let w = r.pick(WIDE_DBLS);
    let (d, s) = (dig(r), spen(r));
    one(match r.below(18) {
        0 => format!("options(digits = {d})\nprint({v})"),
        1 => format!("options(scipen = {s})\nprint({v})"),
        2 => format!("options(digits = {d}, scipen = {s})\nprint({v})"),
        // Top-level auto-print takes the same path as an explicit `print`.
        3 => format!("options(digits = {d}, scipen = {s})\n{v}"),
        4 => format!("options(digits = {d}, scipen = {s})\ncat({v}, \"\\n\")"),
        5 => format!("options(digits = {d}, scipen = {s})\ncat(format({v}), \"\\n\")"),
        // A whole vector shares one notation and one decimal count.
        6 => format!("options(digits = {d}, scipen = {s})\nprint(c({v}, {w}))"),
        7 => format!("options(digits = {d}, scipen = {s})\nprint(c(a = {v}, b = {w}))"),
        8 => format!("options(digits = {d}, scipen = {s})\nprint(matrix(c({v}, {w}, 1, 2), 2))"),
        9 => format!("options(digits = {d}, scipen = {s})\nprint(list({v}))"),
        // `digits` must NOT reach these; `scipen` must.
        10 => format!("options(digits = {d}, scipen = {s})\ncat(as.character({v}), \"\\n\")"),
        11 => format!("options(digits = {d}, scipen = {s})\ncat(paste({v}), \"\\n\")"),
        12 => format!("options(digits = {d}, scipen = {s})\ncat(toString({v}), \"\\n\")"),
        // Query, round-trip restore, and the invisible old-value list.
        13 => "print(getOption(\"digits\"))\nprint(getOption(\"scipen\"))".to_string(),
        14 => format!("old <- options(digits = {d})\nprint({v})\noptions(old)\nprint({v})"),
        15 => format!("options(digits = {d})\nprint(names(options(\"digits\")))"),
        16 => format!("print(getOption(\"nosuchoption\", {}))", ii(r)),
        // `print(x, digits=)` is a one-off that must not leak into the setting.
        _ => format!("options(digits = {d})\nprint({v}, digits = {})\nprint({v})", r.range(1, 8)),
    })
}

/// Every surface that orders character data: the sort family, the ordering
/// permutation, the extremes, the comparison operators, and the default
/// `factor` levels (which are `sort(unique(x))`).
fn gen_collate(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let v = vec_cw(r);
    let (a, b) = (cw(r), cw(r));
    one(match r.below(14) {
        0 => format!("print(sort({v}))"),
        1 => format!("print(sort({v}, decreasing = TRUE))"),
        2 => format!("print(order({v}))"),
        3 => format!("print(rank({v}))"),
        4 => format!("print(xtfrm({v}))"),
        5 => format!("print(sort.list({v}))"),
        6 => format!("cat(min({v}), max({v}), \"\\n\")"),
        7 => format!("print(range({v}))"),
        8 => format!("cat(\"{a}\" < \"{b}\", \"{a}\" > \"{b}\", \"{a}\" <= \"{b}\", \"\\n\")"),
        9 => format!("cat(\"{a}\" == \"{b}\", \"{a}\" != \"{b}\", \"\\n\")"),
        10 => format!("print(levels(factor({v})))"),
        11 => format!("print(table({v}))"),
        12 => format!("print(rev(sort({v})))"),
        _ => format!("print(sort(unique({v})))"),
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
        _ => format!(
            "factorial({}) / factorial({})",
            r.range(3, 8),
            r.range(1, 3)
        ),
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
        5 => format!(
            "tabulate(c({}, {}, {}, {}), {})",
            r.range(1, 4),
            r.range(1, 4),
            r.range(1, 4),
            r.range(1, 4),
            r.range(3, 5)
        ),
        6 => format!(
            "findInterval(c({}, {}), c(1, 2, 3, 4))",
            r.range(0, 5),
            r.range(0, 5)
        ),
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
        1 => format!(
            "strtoi(\"{}\", {})",
            r.range(10, 999),
            *r.pick(&["10", "8", "16"])
        ),
        2 => format!("sprintf(\"%d:%s\", 1:3, \"{w}\")"),
        3 => format!("toupper(chartr(\"aeiou\", \"AEIOU\", \"{w}\"))"),
        4 => format!("paste(rev(strsplit(\"{w}\", \"\")[[1]]), collapse = \"\")"),
        5 => format!("strtoi(\"{}\")", r.range(0, 9999)),
        6 => format!(
            "nchar(chartr(\"{}\", \"X\", \"{w}\"))",
            &w[..1.min(w.len())]
        ),
        7 => format!("sprintf(\"[%s]\", c(\"{w}\", \"{}\"))", ww(r)),
        _ => format!("chartr(\"{w}\", \"{}\", \"{w}{w}\")", ww(r)),
    })
}

fn gen_listx(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let n = r.range(3, 6);
    one(match r.below(8) {
        0 => format!(
            "Position(function(x) x > {}, c({}, {}, {}))",
            r.range(1, 3),
            si(r),
            si(r),
            si(r)
        ),
        1 => format!("Find(function(x) x %% 2 == 0, 1:{n})"),
        2 => format!("Filter(function(x) x > 0, {})", vec_int(r)),
        3 => format!("Reduce(function(a, b) a + b, 1:{n}, accumulate = TRUE)"),
        4 => format!("mapply(function(a, b) a * b, 1:{n}, {n}:1)"),
        5 => format!(
            "lengths(list(1:{}, 1:{}, 1:{}))",
            r.range(1, 4),
            r.range(1, 4),
            r.range(1, 4)
        ),
        6 => "do.call(pmax, list(c(1, 5), c(3, 2)))".to_string(),
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
        1 => format!(
            "trimws(\"  {w}  \", which = \"{}\")",
            *r.pick(&["left", "right", "both"])
        ),
        2 => format!("substring(\"{w}\", 1:{})", r.range(2, 4)),
        3 => format!("encodeString(\"{w}\\t{w}\")"),
        4 => format!(
            "x <- \"{w}\"; substr(x, {}, {}) <- \"XY\"; x",
            r.range(1, 3),
            r.range(3, 5)
        ),
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
        1 => format!(
            "tapply(c({}, {}, {}, {}, {}), {keys}, sum)",
            si(r),
            si(r),
            si(r),
            si(r),
            si(r)
        ),
        2 => format!(
            "modifyList(list(a = {}, b = {}), list(b = {}))",
            si(r),
            si(r),
            si(r)
        ),
        3 => format!("Reduce(`-`, 1:{n}, right = TRUE)"),
        4 => format!("Reduce(`+`, 1:{n}, accumulate = TRUE, right = TRUE)"),
        5 => format!(
            "rapply(list({}, {}), function(x) x * 2, how = \"unlist\")",
            si(r),
            si(r)
        ),
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
        2 => format!(
            "m <- matrix(1:6, nrow = 2); m[{}, {}]",
            r.range(1, 2),
            r.range(1, 3)
        ),
        3 => format!("({v})[-{i}]"),
        4 => format!("({v})[c(TRUE, FALSE)]"),
        5 => format!(
            "x <- c(a = 1, b = 2, c = 3); x[\"{}\"]",
            *r.pick(&["a", "b", "c"])
        ),
        6 => format!("({v})[{i}:{}]", r.range(1, 4)),
        7 => format!(
            "l <- list({}, {}, {}); l[[{}]]",
            si(r),
            si(r),
            si(r),
            r.range(1, 3)
        ),
        8 => format!("({v})[({v}) > 0]"),
        _ => format!("({v})[c(-1, -2)]"),
    })
}

fn gen_replace(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    one(match r.below(9) {
        0 => format!("x <- 1:5; x[{}] <- {}; x", r.range(1, 5), si(r)),
        1 => format!("x <- 1:5; x[x > {}] <- 0; x", r.range(1, 3)),
        2 => "x <- 1:3; names(x) <- c(\"a\", \"b\", \"c\"); x".to_string(),
        3 => "x <- 1:6; dim(x) <- c(2, 3); x".to_string(),
        4 => format!("x <- c(1, 2); length(x) <- {}; x", r.range(3, 5)),
        5 => format!(
            "m <- matrix(1:4, 2); m[{}, {}] <- 9; m",
            r.range(1, 2),
            r.range(1, 2)
        ),
        6 => format!("l <- list(a = 1, b = 2); l$c <- {}; l", si(r)),
        7 => format!("x <- 1:5; x[[{}]] <- {}; x", r.range(1, 5), si(r)),
        _ => "x <- 1:5; x[-1] <- 0; x".to_string(),
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
        4 => format!("f <- function(t) switch(t, a = \"A\", b = \"B\", \"?\"); f(\"{key}\")"),
        5 => format!("x <- switch(\"{key}\", a = 10); is.null(x)"),
        6 => "sapply(c(\"a\", \"b\"), function(x) switch(x, a = 1, b = 2))".to_string(),
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
        4 => format!(
            "grep(\"{p}\", c(\"{s}\", \"{}\"), ignore.case = TRUE)",
            ww(r)
        ),
        _ => format!("gsub(\"[aeiou]\", \"_\", \"{s}\", ignore.case = TRUE)"),
    })
}

fn gen_factorx(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let s = vec_str(r);
    one(match r.below(8) {
        0 => format!("as.integer(cut(1:{}, c(0, 2, 4, 6, 8)))", r.range(3, 8)),
        1 => format!("nlevels(cut(1:{}, c(0, 5, 10)))", r.range(2, 10)),
        2 => format!(
            "cut(c({}, {}, {}), c(0, 3, 6, 9))",
            r.range(1, 8),
            r.range(1, 8),
            r.range(1, 8)
        ),
        3 => "levels(cut(1:9, c(0, 3, 6, 9)))".to_string(),
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
        4 => "deparse(c(TRUE, FALSE, NA))".to_string(),
        5 => format!("deparse({})", si(r)),
        6 => format!(
            "diff(c({}, {}, {}, {}), differences = 2)",
            si(r),
            si(r),
            si(r),
            si(r)
        ),
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
        4 => format!(
            "sprintf(\"%o %x %X\", {}, {}, {})",
            r.range(0, 500),
            r.range(0, 500),
            r.range(0, 500)
        ),
        5 => format!(
            "format(c({}, {}), nsmall = {})",
            ff(r),
            ff(r),
            r.range(1, 4)
        ),
        6 => format!(
            "format(c({}, {}, {}), big.mark = \",\")",
            r.range(1000, 99999),
            r.range(1000, 99999),
            r.range(1000, 99999)
        ),
        _ => format!(
            "format(seq({}, {}, {}))",
            r.range(0, 3),
            r.range(8, 20),
            ff(r)
        ),
    })
}

fn gen_seqx2(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    one(match r.below(9) {
        0 => format!("rep_len(1:{}, {})", r.range(2, 4), r.range(1, 9)),
        1 => format!(
            "seq.int({}, {}, {})",
            r.range(0, 3),
            r.range(8, 16),
            r.range(2, 4)
        ),
        2 => format!("rev(c(a = {}, b = {}, c = {}))", si(r), si(r), si(r)),
        3 => format!("unname(c(x = {}, y = {}))", si(r), si(r)),
        4 => format!(
            "isTRUE(all.equal({}, {} + 1e-10))",
            r.range(1, 9),
            r.range(1, 9)
        ),
        5 => format!("isTRUE(all.equal({}, {}))", si(r), si(r)),
        6 => format!(
            "all.equal(c({}, {}), c({}, {}))",
            ff(r),
            ff(r),
            ff(r),
            ff(r)
        ),
        7 => format!(
            "rep_len(c(\"{}\", \"{}\"), {})",
            ww(r),
            ww(r),
            r.range(1, 7)
        ),
        _ => "rev(setNames(1:3, c(\"a\", \"b\", \"c\")))".to_string(),
    })
}

fn gen_combinator(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let v = vec_int(r);
    one(match r.below(8) {
        0 => format!("Negate(is.na)(c({}, NA, {}))", si(r), si(r)),
        1 => format!(
            "Filter(Negate(is.na), c({}, NA, {}, NA, {}))",
            si(r),
            si(r),
            si(r)
        ),
        2 => format!("Negate(function(x) x > 0)({v})"),
        3 => format!("Vectorize(function(x) x ^ 2)(1:{})", r.range(2, 6)),
        4 => format!(
            "Vectorize(function(x, y) x + y)(1:{n}, {n}:1)",
            n = r.range(2, 5)
        ),
        5 => format!("sapply({v}, Negate(function(x) x > 0))"),
        6 => "is.function(Negate(is.null))".to_string(),
        _ => format!(
            "Filter(Negate(function(x) x %% 2 == 0), 1:{})",
            r.range(3, 9)
        ),
    })
}

fn gen_arrays(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    // Dimensions that multiply to <= 24 so the data 1:N fills exactly.
    let (d0, d1, d2) = (r.range(2, 3), r.range(2, 3), r.range(2, 4));
    let n = d0 * d1 * d2;
    one(match r.below(9) {
        0 => format!(
            "array(1:{n}, c({d0}, {d1}, {d2}))[{}, {}, {}]",
            r.range(1, d0),
            r.range(1, d1),
            r.range(1, d2)
        ),
        1 => format!("dim(array(1:{n}, c({d0}, {d1}, {d2})))"),
        2 => format!("apply(array(1:{n}, c({d0}, {d1}, {d2})), 3, sum)"),
        3 => format!("apply(array(1:{n}, c({d0}, {d1}, {d2})), 1, max)"),
        4 => format!(
            "a <- array(1:{n}, c({d0}, {d1}, {d2})); a[, , {}]",
            r.range(1, d2)
        ),
        5 => format!(
            "a <- array(1:{n}, c({d0}, {d1}, {d2})); a[{}, , ]",
            r.range(1, d0)
        ),
        6 => format!("array(1:{n}, c({d0}, {d1}, {d2}))"),
        7 => format!("aperm(matrix(1:{}, {d0}))", d0 * d1),
        _ => format!("length(array(0, c({d0}, {d1}, {d2})))"),
    })
}

fn gen_stat2(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let v = vec_int(r);
    one(match r.below(9) {
        0 => format!(
            "quantile(1:{}, {})",
            r.range(4, 20),
            *r.pick(&["0.25", "0.5", "0.75", "0.1"])
        ),
        1 => format!("quantile(1:{})", r.range(4, 20)),
        2 => format!("cor(1:{n}, (1:{n}) * {})", r.range(2, 5), n = r.range(3, 8)),
        3 => format!(
            "rle(c({}, {}, {}, {}, {}))$lengths",
            r.range(1, 3),
            r.range(1, 3),
            r.range(1, 3),
            r.range(1, 3),
            r.range(1, 3)
        ),
        4 => format!(
            "rle(c({}, {}, {}, {}, {}))",
            r.range(1, 2),
            r.range(1, 2),
            r.range(1, 2),
            r.range(1, 2),
            r.range(1, 2)
        ),
        5 => format!(
            "inverse.rle(rle(c({}, {}, {}, {})))",
            r.range(1, 3),
            r.range(1, 3),
            r.range(1, 3),
            r.range(1, 3)
        ),
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
    Deparsefn,
    Bindlabels,
    Dimnames,
    Ordering,
    Missing,
    S3methods,
    Catargs,
    Pastex,
    Formatsci,
    Parens,
    Seqfmt,
    Typepred,
    Factorsub,
    Factorops,
    Conditions,
    Calling,
    Restarts,
    Strwidth,
    Collate,
    Optsfmt,
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
    Mode::Deparsefn,
    Mode::Bindlabels,
    Mode::Dimnames,
    Mode::Ordering,
    Mode::Missing,
    Mode::S3methods,
    Mode::Catargs,
    Mode::Pastex,
    Mode::Formatsci,
    Mode::Parens,
    Mode::Seqfmt,
    Mode::Typepred,
    Mode::Factorsub,
    Mode::Factorops,
    Mode::Conditions,
    Mode::Calling,
    Mode::Restarts,
    Mode::Strwidth,
    Mode::Collate,
    Mode::Optsfmt,
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
        Mode::Deparsefn => gen_deparsefn(seed),
        Mode::Bindlabels => gen_bindlabels(seed),
        Mode::Dimnames => gen_dimnames(seed),
        Mode::Ordering => gen_ordering(seed),
        Mode::Missing => gen_missing(seed),
        Mode::S3methods => gen_s3methods(seed),
        Mode::Catargs => gen_catargs(seed),
        Mode::Pastex => gen_pastex(seed),
        Mode::Formatsci => gen_formatsci(seed),
        Mode::Parens => gen_parens(seed),
        Mode::Seqfmt => gen_seqfmt(seed),
        Mode::Typepred => gen_typepred(seed),
        Mode::Factorsub => gen_factorsub(seed),
        Mode::Factorops => gen_factorops(seed),
        Mode::Conditions => gen_conditions(seed),
        Mode::Calling => gen_calling(seed),
        Mode::Restarts => gen_restarts(seed),
        Mode::Strwidth => gen_strwidth(seed),
        Mode::Collate => gen_collate(seed),
        Mode::Optsfmt => gen_optsfmt(seed),
    }
}

/// Closure printing and deparse. Every function here is defined at top level so
/// its environment is the global one — R appends `<environment: 0x…>` to a
/// closure printed from anywhere else, and that address is not reproducible.
fn gen_deparsefn(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let body = match r.below(10) {
        0 => format!("x {} {}", r.pick(&["+", "-", "*", "/", "^"]), ii(r)),
        1 => format!("if (x > {}) x else {}", si(r), si(r)),
        2 => format!(
            "{{\n  y <- x * {}\n  if (y > {}) y else {}\n  y\n}}",
            ii(r),
            si(r),
            si(r)
        ),
        3 => format!("{{\n  for (i in 1:{}) x <- x + i\n  x\n}}", r.range(1, 5)),
        4 => format!("{{\n  while (x > {}) x <- x - 1\n  x\n}}", ii(r)),
        5 => format!("(x + {}) * {}", si(r), ii(r)),
        6 => format!("c(a = {}, b = \"{}\")", ff(r), ww(r)),
        7 => "function(y) x + y".to_string(),
        8 => format!(
            "{{\n  if (x) {{\n    {}\n  }} else {{\n    {}\n  }}\n}}",
            ii(r),
            ii(r)
        ),
        _ => format!("x[[{}]]$k", r.range(1, 3)),
    };
    let params = match r.below(4) {
        0 => "x".to_string(),
        1 => format!("x, y = {}", ii(r)),
        2 => format!("x, y = c({}, {}), z = \"{}\"", ff(r), ff(r), ww(r)),
        _ => "x, ...".to_string(),
    };
    let show = match r.below(3) {
        0 => "print(f)",
        1 => "print(deparse(f))",
        _ => "print(format(f))",
    };
    vec![format!("f <- function({params}) {body}"), show.to_string()]
}

/// `rbind`/`cbind` seam labels: the deparsed argument, an explicit tag, a
/// matrix's own dimnames, and `deparse.level`.
fn gen_bindlabels(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let f = r.pick(&["rbind", "cbind"]);
    let setup = vec![
        format!("x <- c({}, {}, {})", si(r), si(r), si(r)),
        format!("y <- c({}, {}, {})", si(r), si(r), si(r)),
    ];
    let call = match r.below(9) {
        0 => format!("{f}(x, y)"),
        1 => format!("{f}(x, x)"),
        2 => format!("{f}(a = x, y)"),
        3 => format!("{f}(x, c({}, {}, {}))", si(r), si(r), si(r)),
        4 => format!("{f}(x, y, deparse.level = 0)"),
        5 => format!("{f}(x, y, deparse.level = 2)"),
        6 => format!("{f}(x + 0, y)"),
        7 => format!("{f}({f}(x, y), x)"),
        _ => format!("{f}(x)"),
    };
    let mut out = setup;
    out.push(format!("print({call})"));
    out
}

/// `dimnames`/`rownames`/`colnames` as replacement targets, and the labelled
/// `apply` results that ride on them.
fn gen_dimnames(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let mut out = vec![format!(
        "m <- matrix(1:6, nrow = 2{})",
        if r.below(2) == 0 {
            ", byrow = TRUE"
        } else {
            ""
        }
    )];
    match r.below(8) {
        0 => {
            out.push("dimnames(m) <- list(c(\"r1\", \"r2\"), c(\"c1\", \"c2\", \"c3\"))".into());
            out.push("print(m)".into());
        }
        1 => {
            out.push("rownames(m) <- c(\"p\", \"q\")".into());
            out.push("print(m)".into());
            out.push("print(dimnames(m))".into());
        }
        2 => {
            out.push("colnames(m) <- c(\"i\", \"j\", \"k\")".into());
            out.push("print(m)".into());
            out.push("print(colnames(m))".into());
        }
        3 => {
            out.push("dimnames(m) <- list(c(\"r1\", \"r2\"), c(\"c1\", \"c2\", \"c3\"))".into());
            out.push(format!("print(apply(m, {}, sum))", r.range(1, 2)));
        }
        4 => {
            out.push("dimnames(m) <- list(c(\"r1\", \"r2\"), c(\"c1\", \"c2\", \"c3\"))".into());
            out.push(format!("print(apply(m, {}, range))", r.range(1, 2)));
        }
        5 => {
            out.push("rownames(m) <- c(\"p\", \"q\")".into());
            out.push("rownames(m) <- NULL".into());
            out.push("print(m)".into());
        }
        6 => {
            out.push("dimnames(m) <- list(c(\"r1\", \"r2\"), c(\"c1\", \"c2\", \"c3\"))".into());
            out.push("print(m[\"r1\", ])".into());
            out.push("print(m[, \"c2\"])".into());
        }
        _ => {
            out.push("dimnames(m) <- list(NULL, c(\"c1\", \"c2\", \"c3\"))".into());
            out.push("print(m)".into());
        }
    }
    out
}

/// `sort`/`order`/`rank` with missing values, ties, `na.last` and extra keys.
fn gen_ordering(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let v = format!("c({}, NA, {}, {})", si(r), si(r), si(r));
    let s = format!("c(\"{}\", NA, \"{}\")", ww(r), ww(r));
    one(match r.below(10) {
        0 => format!("print(sort({v}))"),
        1 => format!("print(sort({v}, na.last = TRUE))"),
        2 => format!("print(sort({v}, na.last = FALSE))"),
        3 => format!("print(order({v}))"),
        4 => format!("print(order({v}, na.last = FALSE))"),
        5 => format!("print(order({v}, decreasing = TRUE))"),
        6 => format!("print(sort({s}, na.last = TRUE))"),
        7 => format!(
            "print(order(c({a}, {a}, {b}), c({c}, {d}, {d})))",
            a = ii(r),
            b = ii(r),
            c = ii(r),
            d = ii(r)
        ),
        8 => format!(
            "print(sort(c({a}, {a}, {b}), decreasing = TRUE))",
            a = ii(r),
            b = ii(r)
        ),
        _ => format!(
            "print(order(c({a}, {a}, {b}), decreasing = TRUE))",
            a = ii(r),
            b = ii(r)
        ),
    })
}

/// `NA` versus `NaN` through the summaries, which do not agree on which one
/// survives (`mean` keeps the first it meets, `median` always answers NA).
fn gen_missing(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let f = r.pick(&[
        "mean", "median", "sum", "prod", "max", "min", "var", "sd", "range",
    ]);
    let v = match r.below(6) {
        0 => format!("c({}, NA)", ff(r)),
        1 => format!("c({}, NaN)", ff(r)),
        2 => "c(NA, NaN)".to_string(),
        3 => "c(NaN, NA)".to_string(),
        4 => format!("c({}, NA, NaN, {})", ff(r), ff(r)),
        _ => format!("c({}, {}, NA)", ff(r), ff(r)),
    };
    one(if r.below(2) == 0 {
        format!("print({f}({v}))")
    } else {
        format!("print({f}({v}, na.rm = TRUE))")
    })
}

/// S3 methods for the primitives R dispatches on, and the `attr(,"class")`
/// block `print.default` shows for a class with no method.
fn gen_s3methods(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let body = match r.below(3) {
        0 => format!("c({}, {})", ii(r), ii(r)),
        1 => format!("list({}, \"{}\")", ii(r), ww(r)),
        _ => format!("\"{}\"", ww(r)),
    };
    let mut out = vec![format!("obj <- structure({body}, class = \"kk\")")];
    match r.below(6) {
        0 => {
            out.push("print.kk <- function(x, ...) cat(\"<kk>\\n\")".into());
            out.push("print(obj)".into());
            out.push("obj".into());
        }
        1 => {
            out.push(format!("format.kk <- function(x, ...) \"{}\"", ww(r)));
            out.push("print(format(obj))".into());
        }
        2 => {
            out.push(format!("as.character.kk <- function(x, ...) \"{}\"", ww(r)));
            out.push("print(as.character(obj))".into());
        }
        3 => {
            out.push(format!("length.kk <- function(x) {}L", r.range(1, 9)));
            out.push("print(length(obj))".into());
        }
        4 => out.push("print(obj)".into()),
        _ => out.push("obj".into()),
    }
    out
}

/// `cat` argument handling: the separator sits between *arguments*, so a
/// zero-length one still earns its successor a separator, and a list is a
/// hard error.
fn gen_catargs(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    one(match r.below(10) {
        0 => format!("cat(NULL, \"{}\")", ww(r)),
        1 => format!("cat(character(0), \"{}\", \"\\n\")", ww(r)),
        2 => format!("cat(\"{}\", NULL, \"{}\", \"\\n\")", ww(r), ww(r)),
        3 => format!("cat(list({}), \"\\n\")", ii(r)),
        4 => "cat(list())".to_string(),
        5 => format!("cat({}, {}, sep = \"\")", ii(r), ii(r)),
        6 => format!("cat(c(\"{}\", \"{}\"), sep = \"\\n\")", ww(r), ww(r)),
        7 => format!("cat(\"{}\", list({}))", ww(r), ii(r)),
        8 => format!("cat({}:{}, \"\\n\")", ii(r), r.range(3, 8)),
        _ => format!("cat(NULL, NULL, \"{}\")", ww(r)),
    })
}

/// `paste`/`paste0` recycling, `collapse`, and the empty field a zero-length
/// argument contributes.
fn gen_pastex(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    one(match r.below(9) {
        0 => format!("print(paste(\"{}\", NULL, \"{}\"))", ww(r), ww(r)),
        1 => format!("print(paste(\"{}\", character(0), \"{}\"))", ww(r), ww(r)),
        2 => format!("print(paste0(\"{}\", NULL))", ww(r)),
        3 => "print(paste(NULL))".to_string(),
        4 => format!("print(paste(NULL, collapse = \"{}\"))", ww(r)),
        5 => format!("print(paste(1:{}, 1:{}))", r.range(2, 3), r.range(4, 6)),
        6 => format!(
            "print(paste(\"{}\", 1:{}, sep = \"-\"))",
            ww(r),
            r.range(1, 4)
        ),
        7 => format!("print(paste(c(\"{}\", NA), \"{}\"))", ww(r), ww(r)),
        _ => format!(
            "print(paste(c(\"{}\", \"{}\"), collapse = \"{}\"))",
            ww(r),
            ww(r),
            r.pick(&["+", "", ", "])
        ),
    })
}

/// `format`'s fixed-versus-scientific choice, which `digits`, `nsmall`,
/// `big.mark` and `scientific` all interact with.
fn gen_formatsci(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let big = [
        "1e6", "1e5", "123456", "1234567", "1e-4", "1e-10", "0.0001", "100000",
    ];
    let v = r.pick(&big);
    one(match r.below(9) {
        0 => format!("print(format({v}))"),
        1 => format!("print(format({v}, big.mark = \",\"))"),
        2 => format!("print(format({v}, nsmall = {}))", r.range(1, 4)),
        3 => format!("print(format({v}, digits = {}))", r.range(1, 5)),
        4 => format!("print(format({v}, scientific = FALSE))"),
        5 => format!("print(format({v}, scientific = TRUE))"),
        6 => format!("print(format(c({v}, {})))", ii(r)),
        7 => format!("print(format({v}, width = {}))", r.range(1, 12)),
        _ => format!("print(format(c({v}, NA, Inf)))"),
    })
}

/// `(` as a value-returning function: it makes an otherwise invisible result
/// echo at top level.
fn gen_parens(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    one(match r.below(7) {
        0 => format!("(x <- {})", ii(r)),
        1 => format!("x <- {}\n(x)", ii(r)),
        2 => format!("(invisible({}))", ii(r)),
        3 => format!("print(({} + {}) * {})", ii(r), ii(r), ii(r)),
        4 => format!("f <- function() invisible({})\n(f())", ii(r)),
        5 => format!("(({}))", ii(r)),
        _ => format!("x <- c({}, {})\n(x[1])", ii(r), ii(r)),
    })
}

/// `seq` argument forms and `sprintf`'s `*` width, both of which take a value
/// from an argument rather than the literal spec.
fn gen_seqfmt(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    one(match r.below(9) {
        0 => format!(
            "print(seq(along.with = c({}, {}, {})))",
            ii(r),
            ii(r),
            ii(r)
        ),
        1 => format!(
            "print(seq({}, {}, length.out = {}))",
            ii(r),
            r.range(5, 20),
            r.range(2, 5)
        ),
        2 => format!(
            "print(seq({}, {}, by = {}))",
            ii(r),
            r.range(5, 20),
            r.range(2, 4)
        ),
        3 => format!("print(seq_len({}))", r.range(0, 5)),
        4 => format!(
            "print(sprintf(\"%*d\", {}, {}))",
            r.range(1, 8),
            r.range(1, 999)
        ),
        5 => format!(
            "print(sprintf(\"%-*d|\", {}, {}))",
            r.range(1, 8),
            r.range(1, 999)
        ),
        6 => format!("print(sprintf(\"%.*f\", {}, {}))", r.range(0, 5), ff(r)),
        7 => format!("print(sprintf(\"%*s|\", {}, \"{}\"))", r.range(1, 9), ww(r)),
        _ => format!("print(seq({}))", r.range(0, 6)),
    })
}

/// The type predicates, which disagree about factors and about `1` versus `1L`.
fn gen_typepred(seed: u64) -> Vec<String> {
    let r = &mut Rng::seed(seed);
    let f = r.pick(&[
        "is.numeric",
        "is.double",
        "is.integer",
        "is.character",
        "is.logical",
        "is.vector",
        "is.list",
        "is.null",
    ]);
    let v = match r.below(8) {
        0 => format!("{}L", ii(r)),
        1 => ff(r).to_string(),
        2 => format!("\"{}\"", ww(r)),
        3 => format!("factor(c(\"{}\", \"{}\"))", ww(r), ww(r)),
        4 => "TRUE".to_string(),
        5 => "NULL".to_string(),
        6 => format!("list({})", ii(r)),
        _ => format!("1:{}", r.range(2, 5)),
    };
    one(match r.below(3) {
        0 => format!("print({f}({v}))"),
        1 => format!("print(class({v}))"),
        _ => format!("print(c(class({v}), typeof({v})))"),
    })
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
        Mode::Deparsefn => "deparsefn",
        Mode::Bindlabels => "bindlabels",
        Mode::Dimnames => "dimnames",
        Mode::Ordering => "ordering",
        Mode::Missing => "missing",
        Mode::S3methods => "s3methods",
        Mode::Catargs => "catargs",
        Mode::Pastex => "pastex",
        Mode::Formatsci => "formatsci",
        Mode::Parens => "parens",
        Mode::Seqfmt => "seqfmt",
        Mode::Typepred => "typepred",
        Mode::Factorsub => "factorsub",
        Mode::Factorops => "factorops",
        Mode::Conditions => "conditions",
        Mode::Calling => "calling",
        Mode::Restarts => "restarts",
        Mode::Strwidth => "strwidth",
        Mode::Collate => "collate",
        Mode::Optsfmt => "optsfmt",
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
        if o.timed_out || o.exit == -999 || o.exit == -998 {
            eprintln!("parity-fuzz: the oracle did not answer this case — nothing to compare");
            std::process::exit(2);
        }
        let diverged = differs(&o, &r);
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
    // Cases the oracle could not answer (it timed out, or would not spawn) and
    // cases where neither side said anything: both drain out of the divergence
    // count without ever having been compared.
    let drained = AtomicU64::new(0);
    let silent = AtomicU64::new(0);
    let compared = AtomicU64::new(0);
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
                if o.timed_out || o.exit == -999 || o.exit == -998 {
                    drained.fetch_add(1, Ordering::Relaxed);
                } else if no_signal(&o, &r) {
                    silent.fetch_add(1, Ordering::Relaxed);
                } else {
                    compared.fetch_add(1, Ordering::Relaxed);
                }
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
    let drained = drained.load(Ordering::Relaxed);
    let silent = silent.load(Ordering::Relaxed);
    let compared = compared.load(Ordering::Relaxed);
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
         compared    : {compared} (drained {drained} / no-signal {silent})\n\
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

    // "0 divergences" is only good news when something was compared. A run that
    // generated no cases (`--count 0`), whose oracle never answered (a timeout
    // short enough to kill every case, an unusable `Rscript`), or whose cases
    // were all silent on both sides, has proved nothing — fail it rather than
    // report a clean sheet. This is the difference between "no gaps found" and
    // "no look taken". It runs after the report is written, so whatever evidence
    // the run did collect survives on disk.
    if compared == 0 {
        eprintln!(
            "parity-fuzz: nothing was compared — {checked} case(s) generated, \
             {drained} drained, {silent} with no output on either side. \
             A run that compares nothing cannot pass."
        );
        std::process::exit(2);
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
