//! Calendar and clock helpers. Days are days since 1970-01-01 (Garmin's local calendar date); weekdays count from Monday = 0.

pub const MS_DAY: i64 = 86_400_000;
pub const MS_MIN: i64 = 60_000;
pub const MS_HOUR: i64 = 3_600_000;

/// The local calendar day of a UTC instant (ms) at a UTC offset in minutes.
pub fn day_of_ms(t: i64, offset_min: i32) -> i32 {
    (t + offset_min as i64 * MS_MIN).div_euclid(MS_DAY) as i32
}

/// Local clock hour in [0, 24) of a UTC instant (ms).
pub fn local_hour(t: i64, offset_min: i32) -> f64 {
    ((t + offset_min as i64 * MS_MIN).rem_euclid(MS_DAY)) as f64 / MS_HOUR as f64
}

/// Monday = 0. 1970-01-01 was a Thursday.
pub fn weekday(day: i32) -> u8 {
    (day + 3).rem_euclid(7) as u8
}

/// (year, month, day) of a days-since-epoch value (civil-from-days).
pub fn ymd(day: i32) -> (i32, u32, u32) {
    let z = day as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    ((y + (m <= 2) as i64) as i32, m, d)
}

/// Days since 1970-01-01 of a civil date.
pub fn days_from_ymd(y: i32, m: u32, d: u32) -> i32 {
    let y = y as i64 - (m <= 2) as i64;
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if m > 2 { m - 3 } else { m + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era * 146_097 + doe - 719_468) as i32
}

/// Day of the year, 1-based.
pub fn doy(day: i32) -> u16 {
    let (y, _, _) = ymd(day);
    (day - days_from_ymd(y, 1, 1) + 1) as u16
}

/// "2026-11-04"
pub fn iso(day: i32) -> String {
    let (y, m, d) = ymd(day);
    format!("{y:04}-{m:02}-{d:02}")
}

pub fn parse_iso(s: &str) -> Option<i32> {
    let mut it = s.get(..10)?.split('-');
    let y: i32 = it.next()?.parse().ok()?;
    let m: u32 = it.next()?.parse().ok()?;
    let d: u32 = it.next()?.parse().ok()?;
    Some(days_from_ymd(y, m, d))
}

/// Circular mean of clock hours (period 24).
pub fn circ_mean_hours(x: &[f64]) -> f64 {
    let (mut s, mut c, mut n) = (0.0, 0.0, 0);
    for &h in x.iter().filter(|h| h.is_finite()) {
        let a = h * std::f64::consts::TAU / 24.0;
        s += a.sin();
        c += a.cos();
        n += 1;
    }
    if n == 0 {
        return f64::NAN;
    }
    (s.atan2(c) * 24.0 / std::f64::consts::TAU).rem_euclid(24.0)
}

/// a - b on a 24 h clock, in [-12, 12).
pub fn circ_diff_hours(a: f64, b: f64) -> f64 {
    (a - b + 12.0).rem_euclid(24.0) - 12.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_round_trip() {
        assert_eq!(days_from_ymd(1970, 1, 1), 0);
        assert_eq!(ymd(20_381), (2025, 10, 20));
        for d in [-1000, 0, 59, 60, 11_016, 19_723, 20_761, 30_000] {
            let (y, m, dd) = ymd(d);
            assert_eq!(days_from_ymd(y, m, dd), d);
        }
        assert_eq!(iso(20_761), "2026-11-04");
        assert_eq!(parse_iso("2026-11-04"), Some(20_761));
    }

    #[test]
    fn weekdays_and_clock() {
        assert_eq!(weekday(0), 3); // Thursday
        assert_eq!(weekday(parse_iso("2026-11-02").unwrap()), 0); // a Monday
        assert_eq!(doy(parse_iso("2024-03-01").unwrap()), 61);
        // 07:06 UTC at -07:00 is 00:06 local
        let t = 20_761i64 * MS_DAY + 7 * MS_HOUR + 6 * MS_MIN;
        assert!((local_hour(t, -420) - 0.1).abs() < 1e-9);
        assert_eq!(day_of_ms(t, -420), 20_761);
        assert_eq!(day_of_ms(20_761 * MS_DAY + MS_HOUR, -420), 20_760);
    }

    #[test]
    fn circular_hours() {
        assert!(
            (circ_mean_hours(&[23.0, 1.0]) - 0.0).abs() < 1e-9
                || (circ_mean_hours(&[23.0, 1.0]) - 24.0).abs() < 1e-9
        );
        assert!((circ_diff_hours(1.0, 23.0) - 2.0).abs() < 1e-9);
        assert!((circ_diff_hours(23.0, 1.0) + 2.0).abs() < 1e-9);
    }
}
