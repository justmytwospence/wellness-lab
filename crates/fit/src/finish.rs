//! `finish`: steps increments, stage ends, device eras, sorted and deduplicated series, typed columns.

use crate::schema::schema;
use crate::tables::{Column, Columns, Tables};
use crate::{setf, Buf, ExportBuilder, ImportReport, Row, Val};
use std::collections::BTreeMap;

/// Typed columns of rows, in the schema's order for `table`.
fn columns(table: &str, rows: &[&Row]) -> Columns {
    schema().tables[table]
        .iter()
        .map(|(name, ty)| {
            let c = if ty == "VARCHAR" {
                Column::Str(
                    rows.iter()
                        .map(|r| match r.get(name.as_str()) {
                            Some(Val::S(s)) => Some(s.clone()),
                            Some(Val::F(x)) => Some(x.to_string()),
                            None => None,
                        })
                        .collect(),
                )
            } else {
                let v: Vec<f64> = rows
                    .iter()
                    .map(|r| match r.get(name.as_str()) {
                        Some(Val::F(x)) => *x,
                        _ => f64::NAN,
                    })
                    .collect();
                Column::from_f64(ty, &v).unwrap()
            };
            (name.clone(), c)
        })
        .collect()
}

/// Sort a series by `t` (stable, so the first producer row wins among equal instants) and drop duplicates.
fn sort_dedupe(b: Buf) -> Buf {
    let mut idx: Vec<usize> = (0..b.t.len()).collect();
    idx.sort_by_key(|&i| b.t[i]);
    idx.dedup_by_key(|i| b.t[*i]);
    Buf {
        t: idx.iter().map(|&i| b.t[i]).collect(),
        cols: b
            .cols
            .iter()
            .map(|c| idx.iter().map(|&i| c[i]).collect())
            .collect(),
    }
}

impl ExportBuilder {
    /// The tables and the report. Missing values are nulls; every table and series follows the schema.
    pub fn finish(mut self) -> (Tables, ImportReport) {
        let sch = schema();

        // monitoring: each local day keeps the samples of the device with the most samples that day
        let mon = std::mem::take(&mut self.mon);
        let mut counts: BTreeMap<(i32, &str), u64> = BTreeMap::new();
        let mut days_of: BTreeMap<(&str, &str), Vec<i32>> = BTreeMap::new();
        for (serial, series) in &mon {
            for (name, b) in series {
                let days: Vec<i32> = b.t.iter().map(|&t| self.local_day(t)).collect();
                for &d in &days {
                    *counts.entry((d, serial.as_str())).or_default() += 1;
                }
                days_of.insert((serial.as_str(), name), days);
            }
        }
        let mut main: BTreeMap<i32, (u64, &str)> = BTreeMap::new();
        for ((d, serial), n) in counts {
            let e = main.entry(d).or_insert((n, serial));
            if n > e.0 {
                *e = (n, serial);
            }
        }
        let is_main = |d: i32, serial: &str| main.get(&d).is_none_or(|m| m.1 == serial);
        for (serial, series) in &mon {
            for (name, b) in series {
                let days = &days_of[&(serial.as_str(), *name)];
                for (i, &t) in b.t.iter().enumerate() {
                    if is_main(days[i], serial) {
                        let vals: Vec<f64> = b.cols.iter().map(|c| c[i]).collect();
                        self.push(name, t, &vals);
                    }
                }
            }
        }

        // steps: increments of the cumulative count per (day, device, activity type), summed per instant
        let mut per_t: BTreeMap<i64, (f64, f64)> = BTreeMap::new();
        for ((d, serial, _), mut v) in std::mem::take(&mut self.steps) {
            if !is_main(d, &serial) {
                continue;
            }
            v.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
            // The count only grows within a day, so an increment is what exceeds the highest count so far: a sample
            // below it (a stale or re-synced file interleaved with the live one) adds nothing instead of a sawtooth.
            let mut high = 0.0;
            for (t, cum, intensity) in v {
                let inc = (cum - high).max(0.0);
                high = cum.max(high);
                let e = per_t.entry(t).or_insert((0.0, f64::NAN));
                e.0 += inc;
                if intensity.is_finite() && (e.1.is_nan() || intensity > e.1) {
                    e.1 = intensity;
                }
            }
        }
        for (t, (v, i)) in per_t {
            self.push("steps", t, &[v, i]);
        }

        // main sleeps sorted by start, for stage ends and days
        let mut mains: Vec<(i64, i64, i32)> = self
            .sleeps
            .values()
            .filter(|r| matches!(r.get("kind"), Some(Val::F(k)) if *k == 0.0))
            .filter_map(|r| match (r.get("start"), r.get("end"), r.get("day")) {
                (Some(Val::F(s)), Some(Val::F(e)), Some(Val::F(d))) => {
                    Some((*s as i64, *e as i64, *d as i32))
                }
                _ => None,
            })
            .collect();
        mains.sort();
        let containing = |t: i64| -> Option<(i64, i32)> {
            // the earliest-ending main sleep with start <= t < end
            let i = mains.partition_point(|m| m.0 <= t);
            mains[..i]
                .iter()
                .rev()
                .take(4)
                .filter(|m| t < m.1)
                .min_by_key(|m| m.1)
                .map(|m| (m.1, m.2))
        };

        // stages: each change lasts until the next one in its file, the last one until the end of its main sleep
        let mut marks = std::mem::take(&mut self.marks);
        marks.sort_by_key(|m| (m.2, m.0));
        let mut stages: Vec<(i64, i64, i32, Option<i32>)> = Vec::with_capacity(marks.len());
        for (i, &(start, stage, file)) in marks.iter().enumerate() {
            let main = containing(start);
            let end = match marks.get(i + 1) {
                Some(n) if n.2 == file => n.0,
                _ => main.map(|m| m.0).unwrap_or(start + 60_000),
            };
            if end > start {
                stages.push((start, end, stage, main.map(|m| m.1)));
            }
        }
        stages.sort_by_key(|s| s.0);
        stages.dedup_by_key(|s| s.0);

        // sleep FIT assessments fill the scores the JSON left empty
        for sf in std::mem::take(&mut self.sleep_files) {
            if sf.scores.is_empty() {
                continue;
            }
            let d = sf
                .first
                .and_then(|t| containing(t).map(|m| m.1))
                .or_else(|| sf.last.map(|t| self.local_day(t)));
            let Some(d) = d else { continue };
            let row = self.day_row(d);
            for (k, v) in sf.scores {
                row.entry(k).or_insert(v);
            }
        }

        // monitoring resting HR where the daily summary has none
        for (d, rhr) in std::mem::take(&mut self.rhr_fit) {
            let row = self.day_row(d);
            if !row.contains_key("rhr") {
                setf(row, "rhr", Some(rhr));
            }
        }

        // days (a record that set nothing makes no row)
        let day_rows: Vec<Row> = std::mem::take(&mut self.days)
            .into_iter()
            .filter(|(_, r)| !r.is_empty())
            .map(|(d, mut r)| {
                r.insert("day", Val::F(d as f64));
                r
            })
            .collect();
        let mut t = Tables {
            days: columns("days", &day_rows.iter().collect::<Vec<_>>()),
            ..Default::default()
        };
        let key = |r: &Row, k: &str| match r.get(k) {
            Some(Val::F(x)) => *x,
            _ => f64::NAN,
        };
        let mut sleeps: Vec<&Row> = self.sleeps.values().collect();
        sleeps.sort_by(|a, b| key(a, "start").total_cmp(&key(b, "start")));
        t.sleeps = columns("sleeps", &sleeps);

        let stage_cols = &sch.tables["stages"];
        let mut sc: BTreeMap<&str, Vec<f64>> = BTreeMap::new();
        sc.insert("start", stages.iter().map(|s| s.0 as f64).collect());
        sc.insert("end", stages.iter().map(|s| s.1 as f64).collect());
        sc.insert("stage", stages.iter().map(|s| s.2 as f64).collect());
        sc.insert(
            "day",
            stages
                .iter()
                .map(|s| s.3.map_or(f64::NAN, |d| d as f64))
                .collect(),
        );
        t.stages = stage_cols
            .iter()
            .map(|(n, ty)| {
                let v = sc.remove(n.as_str()).unwrap_or_default();
                (n.clone(), Column::from_f64(ty, &v).unwrap())
            })
            .collect();

        let mut acts: Vec<&mut Row> = self.activities.values_mut().collect();
        for a in acts.iter_mut() {
            a.entry("rr_beats").or_insert(Val::F(0.0));
        }
        let mut acts: Vec<&Row> = self.activities.values().collect();
        acts.sort_by(|a, b| key(a, "start").total_cmp(&key(b, "start")));
        t.activities = columns("activities", &acts);

        let mut tags: Vec<&Row> = self.tags.iter().collect();
        let tkey = |r: &Row| {
            (
                key(r, "day") as i64,
                match r.get("name") {
                    Some(Val::S(s)) => s.clone(),
                    _ => String::new(),
                },
            )
        };
        tags.sort_by_key(|a| tkey(a));
        t.tags = columns("tags", &tags);

        let mut devs: Vec<Row> = self
            .devices
            .iter()
            .map(|((p, s), (a, b))| {
                let mut r = Row::new();
                r.insert("first_day", Val::F(*a as f64));
                r.insert("last_day", Val::F(*b as f64));
                if !p.is_empty() {
                    r.insert("product", Val::S(p.clone()));
                }
                if !s.is_empty() {
                    r.insert("serial", Val::S(s.clone()));
                }
                r
            })
            .collect();
        devs.sort_by(|a, b| key(a, "first_day").total_cmp(&key(b, "first_day")));
        t.devices = columns("devices", &devs.iter().collect::<Vec<_>>());

        // series in schema order
        let mut report = std::mem::take(&mut self.report);
        for (name, buf) in std::mem::take(&mut self.series) {
            let Some(cols) = sch.series.get(name) else {
                continue;
            };
            let b = sort_dedupe(buf);
            report
                .series_rows
                .insert(name.to_string(), b.t.len() as u64);
            let mut c: Columns = vec![(
                "t".to_string(),
                Column::I64(b.t.iter().map(|&x| Some(x)).collect()),
            )];
            for ((n, ty), v) in cols[1..].iter().zip(&b.cols) {
                c.push((n.clone(), Column::from_f64(ty, v).unwrap()));
            }
            t.series.insert(name.to_string(), c);
        }
        report.activities_with_rr = self.rr.len() as u32;
        t.rr = std::mem::take(&mut self.rr);
        report.days = match (day_rows.first(), day_rows.last()) {
            (Some(a), Some(b)) => Some((key(a, "day") as i32, key(b, "day") as i32)),
            _ => None,
        };
        (t, report)
    }
}
