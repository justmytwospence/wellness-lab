//! Event studies: how every night's number moves around the nights of an event, against matched control nights of the same
//! person (same weekday, same season, the same load the day before, nowhere near any other event), with a bootstrap over events.
//! The design follows superposed-epoch analysis and the case-crossover idea of self-matched controls.

use crate::data::Table;
use crate::labels::Label;
use crate::stats::{bootstrap_ci, mean, std};
use crate::time::{doy, weekday};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct Controls {
    pub n: usize,
    pub doy_window: i32,
    pub same_weekday: bool,
    pub same_load_class: bool,
    pub exclude_days: i32,
}

impl Default for Controls {
    fn default() -> Self {
        Controls {
            n: 5,
            doy_window: 45,
            same_weekday: true,
            same_load_class: true,
            exclude_days: 14,
        }
    }
}

#[derive(Serialize, Default)]
pub struct MetricStudy {
    pub metric: String,
    pub rel: Vec<f64>,
    pub event: Vec<f64>,
    pub control: Vec<f64>,
    pub diff: Vec<f64>,
    pub lo: Vec<f64>,
    pub hi: Vec<f64>,
    pub n: Vec<f64>,
    /// the difference in units of the metric's night-to-night spread
    pub std_effect: Vec<f64>,
    pub sd: f64,
    /// each episode's own line: episode id, then its values by relative night
    pub episodes: Vec<(String, Vec<f64>)>,
}

#[derive(Serialize, Default)]
pub struct Study {
    pub episodes: usize,
    pub too_few: bool,
    pub controls_found: usize,
    pub window: (i32, i32),
    pub metrics: Vec<MetricStudy>,
}

fn circ_doy(a: i32, b: i32) -> i32 {
    let d = (doy(a) as i32 - doy(b) as i32).abs();
    d.min(365 - d)
}

/// `episodes`: the labels to study (onset = the label's onset day).
pub fn event_study(
    nights: &Table,
    all_labels: &[Label],
    episodes: &[&Label],
    metrics: &[String],
    window: (i32, i32),
    controls: &Controls,
    n_boot: usize,
    seed: u64,
) -> Study {
    let days: Vec<i32> = nights.col("day").iter().map(|&d| d as i32).collect();
    let by_day: BTreeMap<i32, usize> = days.iter().enumerate().map(|(i, &d)| (d, i)).collect();
    let class = nights.text("load_class");
    let live: Vec<&Label> = all_labels.iter().filter(|l| l.live()).collect();
    let near_label = |d: i32| {
        live.iter().any(|l| {
            d >= l.start_day - controls.exclude_days && d <= l.end_day + controls.exclude_days
        })
    };
    let (w0, w1) = window;
    let len = (w1 - w0 + 1) as usize;
    let mut study = Study {
        episodes: episodes.len(),
        too_few: episodes.len() < 3,
        window,
        ..Default::default()
    };
    // controls per episode
    let mut ctrl: Vec<Vec<i32>> = vec![];
    for e in episodes {
        let onset = e.onset();
        let ci = by_day.get(&onset).copied();
        let mut cand: Vec<(i32, i32, i32)> = days
            .iter()
            .filter(|&&d| {
                (!controls.same_weekday || weekday(d) == weekday(onset))
                    && !near_label(d)
                    && circ_doy(d, onset) <= controls.doy_window
                    && (!controls.same_load_class
                        || ci
                            .map(|c| {
                                class[c]
                                    == by_day.get(&d).map(|&i| class[i].clone()).unwrap_or(None)
                            })
                            .unwrap_or(false))
                    && (w0..=w1).filter(|r| by_day.contains_key(&(d + r))).count() as f64
                        >= 0.6 * len as f64
            })
            .map(|&d| (circ_doy(d, onset), ((d - onset).abs() / 365), d))
            .collect();
        cand.sort();
        ctrl.push(cand.into_iter().take(controls.n).map(|c| c.2).collect());
    }
    study.controls_found = ctrl.iter().map(|c| c.len()).sum();
    for m in metrics {
        let col = nights.col(m);
        let at = |d: i32| by_day.get(&d).map(|&i| col[i]).unwrap_or(f64::NAN);
        let quiet: Vec<f64> = (0..days.len())
            .filter(|&i| !near_label(days[i]) && col[i].is_finite())
            .map(|i| col[i])
            .collect();
        let sd = std(&quiet, 1);
        let mut ms = MetricStudy {
            metric: m.clone(),
            sd,
            ..Default::default()
        };
        let ev: Vec<Vec<f64>> = episodes
            .iter()
            .map(|e| (w0..=w1).map(|r| at(e.onset() + r)).collect())
            .collect();
        let ct: Vec<Vec<f64>> = ctrl
            .iter()
            .map(|cs| {
                (w0..=w1)
                    .map(|r| {
                        if cs.is_empty() {
                            f64::NAN
                        } else {
                            mean(&cs.iter().map(|&d| at(d + r)).collect::<Vec<_>>())
                        }
                    })
                    .collect()
            })
            .collect();
        for (k, r) in (w0..=w1).enumerate() {
            let diffs: Vec<f64> = (0..episodes.len())
                .map(|e| ev[e][k] - ct[e][k])
                .filter(|x| x.is_finite())
                .collect();
            ms.rel.push(r as f64);
            ms.event
                .push(mean(&ev.iter().map(|e| e[k]).collect::<Vec<_>>()));
            ms.control
                .push(mean(&ct.iter().map(|e| e[k]).collect::<Vec<_>>()));
            let d = mean(&diffs);
            ms.diff.push(d);
            ms.n.push(diffs.len() as f64);
            let (lo, hi) = if diffs.len() >= 3 {
                bootstrap_ci(
                    diffs.len(),
                    |idx| idx.iter().map(|&i| diffs[i]).sum::<f64>() / idx.len() as f64,
                    n_boot,
                    seed + k as u64,
                    0.05,
                )
            } else {
                (f64::NAN, f64::NAN)
            };
            ms.lo.push(lo);
            ms.hi.push(hi);
            ms.std_effect.push(d / sd);
        }
        ms.episodes = episodes
            .iter()
            .zip(&ev)
            .map(|(e, v)| (e.id.clone(), v.clone()))
            .collect();
        study.metrics.push(ms);
    }
    study
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::labels::{LabelKind, Settings};
    use crate::nights::night_features;
    use crate::synth::synth;

    #[test]
    fn illness_raises_heart_rate_against_matched_controls() {
        let (d, truth) = synth(4, 730, 19_723);
        let nt = night_features(&d, &truth, &Settings::default());
        let ill: Vec<&Label> = truth
            .iter()
            .filter(|l| l.kind == LabelKind::Illness)
            .collect();
        let s = event_study(
            &nt,
            &truth,
            &ill,
            &["hr_min30".to_string(), "ln_rmssd".to_string()],
            (-3, 6),
            &Controls {
                same_weekday: false,
                same_load_class: false,
                ..Default::default()
            },
            300,
            1,
        );
        assert_eq!(s.episodes, ill.len());
        assert!(!s.too_few);
        let m = &s.metrics[0];
        let k = m.rel.iter().position(|&r| r == 1.0).unwrap();
        assert!(m.diff[k] > 3.0, "diff {}", m.diff[k]);
        assert!(m.lo[k] > 0.0, "ci {} {}", m.lo[k], m.hi[k]);
        let k0 = m.rel.iter().position(|&r| r == -2.0).unwrap();
        assert!(m.diff[k0].abs() < 2.5);
        let r = &s.metrics[1];
        assert!(r.diff[k] < -0.05);
        assert!(m.std_effect[k] > 1.0);
    }

    #[test]
    fn few_events_are_flagged() {
        let (d, truth) = synth(4, 400, 19_723);
        let nt = night_features(&d, &truth, &Settings::default());
        let one: Vec<&Label> = truth
            .iter()
            .filter(|l| l.kind == LabelKind::Illness)
            .take(2)
            .collect();
        let s = event_study(
            &nt,
            &truth,
            &one,
            &["hr_min30".to_string()],
            (-2, 3),
            &Controls::default(),
            50,
            1,
        );
        assert!(s.too_few);
    }
}
