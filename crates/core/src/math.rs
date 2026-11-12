//! Transcendental functions that give the same bits on every target. `f64::ln`, `exp`, `sin`, ... call the platform's
//! math library (glibc natively, a port of musl in wasm), whose last bits differ; the engine must give the browser, the
//! server and the Python wheel byte-equal results (tests/xbind). These go through the `libm` crate everywhere instead:
//! plain IEEE arithmetic, so the same on every target.

/// Deterministic `ln`, `exp`, `sin`, `cos`, `atan2` and `powf` for `f64`.
pub trait Det {
    fn det_ln(self) -> f64;
    fn det_exp(self) -> f64;
    fn det_sin(self) -> f64;
    fn det_cos(self) -> f64;
    fn det_sin_cos(self) -> (f64, f64);
    fn det_atan2(self, x: f64) -> f64;
    fn det_powf(self, p: f64) -> f64;
}

impl Det for f64 {
    #[inline]
    fn det_ln(self) -> f64 {
        libm::log(self)
    }
    #[inline]
    fn det_exp(self) -> f64 {
        libm::exp(self)
    }
    #[inline]
    fn det_sin(self) -> f64 {
        libm::sin(self)
    }
    #[inline]
    fn det_cos(self) -> f64 {
        libm::cos(self)
    }
    #[inline]
    fn det_sin_cos(self) -> (f64, f64) {
        libm::sincos(self)
    }
    #[inline]
    fn det_atan2(self, x: f64) -> f64 {
        libm::atan2(self, x)
    }
    #[inline]
    fn det_powf(self, p: f64) -> f64 {
        libm::pow(self, p)
    }
}

/// ln Γ(x) for x > 0.
pub fn ln_gamma(x: f64) -> f64 {
    libm::lgamma(x)
}

/// The standard normal CDF.
pub fn normal_cdf(z: f64) -> f64 {
    0.5 * libm::erfc(-z / std::f64::consts::SQRT_2)
}

/// The regularized upper incomplete gamma function Q(a, x) = Γ(a, x) / Γ(a), for a > 0 and x >= 0: by its series below
/// x = a + 1 and by its continued fraction (modified Lentz) above (Numerical Recipes, 3rd ed., section 6.2).
pub fn gamma_q(a: f64, x: f64) -> f64 {
    if !(a > 0.0) || !(x >= 0.0) {
        return f64::NAN;
    }
    if x == 0.0 {
        return 1.0;
    }
    let front = (-x + a * x.det_ln() - ln_gamma(a)).det_exp();
    if x < a + 1.0 {
        let (mut ap, mut del) = (a, 1.0 / a);
        let mut sum = del;
        for _ in 0..1000 {
            ap += 1.0;
            del *= x / ap;
            sum += del;
            if del.abs() < sum.abs() * 1e-16 {
                break;
            }
        }
        (1.0 - sum * front).max(0.0)
    } else {
        const TINY: f64 = 1e-300;
        let mut b = x + 1.0 - a;
        let mut c = 1.0 / TINY;
        let mut d = 1.0 / b;
        let mut h = d;
        for i in 1..1000 {
            let an = -(i as f64) * (i as f64 - a);
            b += 2.0;
            d = an * d + b;
            if d.abs() < TINY {
                d = TINY;
            }
            c = b + an / c;
            if c.abs() < TINY {
                c = TINY;
            }
            d = 1.0 / d;
            let del = d * c;
            h *= del;
            if (del - 1.0).abs() < 1e-16 {
                break;
            }
        }
        front * h
    }
}

/// The chi-squared survival function with `k` degrees of freedom.
pub fn chi2_sf(x: f64, k: usize) -> f64 {
    if k == 0 || !x.is_finite() {
        return f64::NAN;
    }
    gamma_q(k as f64 / 2.0, x.max(0.0) / 2.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_known_values() {
        assert!((chi2_sf(3.841_458_820_694_124, 1) - 0.05).abs() < 1e-12);
        assert!((chi2_sf(5.991_464_547_107_979, 2) - 0.05).abs() < 1e-12);
        assert!((chi2_sf(18.307_038_053_275_146, 10) - 0.05).abs() < 1e-12);
        assert!((chi2_sf(0.5, 4) - 0.973_500_978_839_256_1).abs() < 1e-12);
        assert!((normal_cdf(1.959_963_984_540_054) - 0.975).abs() < 1e-14);
        assert!((normal_cdf(0.0) - 0.5).abs() < 1e-16);
        assert!((ln_gamma(5.0) - 24f64.ln()).abs() < 1e-13);
        assert!((2f64.det_ln() - std::f64::consts::LN_2).abs() < 1e-16);
        assert!((1f64.det_exp() - std::f64::consts::E).abs() < 1e-15);
    }
}
