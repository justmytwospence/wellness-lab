//! Statistics with the semantics of polars and numpy, which the HRV notebook's goldens come from.
//! NaN is a missing value: every summary skips it, and rolling windows count only the values present.

use crate::math::Det;
pub use crate::math::{chi2_sf, normal_cdf};

pub fn nan_len(x: &[f64]) -> usize {
    x.iter().filter(|v| !v.is_nan()).count()
}

pub fn mean(x: &[f64]) -> f64 {
    let (mut s, mut n) = (0.0, 0usize);
    for &v in x {
        if !v.is_nan() {
            s += v;
            n += 1;
        }
    }
    if n == 0 {
        f64::NAN
    } else {
        s / n as f64
    }
}

pub fn sum(x: &[f64]) -> f64 {
    x.iter().filter(|v| !v.is_nan()).sum()
}

/// Standard deviation with `ddof` (1 for polars, 0 for numpy's default).
pub fn std(x: &[f64], ddof: usize) -> f64 {
    let n = nan_len(x);
    if n <= ddof {
        return f64::NAN;
    }
    let m = mean(x);
    let ss: f64 = x
        .iter()
        .filter(|v| !v.is_nan())
        .map(|v| (v - m) * (v - m))
        .sum();
    (ss / (n - ddof) as f64).sqrt()
}

pub fn sorted(x: &[f64]) -> Vec<f64> {
    let mut v: Vec<f64> = x.iter().copied().filter(|v| !v.is_nan()).collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v
}

pub fn median(x: &[f64]) -> f64 {
    let v = sorted(x);
    let n = v.len();
    match n {
        0 => f64::NAN,
        _ if n % 2 == 1 => v[n / 2],
        _ => (v[n / 2 - 1] + v[n / 2]) / 2.0,
    }
}

/// Median absolute deviation scaled to a standard deviation for normal data.
pub fn mad(x: &[f64]) -> f64 {
    let m = median(x);
    let d: Vec<f64> = x.iter().map(|v| (v - m).abs()).collect();
    1.4826 * median(&d)
}

/// polars' default `quantile` ("nearest"): the value at `round((n - 1) * q)` of the sorted values.
pub fn quantile_nearest(x: &[f64], q: f64) -> f64 {
    let mut v: Vec<f64> = x.iter().copied().filter(|v| !v.is_nan()).collect();
    if v.is_empty() {
        return f64::NAN;
    }
    let idx = (((v.len() - 1) as f64 * q).round() as usize).min(v.len() - 1);
    // selection, not a full sort: the value at `idx` of the sorted order
    *v.select_nth_unstable_by(idx, |a, b| a.partial_cmp(b).unwrap())
        .1
}

/// numpy's default (linear interpolation) quantile.
pub fn quantile_linear(x: &[f64], q: f64) -> f64 {
    let v = sorted(x);
    quantile_linear_sorted(&v, q)
}

pub fn quantile_linear_sorted(v: &[f64], q: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    let pos = (v.len() - 1) as f64 * q;
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    v[lo] + (v[hi] - v[lo]) * (pos - lo as f64)
}

/// A trailing (or centred) window of `window` rows ending at row i; `f` sees the non-NaN values and runs when at
/// least `min_samples` are present.
pub fn rolling(
    x: &[f64],
    window: usize,
    min_samples: usize,
    center: bool,
    f: impl Fn(&[f64]) -> f64,
) -> Vec<f64> {
    let n = x.len();
    let mut out = vec![f64::NAN; n];
    let mut buf = Vec::with_capacity(window);
    for i in 0..n {
        let (a, b) = if center {
            // odd windows are symmetric about the row (the only centred windows used); even ones take the extra row after
            let half = window / 2;
            let a = i.saturating_sub(half);
            let b = if window % 2 == 1 {
                i + half + 1
            } else {
                i + half
            };
            (a, b.min(n).max(a))
        } else {
            ((i + 1).saturating_sub(window), i + 1)
        };
        buf.clear();
        buf.extend(x[a..b].iter().copied().filter(|v| !v.is_nan()));
        if buf.len() >= min_samples.max(1) {
            out[i] = f(&buf);
        }
    }
    out
}

pub fn rolling_mean(x: &[f64], window: usize, min_samples: usize) -> Vec<f64> {
    rolling(x, window, min_samples, false, mean)
}

pub fn rolling_std(x: &[f64], window: usize, min_samples: usize) -> Vec<f64> {
    rolling(x, window, min_samples, false, |v| std(v, 1))
}

pub fn rolling_median(x: &[f64], window: usize, min_samples: usize, center: bool) -> Vec<f64> {
    rolling(x, window, min_samples, center, median)
}

pub fn rolling_quantile_nearest(
    x: &[f64],
    q: f64,
    window: usize,
    min_samples: usize,
    center: bool,
) -> Vec<f64> {
    rolling(x, window, min_samples, center, |v| quantile_nearest(v, q))
}

/// Shift by `k` rows (positive: later rows take earlier values); vacated rows are NaN.
pub fn shift(x: &[f64], k: isize) -> Vec<f64> {
    let n = x.len() as isize;
    (0..n)
        .map(|i| {
            let j = i - k;
            if (0..n).contains(&j) {
                x[j as usize]
            } else {
                f64::NAN
            }
        })
        .collect()
}

/// Solve the least squares problem min |A x - y| by Householder QR. `a` is row-major (n rows of p columns).
/// Returns None when the columns are rank deficient.
pub fn lstsq(a: &[Vec<f64>], y: &[f64]) -> Option<Vec<f64>> {
    let n = a.len();
    let p = a.first().map(|r| r.len()).unwrap_or(0);
    if n < p || p == 0 {
        return None;
    }
    let mut r: Vec<Vec<f64>> = a.to_vec();
    let mut qty = y.to_vec();
    for k in 0..p {
        let norm: f64 = (k..n).map(|i| r[i][k] * r[i][k]).sum::<f64>().sqrt();
        if norm < 1e-12 {
            return None;
        }
        let alpha = if r[k][k] > 0.0 { -norm } else { norm };
        let mut v: Vec<f64> = (k..n).map(|i| r[i][k]).collect();
        v[0] -= alpha;
        let vnorm2: f64 = v.iter().map(|x| x * x).sum();
        if vnorm2 < 1e-300 {
            continue;
        }
        for j in k..p {
            let dot: f64 = (k..n).map(|i| v[i - k] * r[i][j]).sum();
            let f = 2.0 * dot / vnorm2;
            for i in k..n {
                r[i][j] -= f * v[i - k];
            }
        }
        let dot: f64 = (k..n).map(|i| v[i - k] * qty[i]).sum();
        let f = 2.0 * dot / vnorm2;
        for i in k..n {
            qty[i] -= f * v[i - k];
        }
    }
    let mut x = vec![0.0; p];
    for k in (0..p).rev() {
        let mut s = qty[k];
        for j in k + 1..p {
            s -= r[k][j] * x[j];
        }
        if r[k][k].abs() < 1e-12 {
            return None;
        }
        x[k] = s / r[k][k];
    }
    Some(x)
}

/// numpy.polyfit(x, y, deg, w=w) coefficients, highest power first (the weights multiply the residuals).
pub fn polyfit(x: &[f64], y: &[f64], deg: usize, w: Option<&[f64]>) -> Option<Vec<f64>> {
    let a: Vec<Vec<f64>> = x
        .iter()
        .enumerate()
        .map(|(i, &xi)| {
            let wi = w.map(|w| w[i]).unwrap_or(1.0);
            (0..=deg).map(|k| wi * xi.powi((deg - k) as i32)).collect()
        })
        .collect();
    let yy: Vec<f64> = y
        .iter()
        .enumerate()
        .map(|(i, &v)| v * w.map(|w| w[i]).unwrap_or(1.0))
        .collect();
    lstsq(&a, &yy)
}

#[derive(Debug, Clone)]
pub struct OlsFit {
    pub beta: Vec<f64>,
    pub se: Vec<f64>,
    pub r2: f64,
    pub rmse: f64,
    pub resid: Vec<f64>,
}

/// Ordinary least squares with standard errors. `x` has one row per observation (add the intercept column yourself).
pub fn ols(x: &[Vec<f64>], y: &[f64]) -> Option<OlsFit> {
    let n = x.len();
    let p = x.first()?.len();
    let beta = lstsq(x, y)?;
    let resid: Vec<f64> = (0..n)
        .map(|i| y[i] - x[i].iter().zip(&beta).map(|(a, b)| a * b).sum::<f64>())
        .collect();
    let rss: f64 = resid.iter().map(|r| r * r).sum();
    let ym = mean(y);
    let tss: f64 = y.iter().map(|v| (v - ym) * (v - ym)).sum();
    let dof = n.saturating_sub(p).max(1) as f64;
    let sigma2 = rss / dof;
    // (X'X)^-1 diagonal by solving for each unit vector through the normal equations
    let mut xtx = vec![vec![0.0; p]; p];
    for row in x {
        for i in 0..p {
            for j in 0..p {
                xtx[i][j] += row[i] * row[j];
            }
        }
    }
    let inv = invert(&xtx)?;
    let se = (0..p)
        .map(|i| (sigma2 * inv[i][i]).max(0.0).sqrt())
        .collect();
    Some(OlsFit {
        beta,
        se,
        r2: if tss > 0.0 { 1.0 - rss / tss } else { f64::NAN },
        rmse: (rss / n as f64).sqrt(),
        resid,
    })
}

/// Gauss-Jordan inverse of a small matrix.
pub fn invert(a: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let n = a.len();
    let mut m: Vec<Vec<f64>> = (0..n)
        .map(|i| {
            a[i].iter()
                .copied()
                .chain((0..n).map(|j| (i == j) as u8 as f64))
                .collect()
        })
        .collect();
    for c in 0..n {
        let piv = (c..n).max_by(|&i, &j| m[i][c].abs().partial_cmp(&m[j][c].abs()).unwrap())?;
        if m[piv][c].abs() < 1e-12 {
            return None;
        }
        m.swap(c, piv);
        let d = m[c][c];
        for v in m[c].iter_mut() {
            *v /= d;
        }
        for r in 0..n {
            if r != c {
                let f = m[r][c];
                if f != 0.0 {
                    for k in 0..2 * n {
                        let t = m[c][k];
                        m[r][k] -= f * t;
                    }
                }
            }
        }
    }
    Some(m.into_iter().map(|r| r[n..].to_vec()).collect())
}

/// Non-negative least squares (Lawson and Hanson): min |A x - b| with x >= 0.
pub fn nnls(a: &[Vec<f64>], b: &[f64]) -> Vec<f64> {
    let p = a.first().map(|r| r.len()).unwrap_or(0);
    let n = a.len();
    let mut x = vec![0.0; p];
    let mut passive = vec![false; p];
    let resid = |x: &[f64]| -> Vec<f64> {
        (0..n)
            .map(|i| b[i] - a[i].iter().zip(x).map(|(u, v)| u * v).sum::<f64>())
            .collect()
    };
    for _ in 0..(3 * p + 10) {
        let r = resid(&x);
        let w: Vec<f64> = (0..p)
            .map(|j| (0..n).map(|i| a[i][j] * r[i]).sum())
            .collect();
        let Some((jmax, wmax)) = (0..p)
            .filter(|&j| !passive[j])
            .map(|j| (j, w[j]))
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        else {
            break;
        };
        if wmax <= 1e-10 {
            break;
        }
        passive[jmax] = true;
        loop {
            let idx: Vec<usize> = (0..p).filter(|&j| passive[j]).collect();
            let sub: Vec<Vec<f64>> = a
                .iter()
                .map(|row| idx.iter().map(|&j| row[j]).collect())
                .collect();
            let Some(z) = lstsq(&sub, b) else {
                passive[jmax] = false;
                return x;
            };
            if z.iter().all(|&v| v > 0.0) {
                x = vec![0.0; p];
                for (k, &j) in idx.iter().enumerate() {
                    x[j] = z[k];
                }
                break;
            }
            let mut alpha = f64::INFINITY;
            for (k, &j) in idx.iter().enumerate() {
                if z[k] <= 0.0 {
                    alpha = alpha.min(x[j] / (x[j] - z[k]));
                }
            }
            for (k, &j) in idx.iter().enumerate() {
                x[j] += alpha * (z[k] - x[j]);
            }
            for &j in &idx {
                if x[j].abs() < 1e-12 {
                    passive[j] = false;
                    x[j] = 0.0;
                }
            }
        }
    }
    x
}

/// Local linear regression with tricube weights over the nearest `frac` of the points, evaluated at `xs`.
pub fn loess(x: &[f64], y: &[f64], frac: f64, xs: &[f64]) -> Vec<f64> {
    let n = x.len();
    let k = ((frac * n as f64).ceil() as usize).clamp(2, n.max(2));
    xs.iter()
        .map(|&x0| {
            if n < 2 {
                return f64::NAN;
            }
            let mut d: Vec<(f64, usize)> = (0..n).map(|i| ((x[i] - x0).abs(), i)).collect();
            d.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            let h = d[k.min(n) - 1].0.max(1e-12);
            let (mut sw, mut swx, mut swy, mut swxx, mut swxy) = (0.0, 0.0, 0.0, 0.0, 0.0);
            for &(dist, i) in d.iter().take(k) {
                let u = dist / h;
                let w = if u < 1.0 {
                    (1.0 - u * u * u).powi(3)
                } else {
                    0.0
                };
                sw += w;
                swx += w * x[i];
                swy += w * y[i];
                swxx += w * x[i] * x[i];
                swxy += w * x[i] * y[i];
            }
            let den = sw * swxx - swx * swx;
            if sw <= 0.0 {
                f64::NAN
            } else if den.abs() < 1e-12 {
                swy / sw
            } else {
                let b = (sw * swxy - swx * swy) / den;
                let a = (swy - b * swx) / sw;
                a + b * x0
            }
        })
        .collect()
}

/// Cliff's delta of a against b: P(a > b) - P(a < b), by sorting.
pub fn cliffs_delta(a: &[f64], b: &[f64]) -> f64 {
    let (a, b) = (sorted(a), sorted(b));
    if a.is_empty() || b.is_empty() {
        return f64::NAN;
    }
    let (mut gt, mut lt) = (0u64, 0u64);
    for &x in &a {
        let lo = b.partition_point(|&y| y < x);
        let hi = b.partition_point(|&y| y <= x);
        gt += lo as u64;
        lt += (b.len() - hi) as u64;
    }
    (gt as f64 - lt as f64) / (a.len() as f64 * b.len() as f64)
}

/// Two-sample Kolmogorov-Smirnov statistic.
pub fn ks_d(a: &[f64], b: &[f64]) -> f64 {
    let (a, b) = (sorted(a), sorted(b));
    if a.is_empty() || b.is_empty() {
        return f64::NAN;
    }
    let (mut i, mut j, mut d) = (0, 0, 0.0f64);
    while i < a.len() && j < b.len() {
        let x = a[i].min(b[j]);
        while i < a.len() && a[i] <= x {
            i += 1;
        }
        while j < b.len() && b[j] <= x {
            j += 1;
        }
        d = d.max((i as f64 / a.len() as f64 - j as f64 / b.len() as f64).abs());
    }
    d
}

pub fn spearman_brown(r: f64) -> f64 {
    2.0 * r / (1.0 + r)
}

pub fn corr(a: &[f64], b: &[f64]) -> f64 {
    let pairs: Vec<(f64, f64)> = a
        .iter()
        .zip(b)
        .filter(|(x, y)| !x.is_nan() && !y.is_nan())
        .map(|(x, y)| (*x, *y))
        .collect();
    if pairs.len() < 3 {
        return f64::NAN;
    }
    let ma = pairs.iter().map(|p| p.0).sum::<f64>() / pairs.len() as f64;
    let mb = pairs.iter().map(|p| p.1).sum::<f64>() / pairs.len() as f64;
    let (mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0);
    for (x, y) in pairs {
        sab += (x - ma) * (y - mb);
        saa += (x - ma) * (x - ma);
        sbb += (y - mb) * (y - mb);
    }
    sab / (saa * sbb).sqrt()
}

/// Lag-1 autocorrelation.
pub fn ar1(x: &[f64]) -> f64 {
    let v: Vec<f64> = x.iter().copied().filter(|v| !v.is_nan()).collect();
    if v.len() < 4 {
        return 0.0;
    }
    let m = mean(&v);
    let num: f64 = v.windows(2).map(|w| (w[0] - m) * (w[1] - m)).sum();
    let den: f64 = v.iter().map(|a| (a - m) * (a - m)).sum();
    if den > 0.0 {
        num / den
    } else {
        0.0
    }
}

/// xoshiro256** seeded through SplitMix64.
#[derive(Clone)]
pub struct Rng {
    s: [u64; 4],
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        let mut z = seed;
        let mut next = || {
            z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut x = z;
            x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            x ^ (x >> 31)
        };
        Rng {
            s: [next(), next(), next(), next()],
        }
    }
    pub fn next_u64(&mut self) -> u64 {
        let r = self.s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        r
    }
    /// Uniform in [0, 1).
    pub fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    /// Uniform integer in [0, n).
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
    /// Standard normal (Box-Muller).
    pub fn normal(&mut self) -> f64 {
        let u1 = self.f64().max(1e-300);
        let u2 = self.f64();
        (-2.0 * u1.det_ln()).sqrt() * (std::f64::consts::TAU * u2).det_cos()
    }
}

/// Percentile bootstrap of `f` over resamples of `n_items` indices.
pub fn bootstrap_ci(
    n_items: usize,
    f: impl Fn(&[usize]) -> f64,
    n_boot: usize,
    seed: u64,
    alpha: f64,
) -> (f64, f64) {
    if n_items == 0 {
        return (f64::NAN, f64::NAN);
    }
    let mut rng = Rng::new(seed);
    let mut stats = Vec::with_capacity(n_boot);
    let mut idx = vec![0usize; n_items];
    for _ in 0..n_boot {
        for slot in idx.iter_mut() {
            *slot = rng.below(n_items);
        }
        let v = f(&idx);
        if v.is_finite() {
            stats.push(v);
        }
    }
    (
        quantile_linear(&stats, alpha / 2.0),
        quantile_linear(&stats, 1.0 - alpha / 2.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn summaries_skip_nan() {
        let x = [1.0, f64::NAN, 3.0, 5.0];
        assert!(close(mean(&x), 3.0));
        assert!(close(std(&x, 1), 2.0));
        assert!(close(std(&x, 0), (8.0f64 / 3.0).sqrt()));
        assert!(close(median(&x), 3.0));
        assert!(close(median(&[1.0, 2.0, 3.0, 4.0]), 2.5));
    }

    #[test]
    fn quantiles() {
        let x = [1.0, 2.0, 3.0, 4.0, 5.0];
        assert!(close(quantile_nearest(&x, 0.5), 3.0));
        assert!(close(quantile_nearest(&x, 0.25), 2.0)); // round(1.0) = 1
        assert!(close(quantile_nearest(&x, 1.0 / 3.0), 2.0)); // round(1.33)
        assert!(close(quantile_linear(&x, 0.25), 2.0));
        assert!(close(quantile_linear(&[1.0, 2.0], 0.25), 1.25));
    }

    #[test]
    fn rolling_counts_present_values() {
        let x = [1.0, 2.0, f64::NAN, 4.0, 5.0];
        let m = rolling_mean(&x, 3, 2);
        assert!(m[0].is_nan()); // one value only
        assert!(close(m[1], 1.5));
        assert!(close(m[2], 1.5)); // window 1, 2, NaN
        assert!(close(m[3], 3.0)); // 2, NaN, 4
        assert!(close(m[4], 4.5)); // NaN, 4, 5
        let s = rolling_std(&[1.0, 3.0, 5.0], 3, 3);
        assert!(close(s[2], 2.0));
        let sh = shift(&[1.0, 2.0, 3.0], 1);
        assert!(sh[0].is_nan() && close(sh[1], 1.0) && close(sh[2], 2.0));
        let sh = shift(&[1.0, 2.0, 3.0], -1);
        assert!(close(sh[0], 2.0) && sh[2].is_nan());
    }

    #[test]
    fn polyfit_matches_numpy() {
        // y = 2x + 1 exactly
        let c = polyfit(&[0.0, 1.0, 2.0, 3.0], &[1.0, 3.0, 5.0, 7.0], 1, None).unwrap();
        assert!(close(c[0], 2.0) && close(c[1], 1.0));
        // weights: the heavily weighted points dominate
        let c = polyfit(
            &[0.0, 1.0, 2.0],
            &[0.0, 1.0, 10.0],
            1,
            Some(&[10.0, 10.0, 0.001]),
        )
        .unwrap();
        assert!((c[0] - 1.0).abs() < 0.01 && c[1].abs() < 0.01);
    }

    #[test]
    fn ols_and_nnls() {
        let x: Vec<Vec<f64>> = (0..6).map(|i| vec![1.0, i as f64]).collect();
        let y: Vec<f64> = (0..6).map(|i| 2.0 + 3.0 * i as f64).collect();
        let f = ols(&x, &y).unwrap();
        assert!(close(f.beta[0], 2.0) && close(f.beta[1], 3.0) && f.r2 > 0.999999);
        let a = vec![vec![1.0, 0.0], vec![0.0, 1.0], vec![1.0, 1.0]];
        let sol = nnls(&a, &[1.0, -1.0, 0.0]);
        assert!(sol[1].abs() < 1e-9 && sol[0] >= 0.0);
        let sol = nnls(&a, &[1.0, 2.0, 3.0]);
        assert!(close(sol[0], 1.0) && close(sol[1], 2.0));
    }

    #[test]
    fn rank_statistics() {
        assert!(close(
            cliffs_delta(&[3.0, 4.0, 5.0], &[1.0, 2.0, 3.0]),
            8.0 / 9.0
        ));
        assert!(close(cliffs_delta(&[1.0], &[1.0]), 0.0));
        assert!(close(ks_d(&[1.0, 2.0, 3.0], &[4.0, 5.0, 6.0]), 1.0));
        assert!(close(spearman_brown(0.5), 2.0 / 3.0));
        assert!(close(corr(&[1.0, 2.0, 3.0], &[2.0, 4.0, 6.0]), 1.0));
    }

    #[test]
    fn rng_is_deterministic_and_uniform() {
        let mut a = Rng::new(7);
        let mut b = Rng::new(7);
        assert_eq!(a.next_u64(), b.next_u64());
        let m: f64 = (0..10_000).map(|_| a.f64()).sum::<f64>() / 10_000.0;
        assert!((m - 0.5).abs() < 0.02);
        let s: f64 = (0..10_000).map(|_| a.normal()).sum::<f64>() / 10_000.0;
        assert!(s.abs() < 0.05);
    }

    #[test]
    fn bootstrap_covers_the_mean() {
        let x: Vec<f64> = (0..50).map(|i| i as f64).collect();
        let (lo, hi) = bootstrap_ci(
            50,
            |idx| idx.iter().map(|&i| x[i]).sum::<f64>() / idx.len() as f64,
            500,
            1,
            0.05,
        );
        assert!(lo < 24.5 && hi > 24.5 && hi - lo < 10.0);
    }

    #[test]
    fn chi_square_tail() {
        assert!((chi2_sf(3.841, 1) - 0.05).abs() < 1e-3);
        assert!((chi2_sf(5.991, 2) - 0.05).abs() < 1e-3);
    }

    #[test]
    fn loess_recovers_a_line() {
        let x: Vec<f64> = (0..20).map(|i| i as f64).collect();
        let y: Vec<f64> = x.iter().map(|v| 2.0 * v + 1.0).collect();
        let f = loess(&x, &y, 0.5, &[5.0, 10.0]);
        assert!(close(f[0], 11.0) && close(f[1], 21.0));
    }
}
