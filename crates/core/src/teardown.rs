//! Garmin's scores taken apart: what their own inputs predict, and where the number departs from that.
//! Sleep score and training readiness are regressed on their published components; Body Battery is rebuilt as a table of
//! rates by stress band and sleep; the HRV status band is searched for the window and width that reproduce it.

use crate::data::{Data, Table};
use crate::nights::{main_sleeps, nearest_value};
use crate::stats::{self, mean, nnls, ols, quantile_linear, std};
use crate::time::{MS_HOUR, MS_MIN};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize, Default)]
pub struct Regression {
    pub terms: Vec<String>,
    pub beta: Vec<f64>,
    pub se: Vec<f64>,
    pub r2: f64,
    pub rmse: f64,
    pub n: usize,
    /// the nights the fit misses by the most: day, observed, predicted
    pub worst: Vec<(f64, f64, f64)>,
}

fn regress(
    days: &[f64],
    target: &[f64],
    cols: &[(&str, Vec<f64>)],
    intercept: bool,
    nonneg: bool,
) -> Regression {
    let n = days.len();
    let idx: Vec<usize> = (0..n)
        .filter(|&i| target[i].is_finite() && cols.iter().all(|c| c.1[i].is_finite()))
        .collect();
    let mut r = Regression {
        n: idx.len(),
        ..Default::default()
    };
    if idx.len() < cols.len() + 5 {
        return r;
    }
    let rows: Vec<Vec<f64>> = idx
        .iter()
        .map(|&i| {
            let mut v: Vec<f64> = if intercept { vec![1.0] } else { vec![] };
            v.extend(cols.iter().map(|c| c.1[i]));
            v
        })
        .collect();
    let y: Vec<f64> = idx.iter().map(|&i| target[i]).collect();
    r.terms = if intercept {
        vec!["intercept".to_string()]
    } else {
        vec![]
    };
    r.terms.extend(cols.iter().map(|c| c.0.to_string()));
    let (beta, se, r2, rmse, resid) = if nonneg {
        let b = nnls(&rows, &y);
        let resid: Vec<f64> = (0..y.len())
            .map(|i| y[i] - rows[i].iter().zip(&b).map(|(a, c)| a * c).sum::<f64>())
            .collect();
        let rss: f64 = resid.iter().map(|x| x * x).sum();
        let ym = mean(&y);
        let tss: f64 = y.iter().map(|v| (v - ym).powi(2)).sum();
        (
            b,
            vec![f64::NAN; rows[0].len()],
            1.0 - rss / tss,
            (rss / y.len() as f64).sqrt(),
            resid,
        )
    } else {
        match ols(&rows, &y) {
            Some(f) => (f.beta, f.se, f.r2, f.rmse, f.resid),
            None => return r,
        }
    };
    r.beta = beta;
    r.se = se;
    r.r2 = r2;
    r.rmse = rmse;
    let mut worst: Vec<(f64, f64, f64)> = idx
        .iter()
        .enumerate()
        .map(|(k, &i)| (days[i], y[k], y[k] - resid[k]))
        .collect();
    worst.sort_by(|a, b| (b.1 - b.2).abs().partial_cmp(&(a.1 - a.2).abs()).unwrap());
    worst.truncate(20);
    r.worst = worst;
    r
}

#[derive(Serialize, Default)]
pub struct BodyBattery {
    /// rows: stress band, asleep, mean rate (points per hour), samples
    pub table: Vec<(String, bool, f64, usize)>,
    pub mae: f64,
    pub days: usize,
}

fn band(v: f64, q: f64) -> &'static str {
    if q == 2.0 {
        "activity"
    } else if !v.is_finite() {
        "unmeasured"
    } else if v <= 10.0 {
        "0-10"
    } else if v <= 25.0 {
        "11-25"
    } else if v <= 50.0 {
        "26-50"
    } else if v <= 75.0 {
        "51-75"
    } else {
        "76-100"
    }
}

fn body_battery(data: &Data) -> BodyBattery {
    let (Some(bb), Some(st)) = (data.s("body_battery"), data.s("stress")) else {
        return BodyBattery::default();
    };
    let sleeps = main_sleeps(data);
    let asleep = |t: i64| sleeps.iter().any(|s| t >= s.1 && t < s.2);
    let q = st.c("q");
    let mut cells: BTreeMap<(String, bool), Vec<f64>> = BTreeMap::new();
    let mut samples: Vec<(i64, f64, &'static str, bool, f64)> = vec![]; // t, rate, band, asleep, dt
    for i in 1..bb.len() {
        let dt = (bb.t[i] - bb.t[i - 1]) as f64 / MS_HOUR as f64;
        if dt <= 0.0 || dt > 6.0 * MS_MIN as f64 / MS_HOUR as f64 {
            continue;
        }
        let j = st.t.partition_point(|&x| x < bb.t[i - 1]).min(st.len() - 1);
        let (sv, sq) = (
            nearest_value(st, bb.t[i - 1], 3 * MS_MIN),
            q.map(|q| q[j]).unwrap_or(0.0),
        );
        let b = band(sv, sq);
        let a = asleep(bb.t[i - 1]);
        let rate = (bb.v()[i] - bb.v()[i - 1]) / dt;
        cells.entry((b.to_string(), a)).or_default().push(rate);
        samples.push((bb.t[i - 1], rate, b, a, dt));
    }
    let table: Vec<(String, bool, f64, usize)> = cells
        .iter()
        .map(|((b, a), v)| (b.clone(), *a, mean(v), v.len()))
        .collect();
    // how well the table predicts each sample's change
    let rate_of = |b: &str, a: bool| {
        table
            .iter()
            .find(|t| t.0 == b && t.1 == a)
            .map(|t| t.2)
            .unwrap_or(0.0)
    };
    let errs: Vec<f64> = samples
        .iter()
        .map(|s| ((rate_of(s.2, s.3) - s.1) * s.4).abs())
        .collect();
    BodyBattery {
        table,
        mae: mean(&errs),
        days: samples.len() / 480,
    }
}

#[derive(Serialize, Default)]
pub struct BandGrid {
    pub window: Vec<f64>,
    pub width: Vec<f64>,
    pub mae: Vec<f64>,
    pub kind: Vec<String>,
    pub best_sd: (f64, f64, f64),
    pub best_percentile: (f64, f64, f64),
}

/// Search the window and width that reproduce Garmin's balanced HRV range from the nightly mean of the five-minute values.
fn hrv_band(data: &Data, nights: &Table) -> BandGrid {
    let ln: Vec<f64> = nights.col("rmssd_mean").iter().map(|v| v.ln()).collect();
    let by_day = |c: &str| -> Vec<f64> {
        nights
            .col("day")
            .iter()
            .map(|&d| {
                data.day_index(d as i32)
                    .map(|i| data.days.col(c)[i].ln())
                    .unwrap_or(f64::NAN)
            })
            .collect()
    };
    let (lo, hi) = (by_day("hrv_bal_lo"), by_day("hrv_bal_hi"));
    let n = nights.n;
    let mut g = BandGrid {
        best_sd: (0.0, 0.0, f64::INFINITY),
        best_percentile: (0.0, 0.0, f64::INFINITY),
        ..Default::default()
    };
    let eval = |w: usize, f: &dyn Fn(&[f64]) -> (f64, f64)| -> f64 {
        let mut errs = vec![];
        for i in w..n {
            let hist: Vec<f64> = ln[i - w..i]
                .iter()
                .copied()
                .filter(|x| x.is_finite())
                .collect();
            if hist.len() < w / 2 || !lo[i].is_finite() || !hi[i].is_finite() {
                continue;
            }
            let (a, b) = f(&hist);
            errs.push((a - lo[i]).abs());
            errs.push((b - hi[i]).abs());
        }
        if errs.len() < 20 {
            f64::NAN
        } else {
            mean(&errs)
        }
    };
    for w in [14usize, 21, 28, 35, 42, 49, 56, 60, 70, 90] {
        for k in 6..=30 {
            let c = k as f64 * 0.05;
            let mae = eval(w, &|h| {
                let (m, s) = (mean(h), std(h, 1));
                (m - c * s, m + c * s)
            });
            g.window.push(w as f64);
            g.width.push(c);
            g.mae.push(mae);
            g.kind.push("sd".into());
            if mae < g.best_sd.2 {
                g.best_sd = (w as f64, c, mae);
            }
        }
        for k in 1..=8 {
            let p = k as f64 * 0.05;
            let mae = eval(w, &|h| (quantile_linear(h, p), quantile_linear(h, 1.0 - p)));
            g.window.push(w as f64);
            g.width.push(p);
            g.mae.push(mae);
            g.kind.push("percentile".into());
            if mae < g.best_percentile.2 {
                g.best_percentile = (w as f64, p, mae);
            }
        }
    }
    g
}

#[derive(Serialize, Default)]
pub struct StressVsHr {
    pub hr_bin: Vec<f64>,
    pub since_activity: Vec<String>,
    pub median_stress: Vec<f64>,
    pub n: Vec<f64>,
}

fn stress_vs_hr(data: &Data) -> StressVsHr {
    let (Some(st), Some(hr)) = (data.s("stress"), data.s("hr")) else {
        return StressVsHr::default();
    };
    let q = st.c("q");
    let rhr = data.days.col("rhr");
    let (as_, ae) = (data.activities.col("start"), data.activities.col("end"));
    let sleeps = main_sleeps(data);
    let mut cells: BTreeMap<(i64, usize), Vec<f64>> = BTreeMap::new();
    for i in (0..st.len()).step_by(3) {
        if !st.v()[i].is_finite()
            || q.map(|q| q[i] != 0.0).unwrap_or(false)
            || sleeps.iter().any(|s| st.t[i] >= s.1 && st.t[i] <= s.2)
        {
            continue;
        }
        let h = nearest_value(hr, st.t[i], 90_000);
        let day = data.day_index(crate::time::day_of_ms(st.t[i], data.offset_at(st.t[i])));
        let Some(di) = day else { continue };
        if !h.is_finite() || !rhr[di].is_finite() {
            continue;
        }
        let last_end = (0..data.activities.n)
            .map(|a| ae[a])
            .filter(|&e| e <= st.t[i] as f64 && st.t[i] as f64 - e <= 24.0 * 3.6e6)
            .fold(f64::NAN, f64::max);
        let during =
            (0..data.activities.n).any(|a| st.t[i] as f64 >= as_[a] && st.t[i] as f64 <= ae[a]);
        if during {
            continue;
        }
        let since = if last_end.is_nan() {
            3
        } else {
            let h = (st.t[i] as f64 - last_end) / 3.6e6;
            if h < 1.0 {
                0
            } else if h < 3.0 {
                1
            } else if h < 6.0 {
                2
            } else {
                3
            }
        };
        let bin = ((h - rhr[di]) / 5.0).floor().clamp(-2.0, 8.0) as i64 * 5;
        cells.entry((bin, since)).or_default().push(st.v()[i]);
    }
    let names = ["0-1 h", "1-3 h", "3-6 h", "6+ h"];
    let mut out = StressVsHr::default();
    for ((b, s), v) in cells {
        if v.len() >= 20 {
            out.hr_bin.push(b as f64);
            out.since_activity.push(names[s].to_string());
            out.median_stress.push(stats::median(&v));
            out.n.push(v.len() as f64);
        }
    }
    out
}

#[derive(Serialize, Default)]
pub struct Teardown {
    pub sleep_score: Regression,
    pub readiness_nnls: Regression,
    pub readiness_ols: Regression,
    pub body_battery: BodyBattery,
    pub hrv_band: BandGrid,
    pub stress_vs_hr: StressVsHr,
}

pub fn teardown(data: &Data, nights: &Table) -> Teardown {
    let d = &data.days;
    let days = d.col("day");
    let sleep_cols: Vec<(&str, Vec<f64>)> = [
        ("duration", "sleep_duration_score"),
        ("deep", "sleep_deep_score"),
        ("rem", "sleep_rem_score"),
        ("light", "sleep_light_score"),
        ("awakenings", "sleep_awakenings_score"),
        ("awake_time", "sleep_awake_time_score"),
        ("restfulness", "sleep_restfulness_score"),
        ("interruptions", "sleep_interruptions_score"),
    ]
    .iter()
    .map(|(n, c)| (*n, d.col(c)))
    .collect();
    let tr_cols: Vec<(&str, Vec<f64>)> = [
        ("sleep", "tr_sleep_pct"),
        ("recovery", "tr_recovery_pct"),
        ("acute load", "tr_acwr_pct"),
        ("stress history", "tr_stress_hist_pct"),
        ("hrv", "tr_hrv_pct"),
        ("sleep history", "tr_sleep_hist_pct"),
    ]
    .iter()
    .map(|(n, c)| (*n, d.col(c)))
    .collect();
    Teardown {
        sleep_score: regress(&days, &d.col("sleep_score"), &sleep_cols, true, false),
        readiness_nnls: regress(&days, &d.col("tr_score"), &tr_cols, false, true),
        readiness_ols: regress(&days, &d.col("tr_score"), &tr_cols, true, false),
        body_battery: body_battery(data),
        hrv_band: hrv_band(data, nights),
        stress_vs_hr: stress_vs_hr(data),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::labels::Settings;
    use crate::nights::night_features;
    use crate::synth::synth;

    #[test]
    fn regressions_recover_known_weights() {
        // sleep score = 10 + 0.5 duration + 0.3 deep, exactly
        let n = 80;
        let days: Vec<f64> = (0..n).map(|i| i as f64).collect();
        let dur: Vec<f64> = (0..n).map(|i| 60.0 + (i * 7 % 30) as f64).collect();
        let deep: Vec<f64> = (0..n).map(|i| 50.0 + (i * 11 % 40) as f64).collect();
        let y: Vec<f64> = (0..n)
            .map(|i| 10.0 + 0.5 * dur[i] + 0.3 * deep[i])
            .collect();
        let r = regress(
            &days,
            &y,
            &[("duration", dur.clone()), ("deep", deep.clone())],
            true,
            false,
        );
        assert!(
            (r.beta[1] - 0.5).abs() < 1e-6 && (r.beta[2] - 0.3).abs() < 1e-6 && r.r2 > 0.999999
        );
        let nn = regress(
            &days,
            &y.iter().map(|v| v - 10.0).collect::<Vec<_>>(),
            &[("duration", dur), ("deep", deep)],
            false,
            true,
        );
        assert!((nn.beta[0] - 0.5).abs() < 1e-6 && nn.beta[1] > 0.0);
    }

    #[test]
    fn body_battery_and_bands_on_synthetic_data() {
        let (d, _) = synth(2, 60, 19_723);
        let nt = night_features(&d, &[], &Settings::default());
        let t = teardown(&d, &nt);
        assert!(!t.body_battery.table.is_empty());
        // asleep charges the battery
        let asleep: Vec<f64> = t
            .body_battery
            .table
            .iter()
            .filter(|r| r.1)
            .map(|r| r.2)
            .collect();
        assert!(asleep.iter().any(|&r| r > 0.0));
        assert!(!t.hrv_band.mae.is_empty());
        assert!(t.hrv_band.best_sd.2.is_finite());
        assert!(t.sleep_score.n == 0 || t.sleep_score.r2.is_finite());
    }
}
