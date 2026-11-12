//! Beat-to-beat intervals of activities recorded with Log HRV: how clean each recording is, the cleaned series, RMSSD in a
//! one-minute window and over the recording, the Poincare plot and a Lomb-Scargle spectrum (notebook cells rr_sessions,
//! beats, window_*, poincare_chart).

use crate::data::{Data, Table};
use crate::stats::{self, mean, median, rolling_median, Rng};
use serde::Serialize;

pub const KEPT: u8 = 0;
pub const ARTIFACT: u8 = 1;
pub const OUT_OF_RANGE: u8 = 2;

/// One row per recording with at least 500 usable beats, cleanest first.
pub fn sessions(data: &Data) -> Table {
    let ids = data.activities.text("id");
    let name = data.activities.text("name");
    let start = data.activities.col("start");
    let off = data.activities.col("utc_offset_min");
    let avg_hr = data.activities.col("avg_hr");
    struct S {
        id: String,
        name: String,
        day: f64,
        beats: usize,
        rep: f64,
        art: f64,
        quality: u8,
        avg_hr: f64,
    }
    let mut rows: Vec<S> = vec![];
    for i in 0..data.activities.n {
        let Some(id) = &ids[i] else { continue };
        let key = id.trim_start_matches("garmin:");
        let Some(rr) = data.rr.get(key) else { continue };
        let kept: Vec<f64> = rr
            .iter()
            .copied()
            .filter(|&r| (300.0..=2000.0).contains(&r))
            .collect();
        if kept.len() < 500 {
            continue;
        }
        let n = kept.len();
        let (mut rep_n, mut rep_d, mut art) = (0usize, 0usize, 0usize);
        for k in 0..n {
            if k + 1 < n {
                rep_d += 1;
                if kept[k + 1] == kept[k] {
                    rep_n += 1;
                }
            }
            let (a, b) = (k.saturating_sub(5), (k + 6).min(n));
            let med = median(&kept[a..b]);
            if (kept[k] - med).abs() > 0.2 * med {
                art += 1;
            }
        }
        let (rep, artf) = (rep_n as f64 / rep_d as f64, art as f64 / n as f64);
        let quality = if rep < 0.1 && artf < 0.03 {
            0
        } else if rep >= 0.1 {
            2
        } else {
            1
        };
        rows.push(S {
            id: key.to_string(),
            name: name[i].clone().unwrap_or_default(),
            day: crate::time::day_of_ms(start[i] as i64, off[i] as i32) as f64,
            beats: n,
            rep,
            art: artf,
            quality,
            avg_hr: avg_hr[i],
        });
    }
    rows.sort_by(|a, b| {
        a.quality
            .cmp(&b.quality)
            .then(a.art.partial_cmp(&b.art).unwrap())
    });
    let mut t = Table::new(rows.len());
    t.txt.insert(
        "id".into(),
        rows.iter().map(|r| Some(r.id.clone())).collect(),
    );
    t.txt.insert(
        "name".into(),
        rows.iter().map(|r| Some(r.name.clone())).collect(),
    );
    t.txt.insert(
        "quality".into(),
        rows.iter()
            .map(|r| Some(["clean", "noisy", "smoothed"][r.quality as usize].to_string()))
            .collect(),
    );
    t.set("day", rows.iter().map(|r| r.day).collect());
    t.set("beats", rows.iter().map(|r| r.beats as f64).collect());
    t.set("repeat_share", rows.iter().map(|r| r.rep).collect());
    t.set("artifact_share", rows.iter().map(|r| r.art).collect());
    t.set("avg_hr", rows.iter().map(|r| r.avg_hr).collect());
    t
}

#[derive(Serialize, Default, Clone)]
pub struct Beats {
    /// position in the file, before the sensor gaps were dropped
    pub beat: Vec<f64>,
    pub rr: Vec<f64>,
    pub t: Vec<f64>,
    pub status: Vec<u8>,
}

/// Drop the no-beat filler (4999 ms or more), mark the rest kept, out of range (outside 300 to 2000 ms) or artefact
/// (more than 20 % from the median of 11 neighbours).
pub fn clean(rr: &[f64]) -> Beats {
    let n = rr.len();
    let gap: Vec<bool> = rr.iter().map(|&r| r >= 4999.0).collect();
    let valid: Vec<bool> = (0..n)
        .map(|i| !gap[i] && rr[i] >= 300.0 && rr[i] <= 2000.0)
        .collect();
    let masked: Vec<f64> = (0..n)
        .map(|i| if valid[i] { rr[i] } else { f64::NAN })
        .collect();
    let med = rolling_median(&masked, 11, 3, true);
    let mut out = Beats::default();
    let mut acc = 0.0;
    for i in 0..n {
        if !gap[i] {
            acc += rr[i];
        }
        if gap[i] {
            continue;
        }
        let status = if !valid[i] {
            OUT_OF_RANGE
        } else if (rr[i] - med[i]).abs() > 0.2 * med[i] {
            ARTIFACT
        } else {
            KEPT
        };
        out.beat.push(i as f64);
        out.rr.push(rr[i]);
        out.t.push(acc / 1000.0);
        out.status.push(status);
    }
    out
}

fn successive(b: &Beats) -> Vec<f64> {
    let n = b.rr.len();
    (0..n)
        .map(|i| {
            if i + 1 < n && b.status[i] == KEPT && b.status[i + 1] == KEPT {
                b.rr[i + 1] - b.rr[i]
            } else {
                f64::NAN
            }
        })
        .collect()
}

/// The minute (since the start) with the most variable beats among those with at least 30 kept beats.
pub fn default_minute(b: &Beats) -> usize {
    let kept: Vec<usize> = (0..b.rr.len()).filter(|&i| b.status[i] == KEPT).collect();
    let mut by_min: std::collections::BTreeMap<i64, (f64, usize, usize)> =
        std::collections::BTreeMap::new();
    for (k, &i) in kept.iter().enumerate() {
        let e = by_min
            .entry((b.t[i] / 60.0).floor() as i64)
            .or_insert((0.0, 0, 0));
        e.2 += 1;
        if k > 0 {
            let d = b.rr[i] - b.rr[kept[k - 1]];
            e.0 += d * d;
            e.1 += 1;
        }
    }
    by_min
        .iter()
        .filter(|(_, v)| v.2 >= 30 && v.1 > 0)
        .map(|(m, v)| (*m, v.0 / v.1 as f64))
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .map(|(m, _)| m as usize)
        .unwrap_or(0)
}

#[derive(Serialize, Default)]
pub struct Poincare {
    pub sd1: f64,
    pub sd2: f64,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
}

#[derive(Serialize, Default)]
pub struct Spectrum {
    pub f: Vec<f64>,
    /// relative power, normalised to the peak and clipped at 1e-4
    pub p: Vec<f64>,
    pub vlf: f64,
    pub lf: f64,
    pub hf: f64,
    pub lf_hf: f64,
}

/// scipy.signal.lombscargle (unnormalised) at angular frequencies `w`.
pub fn lombscargle(t: &[f64], y: &[f64], w: &[f64]) -> Vec<f64> {
    w.iter()
        .map(|&w| {
            let (mut xc, mut xs, mut cc, mut ss, mut cs) = (0.0, 0.0, 0.0, 0.0, 0.0);
            for (ti, yi) in t.iter().zip(y) {
                let (s, c) = (w * ti).sin_cos();
                xc += yi * c;
                xs += yi * s;
                cc += c * c;
                ss += s * s;
                cs += c * s;
            }
            let tau = 0.5 * (2.0 * cs).atan2(cc - ss);
            let (st, ct) = tau.sin_cos();
            let (c2, s2, cst) = (ct * ct, st * st, 2.0 * ct * st);
            0.5 * ((ct * xc + st * xs).powi(2) / (c2 * cc + cst * cs + s2 * ss)
                + (ct * xs - st * xc).powi(2) / (c2 * ss - cst * cs + s2 * cc))
        })
        .collect()
}

pub fn spectrum(t: &[f64], rr: &[f64]) -> Spectrum {
    let f: Vec<f64> = (0..300)
        .map(|i| 0.01 + (0.6 - 0.01) * i as f64 / 299.0)
        .collect();
    let w: Vec<f64> = f.iter().map(|v| std::f64::consts::TAU * v).collect();
    let mut segs: Vec<Vec<f64>> = vec![];
    if let (Some(&t0), Some(&t1)) = (t.first(), t.last()) {
        let mut s0 = t0;
        while s0 < t1 - 300.0 {
            let idx: Vec<usize> = (0..t.len())
                .filter(|&i| t[i] >= s0 && t[i] < s0 + 300.0)
                .collect();
            if idx.len() > 100 {
                let tt: Vec<f64> = idx.iter().map(|&i| t[i]).collect();
                let xx: Vec<f64> = idx.iter().map(|&i| rr[i]).collect();
                if let Some(c) = stats::polyfit(&tt, &xx, 1, None) {
                    let det: Vec<f64> = tt
                        .iter()
                        .zip(&xx)
                        .map(|(a, b)| b - (c[0] * a + c[1]))
                        .collect();
                    segs.push(lombscargle(&tt, &det, &w));
                }
            }
            s0 += 150.0;
        }
    }
    let p: Vec<f64> = if segs.is_empty() {
        vec![0.0; f.len()]
    } else {
        (0..f.len())
            .map(|k| segs.iter().map(|s| s[k]).sum::<f64>() / segs.len() as f64)
            .collect()
    };
    let pmax = p.iter().cloned().fold(0.0, f64::max).max(1e-12);
    let rel: Vec<f64> = p.iter().map(|v| (v / pmax).clamp(1e-4, 1.0)).collect();
    let band = |lo: f64, hi: f64| -> f64 {
        let mut a = 0.0;
        for k in 1..f.len() {
            if f[k - 1] >= lo && f[k] < hi {
                a += (p[k - 1] + p[k]) / 2.0 * (f[k] - f[k - 1]);
            }
        }
        a
    };
    let (vlf, lf, hf) = (band(0.0, 0.04), band(0.04, 0.15), band(0.15, 0.4));
    Spectrum {
        f,
        p: rel,
        vlf,
        lf,
        hf,
        lf_hf: lf / hf,
    }
}

#[derive(Serialize, Default)]
pub struct Detail {
    pub id: String,
    pub counts: std::collections::BTreeMap<String, usize>,
    pub beats: Table,
    pub window_minute: usize,
    pub window: Table,
    pub window_rmssd: f64,
    pub session_rmssd: f64,
    pub mean_hr: f64,
    pub poincare: Poincare,
    pub spectrum: Spectrum,
}

pub fn detail(data: &Data, id: &str, window_minute: Option<usize>) -> Option<Detail> {
    let rr = data.rr.get(id)?;
    let b = clean(rr);
    let mut counts = std::collections::BTreeMap::new();
    for (k, name) in [
        (KEPT, "kept"),
        (ARTIFACT, "artifact"),
        (OUT_OF_RANGE, "out of range"),
    ] {
        counts.insert(
            name.to_string(),
            b.status.iter().filter(|&&s| s == k).count(),
        );
    }
    counts.insert(
        "sensor gap".into(),
        rr.iter().filter(|&&r| r >= 4999.0).count(),
    );
    let n = b.rr.len();
    let mut beats = Table::new(n);
    beats.set("beat", b.beat.clone());
    beats.set("rr", b.rr.clone());
    beats.set("t", b.t.clone());
    beats.set("status", b.status.iter().map(|&s| s as f64).collect());
    let d = successive(&b);
    let minute = window_minute.unwrap_or_else(|| default_minute(&b));
    let t0 = minute as f64 * 60.0;
    let sel: Vec<usize> = (0..n)
        .filter(|&i| b.t[i] >= t0 && b.t[i] <= t0 + 60.0)
        .collect();
    // inside the window the last beat has no successor
    let wd: Vec<f64> = (0..sel.len())
        .map(|k| {
            if k + 1 < sel.len() {
                d[sel[k]]
            } else {
                f64::NAN
            }
        })
        .collect();
    let mut w = Table::new(sel.len());
    w.set("beat", sel.iter().map(|&i| b.beat[i]).collect());
    w.set("sec", sel.iter().map(|&i| b.t[i] - t0).collect());
    w.set("rr", sel.iter().map(|&i| b.rr[i]).collect());
    w.set("status", sel.iter().map(|&i| b.status[i] as f64).collect());
    w.set("d", wd.clone());
    w.set("d2", wd.iter().map(|x| x * x).collect());
    let window_rmssd = mean(&wd.iter().map(|x| x * x).collect::<Vec<_>>()).sqrt();
    let session_rmssd = mean(&d.iter().map(|x| x * x).collect::<Vec<_>>()).sqrt();
    let kept: Vec<usize> = (0..n).filter(|&i| b.status[i] == KEPT).collect();
    let krr: Vec<f64> = kept.iter().map(|&i| b.rr[i]).collect();
    let kt: Vec<f64> = kept.iter().map(|&i| b.t[i]).collect();
    let (xs, ys): (Vec<f64>, Vec<f64>) = (0..krr.len().saturating_sub(1))
        .map(|i| (krr[i], krr[i + 1]))
        .unzip();
    let diff: Vec<f64> = xs.iter().zip(&ys).map(|(a, c)| c - a).collect();
    let sum: Vec<f64> = xs.iter().zip(&ys).map(|(a, c)| c + a).collect();
    let mut pc = Poincare {
        sd1: stats::std(&diff, 0) / 2f64.sqrt(),
        sd2: stats::std(&sum, 0) / 2f64.sqrt(),
        ..Default::default()
    };
    let mut order: Vec<usize> = (0..xs.len()).collect();
    if order.len() > 6000 {
        let mut rng = Rng::new(0);
        for i in 0..6000 {
            let j = i + rng.below(order.len() - i);
            order.swap(i, j);
        }
        order.truncate(6000);
    }
    pc.x = order.iter().map(|&i| xs[i]).collect();
    pc.y = order.iter().map(|&i| ys[i]).collect();
    Some(Detail {
        id: id.to_string(),
        counts,
        beats,
        window_minute: minute,
        window: w,
        window_rmssd,
        session_rmssd,
        mean_hr: 60000.0 / mean(&krr),
        poincare: pc,
        spectrum: spectrum(&kt, &krr),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recording() -> Vec<f64> {
        let mut r = Rng::new(5);
        let mut v: Vec<f64> = (0..1500)
            .map(|i| 800.0 + 40.0 * (i as f64 * 0.3).sin() + r.normal() * 10.0)
            .collect();
        v[200] = 4999.0; // a sensor gap
        v[400] = 100.0; // out of range
        v[600] = 1100.0; // an early/late beat: an artefact
        v
    }

    #[test]
    fn cleaning_marks_gaps_range_and_artefacts() {
        let b = clean(&recording());
        assert_eq!(b.rr.len(), 1499); // the gap is dropped
        assert!(b.status.iter().filter(|&&s| s == OUT_OF_RANGE).count() == 1);
        assert!(b.status.iter().filter(|&&s| s == ARTIFACT).count() >= 1);
        // time does not advance across the gap
        assert!((b.t[199] - b.t[198] - b.rr[199] / 1000.0).abs() < 1e-9);
        assert!((b.t[200] - b.t[199] - b.rr[200] / 1000.0).abs() < 1e-9);
    }

    #[test]
    fn rmssd_and_poincare_of_a_known_series() {
        let mut d = Data::default();
        // alternating 800 and 840: every successive difference is 40 ms, so RMSSD is 40 and SD1 is 40 / sqrt(2) ... for ddof 0
        let rr: Vec<f64> = (0..1000)
            .map(|i| if i % 2 == 0 { 800.0 } else { 840.0 })
            .collect();
        d.rr.insert("1".into(), rr);
        d.activities.n = 1;
        d.activities
            .txt
            .insert("id".into(), vec![Some("garmin:1".into())]);
        let det = detail(&d, "1", Some(0)).unwrap();
        assert!((det.session_rmssd - 40.0).abs() < 1e-9);
        assert!(
            (det.poincare.sd1 - 40.0 / 2f64.sqrt()).abs() < 0.1,
            "{}",
            det.poincare.sd1
        );
        assert!(det.poincare.sd2 < 1.0);
        assert!((det.window_rmssd - 40.0).abs() < 1e-9);
        let s = sessions(&d);
        assert_eq!(s.n, 1);
        assert_eq!(s.text("quality")[0].as_deref(), Some("clean"));
    }

    #[test]
    fn lomb_scargle_peaks_at_the_signal_frequency() {
        let t: Vec<f64> = (0..300)
            .map(|i| i as f64 * 0.9 + (i % 3) as f64 * 0.05)
            .collect();
        let y: Vec<f64> = t
            .iter()
            .map(|x| (std::f64::consts::TAU * 0.25 * x).sin())
            .collect();
        let f: Vec<f64> = (1..60).map(|i| i as f64 * 0.01).collect();
        let w: Vec<f64> = f.iter().map(|v| std::f64::consts::TAU * v).collect();
        let p = lombscargle(&t, &y, &w);
        let k = p
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap()
            .0;
        assert!((f[k] - 0.25).abs() < 0.015, "{}", f[k]);
    }
}
