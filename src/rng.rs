//! R's uniform random number generator and the samplers built on it.
//!
//! A port of the default generator stack of R 4.6 — `src/main/RNG.c`
//! (Mersenne-Twister, `set.seed`'s initial scrambling, `R_unif_index`),
//! `src/nmath/snorm.c` (`norm_rand` by inversion), `src/nmath/runif.c` /
//! `rnorm.c`, and `do_sample` from `src/main/random.c` — so that a seeded
//! stream reproduces R's numbers bit for bit: `set.seed(1); runif(3)`,
//! `rnorm`, and `sample` give the values the reference interpreter gives.
//!
//! Only R's default kinds are implemented: `"Mersenne-Twister"` for the
//! uniform stream, `"Inversion"` for normals, and both `"Rejection"` (the
//! default) and `"Rounding"` (pre-3.6.0) for `sample`. The state lives on the
//! host rather than in a `.Random.seed` variable in the global environment.

use crate::nmath::qnorm;

/// Mersenne-Twister state length (`N` in RNG.c).
const N: usize = 624;
/// Mersenne-Twister shift (`M` in RNG.c).
const M: usize = 397;
const MATRIX_A: u32 = 0x9908_b0df;
const UPPER_MASK: u32 = 0x8000_0000;
const LOWER_MASK: u32 = 0x7fff_ffff;
const TEMPERING_MASK_B: u32 = 0x9d2c_5680;
const TEMPERING_MASK_C: u32 = 0xefc6_0000;
/// `i2_32m1` in RNG.c: 1 / (2^32 - 1).
const I2_32M1: f64 = 2.328306437080797e-10;

/// How `sample` turns a uniform into an index (`Sampletype` in RNG.c).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SampleKind {
    /// `floor(n * unif_rand())`, R's sampler before 3.6.0.
    Rounding,
    /// Rejection sampling on 16-bit chunks, R's default since 3.6.0.
    Rejection,
}

/// The generator state: R's `dummy[]` array for Mersenne-Twister, whose
/// element 0 is `mti` and elements 1..=624 the twister's `mt[]`.
pub struct Rng {
    dummy: [u32; N + 1],
    /// Whether the stream has been seeded at all. The first draw of a session
    /// that never called `set.seed` seeds from the clock, as `Randomize` does.
    seeded: bool,
    pub sample_kind: SampleKind,
}

impl Default for Rng {
    fn default() -> Self {
        Rng {
            dummy: [0; N + 1],
            seeded: false,
            sample_kind: SampleKind::Rejection,
        }
    }
}

impl Rng {
    /// `RNG_Init(MERSENNE_TWISTER, seed)`: the 50-step LCG scramble, then one
    /// LCG step per seed word, then `FixupSeeds(kind, initial = 1)`.
    pub fn set_seed(&mut self, seed: i32) {
        let mut seed = seed as u32;
        for _ in 0..50 {
            seed = seed.wrapping_mul(69069).wrapping_add(1);
        }
        for j in 0..=N {
            seed = seed.wrapping_mul(69069).wrapping_add(1);
            self.dummy[j] = seed;
        }
        // FixupSeeds: `initial` sets mti = N, so the first draw regenerates.
        self.dummy[0] = N as u32;
        if self.dummy[1..].iter().all(|&w| w == 0) {
            self.reseed_from_clock();
        }
        self.seeded = true;
    }

    /// `Randomize`: seed from the clock and the process id, as `TimeToSeed`.
    pub fn reseed_from_clock(&mut self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let pid = std::process::id();
        let seed = ((now.subsec_micros() << 16) ^ (now.as_secs() as u32)) ^ (pid << 16);
        self.set_seed(seed as i32);
    }

    fn ensure_seeded(&mut self) {
        if !self.seeded {
            self.reseed_from_clock();
        }
    }

    /// `MT_sgenrand`: only reached when the state was never initialised.
    fn mt_sgenrand(&mut self, seed: u32) {
        let mut seed = seed;
        for i in 0..N {
            let mut w = seed & 0xffff_0000;
            seed = seed.wrapping_mul(69069).wrapping_add(1);
            w |= (seed & 0xffff_0000) >> 16;
            seed = seed.wrapping_mul(69069).wrapping_add(1);
            self.dummy[i + 1] = w;
        }
        self.dummy[0] = N as u32;
    }

    /// `MT_genrand`: one tempered 32-bit word scaled to [0, 1).
    fn mt_genrand(&mut self) -> f64 {
        const MAG01: [u32; 2] = [0, MATRIX_A];
        let mut mti = self.dummy[0] as usize;
        if mti >= N {
            if mti == N + 1 {
                self.mt_sgenrand(4357);
            }
            let mt = &mut self.dummy[1..];
            let mut kk = 0;
            while kk < N - M {
                let y = (mt[kk] & UPPER_MASK) | (mt[kk + 1] & LOWER_MASK);
                mt[kk] = mt[kk + M] ^ (y >> 1) ^ MAG01[(y & 1) as usize];
                kk += 1;
            }
            while kk < N - 1 {
                let y = (mt[kk] & UPPER_MASK) | (mt[kk + 1] & LOWER_MASK);
                mt[kk] = mt[kk + M - N] ^ (y >> 1) ^ MAG01[(y & 1) as usize];
                kk += 1;
            }
            let y = (mt[N - 1] & UPPER_MASK) | (mt[0] & LOWER_MASK);
            mt[N - 1] = mt[M - 1] ^ (y >> 1) ^ MAG01[(y & 1) as usize];
            mti = 0;
        }
        let mut y = self.dummy[mti + 1];
        mti += 1;
        y ^= y >> 11;
        y ^= (y << 7) & TEMPERING_MASK_B;
        y ^= (y << 15) & TEMPERING_MASK_C;
        y ^= y >> 18;
        self.dummy[0] = mti as u32;
        y as f64 * 2.3283064365386963e-10
    }

    /// `unif_rand`: a uniform on the open interval (0, 1).
    pub fn unif_rand(&mut self) -> f64 {
        self.ensure_seeded();
        fixup(self.mt_genrand())
    }

    /// `norm_rand` under `N01_kind == INVERSION`: two uniforms give 27 + 32
    /// bits of a probability, mapped through `qnorm`.
    pub fn norm_rand(&mut self) -> f64 {
        const BIG: f64 = 134_217_728.0; // 2^27
        let u1 = self.unif_rand();
        let u1 = (BIG * u1).trunc() + self.unif_rand();
        qnorm(u1 / BIG, 0.0, 1.0, true, false)
    }

    /// `rbits`: a uniform integer below `2^bits`, drawn in 16-bit chunks.
    fn rbits(&mut self, bits: i32) -> f64 {
        let mut v: u64 = 0;
        let mut n = 0;
        while n <= bits {
            let v1 = (self.unif_rand() * 65536.0).floor() as u64;
            v = v.wrapping_mul(65536).wrapping_add(v1);
            n += 16;
        }
        (v & ((1u64 << bits) - 1)) as f64
    }

    /// `R_unif_index`: a uniform integer in `0..dn`.
    pub fn unif_index(&mut self, dn: f64) -> f64 {
        if self.sample_kind == SampleKind::Rounding {
            // R_unif_index_0: Mersenne-Twister has 32 bits, so `ru()` is
            // only needed beyond INT_MAX.
            let u = if dn > i32::MAX as f64 {
                let big = 33_554_432.0;
                ((big * self.unif_rand()).floor() + self.unif_rand()) / big
            } else {
                self.unif_rand()
            };
            return (dn * u).floor();
        }
        if dn <= 0.0 {
            return 0.0;
        }
        let bits = dn.log2().ceil() as i32;
        loop {
            let dv = self.rbits(bits);
            if dn > dv {
                return dv;
            }
        }
    }
}

/// `fixup`: keep a uniform strictly inside (0, 1).
fn fixup(x: f64) -> f64 {
    if x <= 0.0 {
        return 0.5 * I2_32M1;
    }
    if 1.0 - x <= 0.0 {
        return 1.0 - 0.5 * I2_32M1;
    }
    x
}

/// `runif(a, b)` from nmath; `NaN` is its `ML_WARN_return_NAN`.
pub fn runif(rng: &mut Rng, a: f64, b: f64) -> f64 {
    if !a.is_finite() || !b.is_finite() || b < a {
        return f64::NAN;
    }
    if a == b {
        return a;
    }
    let mut u;
    loop {
        u = rng.unif_rand();
        if u > 0.0 && u < 1.0 {
            break;
        }
    }
    a + (b - a) * u
}

/// `rnorm(mu, sigma)` from nmath.
pub fn rnorm(rng: &mut Rng, mu: f64, sigma: f64) -> f64 {
    if mu.is_nan() || !sigma.is_finite() || sigma < 0.0 {
        return f64::NAN;
    }
    if sigma == 0.0 || !mu.is_finite() {
        return mu;
    }
    mu + sigma * rng.norm_rand()
}

/// `revsort`: heapsort `a` into descending order, permuting `ib` alongside.
fn revsort(a: &mut [f64], ib: &mut [i64]) {
    let n = a.len();
    if n <= 1 {
        return;
    }
    // The C indexes from 1 (`a--; ib--;`); `at(k)` is `a[k]` there.
    let mut l = (n >> 1) + 1;
    let mut ir = n;
    loop {
        let (ra, ii);
        if l > 1 {
            l -= 1;
            ra = a[l - 1];
            ii = ib[l - 1];
        } else {
            ra = a[ir - 1];
            ii = ib[ir - 1];
            a[ir - 1] = a[0];
            ib[ir - 1] = ib[0];
            ir -= 1;
            if ir == 1 {
                a[0] = ra;
                ib[0] = ii;
                return;
            }
        }
        let mut i = l;
        let mut j = l << 1;
        while j <= ir {
            if j < ir && a[j - 1] > a[j] {
                j += 1;
            }
            if ra > a[j - 1] {
                a[i - 1] = a[j - 1];
                ib[i - 1] = ib[j - 1];
                i = j;
                j += i;
            } else {
                j = ir + 1;
            }
        }
        a[i - 1] = ra;
        ib[i - 1] = ii;
    }
}

/// `do_sample` for a uniform draw (`prob = NULL`): `k` indices in `1..=n`.
pub fn sample_uniform(rng: &mut Rng, dn: f64, k: usize, replace: bool) -> Vec<f64> {
    let mut y = Vec::with_capacity(k);
    if replace || k < 2 {
        for _ in 0..k {
            y.push(rng.unif_index(dn) + 1.0);
        }
        return y;
    }
    let mut n = dn as usize;
    let mut x: Vec<f64> = (0..n).map(|i| i as f64).collect();
    for _ in 0..k {
        let j = rng.unif_index(n as f64) as usize;
        y.push(x[j] + 1.0);
        n -= 1;
        x[j] = x[n];
    }
    y
}

/// `FixupProb`: validate and normalise a probability vector in place.
pub fn fixup_prob(p: &mut [f64], require_k: usize, replace: bool) -> Result<(), String> {
    let mut sum = 0.0;
    let mut npos = 0usize;
    for &v in p.iter() {
        if !v.is_finite() {
            return Err("NA in probability vector".into());
        }
        if v < 0.0 {
            return Err("negative probability".into());
        }
        if v > 0.0 {
            npos += 1;
            sum += v;
        }
    }
    if npos == 0 || (!replace && require_k > npos) {
        return Err("too few positive probabilities".into());
    }
    for v in p.iter_mut() {
        *v /= sum;
    }
    Ok(())
}

/// `do_sample` with a (fixed-up) probability vector: `k` indices in `1..=n`.
pub fn sample_prob(rng: &mut Rng, p: &mut [f64], k: usize, replace: bool) -> Vec<i64> {
    let n = p.len();
    if replace {
        let nc = p.iter().filter(|&&v| n as f64 * v > 0.1).count();
        if nc > 200 {
            walker_sample(rng, p, k)
        } else {
            prob_sample_replace(rng, p, k)
        }
    } else {
        prob_sample_no_replace(rng, p, k)
    }
}

/// `ProbSampleReplace`: inversion on the cumulative sorted probabilities.
fn prob_sample_replace(rng: &mut Rng, p: &mut [f64], nans: usize) -> Vec<i64> {
    let n = p.len();
    let mut perm: Vec<i64> = (1..=n as i64).collect();
    revsort(p, &mut perm);
    for i in 1..n {
        p[i] += p[i - 1];
    }
    (0..nans)
        .map(|_| {
            let ru = rng.unif_rand();
            let mut j = 0;
            while j < n - 1 {
                if ru <= p[j] {
                    break;
                }
                j += 1;
            }
            perm[j]
        })
        .collect()
}

/// `walker_ProbSampleReplace`: Walker's alias method (Ripley 1987, Alg 3.13B).
fn walker_sample(rng: &mut Rng, p: &[f64], nans: usize) -> Vec<i64> {
    let n = p.len();
    let mut a = vec![0usize; n];
    let mut hl = vec![0usize; n];
    let mut q: Vec<f64> = p.iter().map(|v| v * n as f64).collect();
    // `H` grows up from the front of `hl`, `L` grows down from the back.
    let (mut h, mut l) = (0usize, n);
    for (i, &qi) in q.iter().enumerate() {
        if qi < 1.0 {
            hl[h] = i;
            h += 1;
        } else {
            l -= 1;
            hl[l] = i;
        }
    }
    if h > 0 && l < n {
        for k in 0..n - 1 {
            let i = hl[k];
            let j = hl[l];
            a[i] = j;
            q[j] += q[i] - 1.0;
            if q[j] < 1.0 {
                l += 1;
            }
            if l >= n {
                break;
            }
        }
    }
    for (i, qi) in q.iter_mut().enumerate() {
        *qi += i as f64;
    }
    (0..nans)
        .map(|_| {
            let (k, ru) = if rng.sample_kind == SampleKind::Rounding {
                let ru = rng.unif_rand() * n as f64;
                (ru as usize, ru)
            } else {
                let k = rng.unif_index(n as f64) as usize;
                (k, k as f64 + rng.unif_rand())
            };
            if ru < q[k] {
                k as i64 + 1
            } else {
                a[k] as i64 + 1
            }
        })
        .collect()
}

/// `ProbSampleNoReplace`: repeated inversion, removing each pick.
fn prob_sample_no_replace(rng: &mut Rng, p: &mut [f64], nans: usize) -> Vec<i64> {
    let n = p.len();
    let mut perm: Vec<i64> = (1..=n as i64).collect();
    revsort(p, &mut perm);
    let mut total = 1.0;
    let mut out = Vec::with_capacity(nans);
    let mut n1 = n as isize - 1;
    for _ in 0..nans {
        let rt = total * rng.unif_rand();
        let mut mass = 0.0;
        let mut j = 0usize;
        while (j as isize) < n1 {
            mass += p[j];
            if rt <= mass {
                break;
            }
            j += 1;
        }
        out.push(perm[j]);
        total -= p[j];
        for k in j..n1.max(0) as usize {
            p[k] = p[k + 1];
            perm[k] = perm[k + 1];
        }
        n1 -= 1;
    }
    out
}

/// `do_sample2` (src/main/unique.c), the sampler `sample.int` picks for
/// `n > 1e7` without replacement when `size <= n / 2`: redraw an index already
/// taken, at most 100 times per element, instead of allocating all `n` slots.
pub fn sample_hashed(rng: &mut Rng, dn: f64, k: usize) -> Vec<f64> {
    let mut seen = std::collections::HashSet::with_capacity(k);
    let mut y = Vec::with_capacity(k);
    for _ in 0..k {
        let mut v = 0.0;
        for _ in 0..100 {
            v = rng.unif_index(dn) + 1.0;
            if seen.insert(v as u64) {
                break;
            }
        }
        y.push(v);
    }
    y
}
