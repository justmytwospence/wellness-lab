//! State-space models of nightly signals: a local level (a signal that wanders plus noise) for the metric bench, and one
//! latent factor behind several signals at once (EM, Shumway and Stoffer 1982) for the "strain" model.

use crate::optim::{nelder_mead, NmOptions};
use crate::stats::{mean, std};
use serde::Serialize;

#[derive(Serialize, Clone, Debug, Default)]
pub struct LocalLevel {
    pub sigma_eps: f64,
    pub sigma_eta: f64,
    /// signal to noise: variance of the level's drift over the variance of the measurement noise
    pub q: f64,
    pub loglik: f64,
    pub level: Vec<f64>,
    pub level_sd: Vec<f64>,
}

fn filter(y: &[f64], se: f64, sn: f64) -> (f64, Vec<f64>, Vec<f64>) {
    let first = y.iter().copied().find(|v| v.is_finite()).unwrap_or(0.0);
    let var = std(y, 1).powi(2).max(1e-12);
    let (mut mu, mut p, mut ll) = (first, 1e3 * var, 0.0);
    let (mut lv, mut ls) = (vec![f64::NAN; y.len()], vec![f64::NAN; y.len()]);
    for t in 0..y.len() {
        let pp = p + sn * sn;
        if y[t].is_finite() {
            let f = pp + se * se;
            let v = y[t] - mu;
            let k = pp / f;
            mu += k * v;
            p = pp * se * se / f;
            ll += -0.5 * ((2.0 * std::f64::consts::PI * f).ln() + v * v / f);
        } else {
            p = pp;
        }
        lv[t] = mu;
        ls[t] = p.sqrt();
    }
    (ll, lv, ls)
}

pub fn local_level_fit(y: &[f64]) -> Option<LocalLevel> {
    if y.iter().filter(|v| v.is_finite()).count() < 20 {
        return None;
    }
    let s = std(y, 1).max(1e-9);
    let x0 = [(0.8 * s).ln(), (0.1 * s).ln()];
    let r = nelder_mead(
        |th| -filter(y, th[0].exp(), th[1].exp()).0,
        &x0,
        &NmOptions::defaults(2),
    );
    let (se, sn) = (r.x[0].exp(), r.x[1].exp());
    let (ll, level, level_sd) = filter(y, se, sn);
    Some(LocalLevel {
        sigma_eps: se,
        sigma_eta: sn,
        q: sn * sn / (se * se),
        loglik: ll,
        level,
        level_sd,
    })
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Dfm {
    pub factor: Vec<f64>,
    pub factor_sd: Vec<f64>,
    pub loadings: Vec<f64>,
    pub noise_sd: Vec<f64>,
    pub phi: f64,
    pub gamma: Vec<f64>,
    /// per signal and night: observed minus loading times factor, in units of the noise sd
    pub resid_z: Vec<Vec<f64>>,
    pub loglik: f64,
    pub iterations: usize,
}

/// One factor, `y[i][t]` the signals (NaN missing) and `x[j][t]` the covariates. The factor's sign is fixed so that
/// signal `anchor` loads positively.
pub fn dfm_fit(y: &[Vec<f64>], x: &[Vec<f64>], anchor: usize) -> Option<Dfm> {
    let k = y.len();
    let n = y.first()?.len();
    if k < 2 || n < 30 {
        return None;
    }
    let nx = x.len();
    let xt = |t: usize| -> Vec<f64> {
        x.iter()
            .map(|c| if c[t].is_finite() { c[t] } else { 0.0 })
            .collect()
    };
    let mut lam: Vec<f64> = vec![0.5; k];
    let mut s2: Vec<f64> = vec![0.5; k];
    let mut phi: f64 = 0.5;
    let mut gam = vec![0.0; nx];
    let mut ll_old = f64::NEG_INFINITY;
    let mut out = Dfm::default();
    for it in 0..150 {
        // filter
        let (mut fp, mut pp, mut ff, mut pf) =
            (vec![0.0; n], vec![0.0; n], vec![0.0; n], vec![0.0; n]);
        let (mut f, mut p) = (0.0, 1.0);
        let mut ll = 0.0;
        for t in 0..n {
            let g: f64 = gam.iter().zip(xt(t)).map(|(a, b)| a * b).sum();
            let (fpred, ppred) = if t == 0 {
                (g, 1.0 / (1.0 - phi * phi).max(0.05))
            } else {
                (phi * f + g, phi * phi * p + 1.0)
            };
            fp[t] = fpred;
            pp[t] = ppred;
            f = fpred;
            p = ppred;
            for i in 0..k {
                let v = y[i][t];
                if v.is_finite() {
                    let s = lam[i] * lam[i] * p + s2[i];
                    let innov = v - lam[i] * f;
                    let kg = p * lam[i] / s;
                    f += kg * innov;
                    p -= kg * lam[i] * p;
                    ll += -0.5 * ((2.0 * std::f64::consts::PI * s).ln() + innov * innov / s);
                }
            }
            ff[t] = f;
            pf[t] = p;
        }
        // smooth
        let (mut fs, mut ps, mut cs) = (ff.clone(), pf.clone(), vec![0.0; n]);
        for t in (0..n - 1).rev() {
            let j = pf[t] * phi / pp[t + 1];
            fs[t] = ff[t] + j * (fs[t + 1] - fp[t + 1]);
            ps[t] = pf[t] + j * j * (ps[t + 1] - pp[t + 1]);
            cs[t + 1] = j * ps[t + 1]; // cov(f_{t+1}, f_t)
        }
        out = Dfm {
            factor: fs.clone(),
            factor_sd: ps.iter().map(|p| p.max(0.0).sqrt()).collect(),
            loadings: lam.clone(),
            noise_sd: s2.iter().map(|s| s.sqrt()).collect(),
            phi,
            gamma: gam.clone(),
            resid_z: vec![],
            loglik: ll,
            iterations: it,
        };
        if (ll - ll_old).abs() < 1e-6 * ll_old.abs().max(1.0) {
            break;
        }
        ll_old = ll;
        // M step
        let ef2: Vec<f64> = (0..n).map(|t| ps[t] + fs[t] * fs[t]).collect();
        for i in 0..k {
            let (mut num, mut den, mut cnt) = (0.0, 0.0, 0.0);
            for t in 0..n {
                if y[i][t].is_finite() {
                    num += y[i][t] * fs[t];
                    den += ef2[t];
                    cnt += 1.0;
                }
            }
            if den > 0.0 && cnt > 5.0 {
                lam[i] = num / den;
                let mut ss = 0.0;
                for t in 0..n {
                    if y[i][t].is_finite() {
                        ss += y[i][t] * y[i][t] - 2.0 * lam[i] * y[i][t] * fs[t]
                            + lam[i] * lam[i] * ef2[t];
                    }
                }
                s2[i] = (ss / cnt).max(1e-3);
            }
        }
        // factor dynamics: regress E f_t on (E f_{t-1}, x_t) with the second moments
        let m = 1 + nx;
        let mut a = vec![vec![0.0; m]; m];
        let mut b = vec![0.0; m];
        for t in 1..n {
            let xv = xt(t);
            let mut zz = vec![ef2[t - 1]];
            zz.extend(xv.iter().map(|v| v * fs[t - 1]));
            a[0][0] += ef2[t - 1];
            for j in 0..nx {
                a[0][1 + j] += xv[j] * fs[t - 1];
                a[1 + j][0] += xv[j] * fs[t - 1];
                for l in 0..nx {
                    a[1 + j][1 + l] += xv[j] * xv[l];
                }
            }
            b[0] += fs[t] * fs[t - 1] + cs[t];
            for j in 0..nx {
                b[1 + j] += xv[j] * fs[t];
            }
        }
        for d in 0..m {
            a[d][d] += 1e-6;
        }
        if let Some(inv) = crate::stats::invert(&a) {
            let sol: Vec<f64> = (0..m)
                .map(|r| (0..m).map(|c| inv[r][c] * b[c]).sum())
                .collect();
            phi = sol[0].clamp(-0.98, 0.98);
            gam = sol[1..].to_vec();
        }
    }
    let sign = if out.loadings[anchor] < 0.0 {
        -1.0
    } else {
        1.0
    };
    out.factor.iter_mut().for_each(|v| *v *= sign);
    out.loadings.iter_mut().for_each(|v| *v *= sign);
    out.gamma.iter_mut().for_each(|v| *v *= sign);
    out.resid_z = (0..k)
        .map(|i| {
            (0..n)
                .map(|t| {
                    if y[i][t].is_finite() {
                        (y[i][t] - out.loadings[i] * out.factor[t]) / out.noise_sd[i]
                    } else {
                        f64::NAN
                    }
                })
                .collect()
        })
        .collect();
    let _ = mean(&[0.0]);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::Rng;

    #[test]
    fn local_level_separates_drift_from_noise() {
        let mut r = Rng::new(3);
        let mut level = 50.0;
        let y: Vec<f64> = (0..300)
            .map(|_| {
                level += r.normal() * 0.3;
                level + r.normal() * 2.0
            })
            .collect();
        let f = local_level_fit(&y).unwrap();
        assert!((f.sigma_eps - 2.0).abs() < 0.5, "{}", f.sigma_eps);
        assert!((f.sigma_eta - 0.3).abs() < 0.25, "{}", f.sigma_eta);
        assert!(f.q < 0.1 && f.level.len() == 300);
    }

    #[test]
    fn one_factor_is_recovered() {
        let mut r = Rng::new(9);
        let n = 300;
        let mut f = vec![0.0; n];
        for t in 1..n {
            f[t] = 0.6 * f[t - 1] + r.normal();
        }
        let loads = [0.9, -0.8, 0.7, 0.6];
        let y: Vec<Vec<f64>> = loads
            .iter()
            .map(|l| f.iter().map(|v| l * v + r.normal() * 0.5).collect())
            .collect();
        let fit = dfm_fit(&y, &[], 0).unwrap();
        let c = crate::stats::corr(&fit.factor, &f);
        assert!(c > 0.9, "{c}");
        assert!(fit.loadings[0] > 0.0 && fit.loadings[1] < 0.0);
        assert!((fit.phi - 0.6).abs() < 0.2, "{}", fit.phi);
    }
}
