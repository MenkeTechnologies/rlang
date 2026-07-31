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
/// dropped — otherwise `%*%`, `[` and `$` would all anchor to the empty string.
pub fn slug(name: &str) -> String {
    let mut out = String::with_capacity(name.len() * 4);
    for ch in name.chars() {
        let piece = match ch {
            c if c.is_ascii_alphanumeric() => {
                out.push(c.to_ascii_lowercase());
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
        "inherits(x, what)",
        "TRUE when any string in `what` appears in class(x). R's `which` argument is accepted and ignored — the result is always a single logical.",
    ),
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
        "The `dimnames` attribute — a list of one label vector per dimension — or NULL. Labels are stored for 2-D only, and there is no dimnames<- replacement: matrix(dimnames = ) and the cbind/rbind seam labels are the ways to attach them.",
    ),
];

const OUTPUT: &[Entry] = &[
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
        "print(x, digits)",
        "Print x in R's default layout and return it invisibly. `digits` overrides the significant-digit setting for this one call. This is the internal print: a user-defined print.myclass method is never reached.",
    ),
    (
        "cat",
        "cat(..., sep = \" \")",
        "Write every argument's elements with no quotes and no trailing newline, joined by `sep`. A separator containing a newline also ends the output with one, matching R.",
    ),
    (
        "message",
        "message(...)",
        "Write the concatenated arguments to stderr and return NULL invisibly. No condition is signalled — nothing can catch a message.",
    ),
    (
        "warning",
        "warning(...)",
        "Write \"Warning message:\" and the concatenated text to stderr and continue. Warnings are never collected or deferred.",
    ),
    (
        "stop",
        "stop(...)",
        "Abort with the concatenated arguments as the error message. Without a condition system there is nothing to catch it: the script exits with `Rscript: <message>` on stderr and status 1.",
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
        "paste",
        "paste(..., sep = \" \", collapse = NULL)",
        "Join the arguments elementwise with recycling to the longest, separated by `sep`. A non-NULL `collapse` then joins the result into a single string. NA elements render as \"NA\".",
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
        "deparse(expr)",
        "R source text for a value: a run of consecutive integers deparses as `a:b`, other integers carry the L suffix, strings are quoted and escaped, and longer vectors are wrapped in c(). Because arguments are evaluated eagerly, this deparses the value, never the unevaluated expression.",
    ),
    (
        "format",
        "format(x, nsmall = 0, digits, big.mark = \"\")",
        "Format to character with a common decimal count across the vector, then pad to a common width — numbers right-justified, strings left. `digits` is the minimum significant digits, `nsmall` the minimum decimals. R's `width`, `justify` and per-call scientific control are not implemented.",
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
        "C-style formatting, vectorized over both the format and the arguments. Supports %d %i %s %f %e %E %g %G %x %X %o and %%, with the flags, field width and precision — including zero-padding after the sign, as C does.",
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
        "seq(from, to, by, length.out)",
        "Build a sequence. With one argument it is seq_len(from). The third positional argument is `by`, so seq(0, 1, 0.25) steps by a quarter; `length.out` computes the step instead. The result stays integer when every element and the step are whole.",
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
        "append(x, values)",
        "Concatenate values onto the end of x, with c()'s type promotion. R's `after` argument is accepted and ignored — insertion is always at the end.",
    ),
];

const ORDERING: &[Entry] = &[
    (
        "sort",
        "sort(x, decreasing = FALSE, index.return = FALSE)",
        "The sorted values with NA and NaN dropped, names carried along. With index.return = TRUE the result is a list of $x and the ordering $ix.",
    ),
    (
        "order",
        "order(x, decreasing = FALSE)",
        "The 1-based permutation that sorts x, with NA positions dropped. Only one sort key is supported.",
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
        "rank(x)",
        "Ranks with tied values sharing the average of the slots they occupy — R's default ties.method = \"average\". The result is always a double vector.",
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
        "sum",
        "sum(..., na.rm = FALSE)",
        "The total over every element of every argument. The result stays integer when all arguments are integer or logical, otherwise it is a double. Integer overflow widens to a double instead of producing NA.",
    ),
    (
        "prod",
        "prod(..., na.rm = FALSE)",
        "The product of every element of every argument, always a double.",
    ),
    (
        "mean",
        "mean(x, na.rm = FALSE)",
        "The arithmetic mean of one vector. An empty vector gives NaN, and without na.rm a single NA or NaN makes the whole result NA. R's `trim` argument is accepted and ignored.",
    ),
    (
        "median",
        "median(x, na.rm = FALSE)",
        "The middle value of the sorted data, or the mean of the two middle values at even length.",
    ),
    (
        "quantile",
        "quantile(x, probs = c(0, 0.25, 0.5, 0.75, 1), names = TRUE)",
        "Sample quantiles by R's default type 7 — linear interpolation at h = (n-1)p — named with the percent labels unless names = FALSE. Other `type` values are not supported.",
    ),
    (
        "cor",
        "cor(x, y)",
        "The Pearson correlation of two equal-length numeric vectors. Zero variance in either vector, or fewer than two pairs, gives NA rather than NaN. Spearman and Kendall are not implemented.",
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
        "var(x, na.rm = FALSE)",
        "The sample variance with the n-1 denominator, computed in the same two-pass form as R's C code so the last printed digit agrees. A covariance matrix from two arguments is not supported.",
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
        "Round half to even on the true decimal value rather than on x * 10^digits, so round(0.15, 1) is 0.1 and round(2.675, 2) is 2.67 exactly as in R. Negative digits round to tens, hundreds and so on.",
    ),
    (
        "signif",
        "signif(x, digits = 6)",
        "Round to `digits` significant figures, half to even: signif(123.456, 2) is 120 and signif(0.0034219, 3) is 0.00342.",
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
        "tabulate",
        "tabulate(bin, nbins = max(bin))",
        "The count of each integer 1..nbins occurring in `bin`. Values outside the range are ignored.",
    ),
    (
        "findInterval",
        "findInterval(x, vec)",
        "For each element of x, how many breakpoints in `vec` are less than or equal to it — the index of the interval it falls in.",
    ),
];

const PREDICATES: &[Entry] = &[
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
        "is.numeric",
        "is.numeric(x)",
        "TRUE for a double or integer vector. A factor is stored as an integer vector with attributes, so this answers TRUE for one where GNU R answers FALSE.",
    ),
    ("is.character", "is.character(x)", "TRUE for a character vector, whatever attributes it carries."),
    ("is.logical", "is.logical(x)", "TRUE for a logical vector, whatever attributes it carries."),
    ("is.list", "is.list(x)", "TRUE for a list, including a list carrying a class attribute."),
    (
        "is.function",
        "is.function(x)",
        "TRUE for a closure, a primitive used as a value, or a Negate/Vectorize combinator.",
    ),
    (
        "is.vector",
        "is.vector(x)",
        "TRUE for any atomic vector or list. Attributes are not inspected, so an object carrying a class still answers TRUE where GNU R answers FALSE.",
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
        "TRUE when both values have the same internal type, the same names and the same elements, comparing lists recursively. Attributes other than names are not compared, so a classed value can be identical to a bare one.",
    ),
    (
        "ifelse",
        "ifelse(test, yes, no)",
        "Elementwise selection: the matching element of `yes` where test is TRUE, of `no` where it is FALSE, and NA where test is NA. Both branches recycle.",
    ),
];

const STRINGS: &[Entry] = &[
    (
        "nchar",
        "nchar(x)",
        "The number of characters in each string — Unicode code points, not bytes.",
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
    ("toupper", "toupper(x)", "Each string converted to upper case, using Unicode case mapping."),
    ("tolower", "tolower(x)", "Each string converted to lower case, using Unicode case mapping."),
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
        "strrep",
        "strrep(x, times)",
        "Repeat each string `times` times, with both arguments recycled.",
    ),
    (
        "encodeString",
        "encodeString(x)",
        "Escape each string the way R prints it — backslash escapes for quotes, tabs and newlines.",
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
        "regmatches",
        "regmatches(x, m)",
        "The matched substrings a regexpr or gregexpr result identifies — a character vector for the former, a list for the latter.",
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
        "Apply f elementwise across several lists, stopping at the shortest, and return a list.",
    ),
    (
        "mapply",
        "mapply(FUN, ...)",
        "Like Map, but the arguments recycle to the longest and the result is simplified, matching R's default SIMPLIFY = TRUE.",
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
        "tapply",
        "tapply(X, INDEX, FUN)",
        "Apply FUN to each group of X defined by INDEX, returning one simplified value per level, named by level.",
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
        "Apply FUN over the given margins of an array, walking the remaining dimensions for each slice. A slice of rank 2 or more keeps its dim, so FUN sees a matrix. Several margins with scalar results reshape into an array.",
    ),
    (
        "diag",
        "diag(x)",
        "Three behaviours, as in R: the main diagonal of a matrix, the n-by-n identity for a length-1 number, and the diagonal matrix built from a longer vector.",
    ),
    (
        "%*%",
        "x %*% y",
        "The matrix product, column-major. A dimensionless vector conforms as a row on the left and as a column on the right. Non-conforming shapes return NA rather than raising.",
    ),
    (
        "crossprod",
        "crossprod(x, y = x)",
        "t(x) %*% y, computed through the same matrix product.",
    ),
    (
        "tcrossprod",
        "tcrossprod(x, y = x)",
        "x %*% t(y), computed through the same matrix product; y defaults to x.",
    ),
    (
        "outer",
        "outer(X, Y, FUN = \"*\")",
        "The outer product: X and Y are tiled to nx*ny and FUN is called once on the pair, so the result keeps FUN's own type — strings from paste0, logicals from ==. FUN may be a function or the name of an operator.",
    ),
    (
        "%o%",
        "X %o% Y",
        "The infix spelling of outer(X, Y), with the default multiplication.",
    ),
    (
        "cbind",
        "cbind(...)",
        "Bind the arguments as columns, recycling each to the tallest. Argument tags become column names. Every input is coerced to double, so character arguments do not survive — that differs from R, which would build a character matrix.",
    ),
    (
        "rbind",
        "rbind(...)",
        "Bind the arguments as rows, with the same recycling, naming and double-coercion rules as cbind. Deparse-derived row labels are not synthesised, because builtins receive values rather than expressions.",
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
        "exists",
        "exists(x)",
        "TRUE when the name is bound in the current environment chain or names a primitive. R's `where`, `envir` and `inherits` arguments are not accepted.",
    ),
    (
        "get",
        "get(x)",
        "The value bound to the name, or the primitive of that name as a function value. An unbound name raises \"object 'x' not found\".",
    ),
    (
        "assign",
        "assign(x, value)",
        "Bind `value` to the name in the current environment and return it invisibly. Assignment into another environment is not supported.",
    ),
    (
        "environment",
        "environment()",
        "The current environment as a value. An argument is accepted and ignored — the environment of a given closure cannot be asked for.",
    ),
    (
        "new.env",
        "new.env()",
        "A fresh environment whose parent is the global environment. Read and write it with $ and [[.",
    ),
    (
        "missing",
        "missing(x)",
        "TRUE when the name is not bound in the current call frame. The argument is evaluated first, so missing(v) on an unsupplied parameter raises \"object 'v' not found\"; the quoted form missing(\"v\") is the portable spelling and works in both engines.",
    ),
    (
        "return",
        "return(value = NULL)",
        "Signal a return from the enclosing closure with the given value.",
    ),
    (
        "UseMethod",
        "UseMethod(generic, object)",
        "S3 dispatch: look for generic.class for each class of the object — the current call's first argument when none is given — then generic.default, and return what it returns. NextMethod does not exist, so dispatch stops at the first match.",
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
        "Return expr. Arguments are evaluated eagerly, so any message has already been written to stderr by the time this runs — the call is accepted for compatibility, not honoured.",
    ),
    (
        "suppressWarnings",
        "suppressWarnings(expr)",
        "Return expr, with the same eager-evaluation caveat as suppressMessages.",
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
