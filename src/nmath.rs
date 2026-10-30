//! The normal distribution from R's standalone math library (`src/nmath`).
//!
//! Ports of `dnorm4` (dnorm.c), `pnorm5` / `pnorm_both` (pnorm.c, Cody's
//! rational Chebyshev approximations) and `qnorm5` (qnorm.c, Wichura's AS 241
//! with Maechler's 2022 extreme-tail asymptotics) from R 4.6, operation for
//! operation, so results match R to the last bit. `NaN` returned for an
//! invalid parameter is nmath's `ML_WARN_return_NAN`; the vectorised caller
//! turns it into R's "NaNs produced" warning.

// The coefficient tables carry every digit R's C source prints, so each one can
// be checked against nmath by eye; they round to the same `f64` either way.
#![allow(clippy::excessive_precision)]

use std::f64::consts::{LN_2, PI, SQRT_2};

/// `M_LN_SQRT_2PI`: log(sqrt(2 * pi)).
const LN_SQRT_2PI: f64 = 0.918_938_533_204_672_741_780_329_736_406;
/// `M_1_SQRT_2PI`: 1 / sqrt(2 * pi).
const ONE_SQRT_2PI: f64 = 0.398_942_280_401_432_677_939_946_059_934;
/// `M_SQRT_32`: sqrt(32).
const SQRT_32: f64 = 5.656_854_249_492_380_195_206_754_896_838;

/// `ldexp(x, e)`: x * 2^e, exact for the small exponents used here.
fn ldexp(x: f64, e: i32) -> f64 {
    x * 2f64.powi(e)
}

/// `R_D__0`: 0 on the probability scale, -Inf on the log scale.
fn d0(log_p: bool) -> f64 {
    if log_p {
        f64::NEG_INFINITY
    } else {
        0.0
    }
}

/// `R_D__1`.
fn d1(log_p: bool) -> f64 {
    if log_p {
        0.0
    } else {
        1.0
    }
}

/// `R_DT_0`.
fn dt0(lower: bool, log_p: bool) -> f64 {
    if lower {
        d0(log_p)
    } else {
        d1(log_p)
    }
}

/// `R_DT_1`.
fn dt1(lower: bool, log_p: bool) -> f64 {
    if lower {
        d1(log_p)
    } else {
        d0(log_p)
    }
}

/// `dnorm4(x, mu, sigma, give_log)`.
pub fn dnorm(x: f64, mu: f64, sigma: f64, give_log: bool) -> f64 {
    if x.is_nan() || mu.is_nan() || sigma.is_nan() {
        return x + mu + sigma;
    }
    if sigma < 0.0 {
        return f64::NAN;
    }
    if !sigma.is_finite() {
        return d0(give_log);
    }
    if !x.is_finite() && mu == x {
        return f64::NAN;
    }
    if sigma == 0.0 {
        return if x == mu { f64::INFINITY } else { d0(give_log) };
    }
    let x = (x - mu) / sigma;
    if !x.is_finite() {
        return d0(give_log);
    }
    let x = x.abs();
    if x >= 2.0 * f64::MAX.sqrt() {
        return d0(give_log);
    }
    if give_log {
        return -(LN_SQRT_2PI + 0.5 * x * x + sigma.ln());
    }
    if x < 5.0 {
        return ONE_SQRT_2PI * (-0.5 * x * x).exp() / sigma;
    }
    // DBL_MIN_EXP + 1 - DBL_MANT_DIG = -1021 + 1 - 53
    if x > (-2.0 * LN_2 * (f64::MIN_EXP + 1 - f64::MANTISSA_DIGITS as i32) as f64).sqrt() {
        return 0.0;
    }
    let x1 = ldexp(ldexp(x, 16).round_ties_even(), -16);
    let x2 = x - x1;
    ONE_SQRT_2PI / sigma * ((-0.5 * x1 * x1).exp() * ((-0.5 * x2 - x1) * x2).exp())
}

/// `pnorm5(x, mu, sigma, lower_tail, log_p)`.
pub fn pnorm(x: f64, mu: f64, sigma: f64, lower: bool, log_p: bool) -> f64 {
    if x.is_nan() || mu.is_nan() || sigma.is_nan() {
        return x + mu + sigma;
    }
    if !x.is_finite() && mu == x {
        return f64::NAN;
    }
    if sigma <= 0.0 {
        if sigma < 0.0 {
            return f64::NAN;
        }
        return if x < mu {
            dt0(lower, log_p)
        } else {
            dt1(lower, log_p)
        };
    }
    let p = (x - mu) / sigma;
    if !p.is_finite() {
        return if x < mu {
            dt0(lower, log_p)
        } else {
            dt1(lower, log_p)
        };
    }
    let (cum, ccum) = pnorm_both(p, if lower { 0 } else { 1 }, log_p);
    if lower {
        cum
    } else {
        ccum
    }
}

/// `pnorm_both`: the lower (`i_tail` 0), upper (1) or both (2) tails of the
/// standard normal at `x`, as `(cum, ccum)`.
pub fn pnorm_both(x: f64, i_tail: u8, log_p: bool) -> (f64, f64) {
    const A: [f64; 5] = [
        2.2352520354606839287,
        161.02823106855587881,
        1067.6894854603709582,
        18154.981253343561249,
        0.065682337918207449113,
    ];
    const B: [f64; 4] = [
        47.20258190468824187,
        976.09855173777669322,
        10260.932208618978205,
        45507.789335026729956,
    ];
    const C: [f64; 9] = [
        0.39894151208813466764,
        8.8831497943883759412,
        93.506656132177855979,
        597.27027639480026226,
        2494.5375852903726711,
        6848.1904505362823326,
        11602.651437647350124,
        9842.7148383839780218,
        1.0765576773720192317e-8,
    ];
    const D: [f64; 8] = [
        22.266688044328115691,
        235.38790178262499861,
        1519.377599407554805,
        6485.558298266760755,
        18615.571640885098091,
        34900.952721145977266,
        38912.003286093271411,
        19685.429676859990727,
    ];
    const P: [f64; 6] = [
        0.21589853405795699,
        0.1274011611602473639,
        0.022235277870649807,
        0.001421619193227893466,
        2.9112874951168792e-5,
        0.02307344176494017303,
    ];
    const Q: [f64; 5] = [
        1.28426009614491121,
        0.468238212480865118,
        0.0659881378689285515,
        0.00378239633202758244,
        7.29751555083966205e-5,
    ];

    if x.is_nan() {
        return (x, x);
    }
    let eps = f64::EPSILON * 0.5;
    let lower = i_tail != 1;
    let upper = i_tail != 0;
    let (mut cum, mut ccum) = (0.0, 0.0);

    // `do_del(X)` then `swap_tail` from pnorm.c.
    let del_and_swap = |xx: f64, temp: f64, cum: &mut f64, ccum: &mut f64| {
        let xsq = ldexp(ldexp(xx, 4).trunc(), -4);
        let del = (xx - xsq) * (xx + xsq);
        if log_p {
            *cum = (-xsq * ldexp(xsq, -1)) - ldexp(del, -1) + temp.ln();
            if (lower && x > 0.0) || (upper && x <= 0.0) {
                *ccum = (-(-xsq * ldexp(xsq, -1)).exp() * (-ldexp(del, -1)).exp() * temp).ln_1p();
            }
        } else {
            *cum = (-xsq * ldexp(xsq, -1)).exp() * (-ldexp(del, -1)).exp() * temp;
            *ccum = 1.0 - *cum;
        }
        if x > 0.0 {
            let t = *cum;
            if lower {
                *cum = *ccum;
            }
            *ccum = t;
        }
    };

    let y = x.abs();
    if y <= 0.67448975 {
        let (xnum, xden) = if y > eps {
            let xsq = x * x;
            let mut xnum = A[4] * xsq;
            let mut xden = xsq;
            for i in 0..3 {
                xnum = (xnum + A[i]) * xsq;
                xden = (xden + B[i]) * xsq;
            }
            (xnum, xden)
        } else {
            (0.0, 0.0)
        };
        let temp = x * (xnum + A[3]) / (xden + B[3]);
        if lower {
            cum = 0.5 + temp;
        }
        if upper {
            ccum = 0.5 - temp;
        }
        if log_p {
            if lower {
                cum = cum.ln();
            }
            if upper {
                ccum = ccum.ln();
            }
        }
    } else if y <= SQRT_32 {
        let mut xnum = C[8] * y;
        let mut xden = y;
        for i in 0..7 {
            xnum = (xnum + C[i]) * y;
            xden = (xden + D[i]) * y;
        }
        let temp = (xnum + C[7]) / (xden + D[7]);
        del_and_swap(y, temp, &mut cum, &mut ccum);
    } else if (log_p && y < 1e170)
        || (lower && -38.4674 < x && x < 8.2924)
        || (upper && -8.2924 < x && x < 38.4674)
    {
        let xsq = 1.0 / (x * x);
        let mut xnum = P[5] * xsq;
        let mut xden = xsq;
        for i in 0..4 {
            xnum = (xnum + P[i]) * xsq;
            xden = (xden + Q[i]) * xsq;
        }
        let temp = xsq * (xnum + P[4]) / (xden + Q[4]);
        let temp = (ONE_SQRT_2PI - temp) / y;
        del_and_swap(x, temp, &mut cum, &mut ccum);
    } else if x > 0.0 {
        cum = d1(log_p);
        ccum = d0(log_p);
    } else {
        cum = d0(log_p);
        ccum = d1(log_p);
    }
    (cum, ccum)
}

/// `qnorm5(p, mu, sigma, lower_tail, log_p)`.
pub fn qnorm(p: f64, mu: f64, sigma: f64, lower: bool, log_p: bool) -> f64 {
    if p.is_nan() || mu.is_nan() || sigma.is_nan() {
        return p + mu + sigma;
    }
    // R_Q_P01_boundaries(p, -Inf, Inf)
    if log_p {
        if p > 0.0 {
            return f64::NAN;
        }
        if p == 0.0 {
            return if lower {
                f64::INFINITY
            } else {
                f64::NEG_INFINITY
            };
        }
        if p == f64::NEG_INFINITY {
            return if lower {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            };
        }
    } else {
        if !(0.0..=1.0).contains(&p) {
            return f64::NAN;
        }
        if p == 0.0 {
            return if lower {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            };
        }
        if p == 1.0 {
            return if lower {
                f64::INFINITY
            } else {
                f64::NEG_INFINITY
            };
        }
    }
    if sigma < 0.0 {
        return f64::NAN;
    }
    if sigma == 0.0 {
        return mu;
    }

    // R_DT_qIv(p): the lower-tail probability.
    let p_ = if log_p {
        if lower {
            p.exp()
        } else {
            -p.exp_m1()
        }
    } else if lower {
        p
    } else {
        0.5 - p + 0.5
    };
    let q = p_ - 0.5;
    let val = if q.abs() <= 0.425 {
        let r = 0.180625 - q * q;
        q * (((((((r * 2509.0809287301226727 + 33430.575583588128105) * r
            + 67265.770927008700853)
            * r
            + 45921.953931549871457)
            * r
            + 13731.693765509461125)
            * r
            + 1971.5909503065514427)
            * r
            + 133.14166789178437745)
            * r
            + 3.387132872796366608)
            / (((((((r * 5226.495278852854561 + 28729.085735721942674) * r
                + 39307.89580009271061)
                * r
                + 21213.794301586595867)
                * r
                + 5394.1960214247511077)
                * r
                + 687.1870074920579083)
                * r
                + 42.313330701600911252)
                * r
                + 1.0)
    } else {
        let lp = if log_p && ((lower && q <= 0.0) || (!lower && q > 0.0)) {
            p
        } else {
            // R_DT_CIv(p) when q > 0, else p_.
            let tail = if q > 0.0 {
                if log_p {
                    if lower {
                        -p.exp_m1()
                    } else {
                        p.exp()
                    }
                } else if lower {
                    0.5 - p + 0.5
                } else {
                    p
                }
            } else {
                p_
            };
            tail.ln()
        };
        let mut r = (-lp).sqrt();
        let mut v;
        if r <= 5.0 {
            r += -1.6;
            v = (((((((r * 7.7454501427834140764e-4 + 0.0227238449892691845833) * r
                + 0.24178072517745061177)
                * r
                + 1.27045825245236838258)
                * r
                + 3.64784832476320460504)
                * r
                + 5.7694972214606914055)
                * r
                + 4.6303378461565452959)
                * r
                + 1.42343711074968357734)
                / (((((((r * 1.05075007164441684324e-9 + 5.475938084995344946e-4) * r
                    + 0.0151986665636164571966)
                    * r
                    + 0.14810397642748007459)
                    * r
                    + 0.68976733498510000455)
                    * r
                    + 1.6763848301838038494)
                    * r
                    + 2.05319162663775882187)
                    * r
                    + 1.0);
        } else if r <= 27.0 {
            r += -5.0;
            v = (((((((r * 2.01033439929228813265e-7 + 2.71155556874348757815e-5) * r
                + 0.0012426609473880784386)
                * r
                + 0.026532189526576123093)
                * r
                + 0.29656057182850489123)
                * r
                + 1.7848265399172913358)
                * r
                + 5.4637849111641143699)
                * r
                + 6.6579046435011037772)
                / (((((((r * 2.04426310338993978564e-15 + 1.4215117583164458887e-7) * r
                    + 1.8463183175100546818e-5)
                    * r
                    + 7.868691311456132591e-4)
                    * r
                    + 0.0148753612908506148525)
                    * r
                    + 0.13692988092273580531)
                    * r
                    + 0.59983220655588793769)
                    * r
                    + 1.0);
        } else if r >= 6.4e8 {
            v = r * SQRT_2;
        } else {
            let two_pi = 2.0 * PI;
            let s2 = -ldexp(lp, 1);
            let mut x2 = s2 - (two_pi * s2).ln();
            if r < 36000.0 {
                x2 = s2 - (two_pi * x2).ln() - 2.0 / (2.0 + x2);
                if r < 840.0 {
                    x2 = s2 - (two_pi * x2).ln()
                        + 2.0 * (-(1.0 - 1.0 / (4.0 + x2)) / (2.0 + x2)).ln_1p();
                    if r < 109.0 {
                        x2 = s2 - (two_pi * x2).ln()
                            + 2.0
                                * (-(1.0 - (1.0 - 5.0 / (6.0 + x2)) / (4.0 + x2)) / (2.0 + x2))
                                    .ln_1p();
                        if r < 55.0 {
                            x2 = s2 - (two_pi * x2).ln()
                                + 2.0
                                    * (-(1.0
                                        - (1.0 - (5.0 - 9.0 / (8.0 + x2)) / (6.0 + x2))
                                            / (4.0 + x2))
                                        / (2.0 + x2))
                                        .ln_1p();
                        }
                    }
                }
            }
            v = x2.sqrt();
        }
        if q < 0.0 {
            v = -v;
        }
        v
    };
    mu + sigma * val
}
