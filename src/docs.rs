//! The primitive reference corpus: a signature and a description for every
//! name in [`crate::builtins::PRIMITIVES`] and [`crate::builtins::OPERATORS`].
//!
//! This is what `cargo run --bin gen-docs` renders into `docs/reference.html`
//! and what `Rscript --lsp` completes from. Both constants are checked against
//! the runtime tables by the tests at the bottom of this file: an entry for a
//! name the runtime does not implement, or an implemented name with no entry,
//! fails the build's test run. That is why the reference can never claim a
//! function that is not there, and never omit one that is.
//!
//! Every signature names the arguments `call_primitive` actually reads, in the
//! order it reads them — not R's documented signature. Where the two differ the
//! description says so, because a reference that repeats R's manual would be
//! wrong about this runtime.

/// One documented callable: `(name, signature, description)`.
pub type Entry = (&'static str, &'static str, &'static str);

/// The primitive chapters, in the order `call_primitive` matches them.
pub const CHAPTERS: &[(&str, &[Entry])] = &[
    ("Construction and coercion", CONSTRUCTION),
    ("Attributes and metadata", ATTRIBUTES),
    ("Output and the inline-Rust FFI", OUTPUT),
    ("Sequences", SEQUENCES),
    ("Ordering and sets", ORDERING),
    ("Numeric summaries", SUMMARIES),
    ("Elementwise math", MATH),
    ("Predicates", PREDICATES),
    ("Strings and regular expressions", STRINGS),
    ("The apply family", APPLY),
    ("Matrices and arrays", MATRICES),
    ("Environments and dispatch", ENVIRONMENTS),
];

/// Every entry in every chapter, flattened.
pub fn entries() -> impl Iterator<Item = &'static Entry> {
    CHAPTERS.iter().flat_map(|(_, rows)| rows.iter())
}

/// The documented primitive named `name`, if there is one.
pub fn find(name: &str) -> Option<&'static Entry> {
    entries().find(|(n, _, _)| *n == name)
}

/// A stable HTML anchor for a callable name. Operator names are all
/// punctuation, so each punctuation character maps to a word rather than being
/// dropped — otherwise `%*%`, `[`, `(`, `{` and `$` would all anchor to the empty string.
/// Letters keep their case, since R names are case-sensitive and HTML ids are
/// too: `NROW` and `nrow` are different functions and need different anchors.
/// The `...` of `...length` and its kin is spelled `dots`, since leading dashes
/// are trimmed and `...length` would otherwise anchor where `length` does.
pub fn slug(name: &str) -> String {
    let mut out = String::with_capacity(name.len() * 4);
    let name = match name.strip_prefix("...") {
        Some(rest) if !rest.is_empty() => {
            out.push_str("dots-");
            rest
        }
        _ => name,
    };
    for ch in name.chars() {
        let piece = match ch {
            c if c.is_ascii_alphanumeric() => {
                out.push(c);
                continue;
            }
            '.' | '_' | ' ' => "-",
            '+' => "plus",
            '-' => "minus",
            '*' => "star",
            '/' => "slash",
            '^' => "caret",
            '%' => "pct",
            '=' => "eq",
            '<' => "lt",
            '>' => "gt",
            '&' => "and",
            '|' => "or",
            '!' => "not",
            ':' => "colon",
            '[' => "bracket",
            '(' => "paren",
            '{' => "brace",
            '$' => "dollar",
            _ => "-",
        };
        out.push_str(piece);
    }
    out.trim_matches('-').to_string()
}

const CONSTRUCTION: &[Entry] = &[
    (
        "c",
        "c(...)",
        "Combine the arguments into one vector, promoting every element to the widest type present (logical, integer, double, character, list). NULL arguments are dropped and argument tags become names, so c(a = 1, b = 2) is a named vector.",
    ),
    (
        "list",
        "list(...)",
        "Build a list holding the arguments unchanged. Tagged arguments name the elements.",
    ),
    (
        "vector",
        "vector(mode = \"logical\", length = 0)",
        "A zero-filled vector of the named mode: \"numeric\" or \"double\" gives 0, \"integer\" gives 0L, \"character\" gives \"\", \"list\" gives NULLs, and any other mode gives FALSE.",
    ),
    (
        "numeric",
        "numeric(length = 0)",
        "A double vector of `length` zeros — the usual way to preallocate a numeric result.",
    ),
    (
        "integer",
        "integer(length = 0)",
        "An integer vector of `length` zeros, for preallocating an integer result.",
    ),
    (
        "character",
        "character(length = 0)",
        "A character vector of `length` empty strings, for preallocating a character result.",
    ),
    (
        "logical",
        "logical(length = 0)",
        "A logical vector of `length` FALSE values, for preallocating a logical result.",
    ),
    (
        "as.numeric",
        "as.numeric(x)",
        "Coerce to double. Strings are parsed as numbers and become NA when they do not parse; TRUE and FALSE become 1 and 0; a list is flattened elementwise.",
    ),
    (
        "as.double",
        "as.double(x)",
        "The same coercion as as.numeric — rlang has one double type, so the two names share an implementation.",
    ),
    (
        "as.integer",
        "as.integer(x)",
        "Coerce to integer, truncating doubles toward zero. Non-finite values and unparseable strings become NA.",
    ),
    (
        "as.character",
        "as.character(x)",
        "Coerce to character using R's own 7-significant-digit number formatting. A factor yields its level labels rather than its integer codes.",
    ),
    (
        "as.logical",
        "as.logical(x)",
        "Coerce to logical: any nonzero number is TRUE, and only \"TRUE\", \"true\", \"T\", \"FALSE\", \"false\" and \"F\" convert from character — every other string is NA.",
    ),
    (
        "as.vector",
        "as.vector(x)",
        "A copy of x with names, dim, dimnames, class and levels stripped, so a table collapses to its plain counts. R's `mode` argument is accepted and ignored: the result keeps x's own type.",
    ),
    (
        "as.list",
        "as.list(x)",
        "A list of x's elements, keeping the names. An atomic vector becomes a list of length-1 vectors.",
    ),
    (
        "call",
        "call(name, ...)",
        "The unevaluated call of the function named by the string `name` on the already-evaluated arguments: call(\"round\", 10.5) is round(10.5). A call headed by an operator is that syntax, so call(\"+\", 1, 2) prints as 1 + 2.",
    ),
    (
        "as.call",
        "as.call(x)",
        "The call made of a list: its first element is the function and the rest, tagged by the list's names, are the arguments. A call is returned unchanged.",
    ),
    (
        "unlist",
        "unlist(x)",
        "Flatten a list recursively into an atomic vector of the widest type, composing names the way R does: list(a = 1, b = list(2, 3)) unlists to names a, b1, b2.",
    ),
];

const ATTRIBUTES: &[Entry] = &[
    (
        "length",
        "length(x)",
        "The number of elements: 0 for NULL, the element count for a vector or list, and the number of bindings for an environment.",
    ),
    (
        "lengths",
        "lengths(x)",
        "An integer vector of the length of each element of x, keeping x's names.",
    ),
    (
        "names",
        "names(x)",
        "The `names` attribute, or NULL when x has none.",
    ),
    (
        "setNames",
        "setNames(object, nm)",
        "A copy of `object` carrying `nm` as its names. Assigning all-NA names removes the attribute.",
    ),
    (
        "attr",
        "attr(x, which)",
        "One attribute of x by name, or NULL when it is not set. Partial matching of `which` is not performed.",
    ),
    (
        "attributes",
        "attributes(x)",
        "Every attribute of x as a named list, or NULL when x carries none.",
    ),
    (
        "class",
        "class(x)",
        "The `class` attribute when set; otherwise the implicit class — c(\"matrix\", \"array\") for a length-2 dim, else \"numeric\", \"integer\", \"character\", \"logical\", \"list\", \"function\", \"environment\" or \"NULL\".",
    ),
    (
        "inherits",
        "inherits(x, what, which = FALSE)",
        "TRUE when any string in `what` appears in class(x). With which = TRUE, an integer vector giving each `what`'s position in class(x), 0 where it is absent.",
    ),
    ("oldClass", "oldClass(x)", "The `class` attribute of x, or NULL when x has only an implicit class."),
    (
        "unclass",
        "unclass(x)",
        "A copy of x with the `class` attribute removed; every other attribute survives.",
    ),
    (
        "structure",
        "structure(.Data, ...)",
        "A copy of `.Data` with each tagged argument set as an attribute. The legacy spelling `.Names` is stored as `names`.",
    ),
    (
        "typeof",
        "typeof(x)",
        "The internal type: \"logical\", \"integer\", \"double\", \"character\", \"list\", \"closure\", \"builtin\", \"environment\", \"externalptr\" for a foreign R object, or \"NULL\".",
    ),
    (
        "mode",
        "mode(x)",
        "Like typeof with integer and double collapsed to \"numeric\" and closure and builtin collapsed to \"function\".",
    ),
    (
        "storage.mode",
        "storage.mode(x)",
        "The same string typeof returns; rlang stores no distinct storage mode.",
    ),
    (
        "dim",
        "dim(x)",
        "The `dim` attribute of a matrix or array, or NULL for a plain vector.",
    ),
    (
        "nrow",
        "nrow(x)",
        "The first element of dim(x), or NULL when x has no dim.",
    ),
    (
        "ncol",
        "ncol(x)",
        "The second element of dim(x), or NULL when x has no dim.",
    ),
    (
        "rownames",
        "rownames(x)",
        "The first component of dimnames(x), or NULL when there are no row labels.",
    ),
    (
        "colnames",
        "colnames(x)",
        "The second component of dimnames(x), or NULL when there are no column labels.",
    ),
    (
        "dimnames",
        "dimnames(x)",
        "The `dimnames` attribute — a list of one label vector per dimension — or NULL. Assignable through dimnames(x) <- list(...), rownames(x) <- and colnames(x) <-, and also produced by matrix(dimnames = ) and the cbind/rbind seam labels.",
    ),
];

const OUTPUT: &[Entry] = &[
    (
        "writeLines",
        "writeLines(text, con, sep = \"\\n\")",
        "Write each string of the character vector text followed by sep, returning NULL invisibly. A missing string is written as NA. A character con is a file path, truncated and written; without one the text goes to stdout. A non-character text is an error, as in R.",
    ),
    (
        ".rust",
        ".rust(code)",
        "Compile a self-contained inline Rust block — its `pub extern \"C\"` exports — to a cached cdylib through fusevm's FFI bridge, and register the exports for .Call. Returns NULL invisibly; the compile happens once per distinct source hash.",
    ),
    (
        ".Call",
        ".Call(.NAME, ...)",
        "Invoke a routine registered by .rust(). Arguments must be length-1 numeric, integer or character and are marshalled to f64, i64 and string; the returned scalar is marshalled back to a length-1 vector.",
    ),
    (
        "print",
        "print(x, digits, quote = TRUE)",
        "Print x in R's default layout and return it invisibly. `digits` overrides the significant-digit setting for this one call, and `quote = FALSE` prints strings bare with a missing one as <NA>. A user-defined print.<class> method takes over first, for both print(x) and top-level autoprint. A closure prints its deparsed source; a class with no method is followed by its attr(,\"class\") block.",
    ),
    (
        "cat",
        "cat(..., sep = \" \", file = \"\", append = FALSE)",
        "Write every argument's elements with no quotes and no trailing newline, joined by `sep`. The separator sits between arguments as well as between elements, so a leading zero-length argument still earns its successor one: cat(NULL, \"x\") writes \" x\". A separator containing a newline also ends the output with one. With a non-empty file the text goes there instead of to stdout, truncating unless append is TRUE. A list or a function argument is an error, as in R.",
    ),
    (
        "message",
        "message(...)",
        "Signal a `simpleMessage` condition and, if nothing catches or muffles it, write the concatenated arguments to stderr and return NULL invisibly.",
    ),
    (
        "warning",
        "warning(...)",
        "Signal a `simpleWarning` condition and continue. Uncaught, it is queued under the default `options(warn = 0)` and the whole batch prints when the top-level statement finishes; `warn = 1` prints it at once and a negative `warn` drops it.",
    ),
    (
        "stop",
        "stop(...)",
        "Signal a `simpleError` condition with the concatenated arguments as its message. Uncaught by any `tryCatch`/`try`, it ends the script with `Rscript: <message>` on stderr and status 1.",
    ),
    (
        "stopifnot",
        "stopifnot(...)",
        "Raise \"not all arguments are TRUE\" unless every argument is non-empty and all-TRUE. The message does not name the failing expression, because builtins receive values rather than expressions.",
    ),
    (
        "invisible",
        "invisible(x)",
        "Return x with the top-level echo suppressed for this value.",
    ),
    (
        "identity",
        "identity(x)",
        "Return the argument unchanged — useful as a default FUN in the apply family.",
    ),
    (
        "force",
        "force(x)",
        "Return the argument, having evaluated it. Written for its effect inside a closure factory, where it forces a promise that would otherwise be evaluated after the loop variable had moved on. Like identity it passes the argument's visibility through, so force(invisible(1)) prints nothing.",
    ),
    (
        "withVisible",
        "withVisible(x)",
        "Evaluate x and report whether its value would auto-print, as a list with elements `value` and `visible`. withVisible(invisible(1))$visible is FALSE; the list itself prints.",
    ),
    (
        "paste",
        "paste(..., sep = \" \", collapse = NULL)",
        "Join the arguments elementwise with recycling to the longest, separated by `sep`. Every argument contributes a field, so a zero-length one contributes an empty string and paste(\"a\", NULL, \"b\") is \"a  b\". A non-NULL `collapse` then joins the result into a single string, and collapsing nothing gives \"\". NA elements render as \"NA\".",
    ),
    (
        "paste0",
        "paste0(..., collapse = NULL)",
        "paste with an empty separator: the arguments are joined elementwise with nothing between them.",
    ),
    (
        "toString",
        "toString(x)",
        "A single string of x's elements joined by \", \" — the inline form R uses when a vector has to fit in one line.",
    ),
    (
        "deparse",
        "deparse(expr, width.cutoff = 60L)",
        "R source text for a value, one element per output line: a run of consecutive integers deparses as `a:b`, other integers carry the L suffix, strings are quoted and escaped, names are written inline (`c(a = 1)`, `list(x = \"q\")`) when they can be, an all-NA vector spells its type (`NA_integer_`), a list is `list(...)`, and any other attribute wraps the value in `structure(..., dim = c(2L, 2L))`. Lines wrap past `width.cutoff` (20 to 500) the way R's do: a vector's continuation is flush left, a list's is indented. A closure deparses to its source lines under R's own keep.source = FALSE layout rules.",
    ),
    (
        "dput",
        "dput(x)",
        "Writes the deparse of `x` to stdout, one line per line, and returns `x` invisibly.",
    ),
    (
        "noquote",
        "noquote(obj)",
        "Marks `obj` with the `noquote` class, so it prints as `print(x, quote = FALSE)` would: strings bare and a missing string as `<NA>`.",
    ),
    (
        "format",
        "format(x, trim = FALSE, nsmall = 0, digits, justify = \"left\", width = 0, scientific, big.mark = \"\")",
        "Format to character. A numeric vector takes fixed notation when its width is no greater than the scientific width plus getOption(\"scipen\") — the same rule print uses, so format(1e6) is \"1e+06\" at the default scipen of 0 — with a common decimal count, then pads to a common width: numbers right-justified, strings left. `digits` is the significant-digit count, defaulting to getOption(\"digits\"); `nsmall` the minimum decimals (fixed notation only), `scientific` forces one notation. `trim = TRUE` drops the common width of a non-character vector; `justify` (left, right, centre, none) steers a character vector's padding. A list formats each element's unlist on its own (trim = TRUE by default) and joins its pieces with \", \". A function formats to its deparsed source lines.",
    ),
    (
        "options",
        "options(...)",
        "Set, query or restore global options. A tagged argument sets one and the call returns the previous values as a named list, invisibly, so `old <- options(digits = 3)` then `options(old)` restores. An untagged string queries without setting and returns visibly; an untagged list restores every named element. `digits` (significant digits for printing, 1..22, default 7) and `scipen` (the penalty added to a scientific rendering's width before it is compared with the fixed one, so positive favours fixed; clamped below at -9, default 0) are the two the printing code reads; any other name is stored and read back but has no effect. A digits outside 1..22 is an error, as in R. Unlike R, options() with no arguments does not enumerate a full default set.",
    ),
    (
        "getOption",
        "getOption(x, default = NULL)",
        "The value of option x, or default when it has never been set.",
    ),
    (
        "formatC",
        "formatC(x, width, digits, format, flag)",
        "Build the equivalent printf spec and route it through sprintf, so sign, zero-padding and exponent rules are shared. `format` defaults to \"d\" for integer input and \"g\" for real.",
    ),
    (
        "prettyNum",
        "prettyNum(x, big.mark = \"\")",
        "Insert `big.mark` between every third digit of the integer part of each formatted number, preserving sign and fraction.",
    ),
    (
        "sprintf",
        "sprintf(fmt, ...)",
        "C-style formatting, vectorized over both the format and the arguments. Supports %d %i %s %f %e %E %g %G %a %A %x %X %o and %%, with the flags, field width and precision — including zero-padding after the sign, as C does. NA, NaN and Inf are printed through %s as R does, so the width and the - and 0 flags still apply to them. A `*` takes the width (or precision) from the preceding argument.",
    ),
];

const SEQUENCES: &[Entry] = &[
    (
        "seq_len",
        "seq_len(n)",
        "The integer vector running from 1 to n, empty when n is 0 or negative.",
    ),
    (
        "seq_along",
        "seq_along(along.with)",
        "The integer vector running from 1 to the length of the argument, whatever its type.",
    ),
    (
        "seq",
        "seq(from, to, by, length.out, along.with)",
        "Build a sequence. With one argument it is 1:from, which counts down when from < 1. The third positional argument is `by`, so seq(0, 1, 0.25) steps by a quarter; `length.out` fixes the term count instead, and `along.with` gives the indices of its argument. A `by` that cannot reach `to` is an error, as in R. The result stays integer when every element and the step are whole.",
    ),
    (
        "seq.int",
        "seq.int(from, to, by, length.out)",
        "The same implementation as seq — rlang draws no distinction between the two.",
    ),
    (
        "rep",
        "rep(x, times = 1, each = 1)",
        "Repeat x: each element `each` times, the whole sequence `times` times. A vector-valued `times` — R's per-element repeat count — is not supported; only its first element is read.",
    ),
    (
        "rep_len",
        "rep_len(x, length.out)",
        "Recycle x to exactly `length.out` elements, truncating or repeating as needed.",
    ),
    (
        "rev",
        "rev(x)",
        "Reverse the elements, reversing the names with them.",
    ),
    (
        "unname",
        "unname(obj)",
        "A copy of obj with the `names` attribute removed.",
    ),
    (
        "all.equal",
        "all.equal(target, current)",
        "TRUE when two numeric vectors agree within a mean relative difference of 1.5e-8, else the string \"Mean relative difference: <d>\". Only differing elements enter the scale, matching R's default countEQ = FALSE. Non-numeric arguments compare with identical.",
    ),
    (
        "head",
        "head(x, n = 6)",
        "The first n elements; a negative n drops the last |n| instead.",
    ),
    (
        "tail",
        "tail(x, n = 6)",
        "The last n elements; a negative n drops the first |n| instead.",
    ),
    (
        "append",
        "append(x, values, after = length(x))",
        "Splice values into x after position `after` (by default at the end), with c()'s type promotion; a named x keeps its labels on both sides of the splice.",
    ),
    (
        "replace",
        "replace(x, list, values)",
        "A copy of x with the elements list selects (positions, names or a logical mask) set to values, recycled — R's x[list] <- values; x.",
    ),
];

const ORDERING: &[Entry] = &[
    (
        "sort",
        "sort(x, decreasing = FALSE, na.last = NA, index.return = FALSE)",
        "The sorted values, names carried along. na.last places the missing values: NA (the default) drops them, TRUE puts them last, FALSE first. Ties keep their original order in both directions. With index.return = TRUE the result is a list of $x and the ordering $ix.",
    ),
    (
        "order",
        "order(..., decreasing = FALSE, na.last = TRUE)",
        "The 1-based permutation that sorts the first key, later keys breaking its ties. A position missing in any key is placed by na.last: last by default, first for FALSE, dropped for NA. Ties keep their original ascending order even when decreasing.",
    ),
    (
        "unique",
        "unique(x)",
        "The first occurrence of each distinct value, keeping the original order. Values are compared by their character form, so 1 and \"1\" are the same key.",
    ),
    (
        "setdiff",
        "setdiff(x, y)",
        "The de-duplicated elements of x that do not occur in y.",
    ),
    (
        "union",
        "union(x, y)",
        "The de-duplicated elements of x followed by the elements of y not already present.",
    ),
    (
        "intersect",
        "intersect(x, y)",
        "The de-duplicated elements of x that also occur in y.",
    ),
    (
        "match",
        "match(x, table)",
        "The first 1-based position of each element of x within `table`, NA where absent. Comparison is by character form, which makes it type-agnostic.",
    ),
    (
        "is.element",
        "is.element(el, table)",
        "TRUE for each element of `el` that occurs in `table` — match(el, table) reduced to a logical.",
    ),
    (
        "duplicated",
        "duplicated(x)",
        "TRUE at each position whose value has already appeared earlier in x.",
    ),
    (
        "rank",
        "rank(x, ties.method = \"average\")",
        "Ranks with tied values sharing the average of the slots they occupy (R's default ties.method = \"average\", a double vector), or with ties.method = \"first\", \"last\", \"min\" or \"max\" an integer vector of the slots in appearance order, reversed, the smallest or the largest. Character input ranks in collation order, and the missing values take the trailing ranks. \"random\" is not supported.",
    ),
    (
        "sort.list",
        "sort.list(x, decreasing = FALSE, na.last = TRUE)",
        "The 1-based permutation that sorts a single key — order() over one vector. na.last places the missing values: last by default, first for FALSE, dropped for NA, in which case the positions index the vector with the missing values already removed.",
    ),
    (
        "xtfrm",
        "xtfrm(x)",
        "The numeric key that sorts x. A plain numeric vector is already that key and is returned unchanged, names and all; character, logical and factor input ranks in collation order with tied values sharing the lowest slot they occupy, and missing values staying NA.",
    ),
    (
        "which",
        "which(x)",
        "The 1-based positions where x is TRUE, carrying the names of the selected elements. `arr.ind` is not supported.",
    ),
    (
        "which.max",
        "which.max(x)",
        "The position of the first maximum, ignoring NA. An all-NA or empty vector yields an empty integer vector.",
    ),
    (
        "which.min",
        "which.min(x)",
        "The position of the first minimum, ignoring NA.",
    ),
];

const SUMMARIES: &[Entry] = &[
    (
        "weighted.mean",
        "weighted.mean(x, w, na.rm = FALSE)",
        "stats' default method: sum((x * w)[w != 0]) / sum(w), so a missing x under a zero weight drops out, and a missing weight makes the result NA. Without w every weight is 1. With na.rm the missing x and their weights are dropped first. x and w of different lengths is an error.",
    ),
    (
        "sum",
        "sum(..., na.rm = FALSE)",
        "The total over every element of every argument. The result stays integer when all arguments are integer or logical, otherwise it is a double. A missing value propagates, NA outranking NaN. Integer overflow widens to a double instead of producing NA.",
    ),
    (
        "prod",
        "prod(..., na.rm = FALSE)",
        "The product of every element of every argument, always a double.",
    ),
    (
        "mean",
        "mean(x, na.rm = FALSE)",
        "The arithmetic mean of one vector. An empty vector gives NaN. Without na.rm a missing value propagates as the first one met, so mean(c(1, NA, NaN)) is NA and mean(c(1, NaN, NA)) is NaN, matching R's IEEE accumulation. R's `trim` argument is accepted and ignored.",
    ),
    (
        "median",
        "median(x, na.rm = FALSE)",
        "The middle value of the sorted data, or the mean of the two middle values at even length.",
    ),
    (
        "quantile",
        "quantile(x, probs = seq(0, 1, 0.25), na.rm = FALSE, names = TRUE, type = 7, digits = 7, fuzz)",
        "Sample quantiles, a port of R's quantile.default: all nine types (7, interpolating (1 - h) * x[lo] + h * x[hi], is the default), names from formatC(100 * probs, format = \"fg\", digits = digits) plus a percent sign, an empty name and a missing value for a missing prob. A missing value in x is an error unless na.rm = TRUE; an ordered factor takes type 1 or 3 and gives an ordered factor.",
    ),
    (
        "summary",
        "summary(object, maxsum, digits, quantile.type = 7)",
        "R's summary.default: for a numeric vector the minimum, quartiles, mean and maximum (quantile type quantile.type, rounded to digits significant digits when given) and the count of NAs; for a logical one its mode and value counts; for a character one its length, distinct and blank counts and nchar range; for a factor each level's count, the least frequent pooled as (Other) past maxsum levels. All but the factor's print as a table, a numeric one to getOption(\"digits\") - 3 significant digits. A matrix, list or data frame is summarised in the embedded R.",
    ),
    (
        "cor",
        "cor(x, y)",
        "The Pearson correlation of two equal-length numeric vectors. Zero variance in either vector, or fewer than two pairs, gives NA rather than NaN. Spearman and Kendall are not implemented.",
    ),
    (
        "cov",
        "cov(x, y)",
        "The sample covariance of two equal-length numeric vectors (n-1 denominator), each mean refined by a correction pass as R's cov.c does. A missing value in either gives NA; the use argument and matrix arguments are not supported.",
    ),
    (
        "rle",
        "rle(x)",
        "Run-length encoding: a list of $lengths and $values for each run of equal consecutive elements, classed \"rle\" so it prints in R's layout.",
    ),
    (
        "inverse.rle",
        "inverse.rle(x)",
        "Expand an rle list back into the original vector.",
    ),
    (
        "var",
        "var(x, y = NULL, na.rm = FALSE)",
        "The sample variance with the n-1 denominator, computed in the same two-pass form as R's C code so the last printed digit agrees. With y, the covariance of two equal-length vectors, as cov(x, y). A covariance matrix of a matrix argument is not supported.",
    ),
    (
        "sd",
        "sd(x, na.rm = FALSE)",
        "The square root of var(x) — the sample standard deviation.",
    ),
    (
        "min",
        "min(..., na.rm = FALSE)",
        "The smallest value across every argument. Character arguments compare lexically. With no values at all the answer is Inf, as in R, but the accompanying warning is not raised.",
    ),
    (
        "max",
        "max(..., na.rm = FALSE)",
        "The largest value across every argument; -Inf when there are no values. NA dominates NaN, so max(c(1, NA, NaN)) is NA.",
    ),
    (
        "range",
        "range(..., na.rm = FALSE)",
        "c(min, max) over every argument; c(Inf, -Inf) when there are no values.",
    ),
    (
        "cumsum",
        "cumsum(x)",
        "The running total. Integer and logical input stays integer. An NA poisons every later element, since the accumulated value is no longer known.",
    ),
    (
        "cumprod",
        "cumprod(x)",
        "The running product, always a double, with the same NA propagation as cumsum.",
    ),
    (
        "diff",
        "diff(x, lag = 1, differences = 1)",
        "The lag-`lag` differences, applied `differences` times. Integer input stays integer; a vector shorter than the lag yields an empty result.",
    ),
];

const MATH: &[Entry] = &[
    (
        "abs",
        "abs(x)",
        "Absolute value, elementwise. An integer vector stays integer; names and dim are carried through.",
    ),
    ("sqrt", "sqrt(x)", "Square root, elementwise. A negative argument gives NaN."),
    ("exp", "exp(x)", "e raised to the power of each element, computed elementwise."),
    (
        "log",
        "log(x, base)",
        "The natural logarithm, or the logarithm to `base` when a second argument is given.",
    ),
    ("log2", "log2(x)", "The base-2 logarithm of each element; zero gives -Inf and a negative gives NaN."),
    ("log10", "log10(x)", "The base-10 logarithm of each element; zero gives -Inf and a negative gives NaN."),
    (
        "log1p",
        "log1p(x)",
        "log(1 + x) computed to full precision for small x.",
    ),
    (
        "expm1",
        "expm1(x)",
        "exp(x) - 1 computed to full precision for small x.",
    ),
    ("floor", "floor(x)", "The largest integer value not greater than each element."),
    ("ceiling", "ceiling(x)", "The smallest integer value not less than each element."),
    (
        "trunc",
        "trunc(x)",
        "Each element truncated toward zero, so trunc(-1.7) is -1 where floor gives -2.",
    ),
    (
        "round",
        "round(x, digits = 0)",
        "R's fround: of x rounded down and up at `digits` decimals, the nearer in double arithmetic, and on a tie the one whose scaled value is even, so round(0.15, 1) is 0.1. A fractional `digits` rounds to the nearest whole count; negative digits round to tens, hundreds and so on. x and digits recycle against each other.",
    ),
    (
        "signif",
        "signif(x, digits = 6)",
        "R's fprec: round to `digits` (at least 1) significant figures by scaling with an exact power of ten, nearbyint, and scaling back: signif(123.456, 2) is 120 and signif(0.0034219, 3) is 0.00342. x and digits recycle against each other.",
    ),
    (
        "sign",
        "sign(x)",
        "-1, 0 or 1 according to the sign of each element.",
    ),
    ("sin", "sin(x)", "The sine of each element, with the argument taken in radians."),
    ("cos", "cos(x)", "The cosine of each element, with the argument taken in radians."),
    ("tan", "tan(x)", "The tangent of each element, with the argument taken in radians."),
    ("asin", "asin(x)", "The arc sine of each element, in radians; an argument outside [-1, 1] gives NaN."),
    ("acos", "acos(x)", "The arc cosine of each element, in radians; an argument outside [-1, 1] gives NaN."),
    ("atan", "atan(x)", "The arc tangent of each element, in radians; use atan2 when the quadrant matters."),
    (
        "atan2",
        "atan2(y, x)",
        "The angle in radians from the positive x-axis to the point (x, y), with the arguments recycled to the longer length.",
    ),
    ("sinh", "sinh(x)", "The hyperbolic sine of each element."),
    ("cosh", "cosh(x)", "The hyperbolic cosine of each element."),
    ("tanh", "tanh(x)", "The hyperbolic tangent of each element."),
    (
        "gamma",
        "gamma(x)",
        "The gamma function, computed through the same system libm that R links, so the printed result matches digit for digit.",
    ),
    (
        "lgamma",
        "lgamma(x)",
        "The natural logarithm of the absolute value of the gamma function.",
    ),
    (
        "factorial",
        "factorial(x)",
        "gamma(x + 1), so non-integer arguments are defined too.",
    ),
    (
        "lfactorial",
        "lfactorial(x)",
        "lgamma(x + 1) — the log factorial, finite far past the point where factorial overflows.",
    ),
    (
        "choose",
        "choose(n, k)",
        "The binomial coefficient, computed through lgamma and recycled over both arguments. A negative k gives 0.",
    ),
    (
        "beta",
        "beta(a, b)",
        "The beta function gamma(a)gamma(b)/gamma(a+b), computed through lgamma so intermediate values stay finite. Arguments recycle.",
    ),
    (
        "lbeta",
        "lbeta(a, b)",
        "The natural logarithm of beta(a, b), which stays finite for large arguments.",
    ),
    (
        "cummax",
        "cummax(x)",
        "The running maximum; an NA makes every later element NA.",
    ),
    (
        "cummin",
        "cummin(x)",
        "The running minimum, with the same NA propagation.",
    ),
    (
        "pmax",
        "pmax(..., na.rm = FALSE)",
        "The elementwise maximum across the arguments, recycled to the longest. The result is always a double.",
    ),
    (
        "pmin",
        "pmin(..., na.rm = FALSE)",
        "The elementwise minimum across the arguments, recycled to the longest.",
    ),
    (
        "sweep",
        "sweep(x, MARGIN, STATS, FUN = \"-\", check.margin = TRUE, ...)",
        "Apply FUN to x and STATS laid out along the MARGIN dimensions (aperm(array(STATS, dim(x)[perm]), order(perm)), margins first). check.margin warns when STATS cannot line up with those margins; MARGIN may name dimensions.",
    ),
    (
        "scale",
        "scale(x, center = TRUE, scale = TRUE)",
        "Center the columns of as.matrix(x) on their means (or the given values) and divide them by their root-mean-square (or the given values), recording both as the scaled:center and scaled:scale attributes.",
    ),
    (
        "as.matrix",
        "as.matrix(x)",
        "A matrix is returned as is; any other vector becomes a single column whose row names are its names.",
    ),
    (
        "row",
        "row(x)",
        "The integer matrix of each cell's row index, for a matrix-like x.",
    ),
    (
        "col",
        "col(x)",
        "The integer matrix of each cell's column index, for a matrix-like x.",
    ),
    (
        "is.unsorted",
        "is.unsorted(x, na.rm = FALSE, strictly = FALSE)",
        "Whether some element sorts after the next (with strictly, not before it), in sort's order; NA when x holds a missing value and na.rm is FALSE.",
    ),
    (
        "anyDuplicated",
        "anyDuplicated(x, incomparables = FALSE, fromLast = FALSE)",
        "The index of the first element equal to an earlier one (scanning from the end with fromLast), or 0 when every element is unique.",
    ),
    (
        "kronecker",
        "kronecker(X, Y, FUN = \"*\", make.dimnames = FALSE, ...)",
        "The Kronecker product: outer(X, Y, FUN) with its dimensions interleaved so each cell of X scales a block of Y, reshaped to dim(X) * dim(Y) after padding the shorter dim with ones. make.dimnames labels each margin with the x:y pairs of the dimnames.",
    ),
    (
        "rowsum",
        "rowsum(x, group, reorder = TRUE, na.rm = FALSE)",
        "Column sums of the rows of x (a vector is one column) within each group: one row per distinct group, sorted with missing last unless reorder = FALSE, labelled with the group as text. Integer sums that overflow are NA.",
    ),
    (
        "fivenum",
        "fivenum(x, na.rm = TRUE)",
        "Tukey's five-number summary (minimum, lower hinge, median, upper hinge, maximum), each the mean of the order statistics either side of its depth. A missing value without na.rm, or an empty x, gives five logical NAs.",
    ),
    (
        "IQR",
        "IQR(x, na.rm = FALSE, type = 7)",
        "The interquartile range, diff(quantile(as.numeric(x), c(0.25, 0.75), na.rm =, names = FALSE, type =)).",
    ),
    (
        "mad",
        "mad(x, center = median(x), constant = 1.4826, na.rm = FALSE, low = FALSE, high = FALSE)",
        "The scaled median absolute deviation, constant * median(abs(x - center)); low or high takes the lo- or hi-median of an even count.",
    ),
    (
        "zapsmall",
        "zapsmall(x, digits = getOption(\"digits\"))",
        "round(x, max(0, digits - log10(max(abs(x))))): entries negligible next to the largest become zero. All-missing x is returned unchanged.",
    ),
    (
        "tabulate",
        "tabulate(bin, nbins = max(1L, bin, na.rm = TRUE))",
        "The count of each integer 1..nbins occurring in `bin`, as an integer vector of length nbins. Values outside the range and NAs are ignored.",
    ),
    (
        "findInterval",
        "findInterval(x, vec, rightmost.closed = FALSE, all.inside = FALSE, left.open = FALSE, checkSorted = TRUE)",
        "For each x, the index i of the interval vec[i] <= x < vec[i+1] of the sorted breakpoints (vec[i] < x <= vec[i+1] with left.open): 0 below the first, length(vec) past the last. rightmost.closed folds a hit on the closing breakpoint into the last interval, all.inside folds both ends in. An unsorted vec is an error.",
    ),
];

const PREDICATES: &[Entry] = &[
    ("is.atomic", "is.atomic(x)", "TRUE for a logical, integer, double or character vector. NULL is not atomic, as in R 4.4 and later, and neither is a list."),
    ("is.null", "is.null(x)", "TRUE when x is NULL. A zero-length vector is not NULL and answers FALSE."),
    (
        "is.na",
        "is.na(x)",
        "TRUE at each missing element. NaN counts as missing in a double vector, and a list element counts when it is itself a length-1 NA.",
    ),
    (
        "is.nan",
        "is.nan(x)",
        "TRUE at each NaN. Only doubles can carry NaN, so every other type answers all-FALSE.",
    ),
    (
        "is.finite",
        "is.finite(x)",
        "TRUE where the element is a finite number — never for character or list input.",
    ),
    (
        "is.infinite",
        "is.infinite(x)",
        "TRUE at each Inf or -Inf; FALSE everywhere else, including NA.",
    ),
    (
        "anyNA",
        "anyNA(x)",
        "TRUE when any element is NA or NaN, checked without building the full is.na vector.",
    ),
    (
        "complete.cases",
        "complete.cases(x)",
        "TRUE at each non-missing element. Over a plain vector this is the negation of is.na; data-frame input is not supported natively.",
    ),
    (
        "na.omit",
        "na.omit(object)",
        "na.omit.default: drops the missing elements of an atomic vector (the rows holding one, for a matrix) and records their positions, named by the dropped labels, in an omit-classed na.action attribute. A list or a higher-rank array is returned unchanged.",
    ),
    (
        "is.numeric",
        "is.numeric(x)",
        "TRUE for a double or integer vector, and FALSE for a factor — which R excludes explicitly even though a factor is stored as an integer vector.",
    ),
    ("is.double", "is.double(x)", "TRUE only for a double vector: is.double(1L) is FALSE where is.numeric(1L) is TRUE."),
    ("is.integer", "is.integer(x)", "TRUE only for an integer vector, and FALSE for a factor, matching R's own factor exclusion."),
    ("is.character", "is.character(x)", "TRUE for a character vector, whatever attributes it carries."),
    ("is.logical", "is.logical(x)", "TRUE for a logical vector, whatever attributes it carries."),
    ("is.list", "is.list(x)", "TRUE for a list, including a list carrying a class attribute."),
    ("is.primitive", "is.primitive(x)", "TRUE for a function R implements as a primitive (typeof builtin or special), FALSE for a closure."),
    ("is.environment", "is.environment(x)", "TRUE for an environment, FALSE for anything else."),
    (
        "is.function",
        "is.function(x)",
        "TRUE for a closure, a primitive used as a value, or a Negate/Vectorize combinator.",
    ),
    (
        "is.vector",
        "is.vector(x)",
        "TRUE for an atomic vector or list carrying no attribute other than `names` — a matrix, a factor, or anything with a stray attr answers FALSE, as in R.",
    ),
    (
        "any",
        "any(..., na.rm = FALSE)",
        "TRUE when any element of any argument is TRUE. Three-valued: an NA with no TRUE present gives NA unless na.rm is set.",
    ),
    (
        "all",
        "all(..., na.rm = FALSE)",
        "TRUE when no element is FALSE. An NA with no FALSE present gives NA unless na.rm is set.",
    ),
    (
        "isTRUE",
        "isTRUE(x)",
        "TRUE when x is a length-1 TRUE. The argument is coerced first, so isTRUE(1) answers TRUE here where GNU R — which demands an actual logical — answers FALSE.",
    ),
    (
        "isFALSE",
        "isFALSE(x)",
        "TRUE when x is a length-1 FALSE, with the same coercion caveat as isTRUE.",
    ),
    (
        "xor",
        "xor(x, y)",
        "Elementwise exclusive or, recycled to the longer argument; NA on either side gives NA.",
    ),
    (
        "bitwAnd",
        "bitwAnd(a, b)",
        "Bitwise AND, recycled. rlang computes on 64-bit integers, so values beyond R's 32-bit range keep their bits instead of becoming NA.",
    ),
    ("bitwOr", "bitwOr(a, b)", "Bitwise OR of each pair, with the arguments recycled to the longer one."),
    ("bitwXor", "bitwXor(a, b)", "Bitwise exclusive OR, recycled over both arguments."),
    ("bitwNot", "bitwNot(a)", "The bitwise complement of each element, computed on 64-bit integers."),
    (
        "bitwShiftL",
        "bitwShiftL(a, b)",
        "Shift each element left by b bits, on 64-bit integers rather than R 32-bit ones.",
    ),
    (
        "bitwShiftR",
        "bitwShiftR(a, b)",
        "Shift each element right by b bits — an arithmetic shift, so the sign bit is preserved.",
    ),
    (
        "identical",
        "identical(x, y)",
        "TRUE when both values have the same internal type, the same names and the same elements, comparing lists recursively. Environments are identical only to themselves; a primitive or a symbol to the one of the same name; calls part by part; closures by their formals, body and environment. Attributes other than names are not compared, so a classed value can be identical to a bare one.",
    ),
    (
        "ifelse",
        "ifelse(test, yes, no)",
        "Elementwise selection: the matching element of `yes` where test is TRUE, of `no` where it is FALSE, and NA where test is NA. Both branches recycle.",
    ),
];

const STRINGS: &[Entry] = &[
    (
        "file.path",
        "file.path(..., fsep = \"/\")",
        "Join the parts element-wise with fsep, recycling to the longest part. Any zero-length part, or no parts at all, gives character(0).",
    ),
    (
        "basename",
        "basename(path)",
        "The part of each path after its last /, ignoring trailing separators: basename(\"/a/b/\") is \"b\", and the root and the empty path give \"\".",
    ),
    (
        "dirname",
        "dirname(path)",
        "Each path up to its last /, ignoring trailing separators and collapsing the run of separators before the last component. A path with no / is \".\", the root stays \"/\", and the empty path stays \"\".",
    ),
    (
        "sQuote",
        "sQuote(x, q = TRUE)",
        "Each string wrapped in single quotes. q = TRUE gives the typographic pair R uses in a UTF-8 locale; q = FALSE gives plain ASCII apostrophes. The result keeps x's names and shape.",
    ),
    (
        "dQuote",
        "dQuote(x, q = TRUE)",
        "Each string wrapped in double quotes: the typographic pair by default, plain ASCII quotes with q = FALSE. The result keeps x's names and shape.",
    ),
    (
        "shQuote",
        "shQuote(string)",
        "Quote each string for a POSIX shell (R's type = \"sh\"): single quotes, unless any element holds a single quote, in which case every element is double-quoted with \", $, ` and \\ escaped. The type argument is not read.",
    ),
    (
        "nchar",
        "nchar(x, type = \"chars\")",
        "The size of each string in the unit `type` names: \"chars\" (Unicode code points, the default), \"bytes\" (its UTF-8 length), or \"width\" (terminal columns, where a CJK character counts two and a combining mark none).",
    ),
    (
        "strtrim",
        "strtrim(x, width)",
        "The longest prefix of each string that fits in `width` terminal columns, recycling `width`. A double-width character is dropped whole rather than split.",
    ),
    (
        "utf8ToInt",
        "utf8ToInt(x)",
        "The Unicode code points of one string, as an integer vector. NA for anything that is not a string.",
    ),
    (
        "intToUtf8",
        "intToUtf8(x, multiple = FALSE)",
        "The inverse of utf8ToInt: one string built from all the code points, or — with multiple = TRUE — one string per code point. A 0 is dropped.",
    ),
    (
        "substr",
        "substr(x, start, stop)",
        "The characters of each element from `start` to `stop` inclusive, 1-based. start and stop are read as single numbers; use substring to vary them per element.",
    ),
    (
        "substring",
        "substring(text, first = 1, last = 1000000)",
        "Like substr, but text, first and last all recycle to the longest, so substring(\"hello\", 1:3) returns three pieces.",
    ),
    ("toupper", "toupper(x)", "Each string converted to upper case, one character at a time — so the result has the same number of characters as the input and toupper(\"straße\") is \"STRAßE\"."),
    ("tolower", "tolower(x)", "Each string converted to lower case, one character at a time, so the character count is preserved."),
    (
        "casefold",
        "casefold(x, upper = FALSE)",
        "tolower by default, toupper when upper = TRUE — one function behind a flag.",
    ),
    (
        "chartr",
        "chartr(old, new, x)",
        "Translate the characters of `old` to the matching characters of `new`, expanding a-c ranges in both. An `old` longer than `new` is an error; a character repeated in `old` takes its last mapping.",
    ),
    (
        "strtoi",
        "strtoi(x, base = 10)",
        "Parse each string as an integer in the given base, accepting an optional 0x or 0X prefix at base 16. Unparseable strings become NA.",
    ),
    (
        "as.hexmode",
        "as.hexmode(x)",
        "An integer vector classed hexmode: integers as they are, whole doubles through as.integer, strings read as base-16 digits; anything else is an error. It formats (format(x, width, upper.case)), prints and converts with as.character as hexadecimal digits, zero-padded to a common width, and keeps its class when subset with [.",
    ),
    (
        "strrep",
        "strrep(x, times)",
        "Repeat each string `times` times, with both arguments recycled.",
    ),
    (
        "encodeString",
        "encodeString(x, width = 0, quote = \"\", na.encode = TRUE, justify = \"left\")",
        "Each string with its control characters escaped, wrapped in `quote` (whose own character is then escaped), and padded to `width` columns — the widest element for width = NA — by `justify`. NA becomes \"NA\" quoted or \"<NA>\" unquoted, or stays NA with na.encode = FALSE.",
    ),
    (
        "make.unique",
        "make.unique(names, sep = \".\")",
        "names with every repeat of an earlier element suffixed by `sep` and the lowest count that names nothing else in the vector.",
    ),
    (
        "make.names",
        "make.names(names, unique = FALSE, allow_ = TRUE)",
        "Syntactic names: an X prefixed where a name cannot start, every invalid character replaced by a dot, a dot appended to a reserved word; unique = TRUE then applies make.unique.",
    ),
    (
        "strsplit",
        "strsplit(x, split, fixed = FALSE)",
        "Split each string by a regular expression, returning one character vector per element in a list. An empty pattern splits into single characters; fixed = TRUE splits on the literal text.",
    ),
    (
        "sub",
        "sub(pattern, replacement, x, fixed = FALSE, ignore.case = FALSE)",
        "Replace the first match of `pattern` in each element. Back-references are written R's way, as \\1..\\9.",
    ),
    (
        "gsub",
        "gsub(pattern, replacement, x, fixed = FALSE, ignore.case = FALSE)",
        "Replace every match of `pattern` in each element, with the same back-reference and flag handling as sub.",
    ),
    (
        "grepl",
        "grepl(pattern, x, fixed = FALSE, ignore.case = FALSE)",
        "TRUE at each element of x the pattern matches, and NA where the element is NA.",
    ),
    (
        "grep",
        "grep(pattern, x, value = FALSE, fixed = FALSE, ignore.case = FALSE)",
        "The 1-based positions of the matching elements, or the matching strings themselves when value = TRUE.",
    ),
    (
        "regexpr",
        "regexpr(pattern, text)",
        "The 1-based character position of the first match in each element, or -1 for no match, carrying the width of each match on the `match.length` attribute.",
    ),
    (
        "gregexpr",
        "gregexpr(pattern, text)",
        "A list with every match position for each element, each vector carrying its own `match.length`.",
    ),
    (
        "regexec",
        "regexec(pattern, text)",
        "A list whose element per text is the first match's position followed by each capture group's, with the widths on `match.length`.",
    ),
    (
        "regmatches",
        "regmatches(x, m)",
        "The matched substrings a regexpr or gregexpr result identifies — a character vector for the former, a list for the latter.",
    ),
    (
        "str",
        "str(object)",
        "A one-line-per-level sketch of a value: its type, dimensions, first few elements and attributes.",
    ),
    (
        "tempfile",
        "tempfile(pattern = \"file\", tmpdir = tempdir(), fileext = \"\")",
        "A path under the temporary directory that no file occupies yet — naming one does not create it.",
    ),
    (
        "readLines",
        "readLines(con)",
        "The lines of a file as a character vector, without their newlines; a final line with no newline is still a line.",
    ),
    (
        "file.exists",
        "file.exists(...)",
        "TRUE for each path that exists, vectorized over its argument.",
    ),
    (
        "unlink",
        "unlink(x, recursive = FALSE)",
        "Remove the named files, answering 0 when there is nothing left to remove (a missing path included) and 1 on failure. Invisible.",
    ),
    (
        "trimws",
        "trimws(x, which = \"both\")",
        "Strip whitespace from both ends, or only the left or right end.",
    ),
    (
        "startsWith",
        "startsWith(x, prefix)",
        "TRUE where the element begins with the prefix; both arguments recycle.",
    ),
    (
        "endsWith",
        "endsWith(x, suffix)",
        "TRUE where the element ends with the suffix; both arguments recycle.",
    ),
];

const APPLY: &[Entry] = &[
    (
        "lapply",
        "lapply(X, FUN, ...)",
        "Apply FUN to each element of X and return a list carrying X's names. Extra arguments are passed on to FUN.",
    ),
    (
        "sapply",
        "sapply(X, FUN, ...)",
        "lapply followed by simplification: results that are all length 1 collapse to a vector, results that are all length k become a k-by-n matrix, and ragged results stay a list. A character X supplies the names.",
    ),
    (
        "vapply",
        "vapply(X, FUN, FUN.VALUE)",
        "Apply FUN to each element and simplify like sapply. FUN.VALUE is accepted but the result is not type-checked against it, and extra arguments are not forwarded to FUN.",
    ),
    (
        "Map",
        "Map(f, ...)",
        "mapply(FUN = f, ..., SIMPLIFY = FALSE): apply f elementwise across several vectors or lists, recycled to the longest, and return the list of answers.",
    ),
    (
        "mapply",
        "mapply(FUN, ..., MoreArgs, SIMPLIFY = TRUE, USE.NAMES = TRUE)",
        "Call FUN once per position across the vectors or lists in ..., recycled to the longest (an empty one makes the answer empty). A tag names that argument in each call, MoreArgs adds the same arguments to every call, the answer is labelled from the first argument, and with SIMPLIFY it collapses the way sapply does.",
    ),
    (
        "Reduce",
        "Reduce(f, x, init, right = FALSE, accumulate = FALSE)",
        "Fold x with the binary function f, left to right by default. `init` seeds the accumulator, `right = TRUE` folds as f(element, acc), and `accumulate = TRUE` returns every intermediate value in original order.",
    ),
    (
        "Filter",
        "Filter(f, x)",
        "The elements of x for which f returns TRUE, keeping the corresponding names.",
    ),
    (
        "Find",
        "Find(f, x)",
        "The first element for which f returns TRUE, or NULL when none does.",
    ),
    (
        "Position",
        "Position(f, x)",
        "The 1-based position of the first element for which f returns TRUE, or integer NA.",
    ),
    (
        "split",
        "split(x, f)",
        "Split x into a list of groups, one per distinct value of f, named by the sorted levels.",
    ),
    (
        "unsplit",
        "unsplit(value, f, drop = FALSE)",
        "Reverse split(): a vector as long as f (as long as its first element when f is a list) of value[[1]]'s type, each group of value written back to the positions split(x, f) took it from, value recycled when it has fewer groups. A list of data frames is reassembled in the embedded R.",
    ),
    (
        "tapply",
        "tapply(X, INDEX, FUN, ...)",
        "Apply FUN, with the extra arguments, to each cell of the cross-classification of X by INDEX (one grouping vector, or a list of them). The answer is an array over the level grid, labelled by the levels and by INDEX's names, NA for an empty cell; it is a list array when FUN's answers are not all single atomic values. Elements with a missing group are dropped.",
    ),
    (
        "modifyList",
        "modifyList(x, val)",
        "Replace the elements of x whose names appear in `val` and append the names that do not.",
    ),
    (
        "rapply",
        "rapply(object, f)",
        "Apply f to every leaf of a nested list and flatten the result. Only R's how = \"unlist\" behaviour is implemented.",
    ),
    (
        "do.call",
        "do.call(what, args)",
        "Call a function — given as a value or as a name — with the elements of `args` as its arguments; the list's names become argument tags.",
    ),
    (
        "Negate",
        "Negate(f)",
        "A new function returning the logical negation of f's result. It is a runtime combinator, not a closure, so it has no body to print.",
    ),
    (
        "Vectorize",
        "Vectorize(FUN)",
        "A new function that applies FUN elementwise over its recycled arguments and simplifies the result.",
    ),
];

const MATRICES: &[Entry] = &[
    (
        "NROW",
        "NROW(x)",
        "The number of rows: dim(x)[1] for an array, else length(x), so a vector counts as one column.",
    ),
    (
        "NCOL",
        "NCOL(x)",
        "The number of columns: dim(x)[2] for a matrix, 0 for NULL, and 1 for any other vector.",
    ),
    (
        "prop.table",
        "prop.table(x, margin = NULL)",
        "x divided by its total, keeping names, dim and the other attributes. With margin = 1 each entry of a matrix is divided by its row total, with margin = 2 by its column total; margins of a higher-dimensional array are not supported.",
    ),
    (
        "proportions",
        "proportions(x, margin = NULL)",
        "The same function as prop.table, under its newer name.",
    ),
    (
        "matrix",
        "matrix(data = NA, nrow, ncol, dimnames = NULL, byrow = FALSE)",
        "Build a matrix, filling column-major and recycling `data` to nrow*ncol. Only one of nrow and ncol is needed. `byrow` must be passed by name — a fourth positional argument is not read as byrow.",
    ),
    (
        "t",
        "t(x)",
        "Transpose a matrix. A dimensionless vector is treated as a one-row matrix, which means t() of a plain vector returns an n-by-1 column where GNU R returns a 1-by-n row.",
    ),
    (
        "array",
        "array(data = NA, dim = length(data))",
        "Build an N-dimensional array, recycling `data` column-major to fill the shape. dimnames are not stored for rank 3 and above.",
    ),
    (
        "aperm",
        "aperm(a, perm)",
        "Permute the dimensions of an array; the default reverses them, which transposes a matrix.",
    ),
    (
        "apply",
        "apply(X, MARGIN, FUN)",
        "Apply FUN over the given margins of an array, walking the remaining dimensions for each slice. A slice keeps the labels of the dimensions it spans — its dim and dimnames at rank 2 or more, its names at rank 1 — so FUN sees a named row or a matrix. Several margins with scalar results reshape into an array, and the margins' labels land on the result.",
    ),
    (
        "diag",
        "diag(x)",
        "Three behaviours, as in R: the main diagonal of a matrix, the n-by-n identity for a length-1 number, and the diagonal matrix built from a longer vector.",
    ),
    (
        "lower.tri",
        "lower.tri(x, diag = FALSE)",
        "A logical matrix the shape of x (a vector counts as one column), TRUE strictly below the diagonal, and on it too with diag = TRUE.",
    ),
    (
        "upper.tri",
        "upper.tri(x, diag = FALSE)",
        "A logical matrix the shape of x, TRUE strictly above the diagonal, and on it too with diag = TRUE.",
    ),
    (
        "%*%",
        "x %*% y",
        "The matrix product, column-major, summed in the order the reference R's BLAS sums, so results agree with R to the last bit; NA propagates. A dimensionless vector is shaped by the other operand as R's do_matprod does (a row or a column, whichever conforms; two vectors make an inner product). The result keeps x's row names and y's column names. Non-conforming shapes are an error.",
    ),
    (
        "crossprod",
        "crossprod(x, y = x)",
        "t(x) %*% y, with R's do_matprod shaping, conformability error and dimnames (x's column names, y's column names).",
    ),
    (
        "tcrossprod",
        "tcrossprod(x, y = x)",
        "x %*% t(y), computed through the same matrix product; y defaults to x.",
    ),
    (
        "solve",
        "solve(a, b, tol = .Machine$double.eps)",
        "Solve a %*% x = b by LU with partial pivoting — the inverse when b is missing — as solve.default reaches LAPACK's dgesv. An exactly zero pivot and a reciprocal condition number below tol raise R's own errors; rownames of the result are colnames(a).",
    ),
    (
        "det",
        "det(x)",
        "The determinant of a square numeric matrix, as R's sign * exp(log-modulus) from the same LU factorisation.",
    ),
    (
        "determinant",
        "determinant(x, logarithm = TRUE)",
        "The modulus (as its log by default, carrying a logarithm attribute) and sign of the determinant, returned as a list of class \"det\".",
    ),
    (
        "arrayInd",
        "arrayInd(ind, .dim, .dimnames = NULL, useNames = FALSE)",
        "Linear indices as an integer matrix of per-dimension subscripts; useNames labels the rows from the first margin's names and the columns row/col or dim1, dim2, ….",
    ),
    (
        "slice.index",
        "slice.index(x, MARGIN)",
        "An integer array shaped like x holding each cell's index along MARGIN (a combined index for several margins).",
    ),
    (
        "outer",
        "outer(X, Y, FUN = \"*\")",
        "The outer product: X and Y are tiled to nx*ny and FUN is called once on the pair, so the result keeps FUN's own type — strings from paste0, logicals from ==. FUN may be a function or the name of an operator; the default \"*\" is the matrix product R computes, so it is always double.",
    ),
    (
        "%o%",
        "X %o% Y",
        "The infix spelling of outer(X, Y), with the default multiplication.",
    ),
    (
        "cbind",
        "cbind(..., deparse.level = 1)",
        "Bind the arguments as columns, recycling each to the tallest and promoting to the widest type present, as c() does. Column names come from an argument's tag, from a matrix argument's own dimnames, or from the deparsed argument expression — a bare symbol at deparse.level 1, any expression at 2, none at 0. NULL and zero-length arguments are dropped.",
    ),
    (
        "rbind",
        "rbind(..., deparse.level = 1)",
        "Bind the arguments as rows, with the same recycling, promotion and naming rules as cbind — so rbind(x, x) carries the rownames \"x\", \"x\". The deparsed labels come from the call site, so they are unavailable when the arguments arrive through `...`.",
    ),
    (
        "rowSums",
        "rowSums(x)",
        "Sum along every dimension but the first, keeping the first. A 1-D result takes that margin's dimnames as its names.",
    ),
    (
        "colSums",
        "colSums(x)",
        "Sum along the first dimension, keeping the rest — a vector for a matrix, a matrix for a 3-D array.",
    ),
    (
        "rowMeans",
        "rowMeans(x)",
        "The mean along every dimension but the first — one value per row of a matrix.",
    ),
    (
        "colMeans",
        "colMeans(x)",
        "The mean along the first dimension — one value per column of a matrix.",
    ),
];

const ENVIRONMENTS: &[Entry] = &[
    (
        "mget",
        "mget(x, envir)",
        "A list holding the value bound to each name in x, named by x, looked up in envir (with its enclosures) or the current environment chain. An unbound name is an error. R's mode, ifnotfound and inherits arguments are not read.",
    ),
    (
        "Sys.getenv",
        "Sys.getenv(x, unset = \"\", names = NA)",
        "The value of each environment variable named in x, or unset for one that is not set. The result is named by x when x has more than one element or names = TRUE. Calling it with no x, which lists the whole environment in R, is an error here.",
    ),
    ("getwd", "getwd()", "The absolute path of the working directory."),
    (
        "Sys.setenv",
        "Sys.setenv(...)",
        "Set each named argument as an environment variable of the running process, its value converted to a string. Every argument must be named. Returns a logical vector of TRUE, one per variable, invisibly.",
    ),
    (
        "exists",
        "exists(x, where, envir, inherits = TRUE)",
        "TRUE when the name is bound in the current environment chain or names a primitive. An environment given as envir, or positionally as where, is searched instead, together with its enclosures unless inherits = FALSE.",
    ),
    (
        "get",
        "get(x, pos, envir, inherits = TRUE)",
        "The value bound to the name, or the primitive of that name as a function value. An environment given as envir, or positionally as pos, is searched instead, together with its enclosures unless inherits = FALSE. An unbound name raises \"object 'x' not found\".",
    ),
    (
        "assign",
        "assign(x, value, pos, envir)",
        "Bind `value` to the name in the current environment, or in the environment given as envir or positionally as pos, and return it invisibly.",
    ),
    (
        "rm",
        "rm(..., list = character(), envir)",
        "Remove the bindings named by bare symbols or strings in ... and by list from the current environment, or from envir; enclosing environments are untouched. A name with no binding there warns \"object 'x' not found\". Returns NULL invisibly.",
    ),
    (
        "remove",
        "remove(..., list = character(), envir)",
        "The same function as rm.",
    ),
    (
        "environment",
        "environment()",
        "The current environment as a value. An argument is accepted and ignored — the environment of a given closure cannot be asked for.",
    ),
    (
        "new.env",
        "new.env(hash, parent)",
        "A fresh environment enclosed by parent, by default the environment the call is made from. Read and write it with $ and [[.",
    ),
    (
        "missing",
        "missing(x)",
        "TRUE when the caller supplied no argument for the formal. The argument is not evaluated, so missing(v) reads the name rather than its value and answers for an unsupplied parameter that has no binding at all. A formal with a default is bound before the body runs, and missing() still reports it as absent — the default is not something the caller supplied. R accepts the quoted spelling missing(\"v\") as well, and so does this.",
    ),
    (
        "nargs",
        "nargs()",
        "How many arguments the caller passed to the function this is called from, counting each element of ... separately and counting nothing for a formal left to its default. Zero at top level.",
    ),
    (
        "...length",
        "...length()",
        "The number of arguments in the calling function's ..., found from the calling frame outward; an error where there is no ... to look in.",
    ),
    (
        "...elt",
        "...elt(n)",
        "The n-th argument in ..., forced — the function form of ..1, ..2, …. A non-positive n or one past the end is an error.",
    ),
    (
        "...names",
        "...names()",
        "The tags of the arguments in ..., with \"\" for an untagged one, or NULL when none is tagged.",
    ),
    (
        "return",
        "return(value = NULL)",
        "Signal a return from the enclosing closure with the given value.",
    ),
    (
        "UseMethod",
        "UseMethod(generic, object)",
        "S3 dispatch: look for generic.class for each class of the object — the current call's first argument when none is given — then generic.default, and return what it returns.",
    ),
    (
        "NextMethod",
        "NextMethod()",
        "Continue S3 dispatch from inside a method: run the method for the next class in the vector UseMethod was dispatching on, falling back to the primitive behind the generic when no further method is defined.",
    ),
    (
        "tryCatch",
        "tryCatch(expr, ..., finally)",
        "Evaluate expr with handlers installed. Each named argument other than finally is a handler for the condition class of that name (error, warning, message, condition); if expr signals a matching condition, that handler is called with the condition object and its value becomes the result. finally is evaluated on the way out either way.",
    ),
    (
        "withCallingHandlers",
        "withCallingHandlers(expr, ...)",
        "Evaluate expr with handlers installed, spelled as tryCatch is, but without unwinding: a matching handler runs at the point the condition was signalled, and when it returns the search continues outward and evaluation resumes there. A handler ends the search by transferring to a restart, which is what invokeRestart(\"muffleWarning\") does.",
    ),
    (
        "try",
        "try(expr, silent = FALSE)",
        "Evaluate expr, returning its value, or on error an invisible character string of class \"try-error\" carrying the message (also printed to stderr unless silent is TRUE) rather than aborting.",
    ),
    (
        "on.exit",
        "on.exit(expr, add = FALSE)",
        "Register expr to be evaluated when the enclosing closure exits, whether it returns normally or unwinds with an error. Without add = TRUE a later on.exit replaces the registered expression instead of joining it.",
    ),
    (
        "geterrmessage",
        "geterrmessage()",
        "The last error message: the text of an error raised from a message, as it was raised (even one a tryCatch handler then took), or the whole \"Error in <call> : …\" line try() made for its last error; \"\" before any. A condition object signalled with stop(cond) and caught leaves it unchanged.",
    ),
    (
        "conditionMessage",
        "conditionMessage(c)",
        "The message string carried by a condition object.",
    ),
    (
        "conditionCall",
        "conditionCall(c)",
        "The call a condition was signalled from. rlang records no call, so this is always NULL.",
    ),
    (
        "simpleError",
        "simpleError(message)",
        "Build a condition object of class c(\"simpleError\", \"error\", \"condition\").",
    ),
    (
        "simpleWarning",
        "simpleWarning(message)",
        "Build a condition object of class c(\"simpleWarning\", \"warning\", \"condition\").",
    ),
    (
        "simpleMessage",
        "simpleMessage(message)",
        "Build a condition object of class c(\"simpleMessage\", \"message\", \"condition\").",
    ),
    (
        "simpleCondition",
        "simpleCondition(message)",
        "Build a condition object of class c(\"simpleCondition\", \"condition\").",
    ),
    (
        "errorCondition",
        "errorCondition(message, ..., class = character(), call = NULL)",
        "Build an error condition object: a list of message, call and any further named fields, of class c(class, \"error\", \"condition\"). stop() on it signals the object itself, so a handler sees every field.",
    ),
    (
        "warningCondition",
        "warningCondition(message, ..., class = character(), call = NULL)",
        "Build a warning condition object: a list of message, call and any further named fields, of class c(class, \"warning\", \"condition\").",
    ),
    (
        "signalCondition",
        "signalCondition(cond)",
        "Signal a condition object: every enclosing calling handler for one of its classes runs in place, an enclosing tryCatch for one of them unwinds, and with nothing in scope the call returns NULL and evaluation continues.",
    ),
    (
        "withRestarts",
        "withRestarts(expr, ...)",
        "Evaluate expr with restarts established. Each named argument is a restart: a handler function, a list(handler =, description =), or a description string. If invokeRestart transfers to one, its handler's value becomes the value of this call.",
    ),
    (
        "invokeRestart",
        "invokeRestart(r, ...)",
        "Transfer control to the restart named (or held) by r, passing the remaining arguments to its handler. Does not return: cleanups registered with on.exit and tryCatch's finally still run on the way out, but no handler catches the transfer.",
    ),
    (
        "computeRestarts",
        "computeRestarts(cond = NULL)",
        "The restarts currently established, innermost first, ending with the abort restart the evaluator always provides. warning() establishes muffleWarning around itself and message() establishes muffleMessage.",
    ),
    (
        "restartDescription",
        "restartDescription(r)",
        "The description string a restart was established with, or NULL when it carries none.",
    ),
    (
        "isRestart",
        "isRestart(x)",
        "Whether x is a restart object, i.e. inherits from class \"restart\".",
    ),
    (
        "Recall",
        "Recall(...)",
        "Re-invoke the closure currently executing, which lets an anonymous recursive function call itself.",
    ),
    (
        "factor",
        "factor(x, levels, ordered = FALSE)",
        "Integer codes plus a `levels` attribute and class \"factor\" — c(\"ordered\", \"factor\") when ordered. Levels default to the sorted distinct values; values outside `levels` become NA. R's `labels` argument is not accepted.",
    ),
    (
        "levels",
        "levels(x)",
        "The `levels` attribute of a factor, or NULL for a value that carries none.",
    ),
    ("nlevels", "nlevels(x)", "The number of levels a factor carries; 0 for a value with no levels attribute."),
    (
        "droplevels",
        "droplevels(x)",
        "Drop the levels that no longer occur and renumber the codes to match.",
    ),
    (
        "ordered",
        "ordered(x, levels, labels)",
        "factor(x, levels, labels, ordered = TRUE): an ordered factor whose levels carry a < order.",
    ),
    (
        "as.ordered",
        "as.ordered(x)",
        "x if it is already an ordered factor, else x as an ordered factor over its own levels (or its sorted distinct values).",
    ),
    (
        "is.ordered",
        "is.ordered(x)",
        "TRUE when x is an ordered factor.",
    ),
    (
        "is.object",
        "is.object(x)",
        "TRUE when x carries a class attribute.",
    ),
    (
        "is",
        "is(object, class2)",
        "Whether object is of class class2: its class vector, plus the basic relations R's S4 table records (an integer or double is a \"numeric\", a function is a \"function\"). With one argument, the class vector.",
    ),
    (
        "relevel",
        "relevel(x, ref)",
        "An unordered factor with ref (a level name or position) moved to the front of its levels and the codes renumbered to match. Errors on a non-factor or an ordered factor, and on a ref that is not a level.",
    ),
    (
        "interaction",
        "interaction(..., sep = \".\", drop = FALSE)",
        "The factor crossing its arguments: one level per combination, the first factor varying fastest, labelled by joining the parts with sep. drop = TRUE removes the combinations that do not occur; an NA in any argument is NA.",
    ),
    (
        "margin.table",
        "margin.table(x, margin = NULL)",
        "Sums of an array over every dimension not in margin, keeping the table class and the kept dimensions' dimnames (and their names). With no margin, the grand total.",
    ),
    (
        "marginSums",
        "marginSums(x, margin = NULL)",
        "margin.table under its newer name.",
    ),
    (
        "max.col",
        "max.col(m, ties.method = \"random\")",
        "The column holding each row's maximum, as an integer vector; NA for a row containing NA. ties.method \"first\" and \"last\" pick the leftmost or rightmost of equal maxima; \"random\" takes the first, which is R's answer whenever the maximum is unique.",
    ),
    (
        "sort.int",
        "sort.int(x, decreasing = FALSE, na.last = NA, index.return = FALSE)",
        "sort for a vector, with index.return = TRUE answering list(x = sorted, ix = the permutation that sorted it).",
    ),
    (
        "cut",
        "cut(x, breaks, labels)",
        "Bin numeric x into a factor of right-closed intervals (a, b]. A single-number `breaks` means that many equal-width bins over the range widened by a thousandth, exactly as R computes them; default labels use R's dig.lab = 3.",
    ),
    (
        "table",
        "table(x)",
        "Counts of each level of a factor, or of each distinct value in sorted order, named and classed \"table\". Cross-tabulation of two or more arguments is not implemented — extra arguments are ignored.",
    ),
    (
        "library",
        "library(package)",
        "Load a CRAN package inside the embedded GNU R and return NULL invisibly. Needs an R installation; the package's functions then reach rlang through the bridge.",
    ),
    (
        "require",
        "require(package)",
        "Load a package like library, returning a logical rather than loading invisibly.",
    ),
    (
        "requireNamespace",
        "requireNamespace(package)",
        "Load the package's namespace in the embedded R, returning a logical.",
    ),
    (
        "loadNamespace",
        "loadNamespace(package)",
        "Load the package's namespace in the embedded R.",
    ),
    (
        "suppressMessages",
        "suppressMessages(expr)",
        "Evaluate expr with every message discarded, and return its value. The message is muffled at the point it is signalled, so expr carries on from there.",
    ),
    (
        "suppressWarnings",
        "suppressWarnings(expr)",
        "Evaluate expr with every warning discarded, and return its value. The warning is muffled at the point it is signalled, so expr carries on from there.",
    ),
    (
        "suppressPackageStartupMessages",
        "suppressPackageStartupMessages(expr)",
        "Return expr; package startup output is suppressed by the bridge's own suppressMessages wrapper around the load.",
    ),
    (
        ".rlang_formula",
        ".rlang_formula(src)",
        "Internal, not user surface: what the compiler emits for `lhs ~ rhs`. It takes the deparsed R source and builds a real formula object in the embedded R, so lm, glm and aggregate receive one intact.",
    ),
    (
        ".rlang_quote",
        ".rlang_quote(src, splice)",
        "Internal, not user surface: what the compiler emits for quote(x) and bquote(x). The argument is never compiled; its deparse rides across as a string and this parses it back into the expression the caller wrote. splice = TRUE (bquote) replaces each `.(e)` with the value of e in the caller's frame.",
    ),
    (
        "quote",
        "quote(expr)",
        "The expression itself, unevaluated. quote(1) is the number, quote(x) is a name, and a compound expression is a call — R's own three outcomes. The result prints as its source, deparses back to it, and takes apart with length and as.character.",
    ),
    (
        "bquote",
        "bquote(expr)",
        "The expression, unevaluated like quote, except that each `.(e)` inside it is replaced by the value of e evaluated in the calling frame: y <- 5; bquote(a + .(y)) is a + 5. A spliced call keeps its own grouping, so splicing a + b into .(z) * 2 prints (a + b) * 2, as R's deparser does. The where and splice arguments are not read.",
    ),
    (
        "as.name",
        "as.name(x)",
        "The name (symbol) whose text is x — R's as.symbol. as.name(\"foo\") prints as foo and has class \"name\".",
    ),
    (
        "as.symbol",
        "as.symbol(x)",
        "The name (symbol) whose text is x, identical to as.name.",
    ),
    (
        "is.call",
        "is.call(x)",
        "Whether x is an unevaluated call — TRUE for quote(f(1)) and for quote(a + b), FALSE for a name or a constant.",
    ),
    (
        "is.name",
        "is.name(x)",
        "Whether x is a name — TRUE for quote(x) and as.name(\"x\"), FALSE for a call or a constant. R's is.symbol.",
    ),
    (
        "is.symbol",
        "is.symbol(x)",
        "Whether x is a name, identical to is.name.",
    ),
    (
        "sys.call",
        "sys.call(which = 0)",
        "The call that made the frame now running, as a language object. NULL at top level. which = n > 0 names frame n from the outermost, which = -n the frame n generations above this one; one that does not exist is the error \"not that many frames on the stack\". A sys.call() written as an argument reports the frame whose body wrote it, not the one it is being passed to, the way R's promise does.",
    ),
    (
        "sys.calls",
        "sys.calls()",
        "The calls of every active function frame, outermost first, as a list of language objects; NULL at top level.",
    ),
    (
        "match.call",
        "match.call()",
        "The current call with every argument that binds to a named formal carrying that formal's name, and the arguments in formal order. Arguments absorbed by ... keep their own tag and follow.",
    ),
    (
        "match.arg",
        "match.arg(arg, choices, several.ok = FALSE)",
        "Match arg against choices: an exact match, else a unique prefix. Without choices they are the default of the formal arg names in the calling function, evaluated there, and an argument still equal to that whole default yields its first element. several.ok = TRUE accepts several arguments; no match raises 'arg' should be one of the choices.",
    ),
    (
        "formals",
        "formals(fun)",
        "The formals of a closure as a named list: each default as the expression written, and the empty symbol for a formal without one. Without fun, the calling function's; a string names the function; a primitive has none (NULL).",
    ),
    (
        "formalArgs",
        "formalArgs(def)",
        "The names of a closure's formals, ... included.",
    ),
    (
        "args",
        "args(name)",
        "A function with the formals of name and a NULL body, enclosed by the global environment — what printing it shows is the argument list. For a primitive, the formals R keeps in its stand-in table (args(sum) is function (..., na.rm = FALSE) NULL); NULL for a primitive with none, such as if. A string names the function.",
    ),
    (
        "body",
        "body(fun)",
        "The body of a closure as the language object written: a call, a symbol or a constant. Without fun, the calling function's; a string names the function; a primitive has none (NULL).",
    ),
    (
        "sys.function",
        "sys.function(which = 0)",
        "The closure being executed — the function itself, not its call. which selects a frame as sys.call does.",
    ),
    (
        "sys.nframe",
        "sys.nframe()",
        "The number of function frames up to and including the one sys.nframe() was written in: 0 at top level, 1 in a function called from there. A base function that is a closure in R counts as a frame (print(f()) runs f two deep), as R counts it.",
    ),
    (
        "eval",
        "eval(expr, envir)",
        "Run an expression that was held rather than evaluated. With envir it runs in that environment, reading and binding there; without one it runs where the caller stands, so eval(quote(v + 1)) sees the caller's v and eval(quote(w <- 7)) binds there. A value that is not an expression is already evaluated and comes back unchanged. The result keeps the visibility the expression left it with.",
    ),
    (
        ".rlang_substitute",
        ".rlang_substitute(expr, env)",
        "Internal, not user surface: what the compiler emits for substitute(x). The first argument is never compiled; its deparse rides across as a string and this parses it back before substituting.",
    ),
    (
        "substitute",
        "substitute(expr, env)",
        "The expression with every symbol the current frame accounts for replaced by what it stands for: a formal the caller supplied stands for the expression they wrote, anything else bound in the frame stands for its value, and a symbol neither accounts for is left alone. `...` splices back into the arguments it stands for. At top level nothing is substituted, however much is bound. With env, that table is consulted instead of the frame.",
    ),
    (
        "parent.frame",
        "parent.frame(n = 1)",
        "The environment of the frame that called this one, so a function can read or write the caller's variables. A builtin pushes no frame, so what this finds is the calling closure. Past the outermost frame it answers the global environment, as R does.",
    ),
    (
        "ls",
        "ls(envir, all.names = FALSE)",
        "The names bound in an environment, sorted, defaulting to the current one. Names beginning with a dot are left out unless all.names is TRUE.",
    ),
    (
        "objects",
        "objects(envir, all.names = FALSE)",
        "The names bound in an environment, identical to ls.",
    ),
    (
        "globalenv",
        "globalenv()",
        "The global environment — where a top-level assignment binds.",
    ),
    (
        "topenv",
        "topenv(envir, matchThisEnv)",
        "The first environment up envir's enclosure chain that is matchThisEnv or a top level. envir defaults to the calling environment; rlang's one top level is the global environment.",
    ),
    (
        "environmentName",
        "environmentName(env)",
        "\"R_GlobalEnv\" for the global environment and the empty string for any other, which is what R answers for an anonymous frame.",
    ),
    (
        "evalq",
        "evalq(expr, envir)",
        "Run an expression, identical to eval here: rlang's arguments are eager, so there is nothing left to quote against by the time either is called.",
    ),
];

/// The operators, which R makes ordinary functions — that is what lets
/// ``Reduce(`+`, 1:4)`` and ``sapply(xs, `[`, 1)`` work. Each entry's signature
/// shows the infix form and the backtick call form.
pub const OPERATORS: &[Entry] = &[
    (
        "+",
        "x + y\n`+`(x, y)",
        "Addition, recycled elementwise to the longer operand. Two integer or logical operands give an integer; names and dim are carried from the longer side. Unary + returns its argument unchanged.",
    ),
    (
        "-",
        "x - y\n`-`(x, y)",
        "Subtraction, with the same recycling and integer rule as +. Called with one argument it negates.",
    ),
    (
        "*",
        "x * y\n`*`(x, y)",
        "Multiplication, recycled elementwise; integer times integer stays integer.",
    ),
    (
        "/",
        "x / y\n`/`(x, y)",
        "Division, always producing a double. Division by zero yields Inf, -Inf or NaN rather than an error.",
    ),
    (
        "^",
        "x ^ y\n`^`(x, y)",
        "Exponentiation, recycled elementwise and always producing a double.",
    ),
    (
        "%%",
        "x %% y\n`%%`(x, y)",
        "Remainder taking the sign of the divisor: -7 %% 3 is 2 and 7 %% -3 is -2. Computed as an exact fmod against the stored divisor, so 10 %% 0.04 is 0.04; a zero divisor gives NaN.",
    ),
    (
        "%/%",
        "x %/% y\n`%/%`(x, y)",
        "Integer division, kept consistent with %% as (x - x %% y) / y. A zero divisor or non-finite dividend gives x / y directly, so 49 %/% 0 is Inf.",
    ),
    (
        "==",
        "x == y\n`==`(x, y)",
        "Elementwise equality, recycled. If either side is character both compare as strings. NA or NaN on either side gives NA.",
    ),
    ("!=", "x != y\n`!=`(x, y)", "Elementwise inequality, with the same recycling and NA rules as ==."),
    ("<", "x < y\n`<`(x, y)", "Elementwise less-than; character operands compare lexically."),
    (">", "x > y\n`>`(x, y)", "Elementwise greater-than, with the same recycling and NA rules as the other comparisons."),
    ("<=", "x <= y\n`<=`(x, y)", "Elementwise less-than-or-equal, with the same recycling and NA rules as the other comparisons."),
    (">=", "x >= y\n`>=`(x, y)", "Elementwise greater-than-or-equal, with the same recycling and NA rules as the other comparisons."),
    (
        "&",
        "x & y\n`&`(x, y)",
        "Elementwise logical AND with R's three-valued logic: NA & FALSE is FALSE, because the answer is decided regardless of the missing value.",
    ),
    (
        "|",
        "x | y\n`|`(x, y)",
        "Elementwise logical OR with three-valued logic: NA | TRUE is TRUE.",
    ),
    (
        "!",
        "!x\n`!`(x)",
        "Elementwise logical negation, coercing its argument first; NA stays NA.",
    ),
    (
        ":",
        "from:to\n`:`(from, to)",
        "The sequence from `from` to `to` stepping by one, descending when from is greater. The result is integer when both ends are whole numbers, otherwise a double.",
    ),
    (
        "[",
        "x[i]\nx[i, j]\n`[`(x, i)",
        "Subsetting, which keeps the container type and the names. Positive positions select, negative positions drop, a logical mask recycles, and a character subscript matches names. Given as many subscripts as an array has dimensions it selects a slice and drops the length-1 dimensions; a character subscript against dimnames selects nothing, unlike R.",
    ),
    (
        "[[",
        "x[[i]]\n`[[`(x, i)",
        "Extract exactly one element, by position or by name. A name that is not present yields NULL, an out-of-range position raises \"subscript out of bounds\", and on an environment it reads a binding.",
    ),
    (
        "$",
        "x$name\n`$`(x, name)",
        "The element of a named list or vector bound to `name`, or NULL when there is none. rlang also answers on an atomic vector, where R raises \"$ operator is invalid for atomic vectors\".",
    ),
    (
        "%in%",
        "x %in% table\n`%in%`(x, table)",
        "TRUE for each element of x that occurs in table — match(x, table, nomatch = 0) > 0. Factors match on their labels. As a function value it is callable like any other, as in Reduce(`%in%`, …).",
    ),
    (
        "if",
        "if (cond) yes else no\n`if`(cond, yes, no)",
        "Evaluate yes when cond is TRUE, else no (invisible NULL when there is no else). cond must be one non-NA logical or number: a longer one, NA, an empty one or a string with no logical reading is an error. As a function value only the chosen branch is evaluated.",
    ),
    (
        "for",
        "for (var in seq) body\n`for`(var, seq, body)",
        "Bind var to each element of seq in turn and evaluate body; the value is invisible NULL. Callable as a function value only when written as a call, since it takes the loop variable as a name.",
    ),
    (
        "while",
        "while (cond) body\n`while`(cond, body)",
        "Evaluate body while cond is TRUE; the value is invisible NULL. Callable as a function value only when written as a call.",
    ),
    (
        "repeat",
        "repeat body\n`repeat`(body)",
        "Evaluate body until a break; the value is invisible NULL. Callable as a function value only when written as a call.",
    ),
    (
        "{",
        "{ expr1; expr2; … }\n`{`(expr1, expr2, …)",
        "Evaluate each expression in turn; the value is the last one's (NULL for none).",
    ),
    (
        "(",
        "(expr)\n`(`(expr)",
        "The value of expr, made visible: (x <- 5) prints where x <- 5 does not.",
    ),
    (
        "<-",
        "name <- value\n`<-`(name, value)",
        "Bind value to name in the current environment (or replace part of an object through a replacement function, x[i] <- v); the value is value, invisibly. Callable as a function value only when written as a call, since it takes its target unevaluated.",
    ),
    (
        "<<-",
        "name <<- value\n`<<-`(name, value)",
        "Bind value to name in the nearest enclosing environment that has it, else the global one; invisible. Callable as a function value only when written as a call.",
    ),
    (
        "=",
        "name = value",
        "Assignment, as <-, where it is not an argument tag. Callable as a function value only when written as a call.",
    ),
    (
        "&&",
        "x && y\n`&&`(x, y)",
        "Scalar AND: FALSE when x is FALSE without evaluating y, else y, with NA when x is NA unless y is FALSE. Each operand must be one logical or number; a longer one is an error.",
    ),
    (
        "||",
        "x || y\n`||`(x, y)",
        "Scalar OR: TRUE when x is TRUE without evaluating y, else y, with NA when x is NA unless y is TRUE. Each operand must be one logical or number; a longer one is an error.",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtins;
    use std::collections::HashSet;

    #[test]
    fn every_primitive_is_documented() {
        let documented: HashSet<&str> = entries().map(|(n, _, _)| *n).collect();
        let missing: Vec<&&str> = builtins::PRIMITIVES
            .iter()
            .filter(|n| !documented.contains(**n))
            .collect();
        assert!(missing.is_empty(), "undocumented primitives: {missing:?}");
    }

    #[test]
    fn no_entry_documents_a_name_the_runtime_lacks() {
        let stray: Vec<&str> = entries()
            .map(|(n, _, _)| *n)
            .filter(|n| !builtins::PRIMITIVES.contains(n))
            .collect();
        assert!(
            stray.is_empty(),
            "documented but not implemented: {stray:?}"
        );
    }

    #[test]
    fn every_operator_is_documented_exactly_once() {
        let documented: Vec<&str> = OPERATORS.iter().map(|(n, _, _)| *n).collect();
        let mut sorted = documented.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), documented.len(), "duplicate operator entry");
        for op in builtins::OPERATORS {
            assert!(documented.contains(op), "undocumented operator: {op}");
        }
        assert_eq!(documented.len(), builtins::OPERATORS.len());
    }

    #[test]
    fn no_primitive_is_documented_twice() {
        let mut names: Vec<&str> = entries().map(|(n, _, _)| *n).collect();
        let total = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(
            names.len(),
            total,
            "a primitive is documented in two chapters"
        );
    }

    #[test]
    fn signatures_and_descriptions_are_present() {
        for (name, sig, doc) in entries().chain(OPERATORS.iter()) {
            assert!(!sig.trim().is_empty(), "{name}: empty signature");
            assert!(
                doc.trim().len() > 20,
                "{name}: description is too short to be a description"
            );
            assert!(
                doc.trim_end().ends_with('.'),
                "{name}: description is not a sentence"
            );
        }
    }

    #[test]
    fn a_primitive_signature_starts_with_its_own_name() {
        // The infix operators are the exception: their signature leads with an
        // operand, not the name.
        for (name, sig, _) in entries() {
            if *name == "%*%" || *name == "%o%" {
                continue;
            }
            assert!(
                sig.starts_with(name),
                "{name}: signature `{sig}` does not name the function"
            );
        }
    }

    #[test]
    fn bracketing_operators_anchor_by_name() {
        assert_eq!(slug("("), "paren");
        assert_eq!(slug("{"), "brace");
        assert_eq!(slug("[["), "bracketbracket");
        assert_eq!(slug("<-"), "ltminus");
    }

    #[test]
    fn anchors_are_unique() {
        let mut slugs: Vec<String> = entries()
            .chain(OPERATORS.iter())
            .map(|(n, _, _)| slug(n))
            .collect();
        let total = slugs.len();
        assert!(
            slugs.iter().all(|s| !s.is_empty()),
            "an entry has an empty anchor"
        );
        slugs.sort();
        slugs.dedup();
        assert_eq!(slugs.len(), total, "two entries share an HTML anchor");
    }
}
