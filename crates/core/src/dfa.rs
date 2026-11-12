//! DFA alpha-1 of beat-to-beat intervals during exercise, and the heart rates where it crosses 0.75 and 0.5 (the first and
//! second ventilatory thresholds), as in the HRV deep-dive notebook (cells a1_build, a1_thresholds, durability_chart).

use crate::data::{Data, Table};
use crate::stats::{
    self, median, polyfit, quantile_nearest, rolling_median, rolling_quantile_nearest,
};
use crate::time::{day_of_ms, MS_DAY};
use serde::Serialize;
use std::collections::BTreeMap;

/// Bump when the window, artefact or device rules change: cached windows are rebuilt.
pub const A1_SCHEMA: u32 = 2;

fn nanmax(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}

/// alpha-1: the slope of log F(n) against log n over box sizes 4 to 16 beats.
pub fn dfa_a1(rr: &[f64]) -> f64 {
    let m = stats::mean(rr);
    let mut y = Vec::with_capacity(rr.len());
    let mut acc = 0.0;
    for &v in rr {
        acc += v - m;
        y.push(acc);
    }
    let scales: Vec<usize> = (4..=16).collect();
    let mut lf = vec![];
    for &n in &scales {
        let boxes = y.len() / n;
        if boxes == 0 {
            return f64::NAN;
        }
        let x: Vec<f64> = (0..n).map(|k| k as f64 - (n as f64 - 1.0) / 2.0).collect();
        let xx: f64 = x.iter().map(|v| v * v).sum();
        let mut ss = 0.0;
        for b in 0..boxes {
            let seg = &y[b * n..(b + 1) * n];
            let sm = seg.iter().sum::<f64>() / n as f64;
            let slope = seg.iter().zip(&x).map(|(a, c)| a * c).sum::<f64>() / xx;
            for k in 0..n {
                let r = seg[k] - sm - slope * x[k];
                ss += r * r;
            }
        }
        lf.push((ss / (boxes * n) as f64).sqrt().ln());
    }
    let lx: Vec<f64> = scales.iter().map(|&n| (n as f64).ln()).collect();
    polyfit(&lx, &lf, 1, None).map(|c| c[0]).unwrap_or(f64::NAN)
}

fn qd(x: &[f64]) -> Vec<f64> {
    let a: Vec<f64> = x.iter().map(|v| v.abs()).collect();
    let hi = rolling_quantile_nearest(&a, 0.75, 91, 10, true);
    let lo = rolling_quantile_nearest(&a, 0.25, 91, 10, true);
    hi.iter()
        .zip(&lo)
        .map(|(h, l)| 5.2 * (h - l) / 2.0)
        .collect()
}

/// Artefact correction (after Lipponen and Tarvainen, simplified): halves a missed beat, merges a split one, replaces ectopic ones.
pub fn correct(rr: &[f64]) -> (Vec<f64>, Vec<bool>) {
    let n = rr.len();
    let med = rolling_median(rr, 11, 3, true);
    let mut drr = vec![0.0; n];
    for i in 1..n {
        drr[i] = rr[i] - rr[i - 1];
    }
    let mrr: Vec<f64> = (0..n).map(|i| rr[i] - med[i]).collect();
    let (qd_d, qd_m) = (qd(&drr), qd(&mrr));
    let bad: Vec<bool> = (0..n)
        .map(|i| {
            (drr[i].abs() > nanmax(qd_d[i], 20.0) && mrr[i].abs() > nanmax(qd_m[i], 20.0))
                || mrr[i].abs() > 0.3 * med[i]
        })
        .collect();
    let mut out = vec![];
    let mut i = 0;
    while i < n {
        if !bad[i] {
            out.push(rr[i]);
            i += 1;
        } else if (rr[i] / 2.0 - med[i]).abs() < 0.2 * med[i] {
            out.push(rr[i] / 2.0);
            out.push(rr[i] / 2.0);
            i += 1;
        } else if i + 1 < n && (rr[i] + rr[i + 1] - med[i]).abs() < 0.2 * med[i] {
            out.push(rr[i] + rr[i + 1]);
            i += 2;
        } else {
            out.push(med[i]);
            i += 1;
        }
    }
    (out, bad)
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Windows {
    pub t_s: Vec<f64>,
    pub hr: Vec<f64>,
    pub a1: Vec<f64>,
    pub art: Vec<f64>,
    pub rep: Vec<f64>,
}

/// Two-minute windows every 30 seconds through one recording (RR in ms, with 4999 or more as the no-beat filler).
pub fn windows(rr_raw: &[f64], win_s: f64, step_s: f64) -> Windows {
    let valid: Vec<bool> = rr_raw
        .iter()
        .map(|&r| r < 4999.0 && (250.0..=2000.0).contains(&r))
        .collect();
    let mut seg_id = vec![0usize; rr_raw.len()];
    let mut c = 0;
    for i in 0..rr_raw.len() {
        if !valid[i] {
            c += 1;
        }
        seg_id[i] = c;
    }
    let mut ids: Vec<usize> = (0..rr_raw.len())
        .filter(|&i| valid[i])
        .map(|i| seg_id[i])
        .collect();
    ids.dedup();
    let mut out = Windows::default();
    let mut t_off = 0.0;
    for sid in ids {
        let seg: Vec<f64> = (0..rr_raw.len())
            .filter(|&i| valid[i] && seg_id[i] == sid)
            .map(|i| rr_raw[i])
            .collect();
        let total: f64 = seg.iter().sum();
        if total < win_s * 1000.0 {
            t_off += total / 1000.0;
            continue;
        }
        let (corr, bad) = correct(&seg);
        let cum = |v: &[f64]| -> Vec<f64> {
            let mut a = 0.0;
            v.iter()
                .map(|x| {
                    a += x;
                    a / 1000.0
                })
                .collect()
        };
        let (t_raw, t_cor) = (cum(&seg), cum(&corr));
        let mut start = 0.0;
        while start + win_s <= *t_cor.last().unwrap() {
            let w: Vec<f64> = (0..corr.len())
                .filter(|&i| t_cor[i] > start && t_cor[i] <= start + win_s)
                .map(|i| corr[i])
                .collect();
            let sel: Vec<usize> = (0..seg.len())
                .filter(|&i| t_raw[i] > start && t_raw[i] <= start + win_s)
                .collect();
            if w.len() >= 60 {
                out.t_s.push(t_off + start + win_s);
                out.hr.push(60000.0 / stats::mean(&w));
                out.a1.push(dfa_a1(&w));
                out.art
                    .push(sel.iter().filter(|&&i| bad[i]).count() as f64 / sel.len() as f64);
                let diffs = sel
                    .windows(2)
                    .filter(|p| seg[p[1]] - seg[p[0]] == 0.0)
                    .count();
                out.rep.push(if sel.len() > 1 {
                    diffs as f64 / (sel.len() - 1) as f64
                } else {
                    f64::NAN
                });
            }
            start += step_s;
        }
        t_off += t_raw.last().unwrap();
    }
    out
}

pub fn sport_group(sport: &str) -> &'static str {
    match sport {
        "run" | "trail_run" | "treadmill_run" => "run",
        "ride" | "gravel_ride" | "mountain_bike" | "indoor_ride" | "virtual_ride" => "ride",
        "backcountry_ski" | "skate_ski" | "classic_ski" | "resort_ski" => "ski",
        _ => "other",
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Curve {
    pub hb: Vec<f64>,
    pub a1: Vec<f64>,
    pub n: Vec<f64>,
    pub line: Vec<f64>,
    pub vt1: Option<f64>,
    pub vt2: Option<f64>,
}

/// The median alpha-1 in 2-bpm bins, a weighted straight line through the transition zone, and where it crosses 0.75 and 0.5.
pub fn curve(
    w: &Windows,
    idx: &[usize],
    max_art: f64,
    max_rep: f64,
    bin_bpm: f64,
    min_n: usize,
) -> Curve {
    let mut bins: BTreeMap<i64, Vec<f64>> = BTreeMap::new();
    for &i in idx {
        if w.art[i] <= max_art && w.rep[i] <= max_rep && w.a1[i].is_finite() {
            let hb = (w.hr[i] / bin_bpm).floor() * bin_bpm + bin_bpm / 2.0;
            bins.entry((hb * 1000.0).round() as i64)
                .or_default()
                .push(w.a1[i]);
        }
    }
    let mut c = Curve::default();
    for (k, v) in bins {
        if v.len() >= min_n {
            c.hb.push(k as f64 / 1000.0);
            c.a1.push(median(&v));
            c.n.push(v.len() as f64);
        }
    }
    c.line = vec![f64::NAN; c.hb.len()];
    if c.hb.len() < 5 {
        return c;
    }
    let z: Vec<usize> = (0..c.hb.len())
        .filter(|&i| (0.35..=1.1).contains(&c.a1[i]))
        .collect();
    if z.len() < 4 {
        return c;
    }
    let x: Vec<f64> = z.iter().map(|&i| c.hb[i]).collect();
    let y: Vec<f64> = z.iter().map(|&i| c.a1[i]).collect();
    let wt: Vec<f64> = z.iter().map(|&i| c.n[i].sqrt()).collect();
    let Some(p) = polyfit(&x, &y, 1, Some(&wt)) else {
        return c;
    };
    let (b, a) = (p[0], p[1]);
    let lo = x.iter().cloned().fold(f64::INFINITY, f64::min) - bin_bpm;
    let hi = x.iter().cloned().fold(f64::NEG_INFINITY, f64::max) + bin_bpm;
    for i in 0..c.hb.len() {
        if c.hb[i] >= lo && c.hb[i] <= hi {
            c.line[i] = a + b * c.hb[i];
        }
    }
    let cross = |lv: f64| {
        let v = (lv - a) / b;
        (b < 0.0 && v >= lo && v <= hi).then_some(v)
    };
    c.vt1 = cross(0.75);
    c.vt2 = cross(0.5);
    c
}

/// The activities with beat intervals: id, sport group, local start day, name, minutes.
pub fn recordings(data: &Data) -> Vec<(String, &'static str, i32, String, f64)> {
    let ids = data.activities.text("id");
    let sport = data.activities.text("sport");
    let name = data.activities.text("name");
    let (start, end, off) = (
        data.activities.col("start"),
        data.activities.col("end"),
        data.activities.col("utc_offset_min"),
    );
    let mut out = vec![];
    for i in 0..data.activities.n {
        let Some(id) = &ids[i] else { continue };
        let key = id.trim_start_matches("garmin:").to_string();
        if data.rr.contains_key(&key) {
            out.push((
                key,
                sport_group(sport[i].as_deref().unwrap_or("")),
                day_of_ms(start[i] as i64, off[i] as i32),
                name[i].clone().unwrap_or_default(),
                (end[i] - start[i]) / 60_000.0,
            ));
        }
    }
    out
}

#[derive(Serialize, Default)]
pub struct A1Result {
    pub windows: Table,
    pub acts: Table,
    pub curves: BTreeMap<String, Curve>,
    pub history: Table,
    pub now: BTreeMap<String, serde_json::Value>,
    pub durability: Table,
}

/// Windows for every recording (cached by the caller through `cache`), per-sport curves, thresholds history and durability.
pub fn a1(data: &Data, cache: &mut BTreeMap<String, Windows>) -> A1Result {
    let recs = recordings(data);
    let mut all = Windows::default();
    let mut act_of: Vec<usize> = vec![];
    let mut acts_rows: Vec<(usize, f64, f64, f64)> = vec![];
    for (ai, (id, _, _, _, _)) in recs.iter().enumerate() {
        let w = cache
            .entry(id.clone())
            .or_insert_with(|| windows(&data.rr[id], 120.0, 30.0))
            .clone();
        if w.t_s.is_empty() {
            continue;
        }
        acts_rows.push((ai, w.t_s.len() as f64, stats::mean(&w.art), median(&w.rep)));
        for k in 0..w.t_s.len() {
            all.t_s.push(w.t_s[k]);
            all.hr.push(w.hr[k]);
            all.a1.push(w.a1[k]);
            all.art.push(w.art[k]);
            all.rep.push(w.rep[k]);
            act_of.push(ai);
        }
    }
    let mut res = A1Result::default();
    res.windows = Table::new(all.t_s.len());
    for (k, v) in [
        ("t_s", &all.t_s),
        ("hr", &all.hr),
        ("a1", &all.a1),
        ("art", &all.art),
        ("rep", &all.rep),
    ] {
        res.windows.set(k, v.clone());
    }
    res.windows
        .set("act", act_of.iter().map(|&a| a as f64).collect());
    res.acts = Table::new(acts_rows.len());
    res.acts.txt.insert(
        "id".into(),
        acts_rows
            .iter()
            .map(|r| Some(recs[r.0].0.clone()))
            .collect(),
    );
    res.acts.txt.insert(
        "sport".into(),
        acts_rows
            .iter()
            .map(|r| Some(recs[r.0].1.to_string()))
            .collect(),
    );
    res.acts.txt.insert(
        "name".into(),
        acts_rows
            .iter()
            .map(|r| Some(recs[r.0].3.clone()))
            .collect(),
    );
    res.acts.set(
        "day",
        acts_rows.iter().map(|r| recs[r.0].2 as f64).collect(),
    );
    res.acts
        .set("minutes", acts_rows.iter().map(|r| recs[r.0].4).collect());
    res.acts
        .set("windows", acts_rows.iter().map(|r| r.1).collect());
    res.acts.set("art", acts_rows.iter().map(|r| r.2).collect());
    res.acts.set("rep", acts_rows.iter().map(|r| r.3).collect());
    res.acts
        .set("act", acts_rows.iter().map(|r| r.0 as f64).collect());

    let mut hist: Vec<(String, f64, Option<f64>, Option<f64>, f64)> = vec![];
    for sp in ["run", "ride", "ski"] {
        let fresh: Vec<usize> = (0..all.t_s.len())
            .filter(|&i| all.t_s[i] <= 3720.0 && recs[act_of[i]].1 == sp)
            .collect();
        if fresh.is_empty() {
            continue;
        }
        res.curves
            .insert(sp.to_string(), curve(&all, &fresh, 0.05, 0.3, 2.0, 4));
        let days: Vec<i32> = fresh.iter().map(|&i| recs[act_of[i]].2).collect();
        let (d0, d1) = (*days.iter().min().unwrap(), *days.iter().max().unwrap() + 6);
        let mut end = d0;
        let mut last = (None, None, None::<i32>);
        while end <= d1 {
            let sel: Vec<usize> = fresh
                .iter()
                .zip(&days)
                .filter(|(_, &d)| d >= end - 42 && d <= end)
                .map(|(&i, _)| i)
                .collect();
            let mut acts: Vec<usize> = sel.iter().map(|&i| act_of[i]).collect();
            acts.sort();
            acts.dedup();
            if acts.len() >= 3 {
                let c = curve(&all, &sel, 0.05, 0.3, 2.0, 4);
                hist.push((sp.to_string(), end as f64, c.vt1, c.vt2, acts.len() as f64));
                if c.vt1.is_some() {
                    last.0 = c.vt1;
                    last.2 = Some(end);
                }
                if c.vt2.is_some() {
                    last.1 = c.vt2;
                }
            }
            end += 7;
        }
        let all_c = res.curves[sp].clone();
        let n_acts = recs.iter().filter(|r| r.1 == sp).count();
        res.now.insert(sp.to_string(), serde_json::json!({"vt1": last.0, "vt2": last.1, "as_of": last.2.map(crate::time::iso), "vt1_all": all_c.vt1, "vt2_all": all_c.vt2, "activities": n_acts}));
    }
    res.history = Table::new(hist.len());
    res.history.txt.insert(
        "sport".into(),
        hist.iter().map(|h| Some(h.0.clone())).collect(),
    );
    res.history.set("day", hist.iter().map(|h| h.1).collect());
    res.history.set(
        "vt1",
        hist.iter().map(|h| h.2.unwrap_or(f64::NAN)).collect(),
    );
    res.history.set(
        "vt2",
        hist.iter().map(|h| h.3.unwrap_or(f64::NAN)).collect(),
    );
    res.history
        .set("activities", hist.iter().map(|h| h.4).collect());

    // durability: alpha-1 against the first hour's fit at the same heart rate, in 30-minute blocks of long recordings
    let mut rows: Vec<(usize, f64, f64)> = vec![];
    for r in &acts_rows {
        let ai = r.0;
        let idx: Vec<usize> = (0..all.t_s.len())
            .filter(|&i| act_of[i] == ai && all.art[i] <= 0.05 && all.rep[i] <= 0.3)
            .collect();
        if idx.is_empty() || idx.iter().map(|&i| all.t_s[i]).fold(0.0, f64::max) < 7200.0 {
            continue;
        }
        let f: Vec<usize> = idx
            .iter()
            .copied()
            .filter(|&i| all.t_s[i] <= 3720.0)
            .collect();
        let hmax = f
            .iter()
            .map(|&i| all.hr[i])
            .fold(f64::NEG_INFINITY, f64::max);
        let hmin = f.iter().map(|&i| all.hr[i]).fold(f64::INFINITY, f64::min);
        if f.len() < 12 || hmax - hmin < 8.0 {
            continue;
        }
        let x: Vec<f64> = f.iter().map(|&i| all.hr[i]).collect();
        let y: Vec<f64> = f.iter().map(|&i| all.a1[i]).collect();
        let Some(p) = polyfit(&x, &y, 1, None) else {
            continue;
        };
        let mut blocks: BTreeMap<i64, Vec<f64>> = BTreeMap::new();
        for &i in &idx {
            let block = ((all.t_s[i] - 1.0) / 1800.0).floor() * 0.5 + 0.5;
            blocks
                .entry((block * 10.0).round() as i64)
                .or_default()
                .push(all.a1[i] - (p[1] + p[0] * all.hr[i]));
        }
        for (b, v) in blocks {
            if v.len() >= 8 {
                rows.push((ai, b as f64 / 10.0, stats::mean(&v)));
            }
        }
    }
    res.durability = Table::new(rows.len());
    res.durability
        .set("act", rows.iter().map(|r| r.0 as f64).collect());
    res.durability
        .set("block", rows.iter().map(|r| r.1).collect());
    res.durability
        .set("resid", rows.iter().map(|r| r.2).collect());
    res.durability.txt.insert(
        "sport".into(),
        rows.iter().map(|r| Some(recs[r.0].1.to_string())).collect(),
    );
    let _ = (MS_DAY, quantile_nearest(&[], 0.5));
    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::Rng;

    #[test]
    fn white_noise_has_alpha_near_half_and_a_walk_near_one_and_a_half() {
        let mut r = Rng::new(1);
        let noise: Vec<f64> = (0..2000).map(|_| 800.0 + r.normal() * 30.0).collect();
        let a = dfa_a1(&noise);
        assert!((a - 0.5).abs() < 0.15, "{a}");
        let mut x = 800.0;
        let walk: Vec<f64> = (0..2000)
            .map(|_| {
                x += r.normal() * 5.0;
                x
            })
            .collect();
        assert!(dfa_a1(&walk) > 1.2);
    }

    #[test]
    fn correction_fixes_a_missed_beat_and_flags_ectopics() {
        let mut rr: Vec<f64> = (0..200).map(|i| 800.0 + ((i * 37) % 11) as f64).collect();
        rr[100] = 1600.0; // a missed beat
        rr[150] = 300.0; // an early beat
        let (out, bad) = correct(&rr);
        assert!(bad[100] && bad[150]);
        assert!(
            out.iter().all(|&v| v > 350.0 && v < 1000.0),
            "{:?}",
            out.iter().cloned().fold(0.0, f64::max)
        );
    }

    #[test]
    fn windows_cover_a_clean_recording() {
        let mut r = Rng::new(2);
        let rr: Vec<f64> = (0..600).map(|_| 700.0 + r.normal() * 20.0).collect();
        let w = windows(&rr, 120.0, 30.0);
        assert!(w.t_s.len() >= 8, "{}", w.t_s.len());
        assert!(w.hr.iter().all(|&h| (80.0..90.0).contains(&h)));
        assert!(w.art.iter().all(|&a| a < 0.1));
        // a sensor gap splits the series: windows never straddle it
        let mut gapped = rr.clone();
        gapped.insert(300, 5000.0);
        let g = windows(&gapped, 120.0, 30.0);
        assert!(g.t_s.len() > 0);
    }

    #[test]
    fn curve_finds_thresholds_where_alpha_falls_through_the_levels() {
        // alpha-1 falls linearly from 1.2 at 100 bpm to 0.2 at 180 bpm: 0.75 at 136 bpm, 0.5 at 156 bpm
        let mut w = Windows::default();
        for hr in (100..180).step_by(1) {
            for _ in 0..6 {
                w.t_s.push(100.0);
                w.hr.push(hr as f64 + 0.5);
                w.a1.push(1.2 - (hr as f64 + 0.5 - 100.0) / 80.0);
                w.art.push(0.0);
                w.rep.push(0.0);
            }
        }
        let idx: Vec<usize> = (0..w.t_s.len()).collect();
        let c = curve(&w, &idx, 0.05, 0.3, 2.0, 4);
        assert!((c.vt1.unwrap() - 136.0).abs() < 2.0, "{:?}", c.vt1);
        assert!((c.vt2.unwrap() - 156.0).abs() < 2.0, "{:?}", c.vt2);
    }
}
