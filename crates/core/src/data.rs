//! The in-memory wellness-v1 data: columnar tables and series. NaN means missing.

use serde::Serialize;
use std::collections::BTreeMap;

/// A table of named numeric and string columns, all of length `n`. Serialises with NaN as null.
#[derive(Default, Clone, Debug, Serialize)]
pub struct Table {
    pub n: usize,
    pub num: BTreeMap<String, Vec<f64>>,
    pub txt: BTreeMap<String, Vec<Option<String>>>,
}

impl Table {
    pub fn new(n: usize) -> Self {
        Table {
            n,
            ..Default::default()
        }
    }
    /// A numeric column, NaN-filled when the table has no such column.
    pub fn col(&self, name: &str) -> Vec<f64> {
        self.num
            .get(name)
            .cloned()
            .unwrap_or_else(|| vec![f64::NAN; self.n])
    }
    pub fn has(&self, name: &str) -> bool {
        self.num.contains_key(name)
    }
    /// A string column, None-filled when absent.
    pub fn text(&self, name: &str) -> Vec<Option<String>> {
        self.txt
            .get(name)
            .cloned()
            .unwrap_or_else(|| vec![None; self.n])
    }
    pub fn set(&mut self, name: &str, v: Vec<f64>) {
        self.n = v.len();
        self.num.insert(name.to_string(), v);
    }
}

/// An intraday series: `t` in Unix ms (strictly increasing) and its value columns (`v`, and `q`, `conf`, ... as the schema says).
#[derive(Default, Clone, Debug)]
pub struct Series {
    pub t: Vec<i64>,
    pub cols: BTreeMap<String, Vec<f64>>,
}

impl Series {
    pub fn len(&self) -> usize {
        self.t.len()
    }
    pub fn is_empty(&self) -> bool {
        self.t.is_empty()
    }
    pub fn v(&self) -> &[f64] {
        self.cols.get("v").map(|v| v.as_slice()).unwrap_or(&[])
    }
    pub fn c(&self, name: &str) -> Option<&Vec<f64>> {
        self.cols.get(name)
    }
    /// Index range of the samples with `t0 <= t < t1`.
    pub fn range(&self, t0: i64, t1: i64) -> (usize, usize) {
        let a = self.t.partition_point(|&t| t < t0);
        let b = self.t.partition_point(|&t| t < t1);
        (a, b.max(a))
    }
}

#[derive(Default, Clone, Debug)]
pub struct Data {
    pub days: Table,
    pub sleeps: Table,
    pub stages: Table,
    pub activities: Table,
    pub tags: Table,
    pub devices: Table,
    pub series: BTreeMap<String, Series>,
    pub rr: BTreeMap<String, Vec<f64>>,
}

impl Data {
    pub fn table_mut(&mut self, name: &str) -> Option<&mut Table> {
        match name {
            "days" => Some(&mut self.days),
            "sleeps" => Some(&mut self.sleeps),
            "stages" => Some(&mut self.stages),
            "activities" => Some(&mut self.activities),
            "tags" => Some(&mut self.tags),
            "devices" => Some(&mut self.devices),
            _ => None,
        }
    }
    pub fn s(&self, name: &str) -> Option<&Series> {
        self.series.get(name).filter(|s| !s.is_empty())
    }
    /// Index of a calendar day in `days` (sorted by day).
    pub fn day_index(&self, day: i32) -> Option<usize> {
        let d = self.days.num.get("day")?;
        let i = d.partition_point(|&x| x < day as f64);
        (i < d.len() && d[i] as i32 == day).then_some(i)
    }
    /// The UTC offset in minutes of a calendar day: the day's own, else the nearest earlier known one, else 0.
    pub fn offset_of_day(&self, day: i32) -> i32 {
        let (Some(d), Some(o)) = (
            self.days.num.get("day"),
            self.days.num.get("utc_offset_min"),
        ) else {
            return 0;
        };
        let i = d.partition_point(|&x| x <= day as f64);
        for j in (0..i).rev() {
            if o[j].is_finite() {
                return o[j] as i32;
            }
        }
        o.iter()
            .copied()
            .find(|x| x.is_finite())
            .map(|x| x as i32)
            .unwrap_or(0)
    }
    /// The UTC offset in effect at a UTC instant (ms).
    pub fn offset_at(&self, t: i64) -> i32 {
        let mut day = t.div_euclid(86_400_000) as i32;
        let mut off = self.offset_of_day(day);
        // the local day of t may differ from its UTC day: settle once
        let local = (t + off as i64 * 60_000).div_euclid(86_400_000) as i32;
        if local != day {
            day = local;
            off = self.offset_of_day(day);
        }
        off
    }
}
