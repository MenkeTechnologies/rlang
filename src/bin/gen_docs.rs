//! Offline generator for `docs/reference.html` — the primitive reference page.
//!
//! Run before publishing GitHub Pages; the table is generated from the
//! language-server corpus (`rlang::lsp::corpus`), which is itself derived from
//! `builtins::PRIMITIVES`, so the page can never claim a function the runtime
//! does not implement. Every count on the page is computed here, never typed.

use std::fmt::Write as _;

fn main() {
    let corpus = rlang::lsp::corpus();
    let mut rows = String::new();
    for (name, doc) in &corpus {
        let _ = writeln!(
            rows,
            "        <tr><td><code>{}</code></td><td>{}</td></tr>",
            escape(name),
            escape(doc)
        );
    }

    let version = env!("CARGO_PKG_VERSION");
    let count = corpus.len();
    let page = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="color-scheme" content="dark light">
  <meta name="description" content="rlang — Primitive reference. The {count} primitive functions available in the current rlang build, generated from the language-server corpus. MIT licensed.">
  <title>rlang &mdash; Primitive Reference</title>
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link href="https://fonts.googleapis.com/css2?family=Orbitron:wght@400;600;700;900&amp;family=Share+Tech+Mono&amp;display=swap" rel="stylesheet">
  <link rel="stylesheet" href="hud-static.css">
  <link rel="stylesheet" href="tutorial.css">
  <style>
    .tutorial-main {{ max-width: 76rem; }}
    .file-table {{ width:100%;border-collapse:collapse;margin:0.6rem 0;font-size:12px; }}
    .file-table th {{ background:var(--bg-secondary);color:var(--cyan);font-family:'Orbitron',sans-serif;font-size:10px;font-weight:700;letter-spacing:1.2px;text-transform:uppercase;text-align:left;padding:7px 10px;border:1px solid var(--border); }}
    .file-table td {{ padding:6px 10px;border:1px solid var(--border);color:var(--text-dim);vertical-align:middle; }}
    .file-table tr:hover td {{ background:var(--bg-hover); }}
    .file-table td:first-child {{ font-family:'Share Tech Mono',monospace;color:var(--accent-light);font-weight:600;white-space:nowrap; }}
    .file-table code {{ font-size:11px;color:var(--accent-light);background:var(--bg-primary);padding:1px 4px;border-radius:2px; }}
    .stat-grid {{ display:grid;grid-template-columns:repeat(auto-fill,minmax(14rem,1fr));gap:0.75rem;margin:1.2rem 0; }}
    .stat-card {{ border:1px solid var(--border);border-top:3px solid var(--cyan);background:var(--bg-card);padding:1rem 1.2rem;border-radius:2px;text-align:center; }}
    .stat-card .stat-val {{ font-family:'Orbitron',sans-serif;font-size:28px;font-weight:900;color:var(--cyan);line-height:1.1;text-shadow:0 0 20px var(--cyan-glow); }}
    .stat-card .stat-val.accent {{ color:var(--accent);text-shadow:0 0 20px var(--accent-glow); }}
    .stat-card .stat-label {{ font-family:'Orbitron',sans-serif;font-size:9px;font-weight:700;letter-spacing:2px;text-transform:uppercase;color:var(--text-muted);margin-top:0.5rem; }}
    .docs-build-line {{ margin:0.35rem 0 0;font-family:'Share Tech Mono',ui-monospace,monospace;font-size:11px;color:var(--text-dim);letter-spacing:0.03em;max-width:42rem;opacity:0.75; }}
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
          <p class="docs-build-line">rlang v{version} &middot; generated from the language-server corpus &middot; MIT</p>
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
      <section>
        <p>Every primitive the current build implements. The table is generated
        from the same corpus the language server completes from, so a function
        listed here exists in the runtime and a function missing here raises
        <code>could not find function</code> rather than failing silently.</p>

        <div class="stat-grid">
          <div class="stat-card">
            <div class="stat-val">{count}</div>
            <div class="stat-label">Primitives</div>
          </div>
          <div class="stat-card">
            <div class="stat-val accent">v{version}</div>
            <div class="stat-label">Build</div>
          </div>
        </div>

        <table class="file-table">
          <thead><tr><th>function</th><th>description</th></tr></thead>
          <tbody>
{rows}          </tbody>
        </table>
      </section>
    </main>
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
    println!("wrote {out} ({count} entries)");
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
