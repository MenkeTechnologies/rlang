//! CI-safe replay of the parity corpus. Each snippet in
//! `tests/data/parity_corpus.R` is run through the built `Rscript` binary and its
//! stdout is asserted against `tests/data/parity_expected.txt` — the outputs the
//! reference `Rscript` produced, frozen by `cargo run --bin parity -- --freeze`.
//!
//! This needs no R installed (unlike the `parity` dev tool), so CI runs it.
//! A regression that diverges rlang from the reference fails here with the
//! snippet and the expected-vs-got outputs.

use std::process::Command;

const SEP: &str = "#==#\n";

fn snippets(text: &str) -> Vec<String> {
    text.split(SEP)
        .map(|s| s.trim_end_matches('\n').to_string())
        .filter(|s| !s.trim().is_empty())
        .collect()
}

/// Split the frozen expected file, preserving the (possibly empty) output blocks
/// positionally so they line up with the corpus snippets.
fn expected_blocks(text: &str) -> Vec<String> {
    text.split(SEP).map(|s| s.to_string()).collect()
}

#[test]
fn corpus_matches_reference_r() {
    let corpus = include_str!("data/parity_corpus.R");
    let expected = include_str!("data/parity_expected.txt");
    let rscript = env!("CARGO_BIN_EXE_Rscript");

    let snips = snippets(corpus);
    let wants = expected_blocks(expected);
    assert_eq!(
        snips.len(),
        wants.len(),
        "corpus ({}) and expected ({}) snippet counts differ — re-run `parity --freeze`",
        snips.len(),
        wants.len()
    );

    let mut failures = Vec::new();
    for (i, (snippet, want)) in snips.iter().zip(&wants).enumerate() {
        let out = Command::new(rscript)
            .arg("-e")
            .arg(snippet)
            .output()
            .expect("run Rscript binary");
        let got = String::from_utf8_lossy(&out.stdout).to_string();
        let want = want.strip_suffix('\n').unwrap_or(want);
        // A frozen `<error>` means the reference `Rscript` rejected the snippet;
        // assert rlang also rejects it (non-zero exit) rather than matching
        // stdout, which is empty on both sides.
        if want == "<error>" {
            if out.status.success() {
                failures.push(format!(
                    "── snippet #{i} ──\n{}\n  expected: reference rejected it, but rlang accepted it",
                    snippet.lines().next().unwrap_or("")
                ));
            }
            continue;
        }
        let got_cmp = got.strip_suffix('\n').unwrap_or(&got);
        if got_cmp != want {
            failures.push(format!(
                "── snippet #{i} ──\n{}\n  expected: {:?}\n  got:      {:?}",
                snippet.lines().next().unwrap_or(""),
                want,
                got_cmp
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "{} parity regression(s):\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

/// Counts written into prose go stale silently — the corpus was documented as
/// "121 snippets" in five places while the file held a different number, and
/// `docs/report.html` still claimed 48. Every count the docs quote is derived
/// here from the tree that defines it, so a wave that adds a snippet, a fuzz
/// surface, or a primitive fails until the prose is updated with it.
///
/// The separator between the number and the noun is `\s+`, not a literal space,
/// because prose wraps: `BUGS.md` carried "across 61\nsurfaces" across a line
/// break and a single-space pattern skipped it, so that count sat two behind the
/// tree while this test reported clean. A count is a count wherever the
/// paragraph happens to fold.
#[test]
fn documented_counts_match_the_tree() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    // roff escapes a hyphen as `\-`; unescape so one pattern matches both the
    // man pages and the HTML.
    let read = |rel: &str| {
        std::fs::read_to_string(root.join(rel))
            .expect(rel)
            .replace("\\-", "-")
    };

    let corpus = snippets(&read("tests/data/parity_corpus.R")).len();
    let primitives = rlang::builtins::PRIMITIVES.len();
    // The fuzzer's surface count is the length of its `ALL_MODES` table.
    let fuzz = read("src/bin/parity_fuzz.rs");
    let modes = fuzz
        .split_once("const ALL_MODES: &[Mode] = &[")
        .and_then(|(_, rest)| rest.split_once("];"))
        .map(|(body, _)| body.matches("Mode::").count())
        .expect("ALL_MODES table");

    // (regex over the doc text, what the number must be, what it counts)
    let claims: [(&str, usize, &str); 5] = [
        (r"(\d+)-\s*snippet", corpus, "parity corpus snippets"),
        (r"(\d+)\s+snippets \+", corpus, "parity corpus snippets"),
        (r"(\d+)\s+surfaces", modes, "parity-fuzz surfaces"),
        (r"(\d+)\s+primitives", primitives, "primitives"),
        (r"primitive library \((\d+)\)", primitives, "primitives"),
    ];
    let docs = [
        "README.md",
        "BUGS.md",
        "docs/index.html",
        "docs/report.html",
        "man/Rscript.1",
        "man/Rscriptall.1",
    ];

    let mut wrong = Vec::new();
    for file in docs {
        let text = read(file);
        for (pat, want, what) in &claims {
            let re = regex::Regex::new(pat).unwrap();
            for cap in re.captures_iter(&text) {
                let got: usize = cap[1].parse().unwrap();
                if got != *want {
                    wrong.push(format!(
                        "{file}: {:?} claims {got} {what}, the tree has {want}",
                        &cap[0]
                    ));
                }
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "{} stale count(s) in the docs:\n  {}",
        wrong.len(),
        wrong.join("\n  ")
    );
}
