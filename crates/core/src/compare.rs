//! What is different about a chosen set of nights? Every night feature and every tag and label share, ranked by effect size.

use crate::data::Table;
use crate::labels::Label;
use crate::stats::{cliffs_delta, ks_d, median};
use serde::Serialize;

#[derive(Serialize, Default)]
pub struct FeatureDiff {
    pub feature: String,
    pub median_sel: f64,
    pub median_base: f64,
    pub cliffs_delta: f64,
    pub ks_d: f64,
    pub n_sel: usize,
    pub n_base: usize,
}

#[derive(Serialize, Default)]
pub struct Compare {
    pub features: Vec<FeatureDiff>,
    pub labels: Vec<(String, f64, f64)>,
    pub n_sel: usize,
    pub n_base: usize,
}

const SKIP: [&str; 6] = ["day", "onset", "wake", "mid", "excluded", "weekday"];

/// `sel`: the days of the chosen nights; `base`: the days to compare with (default: every other night in the selection's range).
pub fn compare(nights: &Table, labels: &[Label], sel: &[i32], base: Option<&[i32]>) -> Compare {
    let days: Vec<i32> = nights.col("day").iter().map(|&d| d as i32).collect();
    let in_sel: Vec<bool> = days.iter().map(|d| sel.contains(d)).collect();
    let (lo, hi) = (
        sel.iter().min().copied().unwrap_or(0),
        sel.iter().max().copied().unwrap_or(0),
    );
    let in_base: Vec<bool> = days
        .iter()
        .enumerate()
        .map(|(i, d)| match base {
            Some(b) => b.contains(d),
            None => !in_sel[i] && *d >= lo && *d <= hi,
        })
        .collect();
    let mut out = Compare {
        n_sel: in_sel.iter().filter(|&&b| b).count(),
        n_base: in_base.iter().filter(|&&b| b).count(),
        ..Default::default()
    };
    for (name, col) in &nights.num {
        if SKIP.contains(&name.as_str()) {
            continue;
        }
        let a: Vec<f64> = (0..nights.n)
            .filter(|&i| in_sel[i] && col[i].is_finite())
            .map(|i| col[i])
            .collect();
        let b: Vec<f64> = (0..nights.n)
            .filter(|&i| in_base[i] && col[i].is_finite())
            .map(|i| col[i])
            .collect();
        if a.len() < 3 || b.len() < 3 {
            continue;
        }
        out.features.push(FeatureDiff {
            feature: name.clone(),
            median_sel: median(&a),
            median_base: median(&b),
            cliffs_delta: cliffs_delta(&a, &b),
            ks_d: ks_d(&a, &b),
            n_sel: a.len(),
            n_base: b.len(),
        });
    }
    out.features.sort_by(|x, y| {
        y.cliffs_delta
            .abs()
            .partial_cmp(&x.cliffs_delta.abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut kinds: Vec<String> = labels
        .iter()
        .filter(|l| l.live())
        .map(|l| {
            serde_json::to_string(&l.kind)
                .unwrap()
                .trim_matches('"')
                .to_string()
        })
        .collect();
    kinds.sort();
    kinds.dedup();
    for k in kinds {
        let has = |d: i32| {
            labels.iter().any(|l| {
                l.live()
                    && serde_json::to_string(&l.kind).unwrap().trim_matches('"') == k
                    && d >= l.start_day
                    && d <= l.end_day
            })
        };
        let share = |flags: &[bool]| {
            let n = flags.iter().filter(|&&b| b).count();
            if n == 0 {
                0.0
            } else {
                (0..nights.n).filter(|&i| flags[i] && has(days[i])).count() as f64 / n as f64
            }
        };
        out.labels
            .push((k.clone(), share(&in_sel), share(&in_base)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::labels::{LabelKind, Settings};
    use crate::nights::night_features;
    use crate::synth::synth;

    #[test]
    fn illness_nights_differ_in_heart_rate_and_hrv_first() {
        let (d, truth) = synth(8, 400, 19_723);
        let nt = night_features(&d, &truth, &Settings::default());
        let sel: Vec<i32> = truth
            .iter()
            .filter(|l| l.kind == LabelKind::Illness)
            .flat_map(|l| l.start_day..=l.end_day)
            .collect();
        let c = compare(&nt, &truth, &sel, None);
        let top: Vec<&str> = c
            .features
            .iter()
            .take(8)
            .map(|f| f.feature.as_str())
            .collect();
        assert!(top.iter().any(|f| f.starts_with("hr_")), "{top:?}");
        assert!(c.features[0].cliffs_delta.abs() > 0.5);
        let ill = c.labels.iter().find(|l| l.0 == "illness").unwrap();
        assert!(ill.1 > 0.9 && ill.2 < 0.1);
    }
}
