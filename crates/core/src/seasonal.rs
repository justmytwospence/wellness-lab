//! Seasonal view: a metric's deviation from its own centred one-year median, by year and day of year.
use crate::data::Table;
use crate::stats::{mean, rolling, rolling_median};
use crate::time::{days_from_ymd, ymd};
use std::collections::BTreeMap;

pub const BASE_WINDOW: usize = 365;
pub const BASE_MIN: usize = 120;
pub const SMOOTH_WINDOW: usize = 15;
pub const SMOOTH_MIN: usize = 5;
pub const MIN_YEAR_DAYS: usize = 180;

fn is_leap(y: i32) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}

/// Day of the year on a 366-day calendar: in non-leap years days from March 1 shift by one, so a date has the same
/// number every year.
pub fn leap_doy(day: i32) -> u16 {
    let (y, m, _) = ymd(day);
    let n = (day - days_from_ymd(y, 1, 1) + 1) as u16;
    if !is_leap(y) && m >= 3 {
        n + 1
    } else {
        n
    }
}

/// `day` and `value` are parallel columns (any order, NaN = missing, duplicate days: last wins).
/// Returns columns day, year, doy (leap_doy), value (raw, NaN on gaps), dev (smoothed deviation), for days in
/// from..=to with finite dev, only for years with at least MIN_YEAR_DAYS such days, and only if at least 2 years
/// qualify; otherwise an empty Table (n = 0). The baseline uses the metric's whole history, so the range only cuts
/// the output.
pub fn seasonal(day: &[f64], value: &[f64], from: i32, to: i32) -> Table {
    let pts: Vec<(i32, f64)> = day
        .iter()
        .zip(value)
        .filter(|(d, v)| d.is_finite() && v.is_finite())
        .map(|(d, v)| (*d as i32, *v))
        .collect();
    let (Some(d0), Some(d1)) = (pts.iter().map(|p| p.0).min(), pts.iter().map(|p| p.0).max())
    else {
        return Table::new(0);
    };
    let mut grid = vec![f64::NAN; (d1 - d0 + 1) as usize];
    for (d, v) in pts {
        grid[(d - d0) as usize] = v;
    }
    let base = rolling_median(&grid, BASE_WINDOW, BASE_MIN, true);
    let raw: Vec<f64> = grid.iter().zip(&base).map(|(g, b)| g - b).collect();
    let dev = rolling(&raw, SMOOTH_WINDOW, SMOOTH_MIN, true, mean);

    let rows: Vec<usize> = (0..grid.len())
        .filter(|&i| {
            let d = d0 + i as i32;
            from <= d && d <= to && dev[i].is_finite()
        })
        .collect();
    let mut per_year: BTreeMap<i32, usize> = BTreeMap::new();
    for &i in &rows {
        *per_year.entry(ymd(d0 + i as i32).0).or_default() += 1;
    }
    per_year.retain(|_, n| *n >= MIN_YEAR_DAYS);
    if per_year.len() < 2 {
        return Table::new(0);
    }
    let rows: Vec<usize> = rows
        .into_iter()
        .filter(|&i| per_year.contains_key(&ymd(d0 + i as i32).0))
        .collect();

    let mut o = Table::new(rows.len());
    o.set(
        "day",
        rows.iter().map(|&i| (d0 + i as i32) as f64).collect(),
    );
    o.set(
        "year",
        rows.iter().map(|&i| ymd(d0 + i as i32).0 as f64).collect(),
    );
    o.set(
        "doy",
        rows.iter()
            .map(|&i| leap_doy(d0 + i as i32) as f64)
            .collect(),
    );
    o.set("value", rows.iter().map(|&i| grid[i]).collect());
    o.set("dev", rows.iter().map(|&i| dev[i]).collect());
    o
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::parse_iso;

    fn d(s: &str) -> i32 {
        parse_iso(s).unwrap()
    }

    fn days(start: &str, n: usize) -> Vec<f64> {
        let s = d(start);
        (0..n).map(|i| (s + i as i32) as f64).collect()
    }

    fn years(t: &Table) -> Vec<i32> {
        let mut y: Vec<i32> = t.col("year").iter().map(|v| *v as i32).collect();
        y.dedup();
        y
    }

    #[test]
    fn leap_doy_aligns_years() {
        assert_eq!(leap_doy(d("2024-03-01")), 61);
        assert_eq!(leap_doy(d("2025-03-01")), 61);
        assert_eq!(leap_doy(d("2025-02-28")), 59);
        assert_eq!(leap_doy(d("2024-02-29")), 60);
        assert_eq!(leap_doy(d("2024-12-31")), 366);
        assert_eq!(leap_doy(d("2025-12-31")), 366);
        assert_eq!(leap_doy(d("2025-01-01")), 1);
    }

    #[test]
    fn constant_series_has_zero_deviation() {
        let x = days("2023-01-01", 1095);
        let v = vec![10.0; x.len()];
        let t = seasonal(&x, &v, d("2023-01-01"), d("2025-12-31"));
        assert!(t.n > 0);
        assert!(t.col("dev").iter().all(|&e| e == 0.0));
        assert_eq!(years(&t), vec![2023, 2024, 2025]);
    }

    #[test]
    fn season_survives_trend() {
        let x = days("2023-01-01", 1095);
        let v: Vec<f64> = x
            .iter()
            .enumerate()
            .map(|(i, &dd)| {
                50.0 + 0.005 * i as f64
                    + 2.0 * (std::f64::consts::TAU * leap_doy(dd as i32) as f64 / 366.0).sin()
            })
            .collect();
        let t = seasonal(&x, &v, d("2023-01-01"), d("2025-12-31"));
        let (yr, doy, dev) = (t.col("year"), t.col("doy"), t.col("dev"));
        let avg = |lo: f64, hi: f64| {
            let s: Vec<f64> = (0..t.n)
                .filter(|&i| yr[i] == 2024.0 && doy[i] >= lo && doy[i] <= hi)
                .map(|i| dev[i])
                .collect();
            s.iter().sum::<f64>() / s.len() as f64
        };
        assert!(avg(80.0, 100.0) > 1.5, "{}", avg(80.0, 100.0));
        assert!(avg(263.0, 283.0) < -1.5, "{}", avg(263.0, 283.0));
    }

    #[test]
    fn short_history_is_empty() {
        let x = days("2024-01-01", 300);
        let t = seasonal(&x, &vec![5.0; 300], d("2024-01-01"), d("2024-12-31"));
        assert_eq!(t.n, 0);
    }

    #[test]
    fn sparse_year_is_dropped() {
        // 2023 and 2024 in full, 2025 for its first 150 days
        let x = days("2023-01-01", 365 + 366 + 150);
        let v: Vec<f64> = (0..x.len()).map(|i| 10.0 + (i % 7) as f64).collect();
        let t = seasonal(&x, &v, d("2023-01-01"), d("2025-12-31"));
        assert!(t.n > 0);
        assert_eq!(years(&t), vec![2023, 2024]);
    }

    #[test]
    fn range_cuts_output_not_baseline() {
        let x = days("2022-01-01", 1460);
        let v: Vec<f64> = (0..x.len()).map(|i| i as f64).collect();
        let (from, to) = (d("2023-01-01"), d("2024-12-31"));
        let t = seasonal(&x, &v, from, to);
        assert!(t.n > 0);
        let (day, dev) = (t.col("day"), t.col("dev"));
        assert!(day.iter().all(|&e| e >= from as f64 && e <= to as f64));
        let i = day.iter().position(|&e| e == from as f64).unwrap();
        assert!(dev[i].abs() < 1e-9, "{}", dev[i]);
    }
}
