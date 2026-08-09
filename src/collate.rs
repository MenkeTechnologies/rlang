//! Character ordering in R's collation locale, not by code point.
//!
//! R does not order strings by their bytes. It collates them through the
//! `LC_COLLATE` locale, so in any UTF-8 locale `sort()` answers
//! `a A b B c C` — case-insensitive at the primary level, lowercase before
//! uppercase only to break a tie — where a code-point sort answers
//! `A B C a b c`. The divergence is not an accented-character corner: it is
//! every mixed-case character vector, and `<` on strings along with it
//! (`"a" < "B"` is `TRUE` in R and `FALSE` by code point).
//!
//! Only the `C` locale orders by code point, and there R and a byte sort agree:
//!
//! | `LC_COLLATE`  | `sort(c("z", "a", "é", "B", "b"))` |
//! |---------------|------------------------------------|
//! | `C`           | `B a b z é`  (code point)          |
//! | `C.UTF-8`     | `a b B é z`  (collated)            |
//! | `en_US.UTF-8` | `a b B é z`  (collated)            |
//!
//! ## Provenance
//!
//! The reference `Rscript` (GNU R 4.6.1) reports `capabilities("ICU") == TRUE`
//! and collates through ICU's root ordering. `icu_collator`'s root collation was
//! diffed against it over 500 randomly generated groups mixing Latin, accents,
//! Greek (including final sigma), Cyrillic, CJK, Hangul, digits and punctuation:
//! every group sorted identically. The system `strcoll` was measured too and
//! rejected — Darwin's orders `é ë è e f` where R orders `e é è ë f`.
//!
//! ## Known limit
//!
//! Collation runs at ICU's *root* ordering rather than a per-locale tailoring.
//! `en_US`, `de_DE` and `fr_FR` were measured to agree with root, so this is
//! invisible for them; a locale that genuinely tailors (Swedish, where `ä`
//! sorts after `z`) would diverge. `LC_ALL=POSIX` is treated as `C` here, which
//! is what POSIX specifies and what glibc does; the reference R on Darwin
//! instead falls back to the system `strcoll` there and answers `é a b B z`.

use icu_collator::options::CollatorOptions;
use icu_collator::{Collator, CollatorBorrowed, CollatorPreferences};
use std::cmp::Ordering;
use std::sync::OnceLock;

/// The collation locale as R resolves it: `LC_ALL`, else `LC_COLLATE`, else
/// `LANG`, else the `C` locale. An empty variable does not count as set, which
/// is what `setlocale` does with it.
fn collate_locale() -> String {
    for var in ["LC_ALL", "LC_COLLATE", "LANG"] {
        if let Ok(v) = std::env::var(var) {
            if !v.is_empty() {
                return v;
            }
        }
    }
    "C".to_string()
}

/// Whether ordering runs through the collator. `C` and `POSIX` name the
/// code-point locale; every other locale collates.
fn collating() -> bool {
    !matches!(collate_locale().as_str(), "C" | "POSIX")
}

/// The collator, built once. `None` when the locale orders by code point, and
/// also if ICU declines to build one — in which case ordering falls back to the
/// code-point comparison rather than failing.
fn collator() -> Option<&'static CollatorBorrowed<'static>> {
    static COLLATOR: OnceLock<Option<CollatorBorrowed<'static>>> = OnceLock::new();
    COLLATOR
        .get_or_init(|| {
            collating()
                .then(|| {
                    Collator::try_new(CollatorPreferences::default(), CollatorOptions::default())
                        .ok()
                })
                .flatten()
        })
        .as_ref()
}

/// Compare two strings the way R's ordering does. Every character `sort`,
/// `order`, `rank`, `xtfrm`, `min`/`max`/`range`, `<`/`>`/`<=`/`>=` and the
/// default `factor` levels route through here.
///
/// Equality is deliberately *not* routed through it: R compares `==` bytewise,
/// so `"a" == "A"` stays `FALSE` even though they collate to the same primary
/// weight.
pub fn str_cmp(a: &str, b: &str) -> Ordering {
    match collator() {
        Some(c) => c.compare(a, b),
        None => a.cmp(b),
    }
}
