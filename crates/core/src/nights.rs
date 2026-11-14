//! One row per night: timing, heart rate, HRV, stress, breathing, SpO2, movement and context, computed from the
//! intraday series between sleep onset and wake. The night viewer's payload is built here too.

use crate::data::{Data, Series, Table};
use crate::labels::{excluded_days, Label, Settings};
use crate::math::Det;
use crate::stats::{self, mean, median, quantile_linear, quantile_nearest, rolling};
use crate::time::{day_of_ms, local_hour, weekday, MS_DAY, MS_HOUR, MS_MIN};
use serde::Serialize;
use std::collections::BTreeMap;

/// The main sleep of each day: (day, onset ms, wake ms, row in `sleeps`), the longest when a day has several.
pub fn main_sleeps(data: &Data) -> Vec<(i32, i64, i64, usize)> {
    let (kind, day, start, end) = (
        data.sleeps.col("kind"),
        data.sleeps.col("day"),
        data.sleeps.col("start"),
        data.sleeps.col("end"),
    );
    let mut by_day: BTreeMap<i32, (i64, i64, usize)> = BTreeMap::new();
    for i in 0..data.sleeps.n {
        if kind[i] != 0.0 || !start[i].is_finite() || !end[i].is_finite() || end[i] <= start[i] {
            continue;
        }
        let e = by_day
            .entry(day[i] as i32)
            .or_insert((start[i] as i64, end[i] as i64, i));
        if (end[i] - start[i]) as i64 > e.1 - e.0 {
            *e = (start[i] as i64, end[i] as i64, i);
        }
    }
    by_day
        .into_iter()
        .map(|(d, (s, e, i))| (d, s, e, i))
        .collect()
}

/// The samples of a series with `t0 <= t < t1`: (times, values).
pub fn window<'a>(s: &'a Series, t0: i64, t1: i64) -> (&'a [i64], &'a [f64], usize) {
    let (a, b) = s.range(t0, t1);
    (&s.t[a..b], &s.v()[a..b], a)
}

/// A one-minute grid from `t0`: the mean of the samples in each minute, NaN where there are none.
pub fn minute_grid(t: &[i64], v: &[f64], t0: i64, n_min: usize) -> Vec<f64> {
    let mut sum = vec![0.0; n_min];
    let mut cnt = vec![0u32; n_min];
    for (i, &ti) in t.iter().enumerate() {
        if v[i].is_nan() {
            continue;
        }
        let m = ((ti - t0) / MS_MIN) as usize;
        if m < n_min {
            sum[m] += v[i];
            cnt[m] += 1;
        }
    }
    (0..n_min)
        .map(|m| {
            if cnt[m] > 0 {
                sum[m] / cnt[m] as f64
            } else {
                f64::NAN
            }
        })
        .collect()
}

/// Typical spacing of samples in minutes (at least one).
fn spacing_min(t: &[i64]) -> f64 {
    if t.len() < 2 {
        return 1.0;
    }
    let d: Vec<f64> = t
        .windows(2)
        .map(|w| (w[1] - w[0]) as f64 / MS_MIN as f64)
        .collect();
    median(&d).max(1.0)
}

pub fn nearest_value(s: &Series, t: i64, within_ms: i64) -> f64 {
    let i = s.t.partition_point(|&x| x < t);
    let mut best = (f64::INFINITY, f64::NAN);
    for j in [i.wrapping_sub(1), i] {
        if j < s.len() {
            let d = (s.t[j] - t).abs() as f64;
            if d < best.0 && d <= within_ms as f64 {
                best = (d, s.v()[j]);
            }
        }
    }
    best.1
}

fn slope_per_hour(grid: &[f64]) -> f64 {
    let pts: Vec<(f64, f64)> = grid
        .iter()
        .enumerate()
        .filter(|(_, v)| v.is_finite())
        .map(|(i, v)| (i as f64 / 60.0, *v))
        .collect();
    if pts.len() < 10 {
        return f64::NAN;
    }
    let (mx, my) = (
        pts.iter().map(|p| p.0).sum::<f64>() / pts.len() as f64,
        pts.iter().map(|p| p.1).sum::<f64>() / pts.len() as f64,
    );
    let num: f64 = pts.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
    let den: f64 = pts.iter().map(|p| (p.0 - mx).powi(2)).sum();
    num / den
}

fn argmin(v: &[f64]) -> Option<usize> {
    v.iter()
        .enumerate()
        .filter(|(_, x)| x.is_finite())
        .min_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(i, _)| i)
}

const COLS: &[&str] = &[
    "day",
    "onset",
    "wake",
    "mid",
    "tib_h",
    "tst_h",
    "eff",
    "onset_clock",
    "mid_clock",
    "wake_clock",
    "deep_min",
    "light_min",
    "rem_min",
    "awake_min",
    "waso_min",
    "awakenings",
    "transitions_h",
    "deep_latency_min",
    "rem_latency_min",
    "hr_cov",
    "hr_mean",
    "hr_p10",
    "hr_cv",
    "hr_min30",
    "hr_nadir",
    "nadir_frac",
    "nadir_clock",
    "hr_onset30",
    "hr_decline",
    "hr_last2h_slope",
    "hr_early3h",
    "hr_late3h",
    "hr_early_late",
    "day_rest_hr",
    "dip_pct",
    "n5",
    "rmssd_mean",
    "ln_rmssd",
    "ln_mean",
    "rmssd_qmean",
    "rmssd_median",
    "rmssd_sd",
    "rmssd_peak",
    "rmssd_4h",
    "rmssd_last2h",
    "rmssd_norm",
    "rmssd_odd",
    "rmssd_even",
    "garmin_hrv",
    "sleep_stress",
    "rest_frac",
    "day_stress",
    "day_high_min",
    "bb_change",
    "resp_mean",
    "resp_sd",
    "resp_cv",
    "resp_min",
    "resp_max",
    "spo2_mean",
    "spo2_min",
    "t90_min",
    "odi3",
    "steps_in_sleep",
    "motion_mean",
    "move_mean",
    "skin_temp",
    "sleep_score",
    "tr_score",
    "bb_wake",
    "load_prev",
    "train_h_prev",
    "last_act_end_h",
    "tz_change",
    "altitude",
    "weekday",
    "free_day",
    "excluded",
];

/// The night table: see the module docs. Columns are f64 with NaN for missing; `load_class` is text.
pub fn night_features(data: &Data, labels: &[Label], settings: &Settings) -> Table {
    let sleeps = main_sleeps(data);
    let n = sleeps.len();
    let mut cols: BTreeMap<&str, Vec<f64>> = COLS.iter().map(|c| (*c, vec![f64::NAN; n])).collect();
    let excluded = excluded_days(labels);
    let (hr, stress, resp, spo2, steps, hrv5, bb, altitude, motion, smove) = (
        data.s("hr"),
        data.s("stress"),
        data.s("resp"),
        data.s("spo2"),
        data.s("steps"),
        data.s("hrv5"),
        data.s("body_battery"),
        data.s("altitude"),
        data.s("motion"),
        data.s("sleep_move"),
    );
    let (st_start, st_end, st_code) = (
        data.stages.col("start"),
        data.stages.col("end"),
        data.stages.col("stage"),
    );
    let (a_start, a_end, a_off, a_load) = (
        data.activities.col("start"),
        data.activities.col("end"),
        data.activities.col("utc_offset_min"),
        data.activities.col("load"),
    );
    let a_day: Vec<i32> = (0..data.activities.n)
        .map(|i| day_of_ms(a_start[i] as i64, a_off[i] as i32))
        .collect();
    let dcol = |c: &str| data.days.col(c);
    let (d_skin, d_score, d_tr, d_bbw, d_alt, d_hrv) = (
        dcol("skin_temp_dev"),
        dcol("sleep_score"),
        dcol("tr_score"),
        dcol("bb_wake"),
        dcol("altitude_avg"),
        dcol("hrv_night"),
    );
    let (s_deep, s_light, s_rem, s_awake, s_sleep) = (
        data.sleeps.col("deep_s"),
        data.sleeps.col("light_s"),
        data.sleeps.col("rem_s"),
        data.sleeps.col("awake_s"),
        data.sleeps.col("sleep_s"),
    );
    let (s_ra, s_rl, s_rh) = (
        data.sleeps.col("resp_avg"),
        data.sleeps.col("resp_low"),
        data.sleeps.col("resp_high"),
    );

    let mut set = |name: &str, i: usize, v: f64| {
        cols.get_mut(name).unwrap()[i] = v;
    };
    for (i, &(day, onset, wake, si)) in sleeps.iter().enumerate() {
        let off = data.offset_of_day(day);
        let tib_min = ((wake - onset) / MS_MIN) as usize;
        let tib_h = (wake - onset) as f64 / MS_HOUR as f64;
        set("day", i, day as f64);
        set("onset", i, onset as f64);
        set("wake", i, wake as f64);
        set("mid", i, (onset + wake) as f64 / 2.0);
        set("tib_h", i, tib_h);
        let tst_h = s_sleep[si] / 3600.0;
        set("tst_h", i, tst_h);
        set("eff", i, tst_h / tib_h);
        set("onset_clock", i, local_hour(onset, off));
        set("mid_clock", i, local_hour((onset + wake) / 2, off));
        set("wake_clock", i, local_hour(wake, off));
        set("deep_min", i, s_deep[si] / 60.0);
        set("light_min", i, s_light[si] / 60.0);
        set("rem_min", i, s_rem[si] / 60.0);
        set("awake_min", i, s_awake[si] / 60.0);

        // stages: segments overlapping the night
        let mut segs: Vec<(f64, f64, i32)> = (0..data.stages.n)
            .filter(|&k| st_end[k] > onset as f64 && st_start[k] < wake as f64)
            .map(|k| {
                (
                    st_start[k].max(onset as f64),
                    st_end[k].min(wake as f64),
                    st_code[k] as i32,
                )
            })
            .collect();
        segs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        if !segs.is_empty() {
            let first = segs.iter().position(|s| s.2 != 3 && s.2 != 4);
            let last = segs.iter().rposition(|s| s.2 != 3 && s.2 != 4);
            if let (Some(f), Some(l)) = (first, last) {
                let mid = &segs[f..=l];
                set(
                    "waso_min",
                    i,
                    mid.iter()
                        .filter(|s| s.2 == 3)
                        .map(|s| (s.1 - s.0) / 60_000.0)
                        .sum(),
                );
                set(
                    "awakenings",
                    i,
                    mid.iter()
                        .filter(|s| s.2 == 3 && s.1 - s.0 >= 60_000.0)
                        .count() as f64,
                );
            }
            let changes = segs.windows(2).filter(|w| w[0].2 != w[1].2).count();
            set("transitions_h", i, changes as f64 / tib_h);
            if let Some(s) = segs.iter().find(|s| s.2 == 0) {
                set("deep_latency_min", i, (s.0 - onset as f64) / 60_000.0);
            }
            if let Some(s) = segs.iter().find(|s| s.2 == 2) {
                set("rem_latency_min", i, (s.0 - onset as f64) / 60_000.0);
            }
        }

        // heart rate
        let mut hr_mean_v = f64::NAN;
        let mut hr_grid: Vec<f64> = vec![];
        if let Some(h) = hr {
            let (t, v, _) = window(h, onset, wake);
            hr_grid = minute_grid(t, v, onset, tib_min);
            // share of the expected samples at the watch's own spacing (overnight it often logs every 2 minutes)
            let step = spacing_min(t).min(5.0);
            let cov = (hr_grid.iter().filter(|x| x.is_finite()).count() as f64 * step
                / tib_min.max(1) as f64)
                .min(1.0);
            set("hr_cov", i, cov);
            if cov >= 0.5 {
                let m = mean(v);
                hr_mean_v = m;
                set("hr_mean", i, m);
                set("hr_p10", i, quantile_linear(v, 0.1));
                set("hr_cv", i, stats::std(v, 1) / m);
                let r30 = rolling(&hr_grid, 30, 20, true, mean);
                if let Some(k) = argmin(&r30) {
                    set("hr_min30", i, r30[k]);
                }
                let r15 = rolling(&hr_grid, 15, 8, true, median);
                if let Some(k) = argmin(&r15) {
                    set("hr_nadir", i, r15[k]);
                    set(
                        "nadir_frac",
                        i,
                        (k as f64 + 0.5) * MS_MIN as f64 / (wake - onset) as f64,
                    );
                    set("nadir_clock", i, local_hour(onset + k as i64 * MS_MIN, off));
                    let o30 = mean(&hr_grid[..30.min(hr_grid.len())]);
                    set("hr_onset30", i, o30);
                    set("hr_decline", i, o30 - r15[k]);
                }
                let tail = hr_grid.len().saturating_sub(120);
                set("hr_last2h_slope", i, slope_per_hour(&hr_grid[tail..]));
                let e3 = mean(&hr_grid[..180.min(hr_grid.len())]);
                let l3 = mean(&hr_grid[hr_grid.len().saturating_sub(180)..]);
                set("hr_early3h", i, e3);
                set("hr_late3h", i, l3);
                set("hr_early_late", i, e3 - l3);
                // daytime resting HR of the day before: quiet, unstressed minutes between 10:00 and 20:00
                if let Some(sx) = stress {
                    let prev_start = (day - 1) as i64 * MS_DAY - off as i64 * MS_MIN;
                    let (t, v, _) = window(h, prev_start + 10 * MS_HOUR, prev_start + 20 * MS_HOUR);
                    let mut vals = vec![];
                    for (k, &tk) in t.iter().enumerate() {
                        let sv = nearest_value(sx, tk, 90_000);
                        let q = sx.c("q").map(|q| {
                            let j = sx.t.partition_point(|&x| x < tk).min(sx.len() - 1);
                            q[j]
                        });
                        if sv.is_finite() && sv <= 25.0 && q.map(|q| q == 0.0).unwrap_or(true) {
                            let busy = (0..data.activities.n).any(|a| {
                                tk >= a_start[a] as i64 - 30 * MS_MIN
                                    && tk <= a_end[a] as i64 + 30 * MS_MIN
                            });
                            if !busy {
                                vals.push(v[k]);
                            }
                        }
                    }
                    if vals.len() >= 60 {
                        let d = mean(&vals);
                        set("day_rest_hr", i, d);
                        set("dip_pct", i, 100.0 * (d - m) / d);
                    }
                }
            }
        }

        // HRV
        if let Some(h) = hrv5 {
            let dc = h.c("day");
            let (a, b) = h.range(onset - MS_HOUR, wake + MS_HOUR);
            let mut pts: Vec<(i64, f64)> = vec![];
            for k in 0..h.len() {
                let take = match dc {
                    Some(dc) => dc[k] as i32 == day,
                    None => k >= a && k < b,
                };
                if take && h.v()[k] > 0.0 {
                    pts.push((h.t[k], h.v()[k]));
                }
            }
            if !pts.is_empty() {
                let v: Vec<f64> = pts.iter().map(|p| p.1).collect();
                let m = mean(&v);
                set("n5", i, v.len() as f64);
                set("rmssd_mean", i, m);
                set("ln_rmssd", i, m.det_ln());
                set(
                    "ln_mean",
                    i,
                    mean(&v.iter().map(|x| x.det_ln()).collect::<Vec<_>>()),
                );
                set(
                    "rmssd_qmean",
                    i,
                    (v.iter().map(|x| x * x).sum::<f64>() / v.len() as f64).sqrt(),
                );
                set("rmssd_median", i, median(&v));
                set("rmssd_sd", i, stats::std(&v, 1));
                set("rmssd_peak", i, v.iter().cloned().fold(f64::NAN, f64::max));
                let first4: Vec<f64> = pts
                    .iter()
                    .filter(|p| (30.0..=270.0).contains(&((p.0 - onset) as f64 / MS_MIN as f64)))
                    .map(|p| p.1)
                    .collect();
                set("rmssd_4h", i, mean(&first4));
                let last2: Vec<f64> = pts
                    .iter()
                    .filter(|p| p.0 >= wake - 2 * MS_HOUR)
                    .map(|p| p.1)
                    .collect();
                set("rmssd_last2h", i, mean(&last2));
                let hm = hr_mean_v;
                if hm.is_finite() {
                    set("rmssd_norm", i, m / (60000.0 / hm));
                }
                set(
                    "rmssd_odd",
                    i,
                    mean(&v.iter().step_by(2).copied().collect::<Vec<_>>()),
                );
                set(
                    "rmssd_even",
                    i,
                    mean(&v.iter().skip(1).step_by(2).copied().collect::<Vec<_>>()),
                );
            }
        }
        if let Some(k) = data.day_index(day) {
            set("garmin_hrv", i, d_hrv[k]);
            set("skin_temp", i, d_skin[k]);
            set("sleep_score", i, d_score[k]);
            set("tr_score", i, d_tr[k]);
            set("bb_wake", i, d_bbw[k]);
        }

        // stress and Body Battery
        if let Some(sx) = stress {
            let (_, v, a) = window(sx, onset, wake);
            let q = sx.c("q");
            let meas: Vec<f64> = v
                .iter()
                .enumerate()
                .filter(|(k, x)| x.is_finite() && q.map(|q| q[a + k] == 0.0).unwrap_or(true))
                .map(|(_, x)| *x)
                .collect();
            if !meas.is_empty() {
                set("sleep_stress", i, mean(&meas));
                set(
                    "rest_frac",
                    i,
                    meas.iter().filter(|&&x| x < 26.0).count() as f64 / meas.len() as f64,
                );
            }
            let prev_start = (day - 1) as i64 * MS_DAY - off as i64 * MS_MIN;
            let (t, v, a) = window(sx, prev_start + 8 * MS_HOUR, prev_start + 22 * MS_HOUR);
            let d: Vec<f64> = v
                .iter()
                .enumerate()
                .filter(|(k, x)| x.is_finite() && q.map(|q| q[a + k] == 0.0).unwrap_or(true))
                .map(|(_, x)| *x)
                .collect();
            if d.len() > 30 {
                set("day_stress", i, mean(&d));
                let (pt, pv, _) = window(sx, prev_start, prev_start + MS_DAY);
                set(
                    "day_high_min",
                    i,
                    pv.iter().filter(|&&x| x >= 76.0).count() as f64 * spacing_min(pt),
                );
                let _ = t;
            }
        }
        if let Some(b) = bb {
            let (a, w) = (
                nearest_value(b, onset, 15 * MS_MIN),
                nearest_value(b, wake, 15 * MS_MIN),
            );
            if a.is_finite() && w.is_finite() {
                set("bb_change", i, w - a);
            }
        }

        // breathing, SpO2, movement
        let mut resp_done = false;
        if let Some(r) = resp {
            let (_, v, _) = window(r, onset, wake);
            if v.len() >= 20 {
                resp_done = true;
                let m = mean(v);
                set("resp_mean", i, m);
                set("resp_sd", i, stats::std(v, 1));
                set("resp_cv", i, stats::std(v, 1) / m);
                set(
                    "resp_min",
                    i,
                    v.iter().cloned().fold(f64::INFINITY, f64::min),
                );
                set(
                    "resp_max",
                    i,
                    v.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
                );
            }
        }
        if !resp_done {
            set("resp_mean", i, s_ra[si]);
            set("resp_min", i, s_rl[si]);
            set("resp_max", i, s_rh[si]);
        }
        if let Some(sp) = spo2 {
            let (t, v, _) = window(sp, onset, wake);
            if v.len() >= 10 {
                set("spo2_mean", i, mean(v));
                set(
                    "spo2_min",
                    i,
                    v.iter().cloned().fold(f64::INFINITY, f64::min),
                );
                let dt = spacing_min(t);
                set(
                    "t90_min",
                    i,
                    v.iter().filter(|&&x| x < 90.0).count() as f64 * dt,
                );
                let mut events = 0;
                let mut last_ev = i64::MIN / 2;
                for k in 0..v.len() {
                    let prior: Vec<f64> = (0..k)
                        .rev()
                        .take_while(|&j| t[k] - t[j] <= 10 * MS_MIN)
                        .map(|j| v[j])
                        .collect();
                    if prior.len() >= 3
                        && v[k] <= median(&prior) - 3.0
                        && t[k] - last_ev >= 3 * MS_MIN
                    {
                        events += 1;
                        last_ev = t[k];
                    }
                }
                set("odi3", i, events as f64 / tib_h);
            }
        }
        if let Some(s) = steps {
            let (_, v, _) = window(s, onset, wake);
            set(
                "steps_in_sleep",
                i,
                v.iter().filter(|x| x.is_finite()).sum(),
            );
        }
        if let Some(m) = motion {
            set("motion_mean", i, mean(window(m, onset, wake).1));
        }
        if let Some(m) = smove {
            set("move_mean", i, mean(window(m, onset, wake).1));
        }

        // context
        let prev = day - 1;
        let mut load = 0.0;
        let mut hours = 0.0;
        for a in 0..data.activities.n {
            if a_day[a] == prev {
                if a_load[a].is_finite() {
                    load += a_load[a];
                }
                hours += (a_end[a] - a_start[a]) / 3.6e6;
            }
        }
        set("load_prev", i, load);
        set("train_h_prev", i, hours);
        let ends: Vec<f64> = (0..data.activities.n)
            .map(|a| a_end[a])
            .filter(|&e| e <= onset as f64 && onset as f64 - e <= 12.0 * 3.6e6)
            .collect();
        if let Some(e) = ends
            .iter()
            .cloned()
            .fold(None, |m: Option<f64>, x| Some(m.map_or(x, |m| m.max(x))))
        {
            set("last_act_end_h", i, (onset as f64 - e) / 3.6e6);
        }
        set("tz_change", i, (off - data.offset_of_day(day - 1)) as f64);
        let alt = altitude
            .map(|a| median(window(a, onset, wake).1))
            .unwrap_or(f64::NAN);
        set(
            "altitude",
            i,
            if alt.is_finite() {
                alt
            } else {
                data.day_index(day).map(|k| d_alt[k]).unwrap_or(f64::NAN)
            },
        );
        let wd = weekday(day);
        set("weekday", i, wd as f64);
        set("free_day", i, settings.free_days.contains(&wd) as u8 as f64);
        set("excluded", i, excluded.contains(&day) as u8 as f64);
    }

    // load classes: terciles of the days with training
    let lp = cols["load_prev"].clone();
    let pos: Vec<f64> = lp.iter().copied().filter(|&l| l > 5.0).collect();
    let cuts = (
        quantile_nearest(&pos, 1.0 / 3.0),
        quantile_nearest(&pos, 2.0 / 3.0),
    );
    let class: Vec<Option<String>> = lp
        .iter()
        .map(|&l| {
            Some(
                if l <= 5.0 {
                    "rest"
                } else if l <= cuts.0 {
                    "easy"
                } else if l <= cuts.1 {
                    "moderate"
                } else {
                    "hard"
                }
                .to_string(),
            )
        })
        .collect();
    let mut t = Table::new(n);
    for (k, v) in cols {
        t.set(k, v);
    }
    t.txt.insert("load_class".into(), class);
    t
}

#[derive(Serialize, Default)]
pub struct Chan {
    pub t: Vec<f64>,
    pub v: Vec<f64>,
    pub q: Vec<f64>,
}

#[derive(Serialize, Default)]
pub struct Band {
    pub minute: Vec<f64>,
    pub p10: Vec<f64>,
    pub p50: Vec<f64>,
    pub p90: Vec<f64>,
    pub n: Vec<f64>,
}

#[derive(Serialize, Default)]
pub struct NightChannels {
    pub day: i32,
    pub onset: f64,
    pub wake: f64,
    pub utc_offset_min: i32,
    pub series: BTreeMap<String, Chan>,
    pub stages: Table,
    pub activities: Table,
    pub row: BTreeMap<String, f64>,
    pub bands: BTreeMap<String, Band>,
}

/// The mean of a series in each `bin_min` minutes since `onset` (NaN where empty).
pub fn night_bins(s: &Series, onset: i64, wake: i64, bin_min: i64) -> Vec<f64> {
    let (t, v, _) = window(s, onset, wake);
    let nb = ((wake - onset) / (bin_min * MS_MIN) + 1) as usize;
    let mut sum = vec![0.0; nb];
    let mut cnt = vec![0u32; nb];
    for (k, &tk) in t.iter().enumerate() {
        if v[k].is_finite() {
            let b = ((tk - onset) / (bin_min * MS_MIN)) as usize;
            if b < nb {
                sum[b] += v[k];
                cnt[b] += 1;
            }
        }
    }
    (0..nb)
        .map(|b| {
            if cnt[b] > 0 {
                sum[b] / cnt[b] as f64
            } else {
                f64::NAN
            }
        })
        .collect()
}

/// Everything the night viewer draws for one night, with the typical-night bands of the 30 previous non-excluded nights.
pub fn channels(
    data: &Data,
    labels: &[Label],
    settings: &Settings,
    day: i32,
    pad_min: i64,
) -> Option<NightChannels> {
    let nt = night_features(data, labels, settings);
    channels_with(data, &nt, day, pad_min)
}

/// As `channels`, with the night table already computed.
pub fn channels_with(data: &Data, nt: &Table, day: i32, pad_min: i64) -> Option<NightChannels> {
    let sleeps = main_sleeps(data);
    let &(_, onset, wake, _) = sleeps.iter().find(|s| s.0 == day)?;
    let (t0, t1) = (onset - pad_min * MS_MIN, wake + pad_min * MS_MIN);
    let mut out = NightChannels {
        day,
        onset: onset as f64,
        wake: wake as f64,
        utc_offset_min: data.offset_of_day(day),
        ..Default::default()
    };
    for (name, s) in &data.series {
        if s.is_empty() || name == "motion" && false {
            continue;
        }
        let (a, b) = s.range(t0, t1);
        if b > a {
            let q = s.c("q").map(|q| q[a..b].to_vec()).unwrap_or_default();
            out.series.insert(
                name.clone(),
                Chan {
                    t: s.t[a..b].iter().map(|&t| t as f64).collect(),
                    v: s.v()[a..b].to_vec(),
                    q,
                },
            );
        }
    }
    let sel = |tab: &Table, s: &str, e: &str| -> Table {
        let (ts, te) = (tab.col(s), tab.col(e));
        let idx: Vec<usize> = (0..tab.n)
            .filter(|&k| te[k] > t0 as f64 && ts[k] < t1 as f64)
            .collect();
        let mut o = Table::new(idx.len());
        for (name, col) in &tab.num {
            o.set(name, idx.iter().map(|&k| col[k]).collect());
        }
        for (name, col) in &tab.txt {
            o.txt
                .insert(name.clone(), idx.iter().map(|&k| col[k].clone()).collect());
        }
        o.n = idx.len();
        o
    };
    out.stages = sel(&data.stages, "start", "end");
    out.activities = sel(&data.activities, "start", "end");
    let days = nt.col("day");
    if let Some(r) = (0..nt.n).find(|&k| days[k] as i32 == day) {
        for (k, v) in &nt.num {
            out.row.insert(k.clone(), v[r]);
        }
    }
    let excl = nt.col("excluded");
    let prev: Vec<usize> = (0..nt.n)
        .filter(|&k| (days[k] as i32) < day && excl[k] == 0.0)
        .rev()
        .take(30)
        .collect();
    for (name, key) in [
        ("hr", "hr"),
        ("hrv5", "hrv5"),
        ("stress", "stress"),
        ("resp", "resp"),
    ] {
        let Some(s) = data.s(key) else { continue };
        let mut per_bin: Vec<Vec<f64>> = vec![];
        for &k in &prev {
            let (po, pw) = (nt.col("onset")[k] as i64, nt.col("wake")[k] as i64);
            for (b, v) in night_bins(s, po, pw, 10).into_iter().enumerate() {
                if per_bin.len() <= b {
                    per_bin.resize(b + 1, vec![]);
                }
                if v.is_finite() {
                    per_bin[b].push(v);
                }
            }
        }
        let nb = ((wake - onset) / (10 * MS_MIN) + 1) as usize;
        let mut band = Band::default();
        for b in 0..nb.min(per_bin.len()) {
            if per_bin[b].len() >= 5 {
                band.minute.push(b as f64 * 10.0);
                band.p10.push(quantile_linear(&per_bin[b], 0.1));
                band.p50.push(quantile_linear(&per_bin[b], 0.5));
                band.p90.push(quantile_linear(&per_bin[b], 0.9));
                band.n.push(per_bin[b].len() as f64);
            }
        }
        out.bands.insert(name.to_string(), band);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::synth;

    #[test]
    fn night_table_from_synthetic_data() {
        let (d, _) = synth(7, 120, 19_723);
        let t = night_features(&d, &[], &Settings::default());
        assert!(t.n >= 100);
        let hr_min30 = t.col("hr_min30");
        let ok = hr_min30.iter().filter(|x| x.is_finite()).count();
        assert!(ok as f64 > 0.9 * t.n as f64);
        let m = stats::mean(&hr_min30);
        assert!((40.0..60.0).contains(&m), "{m}");
        let r = stats::mean(&t.col("ln_rmssd"));
        assert!((3.8..4.4).contains(&r), "{r}");
        // timing: sleep starts around 23:00 and ends around 07:00
        let oc = crate::time::circ_mean_hours(&t.col("onset_clock"));
        assert!((22.0..24.0).contains(&oc), "{oc}");
        let tib = stats::mean(&t.col("tib_h"));
        assert!((6.5..9.5).contains(&tib), "{tib}");
        assert!(t.col("weekday").iter().all(|w| (0.0..7.0).contains(w)));
    }

    #[test]
    fn channels_and_bands() {
        let (d, _) = synth(7, 120, 19_723);
        let nt = night_features(&d, &[], &Settings::default());
        let last = nt.col("day")[nt.n - 1] as i32;
        let c = channels(&d, &[], &Settings::default(), last, 60).unwrap();
        assert!(c.series["hr"].t.len() > 300);
        assert!(!c.bands["hr"].minute.is_empty());
        assert!(c.row["hr_min30"].is_finite());
    }
}
