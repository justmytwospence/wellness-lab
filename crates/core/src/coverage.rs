//! How much of each series exists for each day, and what the devices were: provenance, so a gap is never mistaken for a calm day.

use crate::data::{Data, Table};
use crate::time::{day_of_ms, MS_DAY};
use serde::Serialize;
use std::collections::BTreeMap;

/// Expected samples per day at the series' native step.
pub fn expected_per_day(name: &str) -> f64 {
    match name {
        "hr" | "stress" | "resp" | "spo2" | "steps" | "sleep_move" => 1440.0,
        "body_battery" => 480.0,
        "altitude" => 720.0,
        "hrv5" => 96.0,
        "motion" => 1440.0,
        _ => 1440.0,
    }
}

#[derive(Serialize, Default)]
pub struct Coverage {
    pub first_day: i32,
    pub days: usize,
    pub series: BTreeMap<String, Vec<f64>>,
    pub devices: Table,
    pub no_hr_days: Vec<i32>,
}

pub fn coverage(data: &Data) -> Coverage {
    let mut lo = i32::MAX;
    let mut hi = i32::MIN;
    for s in data.series.values().filter(|s| !s.is_empty()) {
        lo = lo.min(s.t[0].div_euclid(MS_DAY) as i32);
        hi = hi.max(s.t[s.len() - 1].div_euclid(MS_DAY) as i32);
    }
    if lo > hi {
        return Coverage::default();
    }
    let n = (hi - lo + 1) as usize;
    let mut out = Coverage {
        first_day: lo,
        days: n,
        devices: data.devices.clone(),
        ..Default::default()
    };
    for (name, s) in &data.series {
        let mut cnt = vec![0.0; n];
        for &t in &s.t {
            let d = day_of_ms(t, data.offset_at(t)) - lo;
            if d >= 0 && (d as usize) < n {
                cnt[d as usize] += 1.0;
            }
        }
        let e = expected_per_day(name);
        out.series
            .insert(name.clone(), cnt.iter().map(|c| (c / e).min(1.0)).collect());
    }
    if let Some(hr) = out.series.get("hr") {
        out.no_hr_days = (0..n)
            .filter(|&i| hr[i] < 0.05)
            .map(|i| lo + i as i32)
            .collect();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::synth;

    #[test]
    fn full_days_are_complete() {
        let (d, _) = synth(1, 30, 19_723);
        let c = coverage(&d);
        assert!(c.days >= 30);
        let hr = &c.series["hr"];
        assert!(hr[5] > 0.95, "{}", hr[5]);
        assert!(c.no_hr_days.len() <= 2);
        assert!(c.series["hrv5"][5] > 0.7);
    }
}
