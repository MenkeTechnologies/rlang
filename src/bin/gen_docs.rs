//! Offline generator for `docs/reference.html` — the primitive reference page.
//!
//! Run before publishing GitHub Pages. Every entry comes from `rlang::docs`,
//! which is checked against `builtins::PRIMITIVES` and `builtins::OPERATORS` by
//! its own tests, so the page can never claim a function the runtime does not
//! implement nor omit one it does. Every count on the page is computed here,
//! never typed.
//!
//! Shape: one `<section>` per chapter, one `<article class="doc-entry">` per
//! callable, each carrying an anchored heading, its signature in a fenced code
//! block, and a prose description. `scripts/reference_pdf.sh` feeds the same
//! markup to pandoc, where `<h2>` becomes a chapter and `<h3>` a section.

use std::fmt::Write as _;

use rlang::docs::{self, Entry};

fn main() {
    // The reference chapters: the primitive groups, then the operators, which
    // are callable functions in R and so belong in the same manual.
    let mut chapters: Vec<(&str, &[Entry])> = docs::CHAPTERS.to_vec();
    chapters.push(("Operators", docs::OPERATORS));

    let primitives = docs::CHAPTERS
        .iter()
        .map(|(_, rows)| rows.len())
        .sum::<usize>();
    let operators = docs::OPERATORS.len();
    let count = primitives + operators;
    let chapter_count = chapters.len();
    let version = env!("CARGO_PKG_VERSION");

    let mut body = String::with_capacity(400_000);

    // Chapter index. This section deliberately carries no `id`: the print
    // pipeline drops it, because the LaTeX table of contents says the same
    // thing. Every real chapter below has one.
    body.push_str("    <section class=\"tutorial-section\">\n      <h2>Chapters</h2>\n      <ul class=\"chapter-index\">\n");
    for (title, rows) in &chapters {
        let _ = writeln!(
            body,
            "        <li><a href=\"#ch-{slug}\">{title}</a> <span class=\"chapter-count\">{n}</span></li>",
            slug = docs::slug(title),
            title = escape(title),
            n = rows.len(),
        );
    }
    body.push_str("      </ul>\n    </section>\n");

    for (title, rows) in &chapters {
        let _ = write!(
            body,
            "    <section class=\"tutorial-section\" id=\"ch-{slug}\">\n      <h2>{title}</h2>\n      <p class=\"chapter-meta\">{n} entries</p>\n",
            slug = docs::slug(title),
            title = escape(title),
            n = rows.len(),
        );
        for (name, signature, doc) in *rows {
            let slug = docs::slug(name);
            let _ = write!(
                body,
                "      <article class=\"doc-entry\" id=\"doc-{slug}\">\n        \
                 <h3><a class=\"doc-anchor\" href=\"#doc-{slug}\">#</a> <code>{name}</code></h3>\n        \
                 <pre><code class=\"lang-r r\">{signature}\n</code></pre>\n        \
                 <p>{doc}</p>\n      </article>\n",
                slug = slug,
                name = escape(name),
                signature = escape(signature),
                doc = escape(doc),
            );
        }
        body.push_str("    </section>\n");
    }

    let page = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="color-scheme" content="dark light">
  <meta name="description" content="rlang — Primitive reference. Every one of the {primitives} primitive functions and {operators} operators the current rlang build implements, each with its signature and what it does. MIT licensed.">
  <title>rlang &mdash; Primitive Reference</title>
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link href="https://fonts.googleapis.com/css2?family=Orbitron:wght@400;600;700;900&amp;family=Share+Tech+Mono&amp;display=swap" rel="stylesheet">
  <link rel="stylesheet" href="hud-static.css">
  <link rel="stylesheet" href="tutorial.css">
  <style>
    .tutorial-main {{ max-width: 72rem; }}
    .docs-build-line {{
      margin: 0.35rem 0 0;
      font-family: 'Share Tech Mono', ui-monospace, monospace;
      font-size: 11px; color: var(--text-dim);
      letter-spacing: 0.03em; max-width: 42rem; opacity: 0.75;
    }}
    .ref-intro {{ font-size: 13px; line-height: 1.65; color: var(--text-dim); max-width: 56rem; }}

    .chapter-index {{
      list-style: none; padding: 0; margin: 0;
      display: grid; grid-template-columns: repeat(auto-fill, minmax(18rem, 1fr));
      gap: 0.3rem;
    }}
    .chapter-index li {{
      border: 1px solid var(--border); padding: 0.45rem 0.65rem; border-radius: 2px;
      background: color-mix(in srgb, var(--bg-card) 92%, transparent);
      display: flex; justify-content: space-between; align-items: baseline;
    }}
    .chapter-index li a {{
      color: var(--cyan); text-decoration: none; font-size: 13px;
      font-family: 'Share Tech Mono', ui-monospace, monospace;
    }}
    .chapter-index li a:hover {{ color: var(--accent-light); }}
    .chapter-count {{
      font-size: 10px; color: var(--text-muted);
      font-family: 'Share Tech Mono', ui-monospace, monospace;
    }}
    .chapter-meta {{
      font-size: 11px; color: var(--text-muted); margin: -0.3rem 0 0.8rem;
      font-family: 'Share Tech Mono', ui-monospace, monospace;
    }}

    .doc-entry {{
      margin: 1rem 0 1.4rem;
      padding: 0.75rem 0.9rem 0.5rem;
      border-left: 2px solid var(--cyan);
      background: color-mix(in srgb, var(--bg) 94%, transparent);
      border-radius: 2px;
    }}
    .doc-entry h3 {{
      margin: 0 0 0.45rem;
      font-family: 'Orbitron', sans-serif;
      font-size: 13px; font-weight: 700; letter-spacing: 1.5px;
      text-transform: uppercase; color: var(--cyan);
    }}
    .doc-entry h3 code {{
      color: var(--accent-light); background: transparent; border: none;
      padding: 0; font-size: 1em; letter-spacing: 0.5px;
    }}
    .doc-entry .doc-anchor {{
      color: var(--text-muted); font-size: 0.85em; margin-right: 0.25rem;
      text-decoration: none;
    }}
    .doc-entry .doc-anchor:hover {{ color: var(--accent); }}
    .doc-entry p {{
      font-size: 13px; line-height: 1.6; color: var(--text-dim);
      margin: 0.35rem 0;
    }}
    .doc-entry p code {{ color: var(--accent-light); font-size: 12px; }}
    .doc-entry pre {{
      font-family: 'Share Tech Mono', ui-monospace, monospace;
      font-size: 12px;
      background: var(--bg); border: 1px solid var(--border);
      border-radius: 2px;
      padding: 0.7rem 0.9rem; overflow-x: auto;
      color: var(--text); margin: 0.5rem 0;
      box-shadow: inset 0 0 18px rgba(0, 0, 0, 0.35);
    }}
    .doc-entry pre code {{ color: var(--text); background: transparent; border: none; padding: 0; }}
    [data-theme="light"] .doc-entry pre {{ box-shadow: inset 0 0 10px rgba(0, 0, 0, 0.05); }}

    kbd {{
      font-family: 'Share Tech Mono', ui-monospace, monospace;
      font-size: 11px;
      padding: 1px 6px;
      background: var(--bg-secondary);
      border: 1px solid var(--border);
      border-bottom-width: 2px;
      border-radius: 3px;
      color: var(--cyan);
    }}
  </style>
</head>
<body>
  <div class="app tutorial-app" id="docsApp">
    <div class="crt-scanline" id="crtH" aria-hidden="true"></div>
    <div class="crt-scanline-v" id="crtV" aria-hidden="true"></div>

    <header class="tutorial-header">
      <div class="tutorial-header-inner">
        <div>
          <h1 class="tutorial-brand">// RLANG &mdash; PRIMITIVE REFERENCE</h1>
          <nav class="tutorial-crumbs" aria-label="Breadcrumb">
            <a href="index.html">Docs</a>
            <span class="sep">/</span>
            <a href="report.html">Engineering Report</a>
            <span class="sep">/</span>
            <span class="current">Primitive Reference</span>
            <span class="sep">/</span>
            <a href="https://github.com/MenkeTechnologies/rlang" target="_blank" rel="noopener noreferrer">GitHub</a>
          </nav>
          <p class="docs-build-line">rlang v{version} &middot; {count} entries &middot; {chapter_count} chapters &middot; generated from <code>rlang/docs.rs</code> &middot; MIT</p>
        </div>
        <div class="tutorial-toolbar">
          <button type="button" class="btn btn-secondary" id="btnTheme" title="Toggle light/dark">Theme</button>
          <button type="button" class="btn btn-secondary active" id="btnCrt" title="CRT scanline overlay">CRT</button>
          <button type="button" class="btn btn-secondary active" id="btnNeon" title="Neon border pulse">Neon</button>
          <a class="btn btn-secondary" href="index.html">Docs</a>
          <a class="btn btn-secondary" href="report.html">Report</a>
          <a class="btn btn-secondary" href="https://github.com/MenkeTechnologies/rlang" target="_blank" rel="noopener noreferrer">GitHub</a>
        </div>
      </div>
    </header>

    <main class="tutorial-main">
      <h2 class="tutorial-title"><span class="step-hash">&gt;_</span>PRIMITIVE REFERENCE</h2>
      <p class="tutorial-subtitle">Every primitive and operator the current build implements, with its signature and what it does. Jump via the chapter index, or <kbd>Ctrl+F</kbd> for a name.</p>
      <p class="ref-intro">The {primitives} primitives and {operators} operators below are generated from the same corpus the language server completes from, which is checked against the runtime's own tables: a function listed here exists in the build, and a function missing here raises <code>could not find function</code> rather than failing silently. Each signature names the arguments the implementation actually reads, in the order it reads them &mdash; where that differs from GNU R, or where the answer differs, the description says so.</p>
{body}    </main>
  </div>
  <script src="hud-theme.js"></script>
</body>
</html>
"#
    );

    let out = "docs/reference.html";
    if let Err(e) = std::fs::write(out, page) {
        eprintln!("gen-docs: cannot write {out}: {e}");
        std::process::exit(1);
    }
    // Explicit user-requested output: this binary exists to report what it wrote.
    println!("wrote {out} ({count} entries in {chapter_count} chapters)");
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
