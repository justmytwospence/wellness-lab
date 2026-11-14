//! The metric bench: for each number the app shows, how much of its night-to-night movement is noise (typical error,
//! reliability, minimal detectable change) and how strongly it responds to the events the user has confirmed.
//! Statistics after Hopkins 2000 and Swinton 2018; the local-level model separates drift from noise.

use crate::data::Table;
use crate::events::{event_study, Controls};
use crate::kalman::local_level_fit;
use crate::labels::{Label, LabelKind};
use crate::math::Det;
use crate::stats::{self, mean, rolling_mean, shift, std};
use serde::Serialize;
use std::collections::BTreeMap;

/// (column, transform to log, validity grade)
pub const METRICS: [(&str, bool, char); 23] = [
    ("hr_min30", false, 'B'),
    ("hr_mean", false, 'B'),
    ("hr_nadir", false, 'B'),
    ("nadir_frac", false, 'B'),
    ("hr_decline", false, 'B'),
    ("dip_pct", false, 'B'),
    ("ln_rmssd", false, 'B'),
    ("ln_mean", false, 'B'),
    ("rmssd_4h", true, 'B'),
    ("rmssd_norm", false, 'B'),
    ("garmin_hrv", true, 'B'),
    ("sleep_stress", false, 'C'),
    ("rest_frac", false, 'C'),
    ("resp_mean", false, 'B'),
    ("skin_temp", false, 'B'),
    ("tst_h", false, 'A'),
    ("eff", false, 'D'),
    ("deep_min", false, 'D'),
    ("rem_min", false, 'D'),
    ("waso_min", false, 'D'),
    ("sleep_score", false, 'C'),
    ("bb_change", false, 'C'),
    ("tr_score", false, 'C'),
];

#[derive(Serialize, Default, Clone)]
pub struct Row {
    pub metric: String,
    pub grade: String,
    pub n_quiet: usize,
    pub mean: f64,
    pub sd_quiet: f64,
    /// typical error: the spread of consecutive quiet nights' differences over root two
    pub te: f64,
    pub cv_pct: f64,
    pub icc: f64,
    pub split_half: f64,
    pub q: f64,
    pub sigma_eps: f64,
    pub sigma_eta: f64,
    pub mdc95: f64,
    pub swc: f64,
    pub sens_hard: f64,
    pub sens: BTreeMap<String, (f64, f64, f64, usize)>,
    /// how many typical errors the largest response is
    pub snr: f64,
}

/// Nights with no event near them, an easy day before, no change of time zone around them and a near-full heart-rate record (60 % of the expected samples; real watches log 65-80 % of a night).
pub fn quiet_nights(nights: &Table, labels: &[Label]) -> Vec<bool> {
    let days = nights.col("day");
    let class = nights.text("load_class");
    let tz = nights.col("tz_change");
    let cov = nights.col("hr_cov");
    let live: Vec<&Label> = labels.iter().filter(|l| l.live()).collect();
    (0..nights.n)
        .map(|i| {
            let d = days[i] as i32;
            !live
                .iter()
                .any(|l| d >= l.start_day - 3 && d <= l.end_day + 3)
                && matches!(class[i].as_deref(), Some("rest") | Some("easy"))
                && (i.saturating_sub(3)..=(i + 3).min(nights.n - 1))
                    .all(|k| tz[k] == 0.0 || tz[k].is_nan())
                && cov[i] >= 0.6
        })
        .collect()
}

fn icc31(pairs: &[(f64, f64)]) -> f64 {
    // ICC(3,1) for two measurements per subject
    let n = pairs.len() as f64;
    if n < 5.0 {
        return f64::NAN;
    }
    let grand = pairs.iter().map(|p| p.0 + p.1).sum::<f64>() / (2.0 * n);
    let means: Vec<f64> = pairs.iter().map(|p| (p.0 + p.1) / 2.0).collect();
    let msb = 2.0 * means.iter().map(|m| (m - grand).powi(2)).sum::<f64>() / (n - 1.0);
    let msw = pairs
        .iter()
        .zip(&means)
        .map(|(p, m)| (p.0 - m).powi(2) + (p.1 - m).powi(2))
        .sum::<f64>()
        / n;
    (msb - msw) / (msb + msw)
}

pub fn bench(
    nights: &Table,
    labels: &[Label],
    metrics: Option<&[String]>,
    n_boot: usize,
) -> Vec<Row> {
    let quiet = quiet_nights(nights, labels);
    let days = nights.col("day");
    let class = nights.text("load_class");
    let kinds: Vec<LabelKind> = {
        let mut k: Vec<LabelKind> = labels.iter().filter(|l| l.live()).map(|l| l.kind).collect();
        k.sort();
        k.dedup();
        k
    };
    let mut rows = vec![];
    for (name, log, grade) in METRICS {
        if metrics
            .map(|m| !m.iter().any(|x| x == name))
            .unwrap_or(false)
        {
            continue;
        }
        let raw = nights.col(name);
        let col: Vec<f64> = if log {
            raw.iter().map(|v| v.det_ln()).collect()
        } else {
            raw.clone()
        };
        let qv: Vec<f64> = (0..nights.n)
            .filter(|&i| quiet[i] && col[i].is_finite())
            .map(|i| col[i])
            .collect();
        if qv.len() < 20 {
            continue;
        }
        let mut r = Row {
            metric: name.to_string(),
            grade: grade.to_string(),
            n_quiet: qv.len(),
            mean: mean(&qv),
            sd_quiet: std(&qv, 1),
            // not enough quiet pairs (or no odd/even split) reads as missing, never as a perfect 0
            te: f64::NAN,
            cv_pct: f64::NAN,
            icc: f64::NAN,
            split_half: f64::NAN,
            ..Default::default()
        };
        // successive quiet nights at most three days apart: strictly consecutive quiet nights are rare in real
        // data (any event, hard day or time-zone change breaks the run), and the level barely drifts in three days
        let qi: Vec<usize> = (0..nights.n)
            .filter(|&i| quiet[i] && col[i].is_finite())
            .collect();
        let pairs: Vec<(f64, f64)> = qi
            .windows(2)
            .filter(|w| days[w[1]] - days[w[0]] <= 3.0)
            .map(|w| (col[w[0]], col[w[1]]))
            .collect();
        if pairs.len() >= 10 {
            let d: Vec<f64> = pairs.iter().map(|p| p.1 - p.0).collect();
            r.te = std(&d, 1) / 2f64.sqrt();
            r.cv_pct = 100.0 * r.te
                / if log || name == "ln_rmssd" || name == "ln_mean" {
                    1.0
                } else {
                    r.mean.abs()
                };
            r.icc = icc31(&pairs);
        }
        if name == "ln_rmssd" {
            let (o, e) = (nights.col("rmssd_odd"), nights.col("rmssd_even"));
            let (lo, le): (Vec<f64>, Vec<f64>) = (0..nights.n)
                .filter(|&i| quiet[i] && o[i] > 0.0 && e[i] > 0.0)
                .map(|i| (o[i].det_ln(), e[i].det_ln()))
                .unzip();
            r.split_half = stats::spearman_brown(stats::corr(&lo, &le));
        }
        if let Some(ll) = local_level_fit(&col) {
            r.q = ll.q;
            r.sigma_eps = ll.sigma_eps;
            r.sigma_eta = ll.sigma_eta;
        }
        r.mdc95 = 1.96 * 2f64.sqrt() * r.te;
        r.swc = 0.5 * r.sd_quiet;
        // the notebook's "which number sees training"
        let base = shift(&rolling_mean(&col, 7, 4), 1);
        let dv: Vec<f64> = col.iter().zip(&base).map(|(a, b)| a - b).collect();
        let sd = std(&dv, 1);
        let pick = |c: &str| {
            mean(
                &(0..dv.len())
                    .filter(|&i| class[i].as_deref() == Some(c))
                    .map(|i| dv[i])
                    .collect::<Vec<_>>(),
            )
        };
        r.sens_hard = (pick("rest") - pick("hard")) / sd;
        // responses to the events the user has confirmed
        let mut best = r.sens_hard.abs();
        for &k in &kinds {
            let eps: Vec<&Label> = labels.iter().filter(|l| l.live() && l.kind == k).collect();
            if eps.len() < 3 {
                continue;
            }
            let s = event_study(
                nights,
                labels,
                &eps,
                &[name.to_string()],
                (0, 2),
                &Controls {
                    same_weekday: false,
                    same_load_class: false,
                    ..Default::default()
                },
                n_boot,
                7,
            );
            let m = &s.metrics[0];
            let effs: Vec<f64> = m
                .std_effect
                .iter()
                .copied()
                .filter(|x| x.is_finite())
                .collect();
            if effs.is_empty() {
                continue;
            }
            let e = mean(&effs);
            let (lo, hi) = (
                mean(&m.lo.iter().map(|x| x / m.sd).collect::<Vec<_>>()),
                mean(&m.hi.iter().map(|x| x / m.sd).collect::<Vec<_>>()),
            );
            best = best.max(e.abs());
            r.sens.insert(
                serde_json::to_string(&k)
                    .unwrap()
                    .trim_matches('"')
                    .to_string(),
                (e, lo, hi, eps.len()),
            );
        }
        r.snr = if r.te > 0.0 {
            best * r.sd_quiet / r.te
        } else {
            f64::NAN
        };
        rows.push(r);
    }
    rows.sort_by(|a, b| {
        b.snr
            .partial_cmp(&a.snr)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::labels::Settings;
    use crate::nights::night_features;
    use crate::synth::synth;

    #[test]
    fn heart_rate_is_reliable_and_responds_to_illness() {
        let (d, truth) = synth(6, 730, 19_723);
        let nt = night_features(&d, &truth, &Settings::default());
        let rows = bench(
            &nt,
            &truth,
            Some(&[
                "hr_min30".to_string(),
                "ln_rmssd".to_string(),
                "tst_h".to_string(),
            ]),
            100,
        );
        let hr = rows.iter().find(|r| r.metric == "hr_min30").unwrap();
        assert!(
            hr.n_quiet > 150 && hr.te > 0.3 && hr.te < 3.0,
            "te {}",
            hr.te
        );
        assert!(hr.sd_quiet > 0.0 && hr.mdc95 > hr.te);
        let ill = hr.sens.get("illness").expect("an illness response");
        assert!(ill.0 > 1.0, "{:?}", ill);
        let rm = rows.iter().find(|r| r.metric == "ln_rmssd").unwrap();
        assert!(
            rm.split_half.is_finite() && rm.split_half > 0.0,
            "{}",
            rm.split_half
        );
        // heart rate separates illness from noise better than sleep duration, which the synthetic illness leaves alone
        let tst = rows.iter().find(|r| r.metric == "tst_h").unwrap();
        assert!(hr.snr > tst.snr);
    }
}
