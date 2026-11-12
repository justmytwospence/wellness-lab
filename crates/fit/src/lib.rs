//! Garmin account export ("Export Your Data") to wellness-v1, in memory.
//!
//! Feed every JSON file to [`ExportBuilder::add_json`] first, then every FIT file to [`ExportBuilder::add_fit`], then
//! call [`ExportBuilder::finish`] for the [`Tables`] and an [`ImportReport`]. Activity FITs are matched to the JSON
//! activity summaries by start time and local offsets come from the daily summaries, hence the order.

mod finish;
mod fitfile;
mod json;
pub mod schema;
mod tables;
mod util;

pub use tables::{col, rows, Column, Columns, Tables};

use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use wellness_lab_core::time::{MS_DAY, MS_MIN};

#[derive(thiserror::Error, Debug)]
pub enum FitError {
    /// An activity FIT arrived before the JSON summaries it is matched against.
    #[error("json-first: add every JSON file before the activity FIT files")]
    JsonFirst,
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    /// The file could not be decoded at all (it is also counted in `ImportReport::errors`).
    #[error("fit: {0}")]
    Decode(String),
}

/// What a JSON file was read as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JsonKind {
    Uds,
    SleepJson,
    HealthStatus,
    Readiness,
    Maxmet,
    Acclim,
    Lifestyle,
    ActivitiesJson,
    Ignored,
}

impl JsonKind {
    pub fn as_str(self) -> &'static str {
        match self {
            JsonKind::Uds => "uds",
            JsonKind::SleepJson => "sleep_json",
            JsonKind::HealthStatus => "health_status",
            JsonKind::Readiness => "readiness",
            JsonKind::Maxmet => "maxmet",
            JsonKind::Acclim => "acclim",
            JsonKind::Lifestyle => "lifestyle",
            JsonKind::ActivitiesJson => "activities_json",
            JsonKind::Ignored => "ignored",
        }
    }
}

/// What a FIT file was read as (from its `file_id.type`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FitKind {
    Monitoring,
    SleepFit,
    HrvFit,
    MotionFit,
    ActivityFit,
    Ignored,
}

impl FitKind {
    pub fn as_str(self) -> &'static str {
        match self {
            FitKind::Monitoring => "monitoring",
            FitKind::SleepFit => "sleep_fit",
            FitKind::HrvFit => "hrv_fit",
            FitKind::MotionFit => "motion_fit",
            FitKind::ActivityFit => "activity_fit",
            FitKind::Ignored => "ignored",
        }
    }
}

/// What an import read and produced.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ImportReport {
    /// JSON files by kind (`ignored` included).
    pub json_files: BTreeMap<String, u32>,
    /// FIT files by `file_id.type` (`monitoring_b`, `activity`, or the number of an unnamed type).
    pub fit_files: BTreeMap<String, u32>,
    /// The first 100 errors.
    pub errors: Vec<String>,
    /// Records without the keys they need, and activity FITs matching no summary.
    pub skipped: u32,
    /// First and last calendar day of the `days` table.
    pub days: Option<(i32, i32)>,
    pub series_rows: BTreeMap<String, u64>,
    pub activities_with_rr: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Val {
    F(f64),
    S(String),
}

pub(crate) type Row = BTreeMap<&'static str, Val>;

pub(crate) fn setf(row: &mut Row, k: &'static str, v: Option<f64>) {
    if let Some(x) = v.filter(|x| x.is_finite()) {
        row.insert(k, Val::F(x));
    }
}

pub(crate) fn sets(row: &mut Row, k: &'static str, v: Option<String>) {
    if let Some(s) = v {
        row.insert(k, Val::S(s));
    }
}

/// A series under construction: `t` and its value columns (schema order after `t`), NaN for null.
#[derive(Default)]
pub(crate) struct Buf {
    pub t: Vec<i64>,
    pub cols: Vec<Vec<f64>>,
}

/// One sleep FIT (type 49): its stage marks' span and its assessment scores.
pub(crate) struct SleepFile {
    pub first: Option<i64>,
    pub last: Option<i64>,
    pub scores: Row,
}

/// Steps samples (t, cumulative count, intensity) by (local day, device serial, activity type).
pub(crate) type StepsAcc = BTreeMap<(i32, String, i32), Vec<(i64, f64, f64)>>;

/// Builds wellness-v1 tables from the files of a Garmin account export.
#[derive(Default)]
pub struct ExportBuilder {
    pub(crate) days: BTreeMap<i32, Row>,
    pub(crate) offsets: BTreeMap<i32, i32>,
    pub(crate) sleeps: BTreeMap<String, Row>,
    pub(crate) activities: BTreeMap<String, Row>,
    /// (start ms, id) of the activity summaries, sorted; rebuilt when `act_dirty`.
    pub(crate) act_starts: Vec<(i64, String)>,
    pub(crate) act_dirty: bool,
    pub(crate) act_used: HashSet<String>,
    pub(crate) tags: Vec<Row>,
    pub(crate) devices: BTreeMap<(String, String), (i32, i32)>,
    pub(crate) series: BTreeMap<&'static str, Buf>,
    /// monitoring samples by device serial, then series name (filtered to each day's main device in `finish`)
    pub(crate) mon: BTreeMap<String, BTreeMap<&'static str, Buf>>,
    /// (local day, device serial, activity type) -> (t, cumulative steps, intensity)
    pub(crate) steps: StepsAcc,
    /// sleep stage changes: (start ms, stage, file number)
    pub(crate) marks: Vec<(i64, i32, u32)>,
    pub(crate) sleep_files: Vec<SleepFile>,
    pub(crate) files: u32,
    pub(crate) rr: BTreeMap<String, Vec<f32>>,
    /// (handler, day) -> the timestamp of the record that won
    pub(crate) picked: BTreeMap<(&'static str, i32), i64>,
    pub(crate) rhr_fit: BTreeMap<i32, f64>,
    pub(crate) errors: u32,
    pub(crate) report: ImportReport,
}

impl ExportBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// The report so far (complete after `finish`).
    pub fn report(&self) -> &ImportReport {
        &self.report
    }

    pub(crate) fn error(&mut self, msg: String) {
        self.errors += 1;
        if self.report.errors.len() < 100 {
            self.report.errors.push(msg);
        }
    }

    pub(crate) fn day_row(&mut self, day: i32) -> &mut Row {
        self.days.entry(day).or_default()
    }

    pub(crate) fn push(&mut self, name: &'static str, t: i64, vals: &[f64]) {
        let b = self.series.entry(name).or_insert_with(|| Buf {
            t: vec![],
            cols: vec![vec![]; vals.len()],
        });
        b.t.push(t);
        for (c, &v) in b.cols.iter_mut().zip(vals) {
            c.push(v);
        }
    }

    /// The UTC offset (minutes) of a local calendar day from the daily summaries: the day's own, else the nearest
    /// earlier one, else the first known, else 0.
    pub(crate) fn offset_of_day(&self, day: i32) -> i32 {
        self.offsets
            .range(..=day)
            .next_back()
            .or_else(|| self.offsets.iter().next())
            .map(|(_, &o)| o)
            .unwrap_or(0)
    }

    /// The UTC offset in effect at a UTC instant (ms).
    pub(crate) fn offset_at(&self, t: i64) -> i32 {
        let day = t.div_euclid(MS_DAY) as i32;
        let off = self.offset_of_day(day);
        let local = (t + off as i64 * MS_MIN).div_euclid(MS_DAY) as i32;
        if local != day {
            self.offset_of_day(local)
        } else {
            off
        }
    }

    /// The local calendar day of a UTC instant (ms).
    pub(crate) fn local_day(&self, t: i64) -> i32 {
        (t + self.offset_at(t) as i64 * MS_MIN).div_euclid(MS_DAY) as i32
    }
}
