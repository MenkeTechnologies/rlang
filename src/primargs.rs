//! The formals R shows for its primitives.
//!
//! A primitive has no formals of its own; R keeps a stand-in closure for each
//! one that takes arguments in two base environments, `.ArgsEnv` and
//! `.GenericArgsEnv` (built in `src/library/base/R/zzz.R`). `print` of a
//! primitive deparses the stand-in's header in front of the `.Primitive` call
//! (`PrintSpecial` in `src/main/print.c`), and `args()` hands back a copy of the
//! stand-in itself (`do_args` in `src/main/builtin.c`). A primitive with no
//! stand-in — the language keywords, `[`, `(` — prints as the bare
//! `.Primitive("if")` and has no `args`.
//!
//! The table is the deparsed header of every stand-in in both environments of
//! R 4.6, sorted by name (byte order) for `binary_search`.

/// `(name, formals)`: the formals as R deparses them, parentheses included.
const FORMALS: &[(&str, &str)] = &[
    ("!", "(x)"),
    ("!=", "(e1, e2)"),
    ("%%", "(e1, e2)"),
    ("%*%", "(x, y)"),
    ("%/%", "(e1, e2)"),
    ("&", "(e1, e2)"),
    ("*", "(e1, e2)"),
    ("+", "(e1, e2)"),
    ("-", "(e1, e2)"),
    ("...elt", "(n)"),
    ("...length", "()"),
    ("...names", "()"),
    (
        ".C",
        "(.NAME, ..., NAOK = FALSE, DUP = TRUE, PACKAGE, ENCODING)",
    ),
    (".Call", "(.NAME, ..., PACKAGE)"),
    (".Call.graphics", "(.NAME, ..., PACKAGE)"),
    (".External", "(.NAME, ..., PACKAGE)"),
    (".External.graphics", "(.NAME, ..., PACKAGE)"),
    (".External2", "(.NAME, ..., PACKAGE)"),
    (
        ".Fortran",
        "(.NAME, ..., NAOK = FALSE, DUP = TRUE, PACKAGE, ENCODING)",
    ),
    (".Internal", "(call)"),
    (".Primitive", "(name)"),
    (".cache_class", "(class, extends)"),
    (".class2", "(x)"),
    (".isMethodsDispatchOn", "(onOff = NULL)"),
    (".primTrace", "(obj)"),
    (".primUntrace", "(obj)"),
    (".subset", "(x, ...)"),
    (".subset2", "(x, ...)"),
    ("/", "(e1, e2)"),
    ("::", "(pkg, name)"),
    (":::", "(pkg, name)"),
    ("<", "(e1, e2)"),
    ("<=", "(e1, e2)"),
    ("==", "(e1, e2)"),
    (">", "(e1, e2)"),
    (">=", "(e1, e2)"),
    ("Arg", "(z)"),
    ("Conj", "(z)"),
    ("Exec", "(expr, envir)"),
    ("Im", "(z)"),
    ("Mod", "(z)"),
    ("Re", "(z)"),
    ("Tailcall", "(FUN, ...)"),
    ("UseMethod", "(generic, object)"),
    ("^", "(e1, e2)"),
    ("abs", "(x)"),
    ("acos", "(x)"),
    ("acosh", "(x)"),
    ("all", "(..., na.rm = FALSE)"),
    ("any", "(..., na.rm = FALSE)"),
    ("anyNA", "(x, recursive = FALSE)"),
    ("as.call", "(x)"),
    ("as.character", "(x, ...)"),
    ("as.complex", "(x, ...)"),
    ("as.double", "(x, ...)"),
    ("as.environment", "(x)"),
    ("as.integer", "(x, ...)"),
    ("as.logical", "(x, ...)"),
    ("as.numeric", "(x, ...)"),
    ("as.raw", "(x)"),
    ("asin", "(x)"),
    ("asinh", "(x)"),
    ("atan", "(x)"),
    ("atanh", "(x)"),
    ("attr", "(x, which, exact = FALSE)"),
    ("attr<-", "(x, which, value)"),
    ("attributes", "(x)"),
    ("attributes<-", "(x, value)"),
    ("baseenv", "()"),
    (
        "browser",
        "(text = \"\", condition = NULL, expr = TRUE, skipCalls = 0L)",
    ),
    ("c", "(...)"),
    ("call", "(name, ...)"),
    ("ceiling", "(x)"),
    ("class", "(x)"),
    ("class<-", "(x, value)"),
    ("cos", "(x)"),
    ("cosh", "(x)"),
    ("cospi", "(x)"),
    ("crossprod", "(x, y = NULL, ...)"),
    ("cummax", "(x)"),
    ("cummin", "(x)"),
    ("cumprod", "(x)"),
    ("cumsum", "(x)"),
    ("declare", "(...)"),
    ("digamma", "(x)"),
    ("dim", "(x)"),
    ("dim<-", "(x, value)"),
    ("dimnames", "(x)"),
    ("dimnames<-", "(x, value)"),
    ("emptyenv", "()"),
    ("enc2native", "(x)"),
    ("enc2utf8", "(x)"),
    ("environment<-", "(fun, value)"),
    ("exp", "(x)"),
    ("expm1", "(x)"),
    ("expression", "(...)"),
    ("floor", "(x)"),
    ("forceAndCall", "(n, FUN, ...)"),
    ("gamma", "(x)"),
    ("gc.time", "(on = TRUE)"),
    ("globalenv", "()"),
    ("interactive", "()"),
    ("invisible", "(x = NULL)"),
    ("is.array", "(x)"),
    ("is.atomic", "(x)"),
    ("is.call", "(x)"),
    ("is.character", "(x)"),
    ("is.complex", "(x)"),
    ("is.double", "(x)"),
    ("is.environment", "(x)"),
    ("is.expression", "(x)"),
    ("is.finite", "(x)"),
    ("is.function", "(x)"),
    ("is.infinite", "(x)"),
    ("is.integer", "(x)"),
    ("is.language", "(x)"),
    ("is.list", "(x)"),
    ("is.logical", "(x)"),
    ("is.matrix", "(x)"),
    ("is.na", "(x)"),
    ("is.name", "(x)"),
    ("is.nan", "(x)"),
    ("is.null", "(x)"),
    ("is.numeric", "(x)"),
    ("is.object", "(x)"),
    ("is.pairlist", "(x)"),
    ("is.raw", "(x)"),
    ("is.recursive", "(x)"),
    ("is.single", "(x)"),
    ("is.symbol", "(x)"),
    ("isS4", "(object)"),
    ("lazyLoadDBfetch", "(key, file, compressed, hook)"),
    ("length", "(x)"),
    ("length<-", "(x, value)"),
    ("levels<-", "(x, value)"),
    ("lgamma", "(x)"),
    ("list", "(...)"),
    ("log", "(x, base = exp(1))"),
    ("log10", "(x)"),
    ("log1p", "(x)"),
    ("log2", "(x)"),
    ("max", "(..., na.rm = FALSE)"),
    ("min", "(..., na.rm = FALSE)"),
    ("missing", "(x)"),
    ("names", "(x)"),
    ("names<-", "(x, value)"),
    ("nargs", "()"),
    ("nzchar", "(x, keepNA = FALSE)"),
    ("oldClass", "(x)"),
    ("oldClass<-", "(x, value)"),
    ("on.exit", "(expr = NULL, add = FALSE, after = TRUE)"),
    ("pos.to.env", "(x)"),
    ("proc.time", "()"),
    ("prod", "(..., na.rm = FALSE)"),
    ("quote", "(expr)"),
    ("range", "(..., na.rm = FALSE)"),
    ("rep", "(x, ...)"),
    ("retracemem", "(x, previous = NULL)"),
    ("round", "(x, digits = 0, ...)"),
    ("seq.int", "(from, to, by, length.out, along.with, ...)"),
    ("seq_along", "(along.with)"),
    ("seq_len", "(length.out)"),
    ("sign", "(x)"),
    ("signif", "(x, digits = 6)"),
    ("sin", "(x)"),
    ("sinh", "(x)"),
    ("sinpi", "(x)"),
    ("sqrt", "(x)"),
    ("standardGeneric", "(f, fdef)"),
    ("storage.mode<-", "(x, value)"),
    ("substitute", "(expr, env)"),
    ("sum", "(..., na.rm = FALSE)"),
    ("switch", "(EXPR, ...)"),
    ("tan", "(x)"),
    ("tanh", "(x)"),
    ("tanpi", "(x)"),
    ("tcrossprod", "(x, y = NULL, ...)"),
    ("tracemem", "(x)"),
    ("trigamma", "(x)"),
    ("trunc", "(x, ...)"),
    ("unCfillPOSIXlt", "(x)"),
    ("unclass", "(x)"),
    ("untracemem", "(x)"),
    ("xtfrm", "(x)"),
    ("|", "(e1, e2)"),
];

/// The deparsed formals R keeps for primitive `name` — `(..., na.rm = FALSE)`
/// for `sum` — or `None` for a primitive that has none (`if`, `[`, `(`).
pub fn formals(name: &str) -> Option<&'static str> {
    FORMALS
        .binary_search_by(|(n, _)| (*n).cmp(name))
        .ok()
        .map(|i| FORMALS[i].1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_sorted_for_binary_search() {
        assert!(FORMALS.windows(2).all(|w| w[0].0 < w[1].0));
    }

    #[test]
    fn looks_up_generic_and_plain_primitives() {
        assert_eq!(formals("sum"), Some("(..., na.rm = FALSE)"));
        assert_eq!(formals("+"), Some("(e1, e2)"));
        assert_eq!(formals("length"), Some("(x)"));
        assert_eq!(formals("if"), None);
        assert_eq!(formals("["), None);
    }
}
