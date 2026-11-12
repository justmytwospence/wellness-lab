//! Rest-activity and sleep-timing rhythms from the minute data: interdaily stability, intradaily variability, L5 and M10
//! (Van Someren 1999), cosinor fits (Cornelissen 2014), the Sleep Regularity Index (Phillips 2017), social jet lag and chronotype
//! (Roenneberg 2012; Jankowski 2017), composite phase deviation (Fischer 2016), sleep debt, a circadian rhythm of heart rate
//! after Bowman 2021, and a model of melatonin onset (Forger, Jewett and Kronauer 1999, in the form of Arcascope's `circadian`).

use crate::data::{Data, Table};
use crate::labels::Settings;
use crate::math::Det;
use crate::nights::{main_sleeps, night_features};
use crate::stats::{self, mean, median, ols};
use crate::time::{circ_diff_hours, circ_mean_hours, MS_MIN};
use std::f64::consts::{PI, TAU};

pub const MIN_DAY: usize = 1440;

/// One local day per 1440 columns, from local midnight of `day0`.
pub struct Grid {
    pub day0: i32,
    pub days: usize,
    pub act: Vec<f64>,
    pub hr: Vec<f64>,
    pub known: Vec<bool>,
    pub asleep: Vec<bool>,
    /// minutes within 30 of an activity
    pub busy: Vec<bool>,
}

fn minute_index(data: &Data, t: i64, day0: i32, days: usize) -> Option<usize> {
    let off = data.offset_at(t);
    let local = t + off as i64 * MS_MIN;
    let m = local.div_euclid(MS_MIN) - day0 as i64 * MIN_DAY as i64;
    (m >= 0 && (m as usize) < days * MIN_DAY).then_some(m as usize)
}

pub fn grid(data: &Data, from: i32, to: i32) -> Grid {
    let days = (to - from + 1).max(1) as usize;
    let n = days * MIN_DAY;
    let mut g = Grid {
        day0: from,
        days,
        act: vec![0.0; n],
        hr: vec![f64::NAN; n],
        known: vec![false; n],
        asleep: vec![false; n],
        busy: vec![false; n],
    };
    let mut seen = vec![false; n];
    if let Some(s) = data.s("steps") {
        for i in 0..s.len() {
            if let Some(m) = minute_index(data, s.t[i], from, days) {
                let gap = if i > 0 {
                    ((s.t[i] - s.t[i - 1]) / MS_MIN).clamp(1, 15) as usize
                } else {
                    1
                };
                let v = s.v()[i];
                if v.is_finite() {
                    for k in 0..gap.min(m + 1) {
                        g.act[m - k] += v / gap as f64;
                    }
                }
                seen[m] = true;
            }
        }
    }
    for name in ["hr", "stress"] {
        if let Some(s) = data.s(name) {
            let mut sum = vec![0.0; n];
            let mut cnt = vec![0u32; n];
            for i in 0..s.len() {
                if let Some(m) = minute_index(data, s.t[i], from, days) {
                    seen[m] = true;
                    if name == "hr" && s.v()[i].is_finite() {
                        sum[m] += s.v()[i];
                        cnt[m] += 1;
                    }
                }
            }
            if name == "hr" {
                for m in 0..n {
                    if cnt[m] > 0 {
                        g.hr[m] = sum[m] / cnt[m] as f64;
                    }
                }
            }
        }
    }
    // known: a sample within 30 minutes
    let mut last = i64::MIN / 4;
    let mut dist = vec![i64::MAX / 4; n];
    for m in 0..n {
        if seen[m] {
            last = m as i64;
        }
        dist[m] = m as i64 - last;
    }
    let mut next = i64::MAX / 4;
    for m in (0..n).rev() {
        if seen[m] {
            next = m as i64;
        }
        dist[m] = dist[m].min(next - m as i64);
        g.known[m] = dist[m] <= 30;
    }
    let (ss, se, sk) = (
        data.sleeps.col("start"),
        data.sleeps.col("end"),
        data.sleeps.col("kind"),
    );
    let _ = sk;
    // minutes of the range an interval covers, clipped to the range; none when it lies wholly outside
    let span = |t0: f64, t1: f64| -> Option<(usize, usize)> {
        if !t0.is_finite() || !t1.is_finite() {
            return None;
        }
        let raw = |t: f64| {
            let t = t as i64;
            let local = t + data.offset_at(t) as i64 * MS_MIN;
            local.div_euclid(MS_MIN) - from as i64 * MIN_DAY as i64
        };
        let (a, b) = (raw(t0), raw(t1));
        if b < 0 || a >= n as i64 {
            return None;
        }
        Some((a.max(0) as usize, b.min(n as i64 - 1) as usize))
    };
    for i in 0..data.sleeps.n {
        if let Some((a, b)) = span(ss[i], se[i]) {
            for m in a..=b {
                g.asleep[m] = true;
            }
        }
    }
    let (as_, ae) = (data.activities.col("start"), data.activities.col("end"));
    for i in 0..data.activities.n {
        if let Some((a, b)) = span(as_[i] - 30.0 * MS_MIN as f64, ae[i] + 30.0 * MS_MIN as f64) {
            for m in a..=b {
                g.busy[m] = true;
            }
        }
    }
    g
}

impl Grid {
    fn day_known_frac(&self, d: usize) -> f64 {
        self.known[d * MIN_DAY..(d + 1) * MIN_DAY]
            .iter()
            .filter(|&&k| k)
            .count() as f64
            / MIN_DAY as f64
    }
    /// hourly means of the activity of one day, NaN for hours with no known minute
    fn hourly(&self, d: usize, src: &[f64]) -> [f64; 24] {
        let mut out = [f64::NAN; 24];
        for (h, o) in out.iter_mut().enumerate() {
            let a = d * MIN_DAY + h * 60;
            let v: Vec<f64> = (a..a + 60)
                .filter(|&m| self.known[m])
                .map(|m| src[m])
                .collect();
            if !v.is_empty() {
                *o = mean(&v);
            }
        }
        out
    }
}

fn window_days(g: &Grid, end: usize, len: usize, min_days: usize) -> Option<Vec<usize>> {
    let start = (end + 1).saturating_sub(len);
    let days: Vec<usize> = (start..=end)
        .filter(|&d| g.day_known_frac(d) >= 0.8)
        .collect();
    (days.len() >= min_days).then_some(days)
}

/// Rolling 14-day IS, IV, relative amplitude, L5 and M10 of the hourly activity.
pub fn nonparametric(g: &Grid) -> Table {
    let mut cols: Vec<(&str, Vec<f64>)> = [
        "day",
        "is",
        "iv",
        "ra",
        "l5",
        "m10",
        "l5_onset",
        "m10_onset",
    ]
    .iter()
    .map(|c| (*c, vec![]))
    .collect();
    for end in 0..g.days {
        let Some(days) = window_days(g, end, 14, 10) else {
            continue;
        };
        let x: Vec<f64> = days.iter().flat_map(|&d| g.hourly(d, &g.act)).collect();
        if x.iter().any(|v| v.is_nan()) {
            continue;
        }
        let n = x.len() as f64;
        let m = mean(&x);
        let tot: f64 = x.iter().map(|v| (v - m).powi(2)).sum();
        if tot <= 0.0 {
            continue;
        }
        let mut prof = [0.0; 24];
        for (h, p) in prof.iter_mut().enumerate() {
            *p = mean(&(0..days.len()).map(|k| x[k * 24 + h]).collect::<Vec<_>>());
        }
        let is = n * prof.iter().map(|p| (p - m).powi(2)).sum::<f64>() / (24.0 * tot);
        let iv = n * x.windows(2).map(|w| (w[1] - w[0]).powi(2)).sum::<f64>() / ((n - 1.0) * tot);
        let circ = |len: usize, want_max: bool| -> (f64, f64) {
            let mut best = (
                if want_max {
                    f64::NEG_INFINITY
                } else {
                    f64::INFINITY
                },
                0.0,
            );
            for s in 0..24 {
                let v = (0..len).map(|k| prof[(s + k) % 24]).sum::<f64>() / len as f64;
                if (want_max && v > best.0) || (!want_max && v < best.0) {
                    best = (v, s as f64);
                }
            }
            best
        };
        let (l5, l5o) = circ(5, false);
        let (m10, m10o) = circ(10, true);
        let vals = [
            (g.day0 + end as i32) as f64,
            is,
            iv,
            (m10 - l5) / (m10 + l5),
            l5,
            m10,
            l5o,
            m10o,
        ];
        for (c, v) in cols.iter_mut().zip(vals) {
            c.1.push(v);
        }
    }
    table(cols)
}

fn table(cols: Vec<(&str, Vec<f64>)>) -> Table {
    let mut t = Table::new(cols.first().map(|c| c.1.len()).unwrap_or(0));
    for (k, v) in cols {
        t.set(k, v);
    }
    t
}

/// A cosine plus constant fit to a 24-hour profile (Cornelissen 2014): mesor, amplitude, acrophase (clock hour of the peak), R squared.
fn cosinor(x: &[f64]) -> Option<(f64, f64, f64, f64)> {
    let rows: Vec<Vec<f64>> = (0..x.len())
        .filter(|&i| x[i].is_finite())
        .map(|i| {
            vec![
                1.0,
                (TAU * (i % 24) as f64 / 24.0).det_cos(),
                (TAU * (i % 24) as f64 / 24.0).det_sin(),
            ]
        })
        .collect();
    let y: Vec<f64> = x.iter().copied().filter(|v| v.is_finite()).collect();
    if y.len() < 24 {
        return None;
    }
    let f = ols(&rows, &y)?;
    let (b1, b2) = (f.beta[1], f.beta[2]);
    Some((
        f.beta[0],
        (b1 * b1 + b2 * b2).sqrt(),
        (b2.det_atan2(b1) * 24.0 / TAU).rem_euclid(24.0),
        f.r2,
    ))
}

/// Weekly cosinor fits of activity and of heart rate outside activities.
pub fn cosinor_weekly(g: &Grid) -> Table {
    let mut cols: Vec<(&str, Vec<f64>)> = [
        "day",
        "act_mesor",
        "act_amp",
        "act_acro",
        "act_r2",
        "hr_mesor",
        "hr_amp",
        "hr_acro",
        "hr_r2",
    ]
    .iter()
    .map(|c| (*c, vec![]))
    .collect();
    let hr_src: Vec<f64> = (0..g.hr.len())
        .map(|m| if g.busy[m] { f64::NAN } else { g.hr[m] })
        .collect();
    for end in (6..g.days).step_by(1) {
        let Some(days) = window_days(g, end, 7, 5) else {
            continue;
        };
        let act: Vec<f64> = days.iter().flat_map(|&d| g.hourly(d, &g.act)).collect();
        let hr: Vec<f64> = days.iter().flat_map(|&d| g.hourly(d, &hr_src)).collect();
        let (Some(a), Some(h)) = (cosinor(&act), cosinor(&hr)) else {
            continue;
        };
        let v = [
            (g.day0 + end as i32) as f64,
            a.0,
            a.1,
            a.2,
            a.3,
            h.0,
            h.1,
            h.2,
            h.3,
        ];
        for (c, x) in cols.iter_mut().zip(v) {
            c.1.push(x);
        }
    }
    table(cols)
}

/// The Sleep Regularity Index of each rolling 14-day window: 200 times the chance of the same state 24 hours apart, minus 100.
pub fn sri(g: &Grid) -> Table {
    let mut d = vec![];
    let mut v = vec![];
    for end in 13..g.days {
        let (a, b) = ((end + 1 - 14) * MIN_DAY, (end + 1) * MIN_DAY);
        let (mut agree, mut valid) = (0u64, 0u64);
        for m in a..b - MIN_DAY {
            if g.known[m] && g.known[m + MIN_DAY] {
                valid += 1;
                if g.asleep[m] == g.asleep[m + MIN_DAY] {
                    agree += 1;
                }
            }
        }
        if valid as f64 > 0.6 * ((13 * MIN_DAY) as f64) {
            d.push((g.day0 + end as i32) as f64);
            v.push(200.0 * agree as f64 / valid as f64 - 100.0);
        }
    }
    table(vec![("day", d), ("sri", v)])
}

/// Social jet lag, chronotype and phase deviation per night, from the 28 nights up to it.
pub fn chronotype(nights: &Table) -> Table {
    let n = nights.n;
    let (day, onset, wake, mid, free) = (
        nights.col("day"),
        nights.col("onset_clock"),
        nights.col("wake_clock"),
        nights.col("mid_clock"),
        nights.col("free_day"),
    );
    let tib = nights.col("tib_h");
    let mut cols: Vec<(&str, Vec<f64>)> = ["day", "msw", "msf", "sjl", "sjl_sc", "msf_sc", "cpd"]
        .iter()
        .map(|c| (*c, vec![]))
        .collect();
    for i in 0..n {
        let r: Vec<usize> = (i.saturating_sub(27)..=i).collect();
        let w: Vec<usize> = r
            .iter()
            .copied()
            .filter(|&k| free[k] == 0.0 && mid[k].is_finite())
            .collect();
        let f: Vec<usize> = r
            .iter()
            .copied()
            .filter(|&k| free[k] == 1.0 && mid[k].is_finite())
            .collect();
        if w.len() < 5 || f.len() < 2 {
            continue;
        }
        let msw = circ_mean_hours(&w.iter().map(|&k| mid[k]).collect::<Vec<_>>());
        let msf = circ_mean_hours(&f.iter().map(|&k| mid[k]).collect::<Vec<_>>());
        let sjl = circ_diff_hours(msf, msw).abs();
        let sd_w = mean(&w.iter().map(|&k| tib[k]).collect::<Vec<_>>());
        let sd_f = mean(&f.iter().map(|&k| tib[k]).collect::<Vec<_>>());
        let sd_week = (5.0 * sd_w + 2.0 * sd_f) / 7.0;
        let m_on = |idx: &[usize], col: &[f64]| {
            circ_mean_hours(&idx.iter().map(|&k| col[k]).collect::<Vec<_>>())
        };
        let sjl_sc = if sd_f > sd_w {
            circ_diff_hours(m_on(&f, &onset), m_on(&w, &onset)).abs()
        } else {
            circ_diff_hours(m_on(&f, &wake), m_on(&w, &wake)).abs()
        };
        let msf_sc = if sd_f > sd_w {
            (msf - (sd_f - sd_week) / 2.0).rem_euclid(24.0)
        } else {
            msf
        };
        // composite phase deviation: mistiming against the median of the previous nights, and the change since the night before
        let prev: Vec<f64> = (i.saturating_sub(28)..i)
            .map(|k| mid[k])
            .filter(|x| x.is_finite())
            .collect();
        let cpd = if prev.len() >= 7 && i > 0 && mid[i - 1].is_finite() {
            let ref_mid = circ_mean_hours(&prev);
            (circ_diff_hours(mid[i], ref_mid).powi(2) + circ_diff_hours(mid[i], mid[i - 1]).powi(2))
                .sqrt()
        } else {
            f64::NAN
        };
        for (c, v) in cols
            .iter_mut()
            .zip([day[i], msw, msf, sjl, sjl_sc, msf_sc, cpd])
        {
            c.1.push(v);
        }
    }
    table(cols)
}

/// Exponentially weighted shortfall against the sleep need (hours per night), and the plain sum of the last 14 nights.
pub fn sleep_debt(nights: &Table, settings: &Settings) -> Table {
    let tst = nights.col("tst_h");
    let free = nights.col("free_day");
    let bias = settings.tst_bias_min / 60.0;
    let corr: Vec<f64> = tst.iter().map(|t| t - bias).collect();
    let n = nights.n;
    let (mut d, mut debt, mut debt14, mut need_col) = (vec![], vec![], vec![], vec![]);
    let w: Vec<f64> = (0..14).map(|k| (-(k as f64) / 4.0).det_exp()).collect();
    let wsum: f64 = w.iter().sum();
    for i in 13..n {
        let hist: Vec<f64> = (i.saturating_sub(90)..i)
            .filter(|&k| free[k] == 1.0)
            .map(|k| corr[k])
            .collect();
        let need = settings.sleep_need_h.unwrap_or_else(|| {
            if hist.len() >= 5 {
                median(&hist)
            } else {
                f64::NAN
            }
        });
        if !need.is_finite() {
            continue;
        }
        let vals: Vec<f64> = (0..14).map(|k| need - corr[i - k]).collect();
        if vals.iter().any(|v| v.is_nan()) {
            continue;
        }
        d.push(nights.col("day")[i]);
        debt.push(vals.iter().zip(&w).map(|(v, w)| v * w).sum::<f64>() / wsum);
        debt14.push(vals.iter().sum());
        need_col.push(need);
    }
    table(vec![
        ("day", d),
        ("debt", debt),
        ("debt14", debt14),
        ("need", need_col),
    ])
}

/// Heart rate across the day as a cosine plus the effects of movement and sleep, fitted weekly: a simplified form of Bowman 2021.
pub fn hr_rhythm(g: &Grid) -> Table {
    let mut cols: Vec<(&str, Vec<f64>)> = [
        "day",
        "mesor",
        "amp",
        "peak",
        "trough",
        "per_log_step",
        "sleep_effect",
        "r2",
    ]
    .iter()
    .map(|c| (*c, vec![]))
    .collect();
    for end in (6..g.days).step_by(3) {
        let Some(days) = window_days(g, end, 7, 5) else {
            continue;
        };
        let mut y = vec![];
        let mut hh = vec![];
        let mut x = vec![];
        let mut s = vec![];
        for &d in &days {
            for m in (d * MIN_DAY..(d + 1) * MIN_DAY).step_by(5) {
                if g.hr[m].is_finite() && !g.busy[m] {
                    y.push(g.hr[m]);
                    hh.push((m % MIN_DAY) as f64 / 60.0);
                    x.push((1.0 + g.act[m]).det_ln());
                    s.push(g.asleep[m] as u8 as f64);
                }
            }
        }
        if y.len() < 200 {
            continue;
        }
        // y = b + c cos(w h) + d sin(w h) + e x + f s: the phase search of a cosine is linear in cos and sin
        let rows: Vec<Vec<f64>> = (0..y.len())
            .map(|i| {
                let a = TAU * hh[i] / 24.0;
                vec![1.0, a.det_cos(), a.det_sin(), x[i], s[i]]
            })
            .collect();
        let Some(f) = ols(&rows, &y) else {
            continue;
        };
        let (cc, ss) = (f.beta[1], f.beta[2]);
        let phi = (ss.det_atan2(cc) * 24.0 / TAU).rem_euclid(24.0);
        let beta = [f.beta[0], (cc * cc + ss * ss).sqrt(), f.beta[3], f.beta[4]];
        let r2 = f.r2;
        for (c, v) in cols.iter_mut().zip([
            (g.day0 + end as i32) as f64,
            beta[0],
            beta[1],
            phi,
            (phi + 12.0).rem_euclid(24.0),
            beta[2],
            beta[3],
            r2,
        ]) {
            c.1.push(v);
        }
    }
    table(cols)
}

/// Light in lux from what the minute data says: dark asleep, a dim 10 lux awake, brighter with movement in the daytime.
fn light_proxy(g: &Grid, m: usize) -> f64 {
    if g.asleep[m] {
        return 0.0;
    }
    let hour = (m % MIN_DAY) as f64 / 60.0;
    if g.act[m] > 0.0 && (6.0..20.0).contains(&hour) {
        (50.0 + 40.0 * g.act[m]).min(10_000.0)
    } else {
        10.0
    }
}

/// Forger, Jewett and Kronauer (1999) in Arcascope's form, driven by the light proxy at one-minute steps (RK4); returns the
/// estimated dim-light melatonin onset (the core-temperature minimum, the minimum of x, minus 7 h) as a clock hour per day.
pub fn forger_dlmo(g: &Grid) -> Table {
    let (taux, mu, big_g, a0, beta, p, i0, k) = (
        24.2f64, 0.23f64, 33.75f64, 0.05f64, 0.0075f64, 0.5f64, 9500.0f64, 0.55f64,
    );
    let deriv = |s: [f64; 3], light: f64| -> [f64; 3] {
        let (x, xc, n) = (s[0], s[1], s[2]);
        let alpha = a0 * (light / i0).det_powf(p);
        let bhat = big_g * (1.0 - n) * alpha * (1.0 - 0.4 * x) * (1.0 - 0.4 * xc);
        let mu_term = mu * (xc - 4.0 / 3.0 * xc.powi(3));
        let taux_term = (24.0 / (0.99669 * taux)).powi(2) + k * bhat;
        [
            PI / 12.0 * (xc + bhat),
            PI / 12.0 * (mu_term - x * taux_term),
            60.0 * (alpha * (1.0 - n) - beta * n),
        ]
    };
    let dt = 1.0 / 60.0; // hours
    let mut s = [-0.0843259, -1.09607546, 0.45584306];
    let n = g.days * MIN_DAY;
    let mut xs = Vec::with_capacity(n);
    // a burn-in of two weeks on the first fortnight's light, repeated, settles the initial state
    let burn = (14 * MIN_DAY).min(n);
    for pass in 0..2 {
        for m in 0..if pass == 0 { burn } else { n } {
            let light = light_proxy(g, m);
            let k1 = deriv(s, light);
            let k2 = deriv(
                [
                    s[0] + 0.5 * dt * k1[0],
                    s[1] + 0.5 * dt * k1[1],
                    s[2] + 0.5 * dt * k1[2],
                ],
                light,
            );
            let k3 = deriv(
                [
                    s[0] + 0.5 * dt * k2[0],
                    s[1] + 0.5 * dt * k2[1],
                    s[2] + 0.5 * dt * k2[2],
                ],
                light,
            );
            let k4 = deriv(
                [s[0] + dt * k3[0], s[1] + dt * k3[1], s[2] + dt * k3[2]],
                light,
            );
            for i in 0..3 {
                s[i] += dt / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
            }
            if pass == 1 {
                xs.push(s[0]);
            }
        }
    }
    let (mut d, mut v) = (vec![], vec![]);
    for day in 0..g.days {
        let slice = &xs[day * MIN_DAY..(day + 1) * MIN_DAY];
        if g.day_known_frac(day) < 0.8 {
            continue;
        }
        // the minimum of x within the day's second half and the next morning's first (a night)
        let (mut best, mut at) = (f64::INFINITY, 0usize);
        for (m, &x) in slice.iter().enumerate() {
            if x < best {
                best = x;
                at = m;
            }
        }
        d.push((g.day0 + day as i32) as f64);
        v.push(((at as f64 / 60.0) - 7.0).rem_euclid(24.0));
    }
    table(vec![("day", d), ("dlmo", v)])
}

/// Double-plotted actogram rows: for each day, 2 x 24 h of activity per `period_h`.
pub fn actogram(g: &Grid, period_h: f64) -> (Vec<Vec<f64>>, Vec<i32>) {
    let per = (period_h * 60.0).round() as usize;
    let mut rows = vec![];
    let mut days = vec![];
    let total = g.days * MIN_DAY;
    let mut start = 0usize;
    while start + 2 * per <= total || start + per < total {
        let row: Vec<f64> = (0..2 * per)
            .map(|k| {
                if start + k < total && g.known[start + k] {
                    g.act[start + k]
                } else {
                    f64::NAN
                }
            })
            .collect();
        rows.push(row);
        days.push(g.day0 + (start / MIN_DAY) as i32);
        start += per;
    }
    (rows, days)
}

pub struct Circadian {
    pub nonparametric: Table,
    pub cosinor: Table,
    pub sri: Table,
    pub chronotype: Table,
    pub sleep_debt: Table,
    pub hr_rhythm: Table,
    pub dlmo: Table,
}

pub fn run(
    data: &Data,
    labels: &[crate::labels::Label],
    settings: &Settings,
    from: i32,
    to: i32,
) -> Circadian {
    let nights = night_features(data, labels, settings);
    run_with(data, &nights, settings, from, to)
}

/// As `run`, with the night table already computed.
pub fn run_with(data: &Data, nights: &Table, settings: &Settings, from: i32, to: i32) -> Circadian {
    let g = grid(data, from, to);
    Circadian {
        nonparametric: nonparametric(&g),
        cosinor: cosinor_weekly(&g),
        sri: sri(&g),
        chronotype: chronotype(nights),
        sleep_debt: sleep_debt(nights, settings),
        hr_rhythm: hr_rhythm(&g),
        dlmo: forger_dlmo(&g),
    }
}

#[allow(dead_code)]
fn _touch(d: &Data) -> usize {
    main_sleeps(d).len() + stats::nan_len(&[0.0])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::synth;

    #[test]
    fn rhythm_metrics_on_regular_synthetic_life() {
        let (d, _) = synth(5, 90, 19_723);
        let g = grid(&d, 19_723, 19_723 + 89);
        let np = nonparametric(&g);
        assert!(np.n > 30);
        let is = np.col("is");
        let iv = np.col("iv");
        assert!(mean(&is) > 0.5 && mean(&is) <= 1.0, "IS {}", mean(&is));
        assert!(mean(&iv) > 0.0 && mean(&iv) < 2.5, "IV {}", mean(&iv));
        let ra = np.col("ra");
        assert!(mean(&ra) > 0.5);
        let sr = sri(&g);
        assert!(
            sr.n > 30 && mean(&sr.col("sri")) > 60.0,
            "SRI {}",
            mean(&sr.col("sri"))
        );
        let cs = cosinor_weekly(&g);
        assert!(cs.n > 30);
        // heart rate peaks in the afternoon or evening
        let acro = cs.col("hr_acro");
        assert!(mean(&acro) > 8.0 && mean(&acro) < 22.0, "{}", mean(&acro));
        let hr = hr_rhythm(&g);
        assert!(hr.n > 10 && mean(&hr.col("amp")) > 0.0);
    }

    #[test]
    fn a_range_after_the_sleeps_is_not_all_asleep() {
        let (d, _) = synth(5, 120, 19_723);
        let g = grid(&d, 19_723 + 60, 19_723 + 100);
        let asleep = g.asleep.iter().filter(|&&a| a).count() as f64 / g.asleep.len() as f64;
        assert!((0.2..0.45).contains(&asleep), "{asleep}");
        let busy = g.busy.iter().filter(|&&a| a).count() as f64 / g.busy.len() as f64;
        assert!(busy < 0.3, "{busy}");
        assert!(hr_rhythm(&g).n > 5);
    }

    #[test]
    fn chronotype_debt_and_dlmo() {
        let (d, _) = synth(5, 120, 19_723);
        let nt = night_features(&d, &[], &Settings::default());
        let c = chronotype(&nt);
        assert!(c.n > 50);
        // the synthetic free nights run an hour later
        let sjl = mean(&c.col("sjl"));
        assert!(sjl > 0.5 && sjl < 2.5, "{sjl}");
        let debt = sleep_debt(&nt, &Settings::default());
        assert!(debt.n > 50 && debt.col("need").iter().all(|n| n.is_finite()));
        let g = grid(&d, 19_723, 19_723 + 119);
        let dl = forger_dlmo(&g);
        assert!(dl.n > 50);
        let mean_dlmo = circ_mean_hours(&dl.col("dlmo"));
        assert!(
            (17.0..24.0).contains(&mean_dlmo) || mean_dlmo < 2.0,
            "{mean_dlmo}"
        );
        let (rows, days) = actogram(&g, 24.0);
        assert!(rows.len() >= 100 && rows[0].len() == 2880 && days[0] == 19_723);
    }
}
