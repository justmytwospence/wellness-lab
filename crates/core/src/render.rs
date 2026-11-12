//! Drawing helpers: per-pixel min/max downsampling of a line, colour maps, and the rasters of nights by time of night.

use crate::data::Data;
use crate::nights::main_sleeps;
use crate::stats::quantile_linear;
use crate::time::{local_hour, MS_HOUR, MS_MIN};
use serde::Serialize;

/// Downsample a line to at most four points per pixel column (first, minimum, maximum, last), Jugel et al. 2014.
pub fn m4(t: &[f64], v: &[f64], t0: f64, t1: f64, px: usize) -> (Vec<f64>, Vec<f64>) {
    if px == 0 || t.is_empty() || t1 <= t0 {
        return (vec![], vec![]);
    }
    let w = (t1 - t0) / px as f64;
    let (mut ot, mut ov) = (vec![], vec![]);
    let mut i = t.partition_point(|&x| x < t0);
    for c in 0..px {
        let end = t0 + (c + 1) as f64 * w;
        let a = i;
        while i < t.len() && t[i] < end {
            i += 1;
        }
        let idx: Vec<usize> = (a..i).filter(|&k| v[k].is_finite()).collect();
        if idx.is_empty() {
            continue;
        }
        let first = idx[0];
        let last = *idx.last().unwrap();
        let mn = *idx
            .iter()
            .min_by(|&&x, &&y| v[x].partial_cmp(&v[y]).unwrap())
            .unwrap();
        let mx = *idx
            .iter()
            .max_by(|&&x, &&y| v[x].partial_cmp(&v[y]).unwrap())
            .unwrap();
        let mut pts = vec![first, mn, mx, last];
        pts.sort();
        pts.dedup();
        for k in pts {
            ot.push(t[k]);
            ov.push(v[k]);
        }
    }
    (ot, ov)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Colormap {
    Night,
    Diverge,
    Cyclic,
    Stages,
}

fn hex(h: u32) -> [f64; 3] {
    [
        ((h >> 16) & 255) as f64,
        ((h >> 8) & 255) as f64,
        (h & 255) as f64,
    ]
}

fn lerp(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn hsl(h: f64, s: f64, l: f64) -> [f64; 3] {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = (h * 6.0) % 6.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    [(r + m) * 255.0, (g + m) * 255.0, (b + m) * 255.0]
}

/// The colour at position `x` in [0, 1].
pub fn color(map: Colormap, x: f64) -> [u8; 3] {
    let x = x.clamp(0.0, 1.0);
    let c = match map {
        Colormap::Night => {
            let (a, b, c) = (hex(0x1c2b55), hex(0x2ee6c5), hex(0xfff3b0));
            if x < 0.5 {
                lerp(a, b, x * 2.0)
            } else {
                lerp(b, c, (x - 0.5) * 2.0)
            }
        }
        Colormap::Diverge => {
            let (a, b, c) = (hex(0x7aa2ff), hex(0x111831), hex(0xff6b6b));
            if x < 0.5 {
                lerp(a, b, x * 2.0)
            } else {
                lerp(b, c, (x - 0.5) * 2.0)
            }
        }
        Colormap::Cyclic => hsl(x, 0.6, 0.6),
        Colormap::Stages => {
            let k = (x * 4.0).round() as u32; // 0 deep .. 4 unmeasurable
            hex([0x3b5bdb, 0x74c0fc, 0xb892ff, 0xffb547, 0x495057][k.min(4) as usize])
        }
    };
    [c[0].round() as u8, c[1].round() as u8, c[2].round() as u8]
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Rgba {
    pub width: usize,
    pub height: usize,
    #[serde(skip)]
    pub data: Vec<u8>,
}

/// Colour a matrix of values (NaN is transparent). `domain` is (low, mid, high): with `mid`, a three-point scale.
pub fn raster(
    rows: &[Vec<f64>],
    map: Colormap,
    domain: (f64, Option<f64>, f64),
    log: bool,
) -> Rgba {
    let height = rows.len();
    let width = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let mut data = vec![0u8; width * height * 4];
    let f = |v: f64| if log { v.ln() } else { v };
    let (lo, hi) = (f(domain.0), f(domain.2));
    let mid = domain.1.map(f);
    for (y, row) in rows.iter().enumerate() {
        for (x, &v) in row.iter().enumerate() {
            if !v.is_finite() || (log && v <= 0.0) {
                continue;
            }
            let u = f(v);
            let pos = match (map, mid) {
                (Colormap::Stages, _) => v / 4.0,
                (_, Some(m)) if u < m => 0.5 * (u - lo) / (m - lo).max(1e-12),
                (_, Some(m)) => 0.5 + 0.5 * (u - m) / (hi - m).max(1e-12),
                _ => (u - lo) / (hi - lo).max(1e-12),
            };
            let c = color(map, pos);
            let i = (y * width + x) * 4;
            data[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    Rgba {
        width,
        height,
        data,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Align {
    /// minutes since falling asleep
    Onset,
    /// local clock time, from 20:00
    Clock,
}

/// One row per night in `[from, to]`, one column per `bin_min` minutes: the mean of the series (or the sleep stage) in each bin.
/// Returns the rows, the day of each row and the number of minutes the columns span.
pub fn bin_matrix(
    data: &Data,
    metric: &str,
    align: Align,
    from: i32,
    to: i32,
    bin_min: i64,
) -> (Vec<Vec<f64>>, Vec<i32>, i64) {
    let span_min: i64 = match align {
        Align::Onset => 12 * 60,
        Align::Clock => 16 * 60,
    };
    let nb = (span_min / bin_min) as usize;
    let sleeps: Vec<_> = main_sleeps(data)
        .into_iter()
        .filter(|s| s.0 >= from && s.0 <= to)
        .collect();
    let mut rows = vec![];
    let mut days = vec![];
    let series = data.s(metric);
    let (st_start, st_end, st_code) = (
        data.stages.col("start"),
        data.stages.col("end"),
        data.stages.col("stage"),
    );
    for &(day, onset, wake, _) in &sleeps {
        let off = data.offset_of_day(day);
        let t0 = match align {
            Align::Onset => onset,
            Align::Clock => {
                // 20:00 local of the evening before the wake day
                let midnight = day as i64 * 86_400_000 - off as i64 * 60_000;
                midnight - 4 * MS_HOUR
            }
        };
        let mut row = vec![f64::NAN; nb];
        if metric == "stage" {
            for k in 0..data.stages.n {
                if st_end[k] <= t0 as f64
                    || st_start[k] >= (t0 + span_min * MS_MIN) as f64
                    || st_start[k] < onset as f64 - 1.0
                    || st_end[k] > wake as f64 + 1.0
                {
                    continue;
                }
                let a = (((st_start[k] - t0 as f64) / (bin_min * MS_MIN) as f64)
                    .floor()
                    .max(0.0)) as usize;
                let b =
                    (((st_end[k] - t0 as f64) / (bin_min * MS_MIN) as f64).ceil() as usize).min(nb);
                for cell in row.iter_mut().take(b).skip(a) {
                    *cell = st_code[k];
                }
            }
        } else if let Some(s) = series {
            let (a, b) = s.range(t0, t0 + span_min * MS_MIN);
            let v = s.v();
            let mut sum = vec![0.0; nb];
            let mut cnt = vec![0u32; nb];
            for k in a..b {
                if v[k].is_finite() {
                    let bin = ((s.t[k] - t0) / (bin_min * MS_MIN)) as usize;
                    if bin < nb {
                        sum[bin] += v[k];
                        cnt[bin] += 1;
                    }
                }
            }
            for bin in 0..nb {
                if cnt[bin] > 0 {
                    row[bin] = sum[bin] / cnt[bin] as f64;
                }
            }
            if align == Align::Clock {
                // outside the night the day's values would swamp the picture of sleep: keep the night only
                for (bin, cell) in row.iter_mut().enumerate() {
                    let t = t0 + bin as i64 * bin_min * MS_MIN;
                    if t + bin_min * MS_MIN <= onset || t >= wake {
                        *cell = f64::NAN;
                    }
                }
            }
        }
        rows.push(row);
        days.push(day);
    }
    (rows, days, span_min)
}

/// The colour domain the app uses for a metric: (low, mid, high, log scale).
pub fn domain_for(metric: &str, rows: &[Vec<f64>]) -> ((f64, Option<f64>, f64), bool, Colormap) {
    let all: Vec<f64> = rows
        .iter()
        .flatten()
        .copied()
        .filter(|v| v.is_finite())
        .collect();
    match metric {
        "stage" => ((0.0, None, 4.0), false, Colormap::Stages),
        "stress" => ((0.0, None, 100.0), false, Colormap::Night),
        "spo2" => ((88.0, None, 100.0), false, Colormap::Night),
        "body_battery" => ((0.0, None, 100.0), false, Colormap::Night),
        "hr" | "hrv5" | "resp" if !all.is_empty() => (
            (
                quantile_linear(&all, 0.03),
                Some(quantile_linear(&all, 0.5)),
                quantile_linear(&all, 0.97),
            ),
            true,
            Colormap::Night,
        ),
        _ if !all.is_empty() => (
            (
                quantile_linear(&all, 0.03),
                None,
                quantile_linear(&all, 0.97),
            ),
            false,
            Colormap::Night,
        ),
        _ => ((0.0, None, 1.0), false, Colormap::Night),
    }
}

#[derive(Serialize, Default)]
pub struct Typical {
    pub minute: Vec<f64>,
    pub p05: Vec<f64>,
    pub p25: Vec<f64>,
    pub p50: Vec<f64>,
    pub p75: Vec<f64>,
    pub p95: Vec<f64>,
    pub n: Vec<f64>,
}

/// Per-bin quantiles across nights: the typical night.
pub fn typical(rows: &[Vec<f64>], bin_min: i64, min_nights: usize) -> Typical {
    let nb = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let mut t = Typical::default();
    for b in 0..nb {
        let v: Vec<f64> = rows
            .iter()
            .filter_map(|r| r.get(b).copied())
            .filter(|x| x.is_finite())
            .collect();
        if v.len() >= min_nights {
            t.minute.push(b as f64 * bin_min as f64);
            for (q, dst) in [
                (0.05, &mut t.p05),
                (0.25, &mut t.p25),
                (0.5, &mut t.p50),
                (0.75, &mut t.p75),
                (0.95, &mut t.p95),
            ] {
                dst.push(quantile_linear(&v, q));
            }
            t.n.push(v.len() as f64);
        }
    }
    t
}

pub fn clock_hour(t: i64, off: i32) -> f64 {
    local_hour(t, off)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::synth;

    #[test]
    fn m4_keeps_the_extremes_of_each_column() {
        let t: Vec<f64> = (0..1000).map(|i| i as f64).collect();
        let mut v: Vec<f64> = (0..1000).map(|i| (i % 7) as f64).collect();
        v[503] = 100.0;
        v[701] = -50.0;
        let (ot, ov) = m4(&t, &v, 0.0, 1000.0, 50);
        assert!(ot.len() <= 200);
        assert!(ov.contains(&100.0) && ov.contains(&-50.0));
        assert!(ot.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn colour_maps_and_rasters() {
        assert_eq!(color(Colormap::Night, 0.0), [0x1c, 0x2b, 0x55]);
        assert_eq!(color(Colormap::Night, 1.0), [0xff, 0xf3, 0xb0]);
        assert_eq!(color(Colormap::Diverge, 0.5), [0x11, 0x18, 0x31]);
        assert_eq!(color(Colormap::Stages, 0.0), [0x3b, 0x5b, 0xdb]);
        let r = raster(
            &[vec![1.0, f64::NAN, 3.0]],
            Colormap::Night,
            (1.0, None, 3.0),
            false,
        );
        assert_eq!((r.width, r.height), (3, 1));
        assert_eq!(r.data[3], 255);
        assert_eq!(r.data[7], 0); // NaN is transparent
        assert_eq!(&r.data[8..11], &[0xff, 0xf3, 0xb0]);
    }

    #[test]
    fn nights_matrix_and_typical_night() {
        let (d, _) = synth(2, 60, 19_723);
        let (rows, days, span) = bin_matrix(&d, "hr", Align::Onset, 19_723, 19_723 + 59, 10);
        assert_eq!(rows.len(), days.len());
        assert!(rows.len() >= 55);
        assert_eq!(span, 720);
        let t = typical(&rows, 10, 10);
        assert!(!t.minute.is_empty());
        // heart rate is highest when falling asleep and falls to its nadir
        assert!(t.p50[0] > t.p50[20]);
        let (dom, log, _) = domain_for("hr", &rows);
        assert!(log && dom.0 < dom.2);
        let (srows, _, _) = bin_matrix(&d, "stage", Align::Onset, 19_723, 19_723 + 10, 10);
        assert!(srows.iter().flatten().any(|v| *v == 0.0));
        let (crows, _, cspan) = bin_matrix(&d, "hr", Align::Clock, 19_723, 19_723 + 5, 10);
        assert_eq!(cspan, 960);
        // on the clock the night sits in the middle of the row, not at its start
        assert!(crows[1][0].is_nan());
        assert!(crows[1].iter().any(|v| v.is_finite()));
    }
}
