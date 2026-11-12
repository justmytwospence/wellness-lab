//! Deterministic synthetic data for tests, the e2e suite and the in-app demo, with injected episodes as ground truth.

use crate::data::{Data, Series, Table};
use crate::labels::{Label, LabelKind, Source};
use crate::math::Det;
use crate::stats::Rng;
use crate::time::{doy, weekday, MS_DAY, MS_HOUR, MS_MIN};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Default)]
struct Night {
    rhr_add: f64,
    early_add: f64,
    lnr_add: f64,
    resp_add: f64,
    temp_add: f64,
    alt: f64,
    tz_delta: i32,
}

/// `days` days of data starting at `start_day`; the second value is the truth: the labels of the injected episodes.
pub fn synth(seed: u64, days: i32, start_day: i32) -> (Data, Vec<Label>) {
    let mut rng = Rng::new(seed);
    let n = days.max(1) as usize;
    let mut nights = vec![Night::default(); n];
    let mut truth: Vec<Label> = vec![];
    let mut n_travel = 0;
    let mut mk = |kind: LabelKind, a: i32, b: i32| {
        truth.push(Label {
            id: format!(
                "truth:{}:{}:{}",
                serde_json::to_string(&kind).unwrap().trim_matches('"'),
                a,
                truth.len()
            ),
            kind,
            start_day: start_day + a,
            end_day: start_day + b,
            onset_day: Some(start_day + a),
            severity: None,
            note: String::new(),
            exclude_baseline: kind.default_exclude(),
            source: Source::Confirmed,
            proposal_id: None,
            created_at: String::new(),
            updated_at: String::new(),
            deleted_at: None,
        });
    };
    // episodes are placed on a grid so that they never overlap: illnesses, trips and altitude blocks every `slot` nights
    let slot = (n as i32 / 14).max(20);
    let mut cursor = 45;
    let mut plan: Vec<(&str, i32)> = vec![];
    let counts = [("illness", 6usize), ("travel", 4), ("altitude", 3)];
    for k in 0..6 {
        for (name, c) in counts {
            if k < c {
                plan.push((name, 0));
            }
        }
    }
    for (name, _) in plan {
        let len = match name {
            "illness" => 3 + rng.below(4) as i32,
            "travel" => 6,
            _ => 3 + rng.below(3) as i32,
        };
        let a = cursor + rng.below(6) as i32;
        let b = a + len - 1;
        cursor += slot;
        if b + 20 >= n as i32 {
            continue;
        }
        match name {
            "illness" => {
                for d in a..=b {
                    nights[d as usize].rhr_add += 6.0;
                    nights[d as usize].lnr_add -= 0.15;
                    nights[d as usize].resp_add += 0.8;
                    nights[d as usize].temp_add += 0.4;
                }
                mk(LabelKind::Illness, a, b);
            }
            "travel" => {
                let delta = if n_travel % 2 == 0 { -120 } else { 60 };
                n_travel += 1;
                for d in a..=b {
                    nights[d as usize].tz_delta = delta;
                }
                mk(LabelKind::Travel, a, b);
            }
            _ => {
                for d in a..=b {
                    nights[d as usize].alt = 3000.0;
                    nights[d as usize].rhr_add += 4.0;
                }
                mk(LabelKind::Altitude, a, b);
            }
        }
    }
    // alcohol on Friday and Saturday nights (the wake day is a Saturday or Sunday)
    let mut alcohol = 0;
    let mut tries = 0;
    while alcohol < 25 && tries < 5000 && n > 60 {
        tries += 1;
        let d = 30 + rng.below(n - 40);
        let wd = weekday(start_day + d as i32);
        let busy = nights
            .iter()
            .skip(d.saturating_sub(3))
            .take(8)
            .any(|x| x.rhr_add != 0.0 || x.tz_delta != 0 || x.alt != 0.0);
        if (wd == 5 || wd == 6) && !busy {
            nights[d].early_add = 5.0;
            nights[d].lnr_add -= 0.18;
            mk(LabelKind::Alcohol, d as i32, d as i32);
            alcohol += 1;
        }
    }

    let mut data = Data::default();
    let mut days_t = Table::new(n);
    let mut sleeps = Table::new(n);
    let mut hr = Series::default();
    let mut stress = Series::default();
    let mut resp = Series::default();
    let mut hrv5 = Series::default();
    let mut steps = Series::default();
    let mut alt = Series::default();
    let mut spo2 = Series::default();
    let mut bb = Series::default();
    for k in ["v", "q"] {
        stress.cols.insert(k.into(), vec![]);
        bb.cols.insert(k.into(), vec![]);
    }
    for k in ["v", "day"] {
        hrv5.cols.insert(k.into(), vec![]);
    }
    for k in ["v", "conf", "mode"] {
        spo2.cols.insert(k.into(), vec![]);
    }
    for k in ["v", "intensity"] {
        steps.cols.insert(k.into(), vec![]);
    }
    for s in [&mut hr, &mut resp, &mut alt] {
        s.cols.insert("v".into(), vec![]);
    }
    let mut stages = Table::new(0);
    let (mut st_start, mut st_end, mut st_code, mut st_day) = (vec![], vec![], vec![], vec![]);
    let mut acts = (vec![], vec![], vec![], vec![], vec![]);
    let mut dc: BTreeMap<&str, Vec<f64>> = BTreeMap::new();
    let mut txt_status: Vec<Option<String>> = vec![];
    let mut offsets = vec![];
    let base_off = -420i32;
    let mut ln_hist: Vec<f64> = vec![];
    for i in 0..n {
        let day = start_day + i as i32;
        let nt = nights[i];
        let off = base_off + nt.tz_delta;
        offsets.push(off);
        let free = weekday(day) >= 5;
        let day_ms = day as i64 * MS_DAY - off as i64 * MS_MIN; // UTC instant of local midnight
        let onset_h = -1.0 + (rng.normal() * 40.0 + if free { 60.0 } else { 0.0 }) / 60.0; // hours after local midnight of the previous day's 24:00
        let onset = day_ms + (onset_h * MS_HOUR as f64) as i64;
        let wake_h = 6.75 + (rng.normal() * 30.0 + if free { 60.0 } else { 0.0 }) / 60.0;
        let wake = (day_ms + (wake_h * MS_HOUR as f64) as i64).max(onset + 5 * MS_HOUR);
        let tib = ((wake - onset) / MS_MIN) as usize;
        let season = (std::f64::consts::TAU * doy(day) as f64 / 365.0).det_sin();
        let rhr = 50.0 + 2.0 * season + rng.normal() * 1.2 + nt.rhr_add;
        let ln_r = 4.1 - 0.02 * season + rng.normal() * 0.12 + nt.lnr_add;
        let resp_n = 14.0 + rng.normal() * 0.4 + nt.resp_add;
        // training: one session on every fifth day at 17:00
        let train = i % 5 == 2;
        let load = 50.0 + rng.f64() * 200.0;
        if train {
            let s = day_ms + 17 * MS_HOUR;
            acts.0.push(format!("garmin:{}", 1_000_000 + i));
            acts.1.push(s as f64);
            acts.2.push((s + MS_HOUR) as f64);
            acts.3.push(off as f64);
            acts.4.push(load);
        }
        // the night
        let mut lnr_vals = vec![];
        let mut hr_sum = 0.0;
        let mut stress_sum = 0.0;
        for m in 0..tib {
            let t = onset + m as i64 * MS_MIN;
            let early = if m < 180 {
                nt.early_add * (1.0 - m as f64 / 240.0)
            } else {
                0.0
            };
            let late = if m + 120 > tib {
                3.0 * (m + 120 - tib) as f64 / 120.0
            } else {
                0.0
            };
            let h = rhr + 6.0 * (-(m as f64) / 90.0).det_exp() + late + early + rng.normal() * 2.0;
            hr.t.push(t);
            hr.cols.get_mut("v").unwrap().push(h);
            hr_sum += h;
            let sv = (20.0 + 2.0 * (h - rhr) + rng.normal() * 5.0).clamp(0.0, 100.0);
            stress_sum += sv;
            stress.t.push(t);
            stress.cols.get_mut("v").unwrap().push(sv);
            stress.cols.get_mut("q").unwrap().push(0.0);
            resp.t.push(t);
            resp.cols
                .get_mut("v")
                .unwrap()
                .push(resp_n + rng.normal() * 0.6);
            spo2.t.push(t);
            spo2.cols
                .get_mut("v")
                .unwrap()
                .push((95.0 - nt.alt / 1500.0 + rng.normal()).clamp(80.0, 100.0));
            spo2.cols.get_mut("conf").unwrap().push(0.0);
            spo2.cols.get_mut("mode").unwrap().push(0.0);
            if m % 5 == 4 {
                let v = (ln_r
                    + 0.15 * (std::f64::consts::TAU * m as f64 / 90.0).det_sin()
                    + rng.normal() * 0.2)
                    .det_exp();
                hrv5.t.push(t);
                hrv5.cols.get_mut("v").unwrap().push(v);
                hrv5.cols.get_mut("day").unwrap().push(day as f64);
                lnr_vals.push(v);
            }
            if m % 3 == 0 {
                bb.t.push(t);
                bb.cols
                    .get_mut("v")
                    .unwrap()
                    .push((25.0 + 70.0 * m as f64 / tib as f64).min(100.0));
                bb.cols.get_mut("q").unwrap().push(0.0);
            }
        }
        // stages: 90-minute cycles
        let mut m = 0usize;
        let mut cyc = 0;
        while m < tib {
            for (code, len) in [
                (1, 40),
                (0, if cyc < 2 { 30 } else { 10 }),
                (2, 20 + 4 * cyc.min(4)),
                (3, 2),
            ] {
                if m >= tib {
                    break;
                }
                let e = (m + len).min(tib);
                st_start.push((onset + m as i64 * MS_MIN) as f64);
                st_end.push((onset + e as i64 * MS_MIN) as f64);
                st_code.push(code as f64);
                st_day.push(day as f64);
                m = e;
            }
            cyc += 1;
        }
        // the day: heart rate and stress each minute, steps on a schedule, altitude every two minutes
        let day_start = wake;
        let next_onset_est = day_ms + 23 * MS_HOUR;
        let mut t = day_start;
        let mut hrd_sum = 0.0;
        while t < next_onset_est {
            let local_h = ((t + off as i64 * MS_MIN).rem_euclid(MS_DAY)) as f64 / MS_HOUR as f64;
            let in_train = train && (17.0..18.0).contains(&local_h);
            let walking = (7.5..9.0).contains(&local_h)
                || (12.0..13.0).contains(&local_h)
                || rng.f64() < 0.05;
            let h = if in_train {
                140.0 + rng.normal() * 6.0
            } else {
                62.0 + if walking { 22.0 } else { 6.0 } + rng.normal() * 3.0
            };
            hr.t.push(t);
            hr.cols.get_mut("v").unwrap().push(h);
            hrd_sum += h;
            stress.t.push(t);
            if in_train {
                stress.cols.get_mut("v").unwrap().push(f64::NAN);
                stress.cols.get_mut("q").unwrap().push(2.0);
            } else {
                stress
                    .cols
                    .get_mut("v")
                    .unwrap()
                    .push((30.0 + (h - 65.0) * 0.8 + rng.normal() * 8.0).clamp(1.0, 99.0));
                stress.cols.get_mut("q").unwrap().push(0.0);
            }
            steps.t.push(t);
            steps.cols.get_mut("v").unwrap().push(if in_train {
                150.0
            } else if walking {
                60.0 + rng.f64() * 40.0
            } else {
                0.0
            });
            steps.cols.get_mut("intensity").unwrap().push(if in_train {
                5.0
            } else if walking {
                2.0
            } else {
                0.0
            });
            if (t / MS_MIN) % 2 == 0 {
                alt.t.push(t);
                alt.cols.get_mut("v").unwrap().push(if nt.alt > 0.0 {
                    nt.alt + rng.normal() * 15.0
                } else {
                    1600.0 + rng.normal() * 8.0
                });
            }
            t += MS_MIN;
        }
        let _ = hrd_sum;
        // the night's altitude samples too
        let mut tt = onset;
        while tt < wake {
            if (tt / MS_MIN) % 2 == 0 {
                alt.t.push(tt);
                alt.cols.get_mut("v").unwrap().push(if nt.alt > 0.0 {
                    nt.alt + rng.normal() * 15.0
                } else {
                    1600.0 + rng.normal() * 8.0
                });
            }
            tt += MS_MIN;
        }
        // rows
        let mean_v = lnr_vals.iter().sum::<f64>() / lnr_vals.len().max(1) as f64;
        ln_hist.push(mean_v.det_ln());
        let recent: Vec<f64> = ln_hist.iter().rev().take(28).cloned().collect();
        let mu = recent.iter().sum::<f64>() / recent.len() as f64;
        let push = |c: &mut BTreeMap<&str, Vec<f64>>, k: &'static str, v: f64| {
            c.entry(k).or_default().push(v)
        };
        push(&mut dc, "day", day as f64);
        push(&mut dc, "utc_offset_min", off as f64);
        push(&mut dc, "rhr", rhr.round());
        push(&mut dc, "hrv_night", mean_v.round());
        push(&mut dc, "hrv_week", mu.det_exp().round());
        push(&mut dc, "hrv_bal_lo", (mu - 0.15).det_exp().round());
        push(&mut dc, "hrv_bal_hi", (mu + 0.15).det_exp().round());
        push(
            &mut dc,
            "sleep_score",
            (78.0 + rng.normal() * 6.0 - (nt.early_add * 1.5) - nt.rhr_add)
                .round()
                .clamp(30.0, 99.0),
        );
        push(
            &mut dc,
            "altitude_avg",
            if nt.alt > 0.0 { nt.alt } else { 1600.0 },
        );
        push(&mut dc, "steps", (6000.0 + rng.normal() * 1500.0).round());
        push(&mut dc, "skin_temp_dev", f64::NAN);
        let _ = nt.temp_add;
        txt_status.push(Some("BALANCED".into()));
        let sl = |k: &'static str, v: f64| (k, v);
        for (k, v) in [
            sl("kind", 0.0),
            sl("day", day as f64),
            sl("start", onset as f64),
            sl("end", wake as f64),
            sl("sleep_s", (tib as f64 * 60.0 * 0.93).round()),
            sl("deep_s", (tib as f64 * 60.0 * 0.17).round()),
            sl("light_s", (tib as f64 * 60.0 * 0.5).round()),
            sl("rem_s", (tib as f64 * 60.0 * 0.23).round()),
            sl("awake_s", (tib as f64 * 60.0 * 0.07).round()),
            sl("avg_hr", hr_sum / tib as f64),
            sl("rhr", rhr.round()),
            sl("resp_avg", resp_n),
            sl("avg_stress", stress_sum / tib as f64),
        ] {
            sleeps.num.entry(k.into()).or_default().push(v);
        }
    }
    days_t.num = dc.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    days_t.txt.insert("hrv_status".into(), txt_status);
    days_t.n = n;
    sleeps.n = n;
    stages.n = st_start.len();
    stages.set("start", st_start);
    stages.set("end", st_end);
    stages.set("stage", st_code);
    stages.set("day", st_day);
    let na = acts.0.len();
    let mut a = Table::new(na);
    a.txt
        .insert("id".into(), acts.0.into_iter().map(Some).collect());
    a.set("start", acts.1);
    a.set("end", acts.2);
    a.set("utc_offset_min", acts.3);
    a.set("load", acts.4);
    a.txt.insert("sport".into(), vec![Some("run".into()); na]);
    a.set("rr_beats", vec![0.0; na]);
    a.n = na;
    // beat intervals for every eighth activity: heart rate ramps from easy to hard over the hour and the beat-to-beat
    // correlation falls with it (a long-memory process when easy, nearly white when hard), so alpha-1 sweeps 1.3 to 0.4
    let mut rr_rng = Rng::new(seed ^ 0xBEA7);
    let mut beats = vec![0.0; na];
    for (k, b) in beats.iter_mut().enumerate().filter(|(k, _)| k % 4 == 3) {
        let (mut x, mut t, mut rr) = (0.0f64, 0.0f64, Vec::<f64>::new());
        while t < 3600.0 {
            let u = t / 3600.0;
            let hr = 105.0 + 75.0 * u;
            let phi = 0.96 * (1.0 - u).powf(0.8);
            x = phi * x + (1.0 - phi * phi).sqrt() * rr_rng.normal();
            let sigma = 45.0 - 35.0 * u;
            let v = (60_000.0 / hr + sigma * x).clamp(300.0, 1500.0);
            rr.push(v.round());
            t += v / 1000.0;
        }
        *b = rr.len() as f64;
        if let Some(Some(id)) = a.txt.get("id").map(|v| v[k].clone()) {
            data.rr
                .insert(id.trim_start_matches("garmin:").to_string(), rr);
        }
    }
    a.set("rr_beats", beats);
    a.txt.insert(
        "name".into(),
        (0..na).map(|k| Some(format!("Run {}", k + 1))).collect(),
    );
    data.days = days_t;
    data.sleeps = sleeps;
    data.stages = stages;
    data.activities = a;
    for (name, mut s) in [
        ("hr", hr),
        ("stress", stress),
        ("resp", resp),
        ("hrv5", hrv5),
        ("steps", steps),
        ("altitude", alt),
        ("spo2", spo2),
        ("body_battery", bb),
    ] {
        sort_series(&mut s);
        data.series.insert(name.to_string(), s);
    }
    (data, truth)
}

/// Sort a series by time (the day's samples are generated after the night's).
fn sort_series(s: &mut Series) {
    let mut idx: Vec<usize> = (0..s.t.len()).collect();
    idx.sort_by_key(|&i| s.t[i]);
    if idx.windows(2).all(|w| w[0] < w[1]) {
        return;
    }
    s.t = idx.iter().map(|&i| s.t[i]).collect();
    for v in s.cols.values_mut() {
        *v = idx.iter().map(|&i| v[i]).collect();
    }
    // drop duplicate instants (the first wins)
    let mut keep = vec![true; s.t.len()];
    for i in 1..s.t.len() {
        if s.t[i] == s.t[i - 1] {
            keep[i] = false;
        }
    }
    if keep.iter().any(|k| !k) {
        let sel: Vec<usize> = (0..s.t.len()).filter(|&i| keep[i]).collect();
        s.t = sel.iter().map(|&i| s.t[i]).collect();
        for v in s.cols.values_mut() {
            *v = sel.iter().map(|&i| v[i]).collect();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_deterministic_and_has_ground_truth() {
        let (a, ta) = synth(3, 400, 19_723);
        let (b, tb) = synth(3, 400, 19_723);
        assert_eq!(a.series["hr"].t.len(), b.series["hr"].t.len());
        assert_eq!(a.series["hr"].v()[100], b.series["hr"].v()[100]);
        assert_eq!(ta.len(), tb.len());
        let count = |k: LabelKind| ta.iter().filter(|l| l.kind == k).count();
        assert!(
            count(LabelKind::Illness) >= 4,
            "{}",
            count(LabelKind::Illness)
        );
        assert!(count(LabelKind::Alcohol) >= 15);
        assert!(count(LabelKind::Travel) >= 2 && count(LabelKind::Altitude) >= 2);
        assert_eq!(a.days.n, 400);
        for s in a.series.values() {
            assert!(s.t.windows(2).all(|w| w[0] < w[1]), "unsorted series");
        }
    }
}
