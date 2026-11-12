//! Nelder-Mead, a port of scipy.optimize's `_minimize_neldermead` (non-adaptive), so fits reproduce the notebook's.

#[derive(Debug, Clone)]
pub struct NmOptions {
    pub maxiter: usize,
    pub maxfev: usize,
    pub xatol: f64,
    pub fatol: f64,
}

impl NmOptions {
    /// scipy's defaults for `n` parameters.
    pub fn defaults(n: usize) -> Self {
        NmOptions {
            maxiter: 200 * n,
            maxfev: 200 * n,
            xatol: 1e-4,
            fatol: 1e-4,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NmResult {
    pub x: Vec<f64>,
    pub fun: f64,
    pub nit: usize,
    pub nfev: usize,
}

pub fn nelder_mead(mut f: impl FnMut(&[f64]) -> f64, x0: &[f64], opts: &NmOptions) -> NmResult {
    let (rho, chi, psi, sigma) = (1.0, 2.0, 0.5, 0.5);
    let n = x0.len();
    let (nonzdelt, zdelt) = (0.05, 0.00025);
    let mut sim: Vec<Vec<f64>> = vec![x0.to_vec()];
    for k in 0..n {
        let mut y = x0.to_vec();
        y[k] = if y[k] != 0.0 {
            (1.0 + nonzdelt) * y[k]
        } else {
            zdelt
        };
        sim.push(y);
    }
    let mut nfev = 0usize;
    let mut fsim: Vec<f64> = sim
        .iter()
        .map(|x| {
            nfev += 1;
            f(x)
        })
        .collect();
    let argsort = |fsim: &[f64]| -> Vec<usize> {
        let mut idx: Vec<usize> = (0..fsim.len()).collect();
        idx.sort_by(|&a, &b| {
            fsim[a]
                .partial_cmp(&fsim[b])
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        idx
    };
    let order = argsort(&fsim);
    sim = order.iter().map(|&i| sim[i].clone()).collect();
    fsim = order.iter().map(|&i| fsim[i]).collect();
    let mut iterations = 1usize;
    while nfev < opts.maxfev && iterations < opts.maxiter {
        let xmax = (1..=n)
            .map(|i| {
                (0..n)
                    .map(|k| (sim[i][k] - sim[0][k]).abs())
                    .fold(0.0, f64::max)
            })
            .fold(0.0, f64::max);
        let fmax = (1..=n)
            .map(|i| (fsim[0] - fsim[i]).abs())
            .fold(0.0, f64::max);
        if xmax <= opts.xatol && fmax <= opts.fatol {
            break;
        }
        let xbar: Vec<f64> = (0..n)
            .map(|k| (0..n).map(|i| sim[i][k]).sum::<f64>() / n as f64)
            .collect();
        let xr: Vec<f64> = (0..n)
            .map(|k| (1.0 + rho) * xbar[k] - rho * sim[n][k])
            .collect();
        nfev += 1;
        let fxr = f(&xr);
        let mut doshrink = false;
        if fxr < fsim[0] {
            let xe: Vec<f64> = (0..n)
                .map(|k| (1.0 + rho * chi) * xbar[k] - rho * chi * sim[n][k])
                .collect();
            nfev += 1;
            let fxe = f(&xe);
            if fxe < fxr {
                sim[n] = xe;
                fsim[n] = fxe;
            } else {
                sim[n] = xr;
                fsim[n] = fxr;
            }
        } else if fxr < fsim[n - 1] {
            sim[n] = xr;
            fsim[n] = fxr;
        } else if fxr < fsim[n] {
            let xc: Vec<f64> = (0..n)
                .map(|k| (1.0 + psi * rho) * xbar[k] - psi * rho * sim[n][k])
                .collect();
            nfev += 1;
            let fxc = f(&xc);
            if fxc <= fxr {
                sim[n] = xc;
                fsim[n] = fxc;
            } else {
                doshrink = true;
            }
        } else {
            let xcc: Vec<f64> = (0..n)
                .map(|k| (1.0 - psi) * xbar[k] + psi * sim[n][k])
                .collect();
            nfev += 1;
            let fxcc = f(&xcc);
            if fxcc < fsim[n] {
                sim[n] = xcc;
                fsim[n] = fxcc;
            } else {
                doshrink = true;
            }
        }
        if doshrink {
            for j in 1..=n {
                sim[j] = (0..n)
                    .map(|k| sim[0][k] + sigma * (sim[j][k] - sim[0][k]))
                    .collect();
                nfev += 1;
                fsim[j] = f(&sim[j]);
            }
        }
        let order = argsort(&fsim);
        sim = order.iter().map(|&i| sim[i].clone()).collect();
        fsim = order.iter().map(|&i| fsim[i]).collect();
        iterations += 1;
    }
    NmResult {
        x: sim[0].clone(),
        fun: fsim[0],
        nit: iterations,
        nfev,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rosen(x: &[f64]) -> f64 {
        (1.0 - x[0]).powi(2) + 100.0 * (x[1] - x[0] * x[0]).powi(2)
    }

    #[test]
    fn rosenbrock_converges() {
        let opts = NmOptions {
            maxiter: 4000,
            maxfev: 4000,
            xatol: 1e-8,
            fatol: 1e-10,
        };
        let r = nelder_mead(rosen, &[1.3, 0.7], &opts);
        assert!(
            (r.x[0] - 1.0).abs() < 1e-4 && (r.x[1] - 1.0).abs() < 1e-4,
            "{:?}",
            r
        );
    }

    #[test]
    fn quadratic_with_scipy_defaults() {
        // scipy.optimize.minimize(lambda x: (x[0]-3)**2 + (x[1]+1)**2, [0, 0], method="Nelder-Mead").x is [3, -1] to ~1e-4
        let r = nelder_mead(
            |x| (x[0] - 3.0).powi(2) + (x[1] + 1.0).powi(2),
            &[0.0, 0.0],
            &NmOptions::defaults(2),
        );
        assert!((r.x[0] - 3.0).abs() < 1e-3 && (r.x[1] + 1.0).abs() < 1e-3);
        assert!(r.nfev <= 400);
    }
}
