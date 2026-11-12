//! Self-experiments: alternating blocks of two conditions (randomised order), with washout nights left out, adherence
//! recorded, and the result as a probability that the effect is worth acting on (Bayesian in spirit, after SleepBandits).

use crate::data::Table;
use crate::stats::{self, mean, normal_cdf, std, Rng};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ExperimentDef {
    pub id: String,
    pub name: String,
    /// what is changed: a label kind or a tag
    pub intervention: String,
    /// the night metric watched
    pub outcome: String,
    #[serde(default = "d7")]
    pub period_nights: usize,
    #[serde(default = "d2")]
    pub washout: usize,
    #[serde(default = "d4")]
    pub blocks: usize,
    #[serde(default = "dt")]
    pub randomized: bool,
    pub start_day: i32,
    #[serde(default = "d1")]
    pub seed: u64,
    #[serde(default = "dt")]
    pub higher_is_better: bool,
}

fn d7() -> usize {
    7
}
fn d2() -> usize {
    2
}
fn d4() -> usize {
    4
}
fn dt() -> bool {
    true
}
fn d1() -> u64 {
    1
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Slot {
    pub day: i32,
    /// "A" (the change) or "B" (as usual)
    pub arm: char,
    pub washout: bool,
}

/// Each block is two periods, A then B or B then A (random), each `period_nights` long, its first `washout` nights flagged.
pub fn schedule(def: &ExperimentDef) -> Vec<Slot> {
    let mut rng = Rng::new(def.seed);
    let mut out = vec![];
    let mut day = def.start_day;
    for b in 0..def.blocks {
        let a_first = if def.randomized {
            rng.f64() < 0.5
        } else {
            b % 2 == 0
        };
        for arm in if a_first { ['A', 'B'] } else { ['B', 'A'] } {
            for k in 0..def.period_nights {
                out.push(Slot {
                    day,
                    arm,
                    washout: k < def.washout,
                });
                day += 1;
            }
        }
    }
    out
}

#[derive(Serialize, Default, Clone)]
pub struct Result1 {
    pub n_a: usize,
    pub n_b: usize,
    pub effect: f64,
    pub se: f64,
    pub p_pos: f64,
    pub p_gt_swc: f64,
    pub p_lt_neg_swc: f64,
    pub p_within_swc: f64,
    pub status: String,
    pub swc: f64,
}

#[derive(Serialize, Default)]
pub struct Analysis {
    pub schedule: Vec<Slot>,
    pub itt: Result1,
    pub per_protocol: Result1,
    pub quantiles: Vec<f64>,
}

fn one(a: &[f64], b: &[f64], swc: f64, higher_is_better: bool) -> Result1 {
    let mut r = Result1 {
        n_a: a.len(),
        n_b: b.len(),
        swc,
        ..Default::default()
    };
    if a.len() < 3 || b.len() < 3 {
        r.status = "inconclusive".into();
        return r;
    }
    let rho = stats::ar1(&a.iter().chain(b).copied().collect::<Vec<_>>()).clamp(0.0, 0.9);
    let neff = |n: usize| n as f64 * (1.0 - rho) / (1.0 + rho);
    r.effect = mean(a) - mean(b);
    r.se = (std(a, 1).powi(2) / neff(a.len()).max(2.0)
        + std(b, 1).powi(2) / neff(b.len()).max(2.0))
    .sqrt();
    let z = |x: f64| normal_cdf((x - r.effect) / r.se);
    let sgn = if higher_is_better { 1.0 } else { -1.0 };
    let (good, bad) = if higher_is_better {
        (1.0 - z(swc), z(-swc))
    } else {
        (z(-swc), 1.0 - z(swc))
    };
    r.p_pos = if higher_is_better {
        1.0 - z(0.0)
    } else {
        z(0.0)
    };
    r.p_gt_swc = good;
    r.p_lt_neg_swc = bad;
    r.p_within_swc = z(swc) - z(-swc);
    r.status = if good >= 0.9 {
        "likely improves"
    } else if bad >= 0.9 {
        "likely worsens"
    } else if r.p_within_swc >= 0.9 {
        "no meaningful effect"
    } else {
        "inconclusive"
    }
    .into();
    let _ = sgn;
    r
}

pub fn analyze(
    def: &ExperimentDef,
    nights: &Table,
    adherence: &BTreeMap<i32, bool>,
    swc: f64,
) -> Analysis {
    let sched = schedule(def);
    let col = nights.col(&def.outcome);
    let days = nights.col("day");
    let at: BTreeMap<i32, f64> = (0..nights.n).map(|i| (days[i] as i32, col[i])).collect();
    let pick = |arm: char, pp: bool| -> Vec<f64> {
        sched
            .iter()
            .filter(|s| {
                s.arm == arm
                    && !s.washout
                    && (!pp || adherence.get(&s.day).copied().unwrap_or(true))
            })
            .filter_map(|s| at.get(&s.day).copied())
            .filter(|v| v.is_finite())
            .collect()
    };
    let itt = one(
        &pick('A', false),
        &pick('B', false),
        swc,
        def.higher_is_better,
    );
    let pp = one(
        &pick('A', true),
        &pick('B', true),
        swc,
        def.higher_is_better,
    );
    // twenty equally likely effects for a quantile dotplot
    let quantiles = if itt.se > 0.0 {
        (0..20)
            .map(|k| itt.effect + itt.se * inv_normal((k as f64 + 0.5) / 20.0))
            .collect()
    } else {
        vec![]
    };
    Analysis {
        schedule: sched,
        itt,
        per_protocol: pp,
        quantiles,
    }
}

fn inv_normal(p: f64) -> f64 {
    // bisection on the normal CDF: ample for twenty dots
    let (mut lo, mut hi) = (-6.0, 6.0);
    for _ in 0..60 {
        let mid = (lo + hi) / 2.0;
        if normal_cdf(mid) < p {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def() -> ExperimentDef {
        ExperimentDef {
            id: "e".into(),
            name: "no caffeine after noon".into(),
            intervention: "caffeine_late".into(),
            outcome: "x".into(),
            period_nights: 7,
            washout: 2,
            blocks: 4,
            randomized: true,
            start_day: 100,
            seed: 3,
            higher_is_better: true,
        }
    }

    #[test]
    fn schedule_alternates_within_blocks_and_flags_washout() {
        let s = schedule(&def());
        assert_eq!(s.len(), 4 * 14);
        for b in 0..4 {
            let blk = &s[b * 14..(b + 1) * 14];
            assert!(
                blk[..7].iter().all(|x| x.arm == blk[0].arm)
                    && blk[7..].iter().all(|x| x.arm != blk[0].arm)
            );
            assert_eq!(blk.iter().filter(|x| x.washout).count(), 4);
        }
        assert_eq!(s, schedule(&def()));
        assert_eq!(s[0].day, 100);
    }

    #[test]
    fn a_real_effect_is_likely_and_noise_is_inconclusive() {
        let d = def();
        let sched = schedule(&d);
        let mut nights = Table::new(sched.len());
        nights.set("day", sched.iter().map(|s| s.day as f64).collect());
        let mut r = Rng::new(1);
        nights.set(
            "x",
            sched
                .iter()
                .map(|s| 50.0 + if s.arm == 'A' { 5.0 } else { 0.0 } + r.normal() * 2.0)
                .collect(),
        );
        let a = analyze(&d, &nights, &BTreeMap::new(), 1.0);
        assert_eq!(a.itt.status, "likely improves", "{:?}", a.itt.effect);
        assert!((a.itt.effect - 5.0).abs() < 1.5 && a.quantiles.len() == 20);
        let mut noise = Table::new(sched.len());
        noise.set("day", sched.iter().map(|s| s.day as f64).collect());
        noise.set("x", sched.iter().map(|_| 50.0 + r.normal() * 4.0).collect());
        let b = analyze(&d, &noise, &BTreeMap::new(), 3.0);
        assert_ne!(b.itt.status, "likely improves");
        // non-adherent nights drop out of the per-protocol analysis
        let mut adh = BTreeMap::new();
        for s in sched.iter().filter(|s| s.arm == 'A').take(10) {
            adh.insert(s.day, false);
        }
        let c = analyze(&d, &nights, &adh, 1.0);
        assert!(c.per_protocol.n_a < c.itt.n_a);
    }
}
