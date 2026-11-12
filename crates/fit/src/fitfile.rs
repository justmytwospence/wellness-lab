//! The FIT files of the export: monitoring (`monitoring_b`), sleep (49), HRV status (68), motion (58) and activities.

use crate::util::FIT_EPOCH_S;
use crate::{setf, sets, Buf, ExportBuilder, FitError, FitKind, Row, SleepFile, Val};
use fitparser::de::{DecodeOption, FitObject, FitStreamProcessor};
use fitparser::profile::MesgNum;
use fitparser::{FitDataRecord, Value};
use std::collections::BTreeMap;
use wellness_lab_core::time::MS_DAY;

/// Global message numbers an activity file needs (the rest are skipped undecoded): file_id, session, hrv.
const ACTIVITY_MESGS: [u16; 3] = [0, 18, 78];
const ALTITUDE_MESG: u16 = 279;
const MOTION_MESG: u16 = 318;

/// Decode a FIT file, keeping every record before a decoding error. Activity files decode only `ACTIVITY_MESGS`.
fn decode(bytes: &[u8]) -> (Vec<FitDataRecord>, Option<String>) {
    let mut p = FitStreamProcessor::new();
    p.add_option(DecodeOption::SkipHeaderCrcValidation);
    p.add_option(DecodeOption::SkipDataCrcValidation);
    let mut out = Vec::new();
    let mut buf = bytes;
    let mut activity = false;
    while !buf.is_empty() {
        let (rest, obj) = match p.deserialize_next(buf) {
            Ok(x) => x,
            Err(e) => return (out, Some(e.to_string())),
        };
        buf = rest;
        match obj {
            FitObject::Crc(_) => p.reset(),
            FitObject::DataMessage(msg) => {
                let g = msg.global_message_number();
                if g == 0 {
                    activity = matches!(msg.fields().get(&0), Some(Value::Enum(4)));
                } else if activity && !ACTIVITY_MESGS.contains(&g) && msg.time_offset().is_none() {
                    continue;
                }
                match p.decode_message(msg) {
                    Ok(r) => out.push(r),
                    Err(e) => return (out, Some(e.to_string())),
                }
            }
            _ => {}
        }
    }
    (out, None)
}

fn field<'a>(r: &'a FitDataRecord, name: &str) -> Option<&'a Value> {
    r.fields()
        .iter()
        .find(|f| f.name() == name)
        .map(|f| f.value())
}

fn field_n(r: &FitDataRecord, n: u8) -> Option<&Value> {
    r.fields()
        .iter()
        .find(|f| f.number() == n)
        .map(|f| f.value())
}

fn vnum(v: &Value) -> Option<f64> {
    let x = match v {
        Value::Byte(x) | Value::Enum(x) | Value::UInt8(x) | Value::UInt8z(x) => *x as f64,
        Value::SInt8(x) => *x as f64,
        Value::SInt16(x) => *x as f64,
        Value::UInt16(x) | Value::UInt16z(x) => *x as f64,
        Value::SInt32(x) => *x as f64,
        Value::UInt32(x) | Value::UInt32z(x) => *x as f64,
        Value::SInt64(x) => *x as f64,
        Value::UInt64(x) | Value::UInt64z(x) => *x as f64,
        Value::Float32(x) => *x as f64,
        Value::Float64(x) => *x,
        _ => return None,
    };
    x.is_finite().then_some(x)
}

fn num(r: &FitDataRecord, name: &str) -> Option<f64> {
    field(r, name).and_then(vnum)
}

/// FIT seconds of a UTC date_time value (decoded or raw).
fn vsecs(v: &Value) -> Option<i64> {
    match v {
        Value::Timestamp(dt) => Some(dt.timestamp() - FIT_EPOCH_S),
        _ => vnum(v).map(|x| x as i64),
    }
}

/// FIT seconds of a local_date_time value, as if it were UTC (so its UTC date is the local date).
fn vlocal_secs(v: &Value) -> Option<i64> {
    match v {
        Value::Timestamp(dt) => Some(dt.naive_local().and_utc().timestamp() - FIT_EPOCH_S),
        _ => vnum(v).map(|x| x as i64),
    }
}

fn vtext(v: &Value) -> Option<String> {
    match v {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Invalid | Value::Array(_) | Value::Timestamp(_) => None,
        _ => vnum(v).map(|x| format!("{}", x as i64)),
    }
}

fn ms(fit_s: i64) -> i64 {
    (fit_s + FIT_EPOCH_S) * 1000
}

/// The FIT seconds of a record's `timestamp` (field 253).
fn ts253(r: &FitDataRecord) -> Option<i64> {
    field_n(r, 253).and_then(vsecs)
}

/// `file_id.type` as text: the profile name (`monitoring_b`, `activity`) or the number of an unnamed type.
fn file_type(recs: &[FitDataRecord]) -> Option<String> {
    let r = recs.iter().find(|r| r.kind() == MesgNum::FileId)?;
    field(r, "type").and_then(vtext)
}

/// A named enum value: its profile name, or its number when unnamed or decoded numerically.
fn enum_is(v: Option<&Value>, name: &str, code: i64) -> bool {
    match v {
        Some(Value::String(s)) => s == name,
        Some(v) => vnum(v).map(|x| x as i64) == Some(code),
        None => false,
    }
}

impl ExportBuilder {
    /// Read one FIT file of the export. Call only after every JSON file (see [`FitError::JsonFirst`]).
    pub fn add_fit(&mut self, name: &str, bytes: &[u8]) -> Result<FitKind, FitError> {
        let (recs, err) = decode(bytes);
        if let Some(e) = &err {
            self.error(format!("{name}: {e}"));
            if recs.is_empty() {
                return Err(FitError::Decode(e.clone()));
            }
        }
        let Some(ty) = file_type(&recs) else {
            *self.report.fit_files.entry("none".into()).or_default() += 1;
            return Ok(FitKind::Ignored);
        };
        let kind = match ty.as_str() {
            "monitoring_b" => FitKind::Monitoring,
            "49" => FitKind::SleepFit,
            "68" => FitKind::HrvFit,
            "58" => FitKind::MotionFit,
            "activity" => FitKind::ActivityFit,
            _ => FitKind::Ignored,
        };
        if kind == FitKind::ActivityFit && (self.activities.is_empty() || self.offsets.is_empty()) {
            return Err(FitError::JsonFirst);
        }
        *self.report.fit_files.entry(ty).or_default() += 1;
        self.files += 1;
        match kind {
            FitKind::Monitoring => self.monitoring(&recs),
            FitKind::SleepFit => self.sleep_fit(&recs),
            FitKind::HrvFit => self.hrv_fit(&recs),
            FitKind::MotionFit => self.motion_fit(&recs),
            FitKind::ActivityFit => self.activity_fit(&recs),
            FitKind::Ignored => {}
        }
        Ok(kind)
    }

    /// The local calendar day a monitoring file belongs to.
    fn file_day(&self, recs: &[FitDataRecord]) -> Option<i32> {
        let info = recs
            .iter()
            .filter(|r| r.kind() == MesgNum::MonitoringInfo)
            .find_map(|r| field(r, "local_timestamp").and_then(vlocal_secs));
        if let Some(s) = info {
            return Some(ms(s).div_euclid(MS_DAY) as i32);
        }
        let first = recs.iter().find_map(ts253)?;
        Some(self.local_day(ms(first)))
    }

    /// Monitoring samples are kept per device until `finish`, which keeps each day's samples from the device that
    /// recorded the most of them that day (the watch worn all day, not a chest strap or bike computer that also
    /// writes monitoring files).
    fn monitoring(&mut self, recs: &[FitDataRecord]) {
        let Some(fday) = self.file_day(recs) else {
            return;
        };
        let creator = recs
            .iter()
            .filter(|r| r.kind() == MesgNum::DeviceInfo)
            .find(|r| enum_is(field_n(r, 0), "creator", 0))
            .and_then(|r| device(r, 4, 3))
            .or_else(|| {
                recs.iter()
                    .find(|r| r.kind() == MesgNum::FileId)
                    .and_then(|r| device(r, 2, 3))
            });
        let serial = creator.as_ref().map(|c| c.1.clone()).unwrap_or_default();
        let mut mon: BTreeMap<&'static str, Buf> = BTreeMap::new();
        let put =
            |mon: &mut BTreeMap<&'static str, Buf>, name: &'static str, t: i64, vals: &[f64]| {
                let b = mon.entry(name).or_insert_with(|| Buf {
                    t: vec![],
                    cols: vec![vec![]; vals.len()],
                });
                b.t.push(t);
                for (c, &v) in b.cols.iter_mut().zip(vals) {
                    c.push(v);
                }
            };
        let mut last: Option<i64> = None;
        let mut steps: Vec<(i64, i32, f64, f64)> = vec![];
        for r in recs {
            let t253 = ts253(r);
            match r.kind() {
                MesgNum::Monitoring => {
                    // timestamp_16 counts from the last full timestamp of a monitoring message: other messages (device
                    // info, events) can carry later times and would push these samples 18.2 h (2^16 s) ahead
                    let ts = match (t253, last, field_n(r, 26).and_then(vnum)) {
                        (Some(t), _, _) => {
                            last = Some(t);
                            t
                        }
                        (None, Some(l), Some(t16)) => {
                            let t = l + ((t16 as i64 - (l & 0xFFFF)) & 0xFFFF);
                            last = Some(t);
                            t
                        }
                        _ => continue,
                    };
                    let t = ms(ts);
                    if let Some(hr) = num(r, "heart_rate").filter(|&h| h > 0.0) {
                        put(&mut mon, "hr", t, &[hr]);
                    }
                    let at = field_n(r, 5);
                    let atype = if enum_is(at, "walking", 6) {
                        6
                    } else if enum_is(at, "running", 1) {
                        1
                    } else {
                        continue;
                    };
                    let cum = r.fields().iter().find(|f| f.number() == 3).and_then(|f| {
                        let v = vnum(f.value())?;
                        // the `steps` sub-field is raw; `cycles` (decoded before its activity type) is raw / 2
                        Some(if f.name() == "steps" {
                            v
                        } else {
                            (v * 2.0).round()
                        })
                    });
                    if let Some(cum) = cum {
                        let intensity = field_n(r, 28).and_then(vnum).unwrap_or(f64::NAN);
                        steps.push((t, atype, cum, intensity));
                    }
                }
                MesgNum::StressLevel => {
                    let (Some(ts), Some(v)) = (
                        field(r, "stress_level_time").and_then(vsecs),
                        num(r, "stress_level_value"),
                    ) else {
                        continue;
                    };
                    let (v, q) = if v >= 0.0 {
                        (v, 0.0)
                    } else if v == -2.0 {
                        (f64::NAN, 2.0)
                    } else {
                        (f64::NAN, 1.0)
                    };
                    put(&mut mon, "stress", ms(ts), &[v, q]);
                }
                MesgNum::RespirationRate => {
                    if let (Some(ts), Some(v)) = (t253, num(r, "respiration_rate")) {
                        if v > 0.0 {
                            put(&mut mon, "resp", ms(ts), &[v]);
                        }
                    }
                }
                MesgNum::Spo2Data => {
                    let (Some(ts), Some(v)) = (t253, num(r, "reading_spo2")) else {
                        continue;
                    };
                    if !(50.0..=100.0).contains(&v) {
                        continue;
                    }
                    let conf = num(r, "reading_confidence").unwrap_or(f64::NAN);
                    let m = field(r, "mode");
                    let mode = if enum_is(m, "periodic", 3) || enum_is(m, "continuous_check", 2) {
                        0.0
                    } else if enum_is(m, "spot_check", 1) {
                        1.0
                    } else {
                        2.0
                    };
                    put(&mut mon, "spo2", ms(ts), &[v, conf, mode]);
                }
                MesgNum::Value(ALTITUDE_MESG) => {
                    if let (Some(ts), Some(raw)) = (t253, field_n(r, 0).and_then(vnum)) {
                        let v = raw / 5.0 - 500.0;
                        if (-500.0..=9000.0).contains(&v) {
                            put(&mut mon, "altitude", ms(ts), &[v]);
                        }
                    }
                }
                MesgNum::MonitoringHrData => {
                    if let Some(rhr) = num(r, "resting_heart_rate").filter(|&x| x > 0.0) {
                        self.rhr_fit.insert(fday, rhr);
                    }
                }
                _ => {}
            }
        }
        // Cumulative counts restart at local midnight, and a file often begins by repeating the previous file's last
        // totals, so each sample is keyed by the local day of the interval ending at it (a sample at midnight closes
        // the previous day) and by the device counting it.
        for (t, atype, cum, intensity) in steps {
            let day = self.local_day(t - 1);
            self.steps
                .entry((day, serial.clone(), atype))
                .or_default()
                .push((t, cum, intensity));
        }
        let dev = self.mon.entry(serial).or_default();
        for (name, b) in mon {
            let d = dev.entry(name).or_default();
            d.t.extend(b.t);
            if d.cols.is_empty() {
                d.cols = b.cols;
            } else {
                for (dc, bc) in d.cols.iter_mut().zip(b.cols) {
                    dc.extend(bc);
                }
            }
        }
        // a file dated outside the daily summaries (a device clock never set) says nothing about device eras
        let dated = self.days.is_empty() || self.days.contains_key(&fday);
        if let (Some(key), true) = (creator, dated) {
            let e = self.devices.entry(key).or_insert((fday, fday));
            e.0 = e.0.min(fday);
            e.1 = e.1.max(fday);
        }
    }

    fn sleep_fit(&mut self, recs: &[FitDataRecord]) {
        let file = self.files;
        let mut sf = SleepFile {
            first: None,
            last: None,
            scores: Row::new(),
        };
        for r in recs {
            match r.kind() {
                MesgNum::SleepLevel => {
                    let Some(ts) = ts253(r) else { continue };
                    let lv = field(r, "sleep_level");
                    let stage = if enum_is(lv, "deep", 3) {
                        0
                    } else if enum_is(lv, "light", 2) {
                        1
                    } else if enum_is(lv, "rem", 4) {
                        2
                    } else if enum_is(lv, "awake", 1) {
                        3
                    } else {
                        4
                    };
                    let t = ms(ts);
                    self.marks.push((t, stage, file));
                    sf.first = Some(sf.first.map_or(t, |f| f.min(t)));
                    sf.last = Some(sf.last.map_or(t, |l| l.max(t)));
                }
                MesgNum::SleepAssessment => {
                    for (col, key) in [
                        ("sleep_score", "overall_sleep_score"),
                        ("sleep_quality_score", "sleep_quality_score"),
                        ("sleep_duration_score", "sleep_duration_score"),
                        ("sleep_recovery_score", "sleep_recovery_score"),
                        ("sleep_deep_score", "deep_sleep_score"),
                        ("sleep_rem_score", "rem_sleep_score"),
                        ("sleep_light_score", "light_sleep_score"),
                        ("sleep_awakenings_score", "awakenings_count_score"),
                        ("sleep_awake_time_score", "awake_time_score"),
                        ("sleep_restfulness_score", "sleep_restlessness_score"),
                        ("sleep_interruptions_score", "interruptions_score"),
                    ] {
                        setf(&mut sf.scores, col, num(r, key));
                    }
                }
                _ => {}
            }
        }
        if sf.first.is_none() {
            sf.last = recs.iter().filter_map(ts253).max().map(ms);
        }
        self.sleep_files.push(sf);
    }

    fn hrv_fit(&mut self, recs: &[FitDataRecord]) {
        let mut values = vec![];
        let mut summary: Option<(i64, Row)> = None;
        for r in recs {
            match r.kind() {
                MesgNum::HrvValue => {
                    if let (Some(ts), Some(v)) = (ts253(r), num(r, "value")) {
                        if v > 0.0 && v < 250.0 {
                            values.push((ms(ts), v));
                        }
                    }
                }
                MesgNum::HrvStatusSummary => {
                    let Some(ts) = ts253(r) else { continue };
                    let mut row = Row::new();
                    for (col, key) in [
                        ("hrv_night", "last_night_average"),
                        ("hrv_week", "weekly_average"),
                        ("hrv_5min_high", "last_night_5_min_high"),
                        ("hrv_bal_lo", "baseline_balanced_lower"),
                        ("hrv_bal_hi", "baseline_balanced_upper"),
                        ("hrv_low_upper", "baseline_low_upper"),
                    ] {
                        setf(&mut row, col, num(r, key));
                    }
                    let status = field(r, "status").and_then(|v| match v {
                        Value::String(s) => Some(s.to_uppercase()),
                        v => vnum(v).map(|x| {
                            match x as i64 {
                                0 => "NONE",
                                1 => "POOR",
                                2 => "LOW",
                                3 => "UNBALANCED",
                                4 => "BALANCED",
                                _ => "UNKNOWN",
                            }
                            .to_string()
                        }),
                    });
                    sets(&mut row, "hrv_status", status);
                    summary = Some((ms(ts), row));
                }
                _ => {}
            }
        }
        let anchor = summary
            .as_ref()
            .map(|s| s.0)
            .or_else(|| values.iter().map(|v| v.0).max());
        let Some(anchor) = anchor else { return };
        let d = self.local_day(anchor);
        if let Some((_, row)) = summary {
            let day = self.day_row(d);
            for (k, v) in row {
                day.insert(k, v);
            }
        }
        for (t, v) in values {
            self.push("hrv5", t, &[v, d as f64]);
        }
    }

    fn motion_fit(&mut self, recs: &[FitDataRecord]) {
        for r in recs {
            if r.kind() != MesgNum::Value(MOTION_MESG) {
                continue;
            }
            let Some(ts) = ts253(r) else { continue };
            let v = field_n(r, 0).and_then(vnum).unwrap_or(f64::NAN);
            let v2 = field_n(r, 3).and_then(vnum).unwrap_or(f64::NAN);
            if v.is_finite() || v2.is_finite() {
                self.push("motion", ms(ts), &[v, v2]);
            }
        }
    }

    /// The unused activity summary starting nearest to `t` (ms), within 120 s.
    fn match_activity(&mut self, t: i64) -> Option<String> {
        if self.act_dirty {
            self.act_starts.sort();
            self.act_dirty = false;
        }
        let i = self.act_starts.partition_point(|(s, _)| *s < t);
        let mut best: Option<(i64, usize)> = None;
        for j in i.saturating_sub(3)..(i + 3).min(self.act_starts.len()) {
            let (s, id) = &self.act_starts[j];
            let gap = (s - t).abs();
            if gap <= 120_000 && !self.act_used.contains(id) && best.is_none_or(|b| gap < b.0) {
                best = Some((gap, j));
            }
        }
        let id = self.act_starts[best?.1].1.clone();
        self.act_used.insert(id.clone());
        Some(id)
    }

    fn activity_fit(&mut self, recs: &[FitDataRecord]) {
        let start = recs
            .iter()
            .filter(|r| r.kind() == MesgNum::Session)
            .filter_map(|r| field(r, "start_time").and_then(vsecs))
            .min()
            .or_else(|| {
                recs.iter()
                    .filter(|r| r.kind() != MesgNum::FileId)
                    .find_map(ts253)
            });
        let Some(id) = start.and_then(|s| self.match_activity(ms(s))) else {
            self.report.skipped += 1;
            return;
        };
        let mut rr: Vec<f32> = vec![];
        for r in recs.iter().filter(|r| r.kind() == MesgNum::Hrv) {
            let Some(v) = field(r, "time") else { continue };
            // 0xFFFF (decoded as invalid) pads the five-slot array and is not a beat; Garmin's own long-interval values
            // (5000 ms and more) are kept as the notebook's gap markers
            let mut beat = |x: &Value| {
                if let Some(s) = vnum(x).filter(|&s| s < 65.5345) {
                    rr.push((s * 1000.0).round() as f32);
                }
            };
            match v {
                Value::Array(a) => a.iter().for_each(&mut beat),
                x => beat(x),
            }
        }
        if rr.len() >= 500 {
            if let Some(row) = self.activities.get_mut(&id) {
                row.insert("rr_beats", Val::F(rr.len() as f64));
            }
            self.rr.insert(id, rr);
        }
    }
}

/// (product, serial) of a device_info or file_id record from its product and serial field numbers.
fn device(r: &FitDataRecord, product: u8, serial: u8) -> Option<(String, String)> {
    let p = r
        .fields()
        .iter()
        .find(|f| f.number() == product && f.name() != "favero_product")
        .and_then(|f| vtext(f.value()));
    let s = field_n(r, serial).and_then(vtext);
    (p.is_some() || s.is_some()).then(|| (p.unwrap_or_default(), s.unwrap_or_default()))
}
