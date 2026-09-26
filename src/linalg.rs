//! Dense linear algebra behind `solve`, `det` and `determinant`.
//!
//! GNU R answers these through `La_solve` and `det_ge_real`
//! (`src/modules/lapack/Lapack.c`), which call LAPACK's `dgesv`, `dgetrf`,
//! `dlange` and `dgecon`. This module ports the reference LAPACK 3.12 routines
//! those reach — `dgetrf2`'s recursive LU with partial pivoting, `dgetrs`,
//! `dlaswp`, and `dgecon`'s condition estimate through `dlacn2` / `dlatrs` —
//! in the same operation order, so the pivots, the exact-singularity index and
//! the reciprocal condition number R reports come out the same. The BLAS
//! calls those routines make follow the reference R's OpenBLAS kernels (see
//! the note above [`gemm_acc`]), which is what makes `solve` and
//! `determinant` agree with R to the last bit rather than to a few ULPs.
//!
//! Matrices are column-major `f64` slices with a leading dimension, exactly
//! LAPACK's layout. An R `NA` travels as R's own NaN payload (`NA_REAL`), so it
//! propagates through the arithmetic the way it does in R and can be told
//! apart from a plain `NaN` afterwards (see [`na_real`] / [`is_na_real`]).

/// R's `NA_real_`: a NaN whose low word is 1954.
pub fn na_real() -> f64 {
    f64::from_bits(0x7FF0_0000_0000_07A2)
}

/// `R_IsNA`: a NaN carrying R's `NA` payload in its low word.
pub fn is_na_real(x: f64) -> bool {
    x.is_nan() && (x.to_bits() & 0xFFFF_FFFF) == 1954
}

/// `dlamch('S')`: the safe minimum, the smallest normal double.
const SFMIN: f64 = f64::MIN_POSITIVE;

/// A column-major matrix view: element `(i, j)` (0-based) is `a[off + i + j*lda]`.
#[derive(Clone, Copy)]
struct At {
    off: usize,
    lda: usize,
}

impl At {
    fn ix(self, i: usize, j: usize) -> usize {
        self.off + i + j * self.lda
    }
    fn sub(self, i: usize, j: usize) -> At {
        At {
            off: self.ix(i, j),
            lda: self.lda,
        }
    }
}

/// BLAS `idamax`: the 0-based index of the first element of largest magnitude.
fn idamax(x: &[f64]) -> usize {
    let mut best = 0;
    let mut max = f64::NEG_INFINITY;
    for (i, v) in x.iter().enumerate() {
        if v.abs() > max {
            max = v.abs();
            best = i;
        }
    }
    best
}

/// BLAS `dasum` in the reference order: the `n mod 6` leading terms one at a
/// time, then six at a time left to right.
fn dasum(x: &[f64]) -> f64 {
    let n = x.len();
    let m = n % 6;
    let mut t = 0.0;
    for v in &x[..m] {
        t += v.abs();
    }
    let mut i = m;
    while i < n {
        t = t
            + x[i].abs()
            + x[i + 1].abs()
            + x[i + 2].abs()
            + x[i + 3].abs()
            + x[i + 4].abs()
            + x[i + 5].abs();
        i += 6;
    }
    t
}

/// `c - a*b`, rounded once: the BLAS kernels are built with floating-point
/// contraction, so an in-kernel update like `c[k] -= bb * a[k]` is a fused
/// multiply-subtract.
fn fms(c: f64, a: f64, b: f64) -> f64 {
    (-a).mul_add(b, c)
}

// ── the BLAS level 3 calls, in the order the reference R's BLAS runs them ──
//
// R hands its LAPACK's `dgemm` / `dtrsm` calls to the BLAS it was linked
// against — OpenBLAS in the reference build — and the last bits of `solve`
// depend on that library's operation order rather than the reference BLAS
// loops. These follow OpenBLAS's kernels: a GEMM update accumulates its
// products for one element in a fused chain from zero and then subtracts the
// sum once; a triangular solve walks the rows in register blocks (8 rows, then
// the 4 / 2 / 1 remainder) with each block's earlier-solved contributions
// applied as such a GEMM update, and multiplies by the reciprocal of the
// diagonal rather than dividing by it.

/// Rows per register block in the triangular-solve kernels (`GEMM_UNROLL_M`).
const UNROLL_M: usize = 8;

/// `c - Σ x_k y_k` the way a GEMM kernel with `alpha = -1, beta = 1` forms
/// it: the products summed in a fused chain from zero, the sum subtracted once.
fn gemm_acc(c: f64, terms: impl Iterator<Item = (f64, f64)>) -> f64 {
    let acc = terms.fold(0.0, |acc, (x, y)| x.mul_add(y, acc));
    c - acc
}

/// `dgemm('N', 'N', m, n, k, -1, A, B, 1, C)`: `C := C - A B` on views of `a`.
/// OpenBLAS forwards a single-column product to `dgemv`, whose kernel folds
/// each product into the element in turn with a fused multiply-subtract, so
/// that shape is computed that way instead of through the GEMM sum.
fn gemm_sub(a: &mut [f64], c: At, m: usize, n: usize, am: At, bm: At, k: usize) {
    for j in 0..n {
        for i in 0..m {
            let terms: Vec<(f64, f64)> = (0..k).map(|l| (a[am.ix(i, l)], a[bm.ix(l, j)])).collect();
            let ix = c.ix(i, j);
            a[ix] = if n == 1 {
                terms.into_iter().fold(a[ix], |c, (x, y)| fms(c, x, y))
            } else {
                gemm_acc(a[ix], terms.into_iter())
            };
        }
    }
}

/// One dot product as the reference R's `dgemv('T')` kernel (OpenBLAS
/// `arm64/gemv_t.S`) forms it: the leading multiple of eight terms go
/// round-robin into eight fused accumulators, which are reduced pairwise —
/// `((l0+l1) + (l2+l3)) + ((l4+l5) + (l6+l7))` — and the remaining terms are
/// then fused onto that sum one at a time.
pub fn gemv_t_dot(terms: impl Iterator<Item = (f64, f64)>) -> f64 {
    let terms: Vec<(f64, f64)> = terms.collect();
    let body = terms.len() / 8 * 8;
    let mut lane = [0.0f64; 8];
    for (k, &(x, y)) in terms[..body].iter().enumerate() {
        lane[k % 8] = x.mul_add(y, lane[k % 8]);
    }
    let sum =
        ((lane[0] + lane[1]) + (lane[2] + lane[3])) + ((lane[4] + lane[5]) + (lane[6] + lane[7]));
    terms[body..]
        .iter()
        .fold(sum, |acc, &(x, y)| x.mul_add(y, acc))
}

/// The row blocks a triangular-solve kernel visits, as `(first row, rows)`:
/// top-down for a lower triangle (full blocks, then the 4 / 2 / 1 remainder),
/// bottom-up for an upper one (the 1 / 2 / 4 remainder at the bottom first,
/// then full blocks upwards).
fn trsm_blocks(m: usize, lower: bool) -> Vec<(usize, usize)> {
    let full = m / UNROLL_M;
    let mut out = Vec::new();
    if lower {
        out.extend((0..full).map(|b| (b * UNROLL_M, UNROLL_M)));
        let mut r = full * UNROLL_M;
        let mut i = UNROLL_M / 2;
        while i > 0 {
            if m & i != 0 {
                out.push((r, i));
                r += i;
            }
            i /= 2;
        }
    } else {
        let mut i = 1;
        while i < UNROLL_M {
            if m & i != 0 {
                out.push(((m & !(i - 1)) - i, i));
            }
            i *= 2;
        }
        out.extend((0..full).rev().map(|b| (b * UNROLL_M, UNROLL_M)));
    }
    out
}

/// `dtrsm('L', uplo, 'N', diag, m, n, 1, T, B)`: `B := T⁻¹ B` for the unit
/// lower or the non-unit upper triangle of the `m × m` matrix at `t`.
fn trsm(tm: &[f64], t: At, m: usize, b: &mut [f64], bt: At, n: usize, tri: Tri) {
    let lower = tri == Tri::LowerUnit;
    let blocks = trsm_blocks(m, lower);
    for j in 0..n {
        for &(r0, sz) in &blocks {
            let rows = r0..r0 + sz;
            // The rows already solved: above the block for a lower triangle,
            // below it for an upper one.
            let done = if lower { 0..r0 } else { r0 + sz..m };
            for row in rows.clone() {
                let terms: Vec<(f64, f64)> = done
                    .clone()
                    .map(|k| (tm[t.ix(row, k)], b[bt.ix(k, j)]))
                    .collect();
                let ix = bt.ix(row, j);
                b[ix] = gemm_acc(b[ix], terms.into_iter());
            }
            if lower {
                for i in rows.clone() {
                    let x = b[bt.ix(i, j)];
                    for k in i + 1..r0 + sz {
                        let ix = bt.ix(k, j);
                        b[ix] = fms(b[ix], x, tm[t.ix(k, i)]);
                    }
                }
            } else {
                for i in rows.clone().rev() {
                    let ix = bt.ix(i, j);
                    let x = b[ix] * (1.0 / tm[t.ix(i, i)]);
                    b[ix] = x;
                    for k in r0..i {
                        let kx = bt.ix(k, j);
                        b[kx] = fms(b[kx], x, tm[t.ix(k, i)]);
                    }
                }
            }
        }
    }
}

/// [`trsm`] where the triangle and the right-hand side share one array — the
/// panel update inside the LU factorisation.
fn trsm_in_place(a: &mut [f64], t: At, m: usize, bt: At, n: usize) {
    let tri: Vec<f64> = (0..m * m).map(|x| a[t.ix(x % m, x / m)]).collect();
    let mut rhs: Vec<f64> = (0..m * n).map(|x| a[bt.ix(x % m, x / m)]).collect();
    trsm(
        &tri,
        At { off: 0, lda: m },
        m,
        &mut rhs,
        At { off: 0, lda: m },
        n,
        Tri::LowerUnit,
    );
    for (x, v) in rhs.into_iter().enumerate() {
        a[bt.ix(x % m, x / m)] = v;
    }
}

/// LAPACK `dlaswp` with `incx = 1`: apply the row interchanges `ipiv[k1..k2]`
/// (0-based targets) to the `ncol` columns starting at `at`.
fn dlaswp(a: &mut [f64], at: At, ncol: usize, k1: usize, k2: usize, ipiv: &[usize]) {
    for j in 0..ncol {
        for (i, &p) in ipiv.iter().enumerate().take(k2).skip(k1) {
            if p != i {
                a.swap(at.ix(i, j), at.ix(p, j));
            }
        }
    }
}

/// LAPACK `dgetrf2`: recursive LU factorisation with partial pivoting of the
/// `m × n` block at `at`. Pivots are written 0-based into `ipiv`; the result
/// is LAPACK's `info` — 0, or the 1-based index of the first exactly-zero
/// pivot `U[i,i]`.
fn dgetrf2(a: &mut [f64], at: At, m: usize, n: usize, ipiv: &mut [usize]) -> usize {
    if m == 0 || n == 0 {
        return 0;
    }
    if m == 1 {
        ipiv[0] = 0;
        return usize::from(a[at.ix(0, 0)] == 0.0);
    }
    if n == 1 {
        let col: Vec<f64> = (0..m).map(|i| a[at.ix(i, 0)]).collect();
        let i = idamax(&col);
        ipiv[0] = i;
        if a[at.ix(i, 0)] == 0.0 {
            return 1;
        }
        if i != 0 {
            a.swap(at.ix(0, 0), at.ix(i, 0));
        }
        let piv = a[at.ix(0, 0)];
        if piv.abs() >= SFMIN {
            // `dscal` by the reciprocal, not a division per element.
            let r = 1.0 / piv;
            for k in 1..m {
                a[at.ix(k, 0)] *= r;
            }
        } else {
            for k in 1..m {
                a[at.ix(k, 0)] /= piv;
            }
        }
        return 0;
    }
    let n1 = m.min(n) / 2;
    let n2 = n - n1;
    let mut info = dgetrf2(a, at, m, n1, ipiv);
    // [A12; A22] gets the left panel's interchanges.
    dlaswp(a, at.sub(0, n1), n2, 0, n1, ipiv);
    // A12 := L11⁻¹ A12 (dtrsm Left Lower NoTrans Unit), then
    // A22 := A22 - A21 A12 (dgemm, alpha = -1, beta = 1).
    trsm_in_place(a, at, n1, at.sub(0, n1), n2);
    gemm_sub(
        a,
        at.sub(n1, n1),
        m - n1,
        n2,
        at.sub(n1, 0),
        at.sub(0, n1),
        n1,
    );
    let k = m.min(n);
    let iinfo = dgetrf2(a, at.sub(n1, n1), m - n1, n2, &mut ipiv[n1..]);
    if info == 0 && iinfo > 0 {
        info = iinfo + n1;
    }
    for p in ipiv.iter_mut().take(k).skip(n1) {
        *p += n1;
    }
    // A21 gets the trailing block's interchanges.
    dlaswp(a, at, n1, n1, k, ipiv);
    info
}

/// `ILAENV`'s block size for `DGETRF`.
const LU_NB: usize = 64;

/// LAPACK `dgetrf`: LU of the square `n × n` matrix `a` in place, returning
/// `(ipiv, info)`. Up to the block size it is one `dgetrf2`; past it, panels
/// of `LU_NB` columns are factored and the trailing matrix updated with
/// `dtrsm` / `dgemm`, as the reference routine does.
pub fn lu(a: &mut [f64], n: usize) -> (Vec<usize>, usize) {
    let at = At { off: 0, lda: n };
    let mut ipiv = vec![0; n];
    if n <= LU_NB {
        let info = dgetrf2(a, at, n, n, &mut ipiv);
        return (ipiv, info);
    }
    let mut info = 0;
    for j in (0..n).step_by(LU_NB) {
        let jb = LU_NB.min(n - j);
        let iinfo = dgetrf2(a, at.sub(j, j), n - j, jb, &mut ipiv[j..]);
        if info == 0 && iinfo > 0 {
            info = iinfo + j;
        }
        for p in &mut ipiv[j..j + jb] {
            *p += j;
        }
        dlaswp(a, at, j, j, j + jb, &ipiv);
        if j + jb < n {
            let rest = n - j - jb;
            dlaswp(a, at.sub(0, j + jb), rest, j, j + jb, &ipiv);
            trsm_in_place(a, at.sub(j, j), jb, at.sub(j, j + jb), rest);
            gemm_sub(
                a,
                at.sub(j + jb, j + jb),
                rest,
                rest,
                at.sub(j + jb, j),
                at.sub(j, j + jb),
                jb,
            );
        }
    }
    (ipiv, info)
}

/// LAPACK `dgetrs` (no transpose): solve `A X = B` for the `nrhs` columns of
/// `b` from the LU factors `a` / `ipiv`.
fn dgetrs(a: &[f64], n: usize, ipiv: &[usize], b: &mut [f64], nrhs: usize) {
    let bt = At { off: 0, lda: n };
    dlaswp(b, bt, nrhs, 0, n, ipiv);
    trsm(a, bt, n, b, bt, nrhs, Tri::LowerUnit);
    trsm(a, bt, n, b, bt, nrhs, Tri::UpperNonUnit);
}

/// LAPACK `dlange('1')`: the largest absolute column sum; NaN wins.
fn one_norm(a: &[f64], n: usize) -> f64 {
    let mut value = 0.0;
    for j in 0..n {
        let mut sum = 0.0;
        for i in 0..n {
            sum += a[j * n + i].abs();
        }
        if value < sum || sum.is_nan() {
            value = sum;
        }
    }
    value
}

/// The triangle a `dlatrs` call works on.
#[derive(Clone, Copy, PartialEq)]
enum Tri {
    UpperNonUnit,
    LowerUnit,
}

/// LAPACK `dlatrs` on its ordinary path, which is `dtrsv`. LAPACK takes that
/// path whenever its growth bound admits the unscaled solve, which holds for
/// every matrix whose entries stay far from the overflow and underflow
/// thresholds. The careful, rescaling path it falls back to beyond that is not
/// ported, so the scale factor returned here is always 1.
fn dlatrs(tri: Tri, trans: bool, a: &[f64], n: usize, x: &mut [f64]) -> f64 {
    let upper = tri == Tri::UpperNonUnit;
    if upper {
        if trans {
            // x := U⁻ᵀ x
            for j in 0..n {
                let mut t = x[j];
                for i in 0..j {
                    t = fms(t, a[j * n + i], x[i]);
                }
                x[j] = t / a[j * n + j];
            }
        } else {
            for j in (0..n).rev() {
                if x[j] != 0.0 {
                    x[j] /= a[j * n + j];
                    let t = x[j];
                    for i in (0..j).rev() {
                        x[i] = fms(x[i], t, a[j * n + i]);
                    }
                }
            }
        }
    } else if trans {
        // x := L⁻ᵀ x, unit diagonal
        for j in (0..n).rev() {
            let mut t = x[j];
            for i in (j + 1..n).rev() {
                t = fms(t, a[j * n + i], x[i]);
            }
            x[j] = t;
        }
    } else {
        for j in 0..n {
            if x[j] != 0.0 {
                let t = x[j];
                for i in j + 1..n {
                    x[i] = fms(x[i], t, a[j * n + i]);
                }
            }
        }
    }
    1.0
}

/// LAPACK `dlacn2`'s saved state between reverse-communication calls.
struct Lacn2 {
    jump: u8,
    j: usize,
    iter: usize,
    est: f64,
    isgn: Vec<i32>,
    v: Vec<f64>,
}

/// `sign(1, x)` as Fortran's `SIGN` with a non-negative first argument.
fn sign1(x: f64) -> f64 {
    if x >= 0.0 {
        1.0
    } else {
        -1.0
    }
}

impl Lacn2 {
    /// One step of `dlacn2`: consume `x` (the product the previous `kase`
    /// asked for), and return the next `kase` — 1 for `A x`, 2 for `Aᵀ x`, 0
    /// when `est` is final.
    fn step(&mut self, x: &mut [f64], kase: u8) -> u8 {
        let n = x.len();
        const ITMAX: usize = 5;
        if kase == 0 {
            for e in x.iter_mut() {
                *e = 1.0 / n as f64;
            }
            self.jump = 1;
            return 1;
        }
        match self.jump {
            1 => {
                if n == 1 {
                    self.v[0] = x[0];
                    self.est = self.v[0].abs();
                    return 0;
                }
                self.est = dasum(x);
                for (e, s) in x.iter_mut().zip(self.isgn.iter_mut()) {
                    *e = sign1(*e);
                    *s = *e as i32;
                }
                self.jump = 2;
                2
            }
            2 => {
                self.j = idamax(x);
                self.iter = 2;
                self.unit_vector(x)
            }
            3 => {
                self.v.copy_from_slice(x);
                let estold = self.est;
                self.est = dasum(&self.v);
                let repeated = x
                    .iter()
                    .zip(&self.isgn)
                    .all(|(e, s)| sign1(*e) as i32 == *s);
                if repeated || self.est <= estold {
                    return self.alternating(x);
                }
                for (e, s) in x.iter_mut().zip(self.isgn.iter_mut()) {
                    *e = sign1(*e);
                    *s = *e as i32;
                }
                self.jump = 4;
                2
            }
            4 => {
                let jlast = self.j;
                self.j = idamax(x);
                if x[jlast] != x[self.j].abs() && self.iter < ITMAX {
                    self.iter += 1;
                    return self.unit_vector(x);
                }
                self.alternating(x)
            }
            _ => {
                let temp = 2.0 * (dasum(x) / (3 * n) as f64);
                if temp > self.est {
                    self.v.copy_from_slice(x);
                    self.est = temp;
                }
                0
            }
        }
    }

    fn unit_vector(&mut self, x: &mut [f64]) -> u8 {
        x.iter_mut().for_each(|e| *e = 0.0);
        x[self.j] = 1.0;
        self.jump = 3;
        1
    }

    fn alternating(&mut self, x: &mut [f64]) -> u8 {
        let n = x.len();
        let mut altsgn = 1.0;
        for (i, e) in x.iter_mut().enumerate() {
            *e = altsgn * (1.0 + i as f64 / (n - 1) as f64);
            altsgn = -altsgn;
        }
        self.jump = 5;
        1
    }
}

/// LAPACK `drscl`: `x := x / sa`, stepping through safe scale factors.
fn drscl(x: &mut [f64], sa: f64) {
    let smlnum = SFMIN;
    let bignum = 1.0 / smlnum;
    let mut cden = sa;
    let mut cnum = 1.0;
    loop {
        let cden1 = cden * smlnum;
        let cnum1 = cnum / bignum;
        let (mul, done) = if cden1.abs() > cnum.abs() && cnum != 0.0 {
            cden = cden1;
            (smlnum, false)
        } else if cnum1.abs() > cden.abs() {
            cnum = cnum1;
            (bignum, false)
        } else {
            (cnum / cden, true)
        };
        x.iter_mut().for_each(|e| *e *= mul);
        if done {
            break;
        }
    }
}

/// LAPACK `dgecon('1')`: the reciprocal 1-norm condition number of the matrix
/// whose LU factors are `lu`, given the original matrix's 1-norm.
fn dgecon(lu: &[f64], n: usize, anorm: f64) -> f64 {
    if n == 0 {
        return 1.0;
    }
    if anorm.is_nan() {
        return anorm;
    }
    if anorm == 0.0 {
        return 0.0;
    }
    let smlnum = SFMIN;
    let mut st = Lacn2 {
        jump: 0,
        j: 0,
        iter: 0,
        est: 0.0,
        isgn: vec![0; n],
        v: vec![0.0; n],
    };
    let mut x = vec![0.0; n];
    let mut kase = st.step(&mut x, 0);
    while kase != 0 {
        let (sl, su) = if kase == 1 {
            let sl = dlatrs(Tri::LowerUnit, false, lu, n, &mut x);
            let su = dlatrs(Tri::UpperNonUnit, false, lu, n, &mut x);
            (sl, su)
        } else {
            let su = dlatrs(Tri::UpperNonUnit, true, lu, n, &mut x);
            let sl = dlatrs(Tri::LowerUnit, true, lu, n, &mut x);
            (sl, su)
        };
        let scale = sl * su;
        if scale != 1.0 {
            let ix = idamax(&x);
            if scale < x[ix].abs() * smlnum || scale == 0.0 {
                return 0.0;
            }
            drscl(&mut x, scale);
        }
        kase = st.step(&mut x, kase);
    }
    let ainvnm = st.est;
    if ainvnm != 0.0 {
        (1.0 / ainvnm) / anorm
    } else {
        0.0
    }
}

/// Why `La_solve` refused a system.
pub enum SolveError {
    /// `dgesv` met an exactly zero pivot `U[i,i]` (1-based `i`).
    Singular(usize),
    /// `dgecon`'s reciprocal condition number fell below `tol`.
    IllConditioned(f64),
}

/// `La_solve`'s numeric core: solve `A X = B` for the `nrhs` columns of `b`
/// (column-major, `n` rows) in place, then reject a system whose reciprocal
/// condition number is below `tol` (skipped when `tol <= 0`).
pub fn solve(a: &[f64], n: usize, b: &mut [f64], nrhs: usize, tol: f64) -> Result<(), SolveError> {
    let mut f = a.to_vec();
    let (ipiv, info) = lu(&mut f, n);
    if info > 0 {
        return Err(SolveError::Singular(info));
    }
    dgetrs(&f, n, &ipiv, b, nrhs);
    if tol > 0.0 {
        let rcond = dgecon(&f, n, one_norm(a, n));
        if rcond < tol {
            return Err(SolveError::IllConditioned(rcond));
        }
    }
    Ok(())
}

/// `det_ge_real`: `(modulus, sign)` of the square matrix `a`, the modulus as
/// its log when `log` is set. An exactly singular matrix is `(-Inf | 0, 1)`.
pub fn determinant(a: &[f64], n: usize, log: bool) -> (f64, i32) {
    let mut f = a.to_vec();
    let (ipiv, info) = lu(&mut f, n);
    let mut sign = 1;
    if info > 0 {
        return (if log { f64::NEG_INFINITY } else { 0.0 }, sign);
    }
    for (i, &p) in ipiv.iter().enumerate() {
        if p != i {
            sign = -sign;
        }
    }
    let mut modulus = if log { 0.0 } else { 1.0 };
    for i in 0..n {
        let dii = f[i * (n + 1)];
        if log {
            modulus += dii.abs().ln();
            if dii < 0.0 {
                sign = -sign;
            }
        } else {
            modulus *= dii;
        }
    }
    if !log && modulus < 0.0 {
        modulus = -modulus;
        sign = -sign;
    }
    (modulus, sign)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lu_pivots_and_singular_index() {
        // Column 1 pivots on row 2; the 2x2 [1 2; 2 4] is singular at U[2,2].
        let (_, info) = lu(&mut [1.0, 2.0, 2.0, 4.0], 2);
        assert_eq!(info, 2);
        let (piv, info) = lu(&mut [1.0, 3.0, 2.0, 4.0], 2);
        assert_eq!((piv, info), (vec![1, 1], 0));
    }

    #[test]
    fn na_payload_survives_arithmetic() {
        let x = na_real() * 2.0 + 1.0;
        assert!(is_na_real(x));
        assert!(!is_na_real(f64::NAN));
    }
}
