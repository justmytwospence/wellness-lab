//! The HRV deep-dive notebook, ported: nightly values, the baseline and normal range, trend patterns, the response to
//! training, the inside of the night, and the state-space model of nightly HRV given training. Every formula follows the
//! notebook cell it comes from (`tools/goldens/reference_hrv.py` holds the transcription the goldens are made with).

use crate::baseline::{baseline, notebook_spec};
use crate::data::{Data, Table};
use crate::math::Det;
use crate::optim::{nelder_mead, NmOptions};
use crate::stats::{self, mean, median, quantile_nearest, rolling_mean, rolling_std, shift, std};
use crate::time::{day_of_ms, iso, MS_DAY, MS_MIN};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

pub const CLASSES: [&str; 4] = ["rest", "easy", "moderate", "hard"];
pub const STAGE_NAMES: [&str; 5] = ["deep", "light", "rem", "awake", "unmeasurable"];

#[derive(Clone, Debug, Default)]
pub struct Nightly {
    pub t: Table,
    pub load_cuts: (f64, f64),
}

fn txt(v: Vec<Option<&str>>) -> Vec<Option<String>> {
    v.into_iter().map(|s| s.map(String::from)).collect()
}

/// `nightly_load`: one row per calendar day from the first to the last day with an HRV status.
pub fn nightly(data: &Data) -> Nightly {
    let days = data.days.col("day");
    let hrv_night = data.days.col("hrv_night");
    let status = data.days.text("hrv_status");
    let mut with_row: Vec<i32> = Vec::new();
    for i in 0..data.days.n {
        if hrv_night[i].is_finite() || status[i].is_some() {
            with_row.push(days[i] as i32);
        }
    }
    let (Some(&d0), Some(&d1)) = (with_row.first(), with_row.last()) else {
        return Nightly::default();
    };
    let n = (d1 - d0 + 1) as usize;
    let idx = |day: i32| -> Option<usize> { data.day_index(day) };

    // main sleeps by day
    let sk = data.sleeps.col("kind");
    let sday = data.sleeps.col("day");
    let sstart = data.sleeps.col("start");
    let mut sleep_of: BTreeMap<i32, usize> = BTreeMap::new();
    for i in 0..data.sleeps.n {
        if sk[i] == 0.0 {
            sleep_of.entry(sday[i] as i32).or_insert(i);
        }
    }
    let (s_hr, s_rhr, s_sec, s_resp, s_stress) = (
        data.sleeps.col("avg_hr"),
        data.sleeps.col("rhr"),
        data.sleeps.col("sleep_s"),
        data.sleeps.col("resp_avg"),
        data.sleeps.col("avg_stress"),
    );

    // the five-minute values by night
    let mut sum = vec![0.0; n];
    let mut cnt = vec![0usize; n];
    let mut sum4 = vec![0.0; n];
    let mut cnt4 = vec![0usize; n];
    if let Some(h) = data.s("hrv5") {
        let v = h.v();
        let dcol = h.c("day");
        for i in 0..h.len() {
            if !(v[i] > 0.0) {
                continue;
            }
            let day = dcol
                .map(|d| d[i] as i32)
                .unwrap_or_else(|| day_of_ms(h.t[i], data.offset_at(h.t[i])));
            if day < d0 || day > d1 {
                continue;
            }
            let k = (day - d0) as usize;
            sum[k] += v[i];
            cnt[k] += 1;
            if let Some(&si) = sleep_of.get(&day) {
                let min_in = (h.t[i] as f64 - sstart[si]) / MS_MIN as f64;
                if (30.0..=270.0).contains(&min_in) {
                    sum4[k] += v[i];
                    cnt4[k] += 1;
                }
            }
        }
    }

    // training load by the day after the activity (the night follows the day's training)
    let mut load = vec![0.0; n];
    let mut train_h = vec![0.0; n];
    let (a_start, a_end, a_off, a_load) = (
        data.activities.col("start"),
        data.activities.col("end"),
        data.activities.col("utc_offset_min"),
        data.activities.col("load"),
    );
    for i in 0..data.activities.n {
        let day = day_of_ms(a_start[i] as i64, a_off[i] as i32) + 1;
        if day >= d0 && day <= d1 {
            let k = (day - d0) as usize;
            if a_load[i].is_finite() {
                load[k] += a_load[i];
            }
            train_h[k] += (a_end[i] - a_start[i]) / 3.6e6;
        }
    }
    let mut pos: Vec<f64> = load.iter().copied().filter(|&l| l > 5.0).collect();
    pos.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let cuts = (
        quantile_nearest(&pos, 1.0 / 3.0),
        quantile_nearest(&pos, 2.0 / 3.0),
    );

    let nan = || vec![f64::NAN; n];
    let (
        mut garmin,
        mut garmin_week,
        mut g_lo,
        mut g_hi,
        mut sleep_hr,
        mut rhr,
        mut sleep_score,
        mut sleep_h,
        mut resp,
        mut sleep_stress,
        mut onset,
    ) = (
        nan(),
        nan(),
        nan(),
        nan(),
        nan(),
        nan(),
        nan(),
        nan(),
        nan(),
        nan(),
        nan(),
    );
    let mut g_status: Vec<Option<String>> = vec![None; n];
    let (hw, hl, hh, ss, dr) = (
        data.days.col("hrv_week"),
        data.days.col("hrv_bal_lo"),
        data.days.col("hrv_bal_hi"),
        data.days.col("sleep_score"),
        data.days.col("rhr"),
    );
    let dstatus = data.days.text("hrv_status");
    for k in 0..n {
        let day = d0 + k as i32;
        if let Some(i) = idx(day) {
            garmin[k] = hrv_night[i];
            garmin_week[k] = hw[i];
            g_lo[k] = hl[i];
            g_hi[k] = hh[i];
            g_status[k] = dstatus[i].clone();
            sleep_score[k] = ss[i];
            rhr[k] = dr[i];
        }
        if let Some(&si) = sleep_of.get(&day) {
            sleep_hr[k] = s_hr[si];
            if s_rhr[si].is_finite() {
                rhr[k] = s_rhr[si];
            }
            sleep_h[k] = s_sec[si] / 3600.0;
            resp[k] = s_resp[si];
            sleep_stress[k] = s_stress[si];
            onset[k] = sstart[si];
        }
    }
    let rmssd5: Vec<f64> = (0..n)
        .map(|k| {
            if cnt[k] > 0 {
                sum[k] / cnt[k] as f64
            } else {
                f64::NAN
            }
        })
        .collect();
    let rmssd_4h: Vec<f64> = (0..n)
        .map(|k| {
            if cnt4[k] > 0 {
                sum4[k] / cnt4[k] as f64
            } else {
                f64::NAN
            }
        })
        .collect();
    let rmssd: Vec<f64> = (0..n)
        .map(|k| {
            if rmssd5[k].is_finite() {
                rmssd5[k]
            } else {
                garmin[k]
            }
        })
        .collect();
    let src: Vec<Option<&str>> = (0..n)
        .map(|k| {
            if rmssd5[k].is_finite() {
                Some("5-min values")
            } else if garmin[k].is_finite() {
                Some("Garmin")
            } else {
                None
            }
        })
        .collect();
    let lnr: Vec<f64> = rmssd.iter().map(|v| v.det_ln()).collect();
    let class: Vec<Option<&str>> = load
        .iter()
        .map(|&l| {
            Some(if l <= 5.0 {
                "rest"
            } else if l <= cuts.0 {
                "easy"
            } else if l <= cuts.1 {
                "moderate"
            } else {
                "hard"
            })
        })
        .collect();

    let mut t = Table::new(n);
    t.set("day", (0..n).map(|k| (d0 + k as i32) as f64).collect());
    for (name, col) in [
        ("rmssd5", rmssd5),
        (
            "n5",
            cnt.iter()
                .map(|&c| if c > 0 { c as f64 } else { f64::NAN })
                .collect(),
        ),
        ("rmssd_4h", rmssd_4h),
        ("garmin", garmin),
        ("garmin_week", garmin_week),
        ("g_lo", g_lo),
        ("g_hi", g_hi),
        ("sleep_hr", sleep_hr),
        ("rhr", rhr),
        ("sleep_score", sleep_score),
        ("sleep_h", sleep_h),
        ("resp", resp),
        ("sleep_stress", sleep_stress),
        ("onset", onset),
        ("load", load),
        ("train_h", train_h),
        ("rmssd", rmssd),
        ("lnr", lnr),
    ] {
        t.set(name, col);
    }
    t.txt.insert("g_status".into(), g_status);
    t.txt.insert("rmssd_src".into(), txt(src));
    t.txt.insert("load_class".into(), txt(class));
    Nightly { t, load_cuts: cuts }
}

/// `scored_base` and `scored`: the 7-night baseline, the normal range from the nights before, and the advice.
pub fn scored(n: &Nightly, range_days: usize, k_daily: f64, k_swc: f64) -> Table {
    let mut t = n.t.clone();
    let spec = notebook_spec(range_days, k_daily, k_swc);
    let days: Vec<i32> = t.col("day").iter().map(|&d| d as i32).collect();
    let b = baseline(&t.col("rmssd"), &days, &BTreeSet::new(), &spec);
    let lnr = &b.x;
    let nn = lnr.len();
    let mut advice: Vec<Option<String>> = vec![None; nn];
    for i in 0..nn {
        if lnr[i].is_nan() || b.center[i].is_nan() {
            continue;
        }
        let low = lnr[i] < b.lo[i];
        let high = lnr[i] > b.hi[i];
        let base_low = b.short[i] < b.slo[i];
        advice[i] = Some(
            if base_low && low {
                "back off"
            } else if base_low {
                "keep it easy"
            } else if low {
                "watch"
            } else if high {
                "high"
            } else {
                "normal"
            }
            .to_string(),
        );
    }
    t.set("base7", b.short.clone());
    t.set("mu", b.center.clone());
    t.set("sd", b.scale.clone());
    t.set("lo", b.lo.clone());
    t.set("hi", b.hi.clone());
    t.set("swc_lo", b.slo.clone());
    t.set("swc_hi", b.shi.clone());
    t.set("score", lnr.iter().map(|v| 20.0 * v).collect());
    t.set("base_score", b.short.iter().map(|v| 20.0 * v).collect());
    t.txt.insert("advice".into(), advice);
    t
}

fn slopes(x: &[f64], win: usize) -> Vec<f64> {
    let tt: Vec<f64> = (0..win).map(|i| i as f64).collect();
    let mut out = vec![f64::NAN; x.len()];
    for i in win.saturating_sub(1)..x.len() {
        if i + 1 < win {
            continue;
        }
        let w = &x[i + 1 - win..=i];
        let ok: Vec<usize> = (0..win).filter(|&k| !w[k].is_nan()).collect();
        if ok.len() as f64 >= win as f64 * 0.6 {
            let tm = ok.iter().map(|&k| tt[k]).sum::<f64>() / ok.len() as f64;
            let wm = ok.iter().map(|&k| w[k]).sum::<f64>() / ok.len() as f64;
            let num: f64 = ok.iter().map(|&k| (tt[k] - tm) * (w[k] - wm)).sum();
            let den: f64 = ok.iter().map(|&k| (tt[k] - tm).powi(2)).sum();
            out[i] = num / den;
        }
    }
    out
}

/// z of each value against all earlier ones (population SD), after a warm-up of `warmup` values.
fn zs(s: &[f64], warmup: usize) -> Vec<f64> {
    let mut z = vec![f64::NAN; s.len()];
    let (mut sum, mut sum2, mut cnt) = (0.0, 0.0, 0usize);
    for i in 0..s.len() {
        if cnt >= warmup && !s[i].is_nan() {
            let m = sum / cnt as f64;
            let var = (sum2 / cnt as f64 - m * m).max(0.0);
            z[i] = (s[i] - m) / var.sqrt();
        }
        if !s[i].is_nan() {
            sum += s[i];
            sum2 += s[i] * s[i];
            cnt += 1;
        }
    }
    z
}

/// `trend_scored`: CV, normalised HRV and the slope z-scores that name the training patterns.
pub fn trends(scored: &Table) -> Table {
    let mut t = scored.clone();
    let lnr = t.col("lnr");
    let rmssd = t.col("rmssd");
    let sleep_hr = t.col("sleep_hr");
    let cv7: Vec<f64> = rolling_std(&lnr, 7, 4)
        .iter()
        .zip(rolling_mean(&lnr, 7, 4))
        .map(|(s, m)| s / m * 100.0)
        .collect();
    let norm: Vec<f64> = rmssd
        .iter()
        .zip(&sleep_hr)
        .map(|(r, h)| r / (60000.0 / h))
        .collect();
    let norm7 = rolling_mean(&norm, 7, 4);
    let hr7 = rolling_mean(&sleep_hr, 7, 4);
    let cv7s = rolling_mean(&cv7, 7, 4);
    let base7 = t.col("base7");
    let z_of = |x: &[f64]| zs(&slopes(x, 14), 60);
    let (zh, zr, zc, zn) = (z_of(&base7), z_of(&hr7), z_of(&cv7s), z_of(&norm7));
    let nn = lnr.len();
    let mut trend: Vec<Option<String>> = vec![None; nn];
    for i in 0..nn {
        let up = |z: f64| z > 1.0;
        let dn = |z: f64| z < -1.0;
        trend[i] = Some(
            if zh[i].is_nan() {
                "warming up"
            } else if dn(zh[i]) && up(zr[i]) {
                "accumulated fatigue"
            } else if dn(zh[i]) && dn(zr[i]) {
                "saturation"
            } else if up(zr[i]) && up(zc[i]) {
                "maladaptation"
            } else if !dn(zh[i]) && !up(zr[i]) && dn(zc[i]) {
                "coping well"
            } else {
                "stable"
            }
            .to_string(),
        );
    }
    t.set("cv7", cv7);
    t.set("norm", norm);
    t.set("norm7", norm7);
    t.set("hr7", hr7);
    t.set("cv7s", cv7s);
    t.set("z_hrv", zh);
    t.set("z_hr", zr);
    t.set("z_cv", zc);
    t.set("z_norm", zn);
    t.txt.insert("trend".into(), trend);
    t
}

/// `acute_calc`: how far each night lands from the baseline that ended the night before, by the class of the day before.
pub fn acute(trend: &Table) -> (Table, Table) {
    let lnr = trend.col("lnr");
    let base_prev = shift(&trend.col("base7"), 1);
    let class = trend.text("load_class");
    let load = trend.col("load");
    let day = trend.col("day");
    let dev: Vec<f64> = lnr.iter().zip(&base_prev).map(|(a, b)| a - b).collect();
    let mut pts = Table::new(dev.len());
    pts.set("day", day);
    pts.set("dev", dev);
    pts.set("load", load);
    pts.txt.insert("load_class".into(), class.clone());

    let mut rows: Vec<(String, f64, f64, f64, f64, f64, f64)> = Vec::new();
    for k in 1..=3usize {
        let dk: Vec<f64> = shift(&lnr, -(k as isize - 1))
            .iter()
            .zip(&base_prev)
            .map(|(a, b)| a - b)
            .collect();
        for c in CLASSES {
            let vals: Vec<f64> = (0..dk.len())
                .filter(|&i| class[i].as_deref() == Some(c))
                .map(|i| dk[i])
                .collect();
            if vals.is_empty() {
                continue;
            }
            let cnt = stats::nan_len(&vals) as f64;
            let m = mean(&vals);
            let se = std(&vals, 1) / cnt.sqrt();
            rows.push((
                c.to_string(),
                k as f64,
                m,
                se,
                cnt,
                m - 1.96 * se,
                m + 1.96 * se,
            ));
        }
    }
    let mut lag = Table::new(rows.len());
    lag.txt.insert(
        "klass".into(),
        rows.iter().map(|r| Some(r.0.clone())).collect(),
    );
    lag.set("night", rows.iter().map(|r| r.1).collect());
    lag.set("mean", rows.iter().map(|r| r.2).collect());
    lag.set("se", rows.iter().map(|r| r.3).collect());
    lag.set("n", rows.iter().map(|r| r.4).collect());
    lag.set("lo", rows.iter().map(|r| r.5).collect());
    lag.set("hi", rows.iter().map(|r| r.6).collect());
    (lag, pts)
}

/// `night_calc`: the barcode of five-minute values since sleep onset, HRV by sleep stage, and which number sees training.
pub fn inside(data: &Data, nightly: &Table, trend: &Table) -> (Table, Table, Table) {
    let d0 = nightly.col("day").first().copied().unwrap_or(0.0) as i32;
    let nrm = nightly.col("rmssd");
    let n = nrm.len();
    let onset = nightly.col("onset");
    // stage starts for the as-of join (backward, within three hours)
    let (st_start, st_code) = (data.stages.col("start"), data.stages.col("stage"));
    let mut order: Vec<usize> = (0..data.stages.n).collect();
    order.sort_by(|&a, &b| st_start[a].partial_cmp(&st_start[b]).unwrap());
    let starts: Vec<f64> = order.iter().map(|&i| st_start[i]).collect();
    let codes: Vec<f64> = order.iter().map(|&i| st_code[i]).collect();
    let mut cells: BTreeMap<(i32, i64), (f64, usize)> = BTreeMap::new();
    let mut rel: BTreeMap<usize, Vec<f64>> = BTreeMap::new();
    if let Some(h) = data.s("hrv5") {
        let v = h.v();
        let dcol = h.c("day");
        for i in 0..h.len() {
            if !(v[i] > 0.0) {
                continue;
            }
            let day = dcol
                .map(|d| d[i] as i32)
                .unwrap_or_else(|| day_of_ms(h.t[i], data.offset_at(h.t[i])));
            let k = day - d0;
            if k < 0 || k as usize >= n || onset[k as usize].is_nan() {
                continue;
            }
            let k = k as usize;
            let min_in = (h.t[i] as f64 - onset[k]) / MS_MIN as f64;
            if (0.0..=600.0).contains(&min_in) {
                let hh = (((min_in / 10.0).floor() * 10.0 / 60.0) * 1000.0).round() as i64; // thousandths of an hour
                let e = cells.entry((day, hh)).or_insert((0.0, 0));
                e.0 += v[i];
                e.1 += 1;
            }
            // stage as-of
            let tt = h.t[i] as f64;
            let p = starts.partition_point(|&s| s <= tt);
            if p > 0 && tt - starts[p - 1] <= 3.0 * 3.6e6 && nrm[k].is_finite() {
                let code = codes[p - 1] as usize;
                if code < 4 {
                    rel.entry(code)
                        .or_default()
                        .push((v[i] / nrm[k]).det_ln() * 100.0);
                }
            }
        }
    }
    let mut barcode = Table::new(cells.len());
    barcode.set("day", cells.keys().map(|k| k.0 as f64).collect());
    barcode.set("h", cells.keys().map(|k| k.1 as f64 / 1000.0).collect());
    barcode.set(
        "rmssd",
        cells.values().map(|(s, c)| s / *c as f64).collect(),
    );

    let mut stage_hrv = Table::new(0);
    let mut names = vec![];
    let (mut m, mut q1, mut q3, mut cnt) = (vec![], vec![], vec![], vec![]);
    for name_idx in [0usize, 1, 2, 3] {
        if let Some(v) = rel.get(&name_idx) {
            names.push(Some(STAGE_NAMES[name_idx].to_string()));
            m.push(median(v));
            q1.push(quantile_nearest(v, 0.25));
            q3.push(quantile_nearest(v, 0.75));
            cnt.push(v.len() as f64);
        }
    }
    stage_hrv.n = names.len();
    stage_hrv.txt.insert("stage".into(), names);
    stage_hrv.set("rel", m);
    stage_hrv.set("q1", q1);
    stage_hrv.set("q3", q3);
    stage_hrv.set("n", cnt);

    let class = trend.text("load_class");
    let r4: Vec<f64> = trend.col("rmssd_4h").iter().map(|v| v.det_ln()).collect();
    let lg: Vec<f64> = trend.col("garmin").iter().map(|v| v.det_ln()).collect();
    let mut sens_rows = vec![];
    for (measure, col) in [
        ("whole night", trend.col("lnr")),
        ("first 4 h", r4),
        ("Garmin", lg),
    ] {
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
        sens_rows.push((measure, (pick("rest") - pick("hard")) / sd, sd));
    }
    let mut sens = Table::new(3);
    sens.txt.insert(
        "measure".into(),
        sens_rows.iter().map(|r| Some(r.0.to_string())).collect(),
    );
    sens.set("effect", sens_rows.iter().map(|r| r.1).collect());
    sens.set("sd", sens_rows.iter().map(|r| r.2).collect());
    (barcode, stage_hrv, sens)
}

/// The notebook's state-space model: `y = level + beta * g + noise`, a random-walk level and a training response `g` that decays.
fn kalman_ir(y: &[f64], load: &[f64], th: &[f64; 4]) -> (f64, Vec<[f64; 6]>) {
    let (se, sn, beta, tau) = (th[0].det_exp(), th[1].det_exp(), th[2], th[3].det_exp());
    let decay = (-1.0 / tau).det_exp();
    let head: Vec<f64> = y.iter().take(14).copied().collect();
    let (mut mu, mut p, mut g, mut ll) = (mean(&head), 1.0, 0.0, 0.0);
    let mut out = vec![[f64::NAN; 6]; y.len()];
    for t in 0..y.len() {
        g = decay * g + load[t] / 100.0;
        let pp = p + sn * sn;
        let pred = mu + beta * g;
        let f = pp + se * se;
        let z;
        if y[t].is_finite() {
            let v = y[t] - pred;
            mu += pp / f * v;
            p = pp * se * se / f;
            ll += -0.5 * ((2.0 * std::f64::consts::PI * f).det_ln() + v * v / f);
            z = v / f.sqrt();
        } else {
            p = pp;
            z = f64::NAN;
        }
        out[t] = [pred, f, mu, p, g, z];
    }
    (ll, out)
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Model {
    pub beta: f64,
    pub tau: f64,
    pub sigma_eps: f64,
    pub sigma_eta: f64,
    pub level: f64,
    pub level_var: f64,
    pub g: f64,
    pub pct_per_100: f64,
    pub half_life_days: f64,
    pub lr: f64,
    pub nights: usize,
    pub last_date: String,
}

/// `model_fit`: maximum likelihood by Nelder-Mead from the notebook's starting point.
pub fn model(nightly: &Table) -> (Model, Table) {
    let y = nightly.col("lnr");
    let load = nightly.col("load");
    let x0 = [0.12f64.det_ln(), 0.02f64.det_ln(), -0.03, 2.0f64.det_ln()];
    let opts = NmOptions {
        maxiter: 4000,
        maxfev: 4000,
        xatol: 1e-5,
        fatol: 1e-6,
    };
    let r = nelder_mead(
        |th| {
            if th[3] > -3.0 && th[3] < 3.5 {
                -kalman_ir(&y, &load, &[th[0], th[1], th[2], th[3]]).0
            } else {
                1e9
            }
        },
        &x0,
        &opts,
    );
    let zero = vec![0.0; load.len()];
    let r0 = nelder_mead(
        |th| -kalman_ir(&y, &zero, &[th[0], th[1], 0.0, 0.0]).0,
        &x0[..2],
        &NmOptions::defaults(2),
    );
    let th = [r.x[0], r.x[1], r.x[2], r.x[3]];
    let (ll, out) = kalman_ir(&y, &load, &th);
    let (se, sn, beta, tau) = (th[0].det_exp(), th[1].det_exp(), th[2], th[3].det_exp());
    let last = out.last().copied().unwrap_or([f64::NAN; 6]);
    let day = nightly.col("day");
    let m = Model {
        beta,
        tau,
        sigma_eps: se,
        sigma_eta: sn,
        level: last[2],
        level_var: last[3],
        g: last[4],
        pct_per_100: (beta.det_exp() - 1.0) * 100.0,
        half_life_days: tau * std::f64::consts::LN_2,
        lr: 2.0 * (ll + r0.fun),
        nights: stats::nan_len(&y),
        last_date: day.last().map(|&d| iso(d as i32)).unwrap_or_default(),
    };
    let mut v = Table::new(y.len());
    v.set("day", day);
    v.set("lnr", y.clone());
    v.set("rmssd", nightly.col("rmssd"));
    v.set("load", load);
    v.txt
        .insert("load_class".into(), nightly.text("load_class"));
    v.set("pred", out.iter().map(|o| o[0]).collect());
    v.set("pred_sd", out.iter().map(|o| o[1].sqrt()).collect());
    v.set("level", out.iter().map(|o| o[2]).collect());
    v.set("z", out.iter().map(|o| o[5]).collect());
    (m, v)
}

pub const ADVICE_TEXT: [(&str, &str); 5] = [
    ("back off", "Baseline below its band and last night low too: rest or train easy until it comes back."),
    ("keep it easy", "Baseline below its band: keep training easy until it returns to the band."),
    ("watch", "Last night was low, the baseline is fine: one night means little, see what tonight does."),
    ("high", "Last night above your range: often good, but a run of highs in a hard block can mean saturation."),
    ("normal", "Within your normal range: train as planned."),
];

#[derive(Serialize, Clone, Debug, Default)]
pub struct Hrv4tResult {
    pub nightly: Table,
    pub scored: Table,
    pub acute_lag: Table,
    pub acute_pts: Table,
    pub barcode: Table,
    pub stage_hrv: Table,
    pub night_sens: Table,
    pub model_view: Table,
    pub model: Model,
    pub load_cuts: (f64, f64),
    pub headline: serde_json::Value,
    pub spark: Table,
}

/// Everything the HRV page shows, for one setting of the controls.
pub fn run(data: &Data, range_days: usize, k_daily: f64, k_swc: f64) -> Hrv4tResult {
    let n = nightly(data);
    if n.t.n == 0 {
        return Hrv4tResult::default();
    }
    let sc = scored(&n, range_days, k_daily, k_swc);
    let tr = trends(&sc);
    let (lag, pts) = acute(&tr);
    let (barcode, stage_hrv, sens) = inside(data, &n.t, &tr);
    let (m, mv) = model(&n.t);
    let headline = headline(&tr, &mv, &m, &n, range_days, k_daily, k_swc);
    let spark = spark(&tr);
    Hrv4tResult {
        nightly: n.t.clone(),
        scored: tr,
        acute_lag: lag,
        acute_pts: pts,
        barcode,
        stage_hrv,
        night_sens: sens,
        model_view: mv,
        model: m,
        load_cuts: n.load_cuts,
        headline,
        spark,
    }
}

fn last_valid(t: &Table, col: &str) -> Option<usize> {
    let v = t.col(col);
    (0..v.len()).rev().find(|&i| v[i].is_finite())
}

/// `headline`: the numbers for the last night with an HRV value.
pub fn headline(
    tr: &Table,
    mv: &Table,
    m: &Model,
    n: &Nightly,
    range_days: usize,
    k_daily: f64,
    k_swc: f64,
) -> serde_json::Value {
    use serde_json::json;
    let Some(i) = last_valid(tr, "lnr") else {
        return serde_json::Value::Null;
    };
    let f = |c: &str| tr.col(c)[i];
    let r = |v: f64, d: i32| {
        let p = 10f64.powi(d);
        if v.is_finite() {
            json!((v * p).round() / p)
        } else {
            serde_json::Value::Null
        }
    };
    let base7 = tr.col("base7");
    let prev: Vec<f64> = base7.iter().copied().filter(|v| v.is_finite()).collect();
    let tail: Vec<f64> = prev.iter().rev().take(8).rev().copied().collect();
    let change = if tail.len() > 1 {
        (tail[tail.len() - 1] - tail[0]) * 20.0
    } else {
        0.0
    };
    let advice = tr.text("advice")[i].clone();
    let text = advice
        .as_deref()
        .and_then(|a| ADVICE_TEXT.iter().find(|(k, _)| *k == a))
        .map(|(_, t)| *t)
        .unwrap_or("");
    let mvi = last_valid(mv, "lnr").unwrap_or(i);
    let pred = mv.col("pred")[mvi];
    let z = mv.col("z")[mvi];
    json!({
        "day": iso(f("day") as i32), "rmssd": r(f("rmssd"), 1), "score": r(f("score"), 1), "base_score": r(f("base_score"), 1), "base_change": r(change, 2),
        "lo": r(20.0 * f("lo"), 2), "hi": r(20.0 * f("hi"), 2), "swc_lo": r(20.0 * f("swc_lo"), 2), "swc_hi": r(20.0 * f("swc_hi"), 2),
        "advice": advice, "advice_text": text,
        "garmin": f("garmin"), "garmin_status": tr.text("g_status")[i].clone().unwrap_or_default().to_lowercase(),
        "sleep_hr": f("sleep_hr"), "rhr": f("rhr"), "cv7": r(f("cv7"), 2), "trend": tr.text("trend")[i].clone(),
        "expected": r(20.0 * pred, 1), "surprise": r(z, 2),
        "nights": stats::nan_len(&tr.col("lnr")), "first": iso(tr.col("day").iter().zip(tr.col("lnr")).find(|(_, l)| l.is_finite()).map(|(d, _)| *d as i32).unwrap_or(0)),
        "load_cuts": [n.load_cuts.0.round(), n.load_cuts.1.round()], "model": m,
        "range_nights": range_days, "k_daily": k_daily, "k_swc": k_swc,
    })
}

pub fn spark(tr: &Table) -> Table {
    let n = tr.n;
    let from = n.saturating_sub(28);
    let sel = |c: &str, k: f64| -> Vec<f64> { tr.col(c)[from..].iter().map(|v| k * v).collect() };
    let mut t = Table::new(n - from);
    t.set("day", sel("day", 1.0));
    t.set(
        "score",
        tr.col("lnr")[from..].iter().map(|v| 20.0 * v).collect(),
    );
    t.set("base", sel("base7", 20.0));
    t.set("lo", sel("lo", 20.0));
    t.set("hi", sel("hi", 20.0));
    t.txt
        .insert("advice".into(), tr.text("advice")[from..].to_vec());
    t
}

#[allow(dead_code)]
fn _unused(_: i64) -> i64 {
    MS_DAY
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::Series;

    /// 90 nights of HRV (ln around 4.1), one training day in five, with a drop after the hard days.
    pub(crate) fn toy() -> Data {
        let mut d = Data::default();
        let n = 90usize;
        let day0 = 19_723;
        let mut days = Table::new(n);
        days.set("day", (0..n).map(|i| (day0 + i as i32) as f64).collect());
        days.set("utc_offset_min", vec![-420.0; n]);
        let ln: Vec<f64> = (0..n)
            .map(|i| {
                4.1 + 0.05 * ((i * 7 % 11) as f64 - 5.0) / 5.0 - if i % 5 == 1 { 0.15 } else { 0.0 }
            })
            .collect();
        days.set(
            "hrv_night",
            ln.iter().map(|l| l.det_exp().round()).collect(),
        );
        days.txt
            .insert("hrv_status".into(), vec![Some("BALANCED".into()); n]);
        d.days = days;
        let mut sl = Table::new(n);
        sl.set("kind", vec![0.0; n]);
        sl.set("day", (0..n).map(|i| (day0 + i as i32) as f64).collect());
        sl.set(
            "start",
            (0..n)
                .map(|i| ((day0 + i as i32) as i64 * MS_DAY - 3_600_000) as f64)
                .collect(),
        );
        sl.set("sleep_s", vec![27_000.0; n]);
        sl.set("avg_hr", vec![52.0; n]);
        d.sleeps = sl;
        let mut a = Table::new(n / 5);
        a.set(
            "start",
            (0..n / 5)
                .map(|k| ((day0 + (k * 5) as i32) as i64 * MS_DAY + 14 * 3_600_000) as f64)
                .collect(),
        );
        a.set(
            "end",
            (0..n / 5)
                .map(|k| ((day0 + (k * 5) as i32) as i64 * MS_DAY + 15 * 3_600_000) as f64)
                .collect(),
        );
        a.set("utc_offset_min", vec![-420.0; n / 5]);
        a.set(
            "load",
            (0..n / 5).map(|k| [60.0, 150.0, 300.0][k % 3]).collect(),
        );
        d.activities = a;
        let mut s = Series::default();
        let mut v = vec![];
        let mut dayc = vec![];
        for i in 0..n {
            for k in 0..20 {
                s.t.push((day0 + i as i32) as i64 * MS_DAY + (k as i64 + 1) * 5 * MS_MIN);
                v.push(ln[i].det_exp() * (1.0 + 0.02 * ((k % 3) as f64 - 1.0)));
                dayc.push((day0 + i as i32) as f64);
            }
        }
        s.cols.insert("v".into(), v);
        s.cols.insert("day".into(), dayc);
        d.series.insert("hrv5".into(), s);
        d
    }

    #[test]
    fn nightly_builds_one_row_per_day() {
        let d = toy();
        let n = nightly(&d);
        assert_eq!(n.t.n, 90);
        let src = n.t.text("rmssd_src");
        assert_eq!(src[0].as_deref(), Some("5-min values"));
        let load = n.t.col("load");
        assert_eq!(load[1], 60.0); // the night after the day's training
        assert_eq!(load[0], 0.0);
        assert_eq!(n.t.text("load_class")[0].as_deref(), Some("rest"));
        assert_eq!(n.t.text("load_class")[1].as_deref(), Some("easy")); // 60 is below the lower tercile cut
    }

    #[test]
    fn scored_gives_advice_after_the_warm_up() {
        let d = toy();
        let n = nightly(&d);
        let s = scored(&n, 60, 1.5, 0.5);
        let adv = s.text("advice");
        assert!(adv[10].is_none()); // fewer than 20 earlier nights
        assert!(adv[80].is_some());
        let t = trends(&s);
        assert_eq!(t.text("trend")[5].as_deref(), Some("warming up"));
        let (lag, _) = acute(&t);
        assert!(lag.n > 0);
        // the night after a hard-ish day sits below the baseline: the toy dips by 0.15 on training days + 1
        let m = lag.col("mean");
        let cls = lag.text("klass");
        let nt = lag.col("night");
        let i = (0..lag.n)
            .find(|&i| cls[i].as_deref() == Some("easy") && nt[i] == 1.0)
            .unwrap();
        assert!(m[i] < -0.05, "{}", m[i]);
    }

    #[test]
    fn model_finds_a_negative_training_response() {
        let d = toy();
        let n = nightly(&d);
        let (m, v) = model(&n.t);
        assert!(m.beta < 0.0 && m.tau > 0.0 && m.sigma_eps > 0.0, "{:?}", m);
        assert_eq!(v.n, 90);
        assert!(m.lr > 0.0);
    }

    #[test]
    fn inside_the_night_bins_and_sensitivity() {
        let d = toy();
        let n = nightly(&d);
        let s = scored(&n, 60, 1.5, 0.5);
        let t = trends(&s);
        let (barcode, _stage, sens) = inside(&d, &n.t, &t);
        assert!(barcode.n > 0);
        let h = barcode.col("h");
        assert!(h.iter().all(|&x| (0.0..=10.0).contains(&x)));
        assert_eq!(sens.n, 3);
        assert!(sens.col("effect")[0] > 0.0); // rest nights sit above hard-day nights
    }

    #[test]
    fn run_produces_the_headline() {
        let d = toy();
        let r = run(&d, 60, 1.5, 0.5);
        assert_eq!(r.headline["day"], "2024-03-30");
        assert!(r.headline["advice_text"].as_str().unwrap().len() > 10);
        assert_eq!(r.spark.n, 28);
    }
}
