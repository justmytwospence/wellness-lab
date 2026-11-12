//! What kind of episode is it? Naive Bayes over the classes below, with literature effect sizes as the starting
//! templates (illness: Mishra 2020, Alavi 2022, Natarajan 2020; vaccination: Quer 2022, Guan 2022; alcohol: Pietila 2018,
//! Altini and Plews 2021; training: Altini and Plews 2021) that each confirmed or denied episode moves toward the user's own.

use crate::labels::LabelKind;
use crate::math::Det;
use std::collections::BTreeMap;

pub const MODEL_VERSION: &str = "classify-1";
pub const CLASSES: [LabelKind; 6] = [
    LabelKind::Illness,
    LabelKind::Vaccination,
    LabelKind::Alcohol,
    LabelKind::HardTraining,
    LabelKind::Stress,
    LabelKind::Other,
];

/// What an episode looks like, as differences from the user's own baseline. NaN means not measured.
#[derive(Clone, Debug, Default)]
pub struct Features {
    pub d_rhr: f64,
    pub d_lnrmssd: f64,
    pub d_resp: f64,
    pub d_temp: f64,
    pub d_stress: f64,
    pub d_early_late: f64,
    pub nights: usize,
    pub hard_day_before: bool,
    pub weekend: bool,
}

impl Features {
    fn cont(&self) -> [f64; 5] {
        [
            self.d_rhr,
            self.d_lnrmssd,
            self.d_resp,
            self.d_temp,
            self.d_early_late,
        ]
    }
}

/// (mean, sd) of each continuous feature, per class: d_rhr, d_lnrmssd, d_resp, d_temp, d_early_late.
fn template(k: LabelKind) -> [(f64, f64); 5] {
    match k {
        LabelKind::Illness => [
            (5.0, 3.0),
            (-0.12, 0.08),
            (0.6, 0.5),
            (0.4, 0.3),
            (0.0, 2.0),
        ],
        LabelKind::Vaccination => [
            (4.0, 3.0),
            (-0.10, 0.08),
            (0.4, 0.5),
            (0.3, 0.3),
            (0.0, 2.0),
        ],
        LabelKind::Alcohol => [
            (4.0, 3.0),
            (-0.15, 0.10),
            (0.3, 0.4),
            (0.1, 0.2),
            (3.0, 2.0),
        ],
        LabelKind::HardTraining => [
            (1.5, 1.5),
            (-0.05, 0.05),
            (0.0, 0.3),
            (0.0, 0.2),
            (1.0, 1.5),
        ],
        LabelKind::Stress => [
            (2.0, 2.0),
            (-0.06, 0.06),
            (0.1, 0.3),
            (0.0, 0.2),
            (0.0, 2.0),
        ],
        _ => [(0.0, 5.0), (0.0, 0.2), (0.0, 1.0), (0.0, 0.5), (0.0, 3.0)],
    }
}

/// P(1 night), P(2 to 3 nights), P(4 or more).
fn night_bins(k: LabelKind) -> [f64; 3] {
    match k {
        LabelKind::Illness => [0.15, 0.35, 0.50],
        LabelKind::Vaccination => [0.35, 0.55, 0.10],
        LabelKind::Alcohol => [0.85, 0.13, 0.02],
        LabelKind::HardTraining => [0.70, 0.25, 0.05],
        LabelKind::Stress => [0.40, 0.40, 0.20],
        _ => [0.40, 0.35, 0.25],
    }
}

fn prior(k: LabelKind) -> f64 {
    match k {
        LabelKind::Illness => 0.25,
        LabelKind::Vaccination => 0.03,
        LabelKind::Alcohol => 0.25,
        LabelKind::HardTraining => 0.20,
        LabelKind::Stress => 0.17,
        _ => 0.10,
    }
}

fn log_normal(x: f64, mu: f64, sd: f64) -> f64 {
    -0.5 * ((x - mu) / sd).powi(2) - sd.det_ln()
}

/// What the user's answers taught the model.
#[derive(Clone, Debug, Default)]
pub struct Learned {
    /// confirmed episodes by class, and denied ones (counted toward "other")
    pub confirmed: BTreeMap<LabelKind, Vec<Features>>,
    pub denied: Vec<Features>,
}

impl Learned {
    pub fn counts(&self) -> (usize, usize) {
        (
            self.confirmed.values().map(|v| v.len()).sum(),
            self.denied.len(),
        )
    }
}

fn learned_template(k: LabelKind, l: &Learned) -> [(f64, f64); 5] {
    let mut t = template(k);
    let mut samples: Vec<&Features> = l
        .confirmed
        .get(&k)
        .map(|v| v.iter().collect())
        .unwrap_or_default();
    if k == LabelKind::Other {
        samples.extend(l.denied.iter());
    }
    if samples.len() >= 3 {
        for (i, tt) in t.iter_mut().enumerate() {
            let xs: Vec<f64> = samples
                .iter()
                .map(|f| f.cont()[i])
                .filter(|x| x.is_finite())
                .collect();
            if !xs.is_empty() {
                let n = xs.len() as f64;
                tt.0 = (3.0 * tt.0 + n * xs.iter().sum::<f64>() / n) / (3.0 + n);
            }
        }
    }
    t
}

/// The probability of each class given an episode's features.
pub fn posterior(f: &Features, l: &Learned) -> BTreeMap<LabelKind, f64> {
    let total: f64 = CLASSES
        .iter()
        .map(|&k| {
            10.0 * prior(k)
                + l.confirmed.get(&k).map(|v| v.len()).unwrap_or(0) as f64
                + if k == LabelKind::Other {
                    l.denied.len() as f64
                } else {
                    0.0
                }
        })
        .sum();
    let mut logp: Vec<(LabelKind, f64)> = CLASSES
        .iter()
        .map(|&k| {
            let alpha = 10.0 * prior(k)
                + l.confirmed.get(&k).map(|v| v.len()).unwrap_or(0) as f64
                + if k == LabelKind::Other {
                    l.denied.len() as f64
                } else {
                    0.0
                };
            let mut lp = (alpha / total).det_ln();
            let t = learned_template(k, l);
            for (i, x) in f.cont().iter().enumerate() {
                if x.is_finite() {
                    lp += log_normal(*x, t[i].0, t[i].1);
                }
            }
            let bin = if f.nights <= 1 {
                0
            } else if f.nights <= 3 {
                1
            } else {
                2
            };
            lp += night_bins(k)[bin].det_ln();
            if f.weekend {
                lp += if k == LabelKind::Alcohol {
                    2f64.det_ln()
                } else {
                    0.0
                };
            }
            lp += match (k == LabelKind::HardTraining, f.hard_day_before) {
                (true, true) => 0.8f64.det_ln(),
                (true, false) => 0.2f64.det_ln(),
                (false, true) => 0.3f64.det_ln(),
                (false, false) => 0.7f64.det_ln(),
            };
            if f.d_stress.is_finite() {
                lp += if k == LabelKind::Stress {
                    log_normal(f.d_stress, 8.0, 5.0)
                } else {
                    log_normal(f.d_stress, 0.0, 5.0)
                };
            }
            (k, lp)
        })
        .collect();
    let mx = logp.iter().map(|x| x.1).fold(f64::NEG_INFINITY, f64::max);
    let z: f64 = logp.iter().map(|x| (x.1 - mx).det_exp()).sum();
    logp.iter_mut()
        .for_each(|x| x.1 = ((x.1 - mx).det_exp()) / z);
    logp.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(d_rhr: f64, d_lnr: f64, nights: usize) -> Features {
        Features {
            d_rhr,
            d_lnrmssd: d_lnr,
            d_resp: f64::NAN,
            d_temp: f64::NAN,
            d_stress: f64::NAN,
            d_early_late: f64::NAN,
            nights,
            hard_day_before: false,
            weekend: false,
        }
    }

    fn top(p: &BTreeMap<LabelKind, f64>) -> LabelKind {
        *p.iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap()
            .0
    }

    #[test]
    fn a_multi_night_rise_is_illness_and_a_one_night_weekend_one_is_alcohol() {
        let l = Learned::default();
        let ill = posterior(&f(6.0, -0.15, 5), &l);
        assert_eq!(top(&ill), LabelKind::Illness);
        assert!((ill.values().sum::<f64>() - 1.0).abs() < 1e-9);
        let mut a = f(5.0, -0.18, 1);
        a.weekend = true;
        a.d_early_late = 4.0;
        assert_eq!(top(&posterior(&a, &l)), LabelKind::Alcohol);
        let mut h = f(1.5, -0.04, 1);
        h.hard_day_before = true;
        assert_eq!(top(&posterior(&h, &l)), LabelKind::HardTraining);
    }

    #[test]
    fn confirmations_move_the_templates() {
        let mut l = Learned::default();
        let p0 = posterior(&f(2.0, -0.05, 2), &l)[&LabelKind::Illness];
        // the user's own illnesses are mild: +2 bpm
        l.confirmed.insert(
            LabelKind::Illness,
            (0..6).map(|_| f(2.0, -0.05, 3)).collect(),
        );
        let p1 = posterior(&f(2.0, -0.05, 2), &l)[&LabelKind::Illness];
        assert!(p1 > p0 + 0.05, "{p0} -> {p1}");
        assert_eq!(l.counts(), (6, 0));
    }
}
