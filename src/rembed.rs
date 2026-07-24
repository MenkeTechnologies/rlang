//! CRAN bridge: an embedded GNU R, reached over FFI.
//!
//! rlang implements a compiled subset of base R. For everything else — loading a
//! CRAN package, calling a routine backed by compiled C/C++/Fortran — it does
//! NOT re-implement R's C runtime (that would just be a second GNU R). Instead it
//! `dlopen`s the real `libR` at run time, spins up an embedded R once, and
//! delegates the call. rlang keeps its JIT for the code it owns; real R handles
//! the ecosystem.
//!
//! The library is loaded lazily and only if present, so rlang still builds and
//! runs on a machine with no R installed — the bridge simply reports itself
//! unavailable and callers fall back to the usual "could not find function".
//!
//! Marshalling is atomic-vector-oriented (logical/integer/double/character, plus
//! NULL and lists), which covers the value types that cross the boundary in
//! practice. A return type with no rlang representation (S4 object, environment,
//! data frame) surfaces as an error rather than a wrong value.

#![cfg(not(target_arch = "wasm32"))]

use crate::builtins::{mk_dbl, mk_int, mk_lgl, mk_list, mk_str, null, set_names};
use crate::host::{with_host, RData};
use fusevm::Value;
use libloading::Library;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::sync::OnceLock;

type Sexp = *mut c_void;

// R's SEXPTYPE tags (Rinternals.h).
const NILSXP: c_int = 0;
const LGLSXP: c_int = 10;
const INTSXP: c_int = 13;
const REALSXP: c_int = 14;
const STRSXP: c_int = 16;
const VECSXP: c_int = 19;

/// The resolved `libR` entry points. Every field is a raw function pointer or an
/// R global, captured once; the `Library` is leaked so the pointers stay valid
/// for the process lifetime.
struct RApi {
    parse: unsafe extern "C" fn(Sexp, c_int, *mut c_int, Sexp) -> Sexp,
    try_eval_silent: unsafe extern "C" fn(Sexp, Sexp, *mut c_int) -> Sexp,
    protect: unsafe extern "C" fn(Sexp) -> Sexp,
    unprotect: unsafe extern "C" fn(c_int),
    mk_string: unsafe extern "C" fn(*const c_char) -> Sexp,
    mk_char: unsafe extern "C" fn(*const c_char) -> Sexp,
    alloc_vector: unsafe extern "C" fn(c_int, isize) -> Sexp,
    set_string_elt: unsafe extern "C" fn(Sexp, isize, Sexp),
    set_vector_elt: unsafe extern "C" fn(Sexp, isize, Sexp) -> Sexp,
    vector_elt: unsafe extern "C" fn(Sexp, isize) -> Sexp,
    string_elt: unsafe extern "C" fn(Sexp, isize) -> Sexp,
    r_char: unsafe extern "C" fn(Sexp) -> *const c_char,
    real: unsafe extern "C" fn(Sexp) -> *mut f64,
    integer: unsafe extern "C" fn(Sexp) -> *mut c_int,
    logical: unsafe extern "C" fn(Sexp) -> *mut c_int,
    xlength: unsafe extern "C" fn(Sexp) -> isize,
    typeof_: unsafe extern "C" fn(Sexp) -> c_int,
    get_attrib: unsafe extern "C" fn(Sexp, Sexp) -> Sexp,
    install: unsafe extern "C" fn(*const c_char) -> Sexp,
    define_var: unsafe extern "C" fn(Sexp, Sexp, Sexp),
    global_env: Sexp,
    nil: Sexp,
    na_int: c_int,
}

// SAFETY: R is single-threaded; rlang evaluates on one thread at a time, so the
// raw pointers are only ever dereferenced under that single-threaded discipline.
unsafe impl Send for RApi {}
unsafe impl Sync for RApi {}

static API: OnceLock<Option<RApi>> = OnceLock::new();

/// Discover `R_HOME` (from the env, else by asking `R RHOME`), set it, and
/// return the path to `libR`.
fn locate_libr() -> Option<String> {
    let home = std::env::var("R_HOME").ok().or_else(|| {
        let out = std::process::Command::new("R").arg("RHOME").output().ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
    })?;
    std::env::set_var("R_HOME", &home);
    for ext in ["dylib", "so"] {
        let p = format!("{home}/lib/libR.{ext}");
        if std::path::Path::new(&p).exists() {
            return Some(p);
        }
    }
    None
}

/// Load `libR`, resolve every entry point, and start the embedded interpreter.
/// Returns `None` if R is not installed or init fails.
fn init() -> Option<RApi> {
    let path = locate_libr()?;
    unsafe {
        let lib = Box::leak(Box::new(Library::new(&path).ok()?));
        macro_rules! sym {
            ($t:ty, $name:expr) => {{
                let s: libloading::Symbol<$t> = lib.get($name).ok()?;
                *s
            }};
        }
        // Start embedded R (quietly, no save/restore) before anything else.
        let init_r: unsafe extern "C" fn(c_int, *const *const c_char) -> c_int =
            sym!(_, b"Rf_initEmbeddedR");
        let args = [
            CString::new("R").ok()?,
            CString::new("--no-save").ok()?,
            CString::new("--silent").ok()?,
            CString::new("--no-restore").ok()?,
        ];
        let argv: Vec<*const c_char> = args.iter().map(|s| s.as_ptr()).collect();
        if init_r(argv.len() as c_int, argv.as_ptr()) == 0 {
            return None;
        }
        // R globals are data symbols: the symbol address holds the SEXP value.
        let ge: libloading::Symbol<*const Sexp> = lib.get(b"R_GlobalEnv").ok()?;
        let nl: libloading::Symbol<*const Sexp> = lib.get(b"R_NilValue").ok()?;
        let na_ptr: libloading::Symbol<*const c_int> = lib.get(b"R_NaInt").ok()?;
        Some(RApi {
            parse: sym!(_, b"R_ParseVector"),
            try_eval_silent: sym!(_, b"R_tryEvalSilent"),
            protect: sym!(_, b"Rf_protect"),
            unprotect: sym!(_, b"Rf_unprotect"),
            mk_string: sym!(_, b"Rf_mkString"),
            mk_char: sym!(_, b"Rf_mkChar"),
            alloc_vector: sym!(_, b"Rf_allocVector"),
            set_string_elt: sym!(_, b"SET_STRING_ELT"),
            set_vector_elt: sym!(_, b"SET_VECTOR_ELT"),
            vector_elt: sym!(_, b"VECTOR_ELT"),
            string_elt: sym!(_, b"STRING_ELT"),
            r_char: sym!(_, b"R_CHAR"),
            real: sym!(_, b"REAL"),
            integer: sym!(_, b"INTEGER"),
            logical: sym!(_, b"LOGICAL"),
            xlength: sym!(_, b"Rf_xlength"),
            typeof_: sym!(_, b"TYPEOF"),
            get_attrib: sym!(_, b"Rf_getAttrib"),
            install: sym!(_, b"Rf_install"),
            define_var: sym!(_, b"Rf_defineVar"),
            global_env: **ge,
            nil: **nl,
            na_int: **na_ptr,
        })
    }
}

fn api() -> Option<&'static RApi> {
    API.get_or_init(init).as_ref()
}

/// Whether an embedded R is available to delegate to.
pub fn available() -> bool {
    api().is_some()
}

impl RApi {
    /// Build an R SEXP from an rlang value (atomic vectors, NULL, lists).
    unsafe fn to_sexp(&self, v: &Value) -> Sexp {
        let data = with_host(|h| h.data_of(v));
        match data {
            RData::Null => self.nil,
            RData::Lgl(xs) => {
                let s = (self.protect)((self.alloc_vector)(LGLSXP, xs.len() as isize));
                let p = (self.logical)(s);
                for (i, e) in xs.iter().enumerate() {
                    *p.add(i) = match e {
                        Some(true) => 1,
                        Some(false) => 0,
                        None => self.na_int,
                    };
                }
                (self.unprotect)(1);
                s
            }
            RData::Int(xs) => {
                let s = (self.protect)((self.alloc_vector)(INTSXP, xs.len() as isize));
                let p = (self.integer)(s);
                for (i, e) in xs.iter().enumerate() {
                    *p.add(i) = e.map(|n| n as c_int).unwrap_or(self.na_int);
                }
                (self.unprotect)(1);
                s
            }
            RData::Dbl(xs) => {
                let s = (self.protect)((self.alloc_vector)(REALSXP, xs.len() as isize));
                let p = (self.real)(s);
                for (i, e) in xs.iter().enumerate() {
                    *p.add(i) = e.unwrap_or(f64::NAN);
                }
                (self.unprotect)(1);
                s
            }
            RData::Str(xs) => {
                let s = (self.protect)((self.alloc_vector)(STRSXP, xs.len() as isize));
                for (i, e) in xs.iter().enumerate() {
                    if let Some(text) = e {
                        if let Ok(c) = CString::new(text.as_str()) {
                            (self.set_string_elt)(s, i as isize, (self.mk_char)(c.as_ptr()));
                        }
                    }
                }
                (self.unprotect)(1);
                s
            }
            RData::List(items) => {
                let s = (self.protect)((self.alloc_vector)(VECSXP, items.len() as isize));
                for (i, it) in items.iter().enumerate() {
                    (self.set_vector_elt)(s, i as isize, self.to_sexp(it));
                }
                (self.unprotect)(1);
                s
            }
            _ => self.nil,
        }
    }

    /// Marshal an R SEXP back into an rlang value, or `Err` for a type rlang has
    /// no representation for.
    unsafe fn from_sexp(&self, s: Sexp) -> Result<Value, String> {
        let ty = (self.typeof_)(s);
        let n = (self.xlength)(s) as usize;
        let names = |api: &RApi| -> Vec<Option<String>> {
            let key = CString::new("names").unwrap();
            let nm = (api.get_attrib)(s, (api.install)(key.as_ptr()));
            if (api.typeof_)(nm) == STRSXP {
                (0..(api.xlength)(nm))
                    .map(|i| {
                        let c = (api.string_elt)(nm, i);
                        Some(CStr::from_ptr((api.r_char)(c)).to_string_lossy().into_owned())
                    })
                    .collect()
            } else {
                Vec::new()
            }
        };
        let out = match ty {
            NILSXP => null(),
            LGLSXP => {
                let p = (self.logical)(s);
                mk_lgl(
                    (0..n)
                        .map(|i| {
                            let v = *p.add(i);
                            if v == self.na_int {
                                None
                            } else {
                                Some(v != 0)
                            }
                        })
                        .collect(),
                )
            }
            INTSXP => {
                let p = (self.integer)(s);
                mk_int(
                    (0..n)
                        .map(|i| {
                            let v = *p.add(i);
                            if v == self.na_int {
                                None
                            } else {
                                Some(v as i64)
                            }
                        })
                        .collect(),
                )
            }
            REALSXP => {
                let p = (self.real)(s);
                mk_dbl(
                    (0..n)
                        .map(|i| {
                            let v = *p.add(i);
                            if v.is_nan() {
                                None
                            } else {
                                Some(v)
                            }
                        })
                        .collect(),
                )
            }
            STRSXP => mk_str(
                (0..n)
                    .map(|i| {
                        let c = (self.string_elt)(s, i as isize);
                        Some(CStr::from_ptr((self.r_char)(c)).to_string_lossy().into_owned())
                    })
                    .collect(),
            ),
            VECSXP => {
                let items: Result<Vec<Value>, String> = (0..n)
                    .map(|i| self.from_sexp((self.vector_elt)(s, i as isize)))
                    .collect();
                mk_list(items?)
            }
            other => {
                return Err(format!(
                    "CRAN bridge: cannot marshal an R value of type {other} back to rlang"
                ))
            }
        };
        let nm = names(self);
        if !nm.is_empty() {
            set_names(&out, nm);
        }
        Ok(out)
    }

    /// Parse and evaluate R source in the global environment, returning the last
    /// value.
    unsafe fn eval(&self, code: &str) -> Result<Sexp, String> {
        let src = CString::new(code).map_err(|_| "CRAN bridge: NUL in R source".to_string())?;
        let mut status: c_int = 0;
        let expr = (self.protect)((self.parse)(
            (self.mk_string)(src.as_ptr()),
            -1,
            &mut status,
            self.nil,
        ));
        let n = (self.xlength)(expr);
        let mut last = self.nil;
        let mut err: c_int = 0;
        for i in 0..n {
            last = (self.try_eval_silent)((self.vector_elt)(expr, i), self.global_env, &mut err);
            if err != 0 {
                (self.unprotect)(1);
                return Err(format!("CRAN bridge: R error evaluating `{code}`"));
            }
        }
        (self.unprotect)(1);
        Ok(last)
    }
}

/// Evaluate R source in the embedded interpreter, marshalling the result back.
pub fn eval_source(code: &str) -> Result<Value, String> {
    let api = api().ok_or_else(|| "CRAN bridge unavailable (no R installation found)".to_string())?;
    unsafe {
        let s = api.eval(code)?;
        let s = (api.protect)(s);
        let r = api.from_sexp(s);
        (api.unprotect)(1);
        r
    }
}

/// Delegate `name(args…)` to embedded R: bind each argument to a temporary in R,
/// evaluate the call, and marshal the result back. `Err` if R has no such
/// function or the call fails.
pub fn call(name: &str, args: &[(Option<String>, Value)]) -> Result<Value, String> {
    let api = api().ok_or_else(|| format!("could not find function \"{name}\""))?;
    unsafe {
        // Bind arguments to `.rlang_argN` in the global environment.
        let mut parts: Vec<String> = Vec::with_capacity(args.len());
        for (i, (tag, v)) in args.iter().enumerate() {
            let var = format!(".rlang_arg{i}");
            let sexp = (api.protect)(api.to_sexp(v));
            (api.define_var)(
                (api.install)(CString::new(var.as_str()).unwrap().as_ptr()),
                sexp,
                api.global_env,
            );
            (api.unprotect)(1);
            parts.push(match tag {
                Some(t) => format!("`{t}` = {var}"),
                None => var,
            });
        }
        // A pre-check keeps a genuine "not found" distinct from a call that
        // errors, so rlang's own message is preserved when R lacks the function.
        let probe = format!("exists(\"{name}\", mode = \"function\")");
        let found = api
            .eval(&probe)
            .ok()
            .map(|s| (api.logical)(s).read() == 1)
            .unwrap_or(false);
        if !found {
            return Err(format!("could not find function \"{name}\""));
        }
        let s = (api.protect)(api.eval(&format!("{name}({})", parts.join(", ")))?);
        let r = api.from_sexp(s);
        (api.unprotect)(1);
        r
    }
}
