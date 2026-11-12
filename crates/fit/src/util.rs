//! Small helpers: JSON access, Garmin time strings, the canonical-v1 sport names.

use serde_json::Value as J;
use wellness_lab_core::time::{days_from_ymd, parse_iso, MS_DAY, MS_MIN};

/// Seconds between the Unix epoch and the FIT epoch (1989-12-31T00:00:00Z).
pub const FIT_EPOCH_S: i64 = 631_065_600;

/// A number at `key`, if present and finite (numeric strings count too).
pub fn num(v: &J, key: &str) -> Option<f64> {
    to_num(v.get(key)?)
}

pub fn to_num(v: &J) -> Option<f64> {
    match v {
        J::Number(n) => n.as_f64().filter(|x| x.is_finite()),
        J::String(s) => s.trim().parse::<f64>().ok().filter(|x| x.is_finite()),
        _ => None,
    }
}

/// A string at `key`, if present and non-empty.
pub fn text(v: &J, key: &str) -> Option<String> {
    match v.get(key)? {
        J::String(s) if !s.is_empty() => Some(s.clone()),
        J::Number(n) => Some(n.to_string()),
        J::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// A calendar day at `key`: `"2026-11-04"` or `[2026, 11, 4]`.
pub fn day(v: &J, key: &str) -> Option<i32> {
    match v.get(key)? {
        J::String(s) => parse_iso(s),
        J::Array(a) if a.len() >= 3 => {
            let y = a[0].as_i64()?;
            let m = a[1].as_i64()?;
            let d = a[2].as_i64()?;
            Some(days_from_ymd(y as i32, m as u32, d as u32))
        }
        _ => None,
    }
}

/// Unix ms of a Garmin time at `key`: a string like `"2026-11-04T07:06:54.0"` (read as UTC) or epoch ms.
pub fn time_ms(v: &J, key: &str) -> Option<i64> {
    match v.get(key)? {
        J::String(s) => parse_time(s),
        J::Number(n) => n
            .as_f64()
            .filter(|x| x.is_finite())
            .map(|x| x.round() as i64),
        _ => None,
    }
}

/// `"2026-11-04T07:06:54.0"` (optionally with `Z`) as UTC Unix ms.
pub fn parse_time(s: &str) -> Option<i64> {
    let s = s.trim().trim_end_matches('Z');
    let day = parse_iso(s)?;
    let rest = s.get(11..).unwrap_or("");
    if rest.is_empty() {
        return Some(day as i64 * MS_DAY);
    }
    let mut it = rest.splitn(3, ':');
    let h: i64 = it.next()?.parse().ok()?;
    let m: i64 = it.next()?.parse().ok()?;
    let sec: f64 = it.next().unwrap_or("0").parse().ok()?;
    Some(day as i64 * MS_DAY + h * 3_600_000 + m * MS_MIN + (sec * 1000.0).round() as i64)
}

/// Text of a numeric amount: integers without a decimal point.
pub fn num_text(x: f64) -> String {
    if x.fract() == 0.0 && x.abs() < 1e15 {
        format!("{}", x as i64)
    } else {
        format!("{x}")
    }
}

/// The canonical-v1 sport of a Garmin activity type key (garmin-mcp `export.py` `SPORTS`), `other` by default.
pub fn sport(type_key: &str) -> &'static str {
    match type_key {
        "running" | "track_running" => "run",
        "trail_running" => "trail_run",
        "treadmill_running" => "treadmill_run",
        "walking" => "walk",
        "hiking" | "mountaineering" | "snow_shoe_ws" => "hike",
        "cycling" | "road_biking" => "ride",
        "gravel_cycling" => "gravel_ride",
        "mountain_biking" => "mountain_bike",
        "indoor_cycling" => "indoor_ride",
        "virtual_ride" => "virtual_ride",
        "backcountry_skiing" => "backcountry_ski",
        "resort_skiing" | "resort_skiing_snowboarding_ws" => "resort_ski",
        "skate_skiing_ws" => "skate_ski",
        "cross_country_skiing_ws" => "classic_ski",
        "lap_swimming" | "open_water_swimming" => "swim",
        "rowing" | "indoor_rowing" => "row",
        "kayaking_v2" | "stand_up_paddleboarding_v2" | "paddling" => "paddle",
        _ => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn garmin_times() {
        assert_eq!(parse_time("1970-01-01T00:00:01.5"), Some(1500));
        assert_eq!(parse_time("2026-11-04T07:06:54.0"), Some(1_793_776_014_000));
        assert_eq!(parse_time("1970-01-02"), Some(MS_DAY));
        assert_eq!(num_text(2.0), "2");
        assert_eq!(num_text(2.5), "2.5");
        assert_eq!(sport("trail_running"), "trail_run");
        assert_eq!(sport("yoga"), "other");
    }
}
