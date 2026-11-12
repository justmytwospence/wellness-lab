//! The JSON files of the export: daily summaries, sleep, Health Status, Training Readiness, VO2max, acclimation,
//! lifestyle logging and activity summaries.

use crate::util::{day, num, num_text, sport, text, time_ms, to_num};
use crate::{setf, sets, ExportBuilder, FitError, JsonKind, Row, Val};
use serde_json::Value as J;
use std::collections::btree_map::Entry;
use wellness_lab_core::time::MS_MIN;

/// Lower case, `\` as `/`, and no `_` or `-` (effort-curves `garmin.ts` `squash`).
pub fn squash(p: &str) -> String {
    p.replace('\\', "/").replace(['_', '-'], "").to_lowercase()
}

/// The handler of a JSON path, from its squashed file name.
pub fn json_kind(path: &str) -> JsonKind {
    let sq = squash(path);
    let f = sq.rsplit('/').next().unwrap_or("");
    if !f.ends_with(".json") {
        return JsonKind::Ignored;
    }
    if f.starts_with("udsfile") {
        JsonKind::Uds
    } else if f.ends_with("sleepdata.json") {
        JsonKind::SleepJson
    } else if f.ends_with("healthstatusdata.json") {
        JsonKind::HealthStatus
    } else if f.starts_with("trainingreadinessdto") {
        JsonKind::Readiness
    } else if f.starts_with("metricsmaxmetdata") {
        JsonKind::Maxmet
    } else if f.starts_with("metricsheataltitudeacclimation") {
        JsonKind::Acclim
    } else if f.ends_with("lifestylelogging.json") {
        JsonKind::Lifestyle
    } else if f.ends_with("summarizedactivities.json") {
        JsonKind::ActivitiesJson
    } else {
        JsonKind::Ignored
    }
}

fn records(v: &J) -> Vec<&J> {
    match v {
        J::Array(a) => a.iter().collect(),
        J::Object(_) => vec![v],
        _ => vec![],
    }
}

fn int(v: &J, key: &str) -> Option<f64> {
    num(v, key).map(|x| x.round())
}

impl ExportBuilder {
    /// Read one JSON file of the export. Unknown files are `JsonKind::Ignored`.
    pub fn add_json(&mut self, path: &str, text: &str) -> Result<JsonKind, FitError> {
        let kind = json_kind(path);
        if kind != JsonKind::Ignored {
            let v: J = match serde_json::from_str(text) {
                Ok(v) => v,
                Err(e) => {
                    self.error(format!("{path}: {e}"));
                    return Err(e.into());
                }
            };
            match kind {
                JsonKind::Uds => records(&v).into_iter().for_each(|r| self.uds(r)),
                JsonKind::SleepJson => records(&v).into_iter().for_each(|r| self.sleep_json(r)),
                JsonKind::HealthStatus => {
                    records(&v).into_iter().for_each(|r| self.health_status(r))
                }
                JsonKind::Readiness => records(&v).into_iter().for_each(|r| self.readiness(r)),
                JsonKind::Maxmet => records(&v).into_iter().for_each(|r| self.maxmet(r)),
                JsonKind::Acclim => records(&v).into_iter().for_each(|r| self.acclim(r)),
                JsonKind::Lifestyle => records(&v).into_iter().for_each(|r| self.lifestyle(r)),
                JsonKind::ActivitiesJson => self.activities_json(&v),
                JsonKind::Ignored => {}
            }
        }
        *self
            .report
            .json_files
            .entry(kind.as_str().to_string())
            .or_default() += 1;
        Ok(kind)
    }

    fn uds(&mut self, r: &J) {
        let Some(d) = day(r, "calendarDate") else {
            self.report.skipped += 1;
            return;
        };
        let off = match (
            time_ms(r, "wellnessStartTimeLocal"),
            time_ms(r, "wellnessStartTimeGmt"),
        ) {
            (Some(l), Some(g)) => Some(((l - g) as f64 / MS_MIN as f64).round() as i32),
            _ => None,
        };
        if let Some(o) = off {
            self.offsets.insert(d, o);
        }
        let row = self.day_row(d);
        setf(row, "utc_offset_min", off.map(|o| o as f64));
        for (col, key) in [
            ("rhr", "restingHeartRate"),
            ("hr_min", "minHeartRate"),
            ("hr_max", "maxHeartRate"),
            ("steps", "totalSteps"),
            ("mod_min", "moderateIntensityMinutes"),
            ("vig_min", "vigorousIntensityMinutes"),
            ("spo2_avg", "averageSpo2Value"),
            ("spo2_low", "lowestSpo2Value"),
            ("altitude_avg", "averageMonitoringEnvironmentAltitude"),
        ] {
            setf(row, col, num(r, key));
        }
        let total = r
            .get("allDayStress")
            .and_then(|s| s.get("aggregatorList"))
            .and_then(|a| a.as_array())
            .and_then(|a| {
                a.iter()
                    .find(|x| x.get("type").and_then(|t| t.as_str()) == Some("TOTAL"))
            });
        if let Some(t) = total {
            for (col, key) in [
                ("stress_avg", "averageStressLevel"),
                ("stress_max", "maxStressLevel"),
                ("rest_s", "restDuration"),
                ("low_s", "lowDuration"),
                ("med_s", "mediumDuration"),
                ("high_s", "highDuration"),
                ("activity_s", "activityDuration"),
                ("uncat_s", "uncategorizedDuration"),
            ] {
                setf(row, col, num(t, key));
            }
        }
        if let Some(bb) = r.get("bodyBattery") {
            setf(row, "bb_charged", num(bb, "chargedValue"));
            setf(row, "bb_drained", num(bb, "drainedValue"));
            if let Some(list) = bb.get("bodyBatteryStatList").and_then(|l| l.as_array()) {
                for s in list {
                    let col = match s.get("bodyBatteryStatType").and_then(|t| t.as_str()) {
                        Some("HIGHEST") => "bb_high",
                        Some("LOWEST") => "bb_low",
                        Some("SLEEPEND") => "bb_wake",
                        _ => continue,
                    };
                    setf(row, col, num(s, "statsValue"));
                }
            }
        }
        if let Some(resp) = r.get("respiration") {
            setf(
                row,
                "resp_awake_avg",
                num(resp, "avgWakingRespirationValue"),
            );
            setf(row, "resp_low", num(resp, "lowestRespirationValue"));
            setf(row, "resp_high", num(resp, "highestRespirationValue"));
        }
    }

    fn sleep_json(&mut self, r: &J) {
        let d = day(r, "calendarDate");
        let mut used = false;
        if let (Some(d), Some(start), Some(end)) = (
            d,
            time_ms(r, "sleepStartTimestampGMT"),
            time_ms(r, "sleepEndTimestampGMT"),
        ) {
            used = true;
            let id = format!("main:{start}");
            if let Entry::Vacant(slot) = self.sleeps.entry(id.clone()) {
                let mut row = Row::new();
                row.insert("id", Val::S(id.clone()));
                setf(&mut row, "day", Some(d as f64));
                setf(&mut row, "kind", Some(0.0));
                setf(&mut row, "start", Some(start as f64));
                setf(&mut row, "end", Some(end as f64));
                sets(
                    &mut row,
                    "confirmation",
                    text(r, "sleepWindowConfirmationType"),
                );
                let deep = int(r, "deepSleepSeconds");
                let light = int(r, "lightSleepSeconds");
                let rem = int(r, "remSleepSeconds");
                let total = [deep, light, rem]
                    .iter()
                    .flatten()
                    .copied()
                    .reduce(|a, b| a + b);
                setf(&mut row, "sleep_s", total);
                setf(&mut row, "deep_s", deep);
                setf(&mut row, "light_s", light);
                setf(&mut row, "rem_s", rem);
                setf(&mut row, "awake_s", int(r, "awakeSleepSeconds"));
                setf(&mut row, "unmeasurable_s", int(r, "unmeasurableSeconds"));
                setf(&mut row, "awake_count", int(r, "awakeCount"));
                setf(&mut row, "restless_count", int(r, "restlessMomentCount"));
                setf(&mut row, "avg_stress", num(r, "avgSleepStress"));
                setf(&mut row, "resp_avg", num(r, "averageRespiration"));
                setf(&mut row, "resp_low", num(r, "lowestRespiration"));
                setf(&mut row, "resp_high", num(r, "highestRespiration"));
                if let Some(s) = r.get("spo2SleepSummary") {
                    setf(&mut row, "spo2_avg", num(s, "averageSPO2"));
                    setf(&mut row, "spo2_low", num(s, "lowestSPO2"));
                    setf(&mut row, "avg_hr", num(s, "averageHR"));
                }
                sets(
                    &mut row,
                    "breathing_disruption",
                    text(r, "breathingDisruptionSeverity"),
                );
                slot.insert(row);
            }
        }
        if let (Some(d), Some(sc)) = (d, r.get("sleepScores")) {
            used = true;
            let row = self.day_row(d);
            for (col, key) in [
                ("sleep_score", "overallScore"),
                ("sleep_quality_score", "qualityScore"),
                ("sleep_duration_score", "durationScore"),
                ("sleep_recovery_score", "recoveryScore"),
                ("sleep_deep_score", "deepScore"),
                ("sleep_rem_score", "remScore"),
                ("sleep_light_score", "lightScore"),
                ("sleep_awakenings_score", "awakeningsCountScore"),
                ("sleep_awake_time_score", "awakeTimeScore"),
                ("sleep_restfulness_score", "restfulnessScore"),
                ("sleep_interruptions_score", "interruptionsScore"),
            ] {
                setf(row, col, num(sc, key));
            }
            sets(row, "sleep_feedback", text(sc, "feedback"));
        }
        if let Some(naps) = r.get("napList").and_then(|n| n.as_array()) {
            for nap in naps {
                let (Some(nd), Some(start), Some(end)) = (
                    day(nap, "calendarDate").or(d),
                    time_ms(nap, "napStartTimestampGMT"),
                    time_ms(nap, "napEndTimestampGMT"),
                ) else {
                    self.report.skipped += 1;
                    continue;
                };
                used = true;
                let id = format!("nap:{start}");
                if self.sleeps.contains_key(&id) {
                    continue;
                }
                let mut row = Row::new();
                row.insert("id", Val::S(id.clone()));
                setf(&mut row, "day", Some(nd as f64));
                setf(&mut row, "kind", Some(1.0));
                setf(&mut row, "start", Some(start as f64));
                setf(&mut row, "end", Some(end as f64));
                setf(&mut row, "sleep_s", int(nap, "napTimeSec"));
                self.sleeps.insert(id, row);
            }
        }
        if !used {
            self.report.skipped += 1;
        }
    }

    fn health_status(&mut self, r: &J) {
        let (Some(d), Some(metrics)) = (
            day(r, "calendarDate"),
            r.get("metrics").and_then(|m| m.as_array()),
        ) else {
            self.report.skipped += 1;
            return;
        };
        let row = self.day_row(d);
        for m in metrics {
            let (v, lo, hi) = match m.get("type").and_then(|t| t.as_str()) {
                Some("HRV") => ("hs_hrv", "hs_hrv_lo", "hs_hrv_hi"),
                Some("HR") => ("hs_hr", "hs_hr_lo", "hs_hr_hi"),
                Some("RESPIRATION") => ("hs_resp", "hs_resp_lo", "hs_resp_hi"),
                Some("SPO2") => ("hs_spo2", "hs_spo2_lo", "hs_spo2_hi"),
                Some("SKIN_TEMP_C") => ("hs_skin", "hs_skin_lo", "hs_skin_hi"),
                _ => continue,
            };
            setf(row, v, num(m, "value"));
            let status = m.get("status").and_then(|s| s.as_str()).unwrap_or("");
            let (l, h) = (num(m, "baselineLowerLimit"), num(m, "baselineUpperLimit"));
            let zero = l.unwrap_or(0.0) == 0.0 && h.unwrap_or(0.0) == 0.0;
            if status != "ONBOARDING" && status != "UNKNOWN" && !zero {
                setf(row, lo, l);
                setf(row, hi, h);
            }
        }
    }

    /// Whether a record with timestamp `ts` for `day` replaces the one kept so far (`earliest` keeps the smallest).
    fn pick(&mut self, handler: &'static str, d: i32, ts: i64, earliest: bool) -> bool {
        match self.picked.get(&(handler, d)) {
            Some(&old) if (earliest && old <= ts) || (!earliest && old >= ts) => false,
            _ => {
                self.picked.insert((handler, d), ts);
                true
            }
        }
    }

    fn readiness(&mut self, r: &J) {
        let Some(d) = day(r, "calendarDate") else {
            self.report.skipped += 1;
            return;
        };
        let ts = time_ms(r, "timestamp").unwrap_or(i64::MAX);
        if !self.pick("readiness", d, ts, true) {
            return;
        }
        let row = self.day_row(d);
        let cols = [
            ("tr_score", "score"),
            ("tr_sleep_pct", "sleepScoreFactorPercent"),
            ("tr_recovery_pct", "recoveryTimeFactorPercent"),
            ("tr_acwr_pct", "acwrFactorPercent"),
            ("tr_stress_hist_pct", "stressHistoryFactorPercent"),
            ("tr_hrv_pct", "hrvFactorPercent"),
            ("tr_sleep_hist_pct", "sleepHistoryFactorPercent"),
            ("tr_acute_load", "acuteLoad"),
            ("tr_recovery_min", "recoveryTime"),
        ];
        row.remove("tr_level");
        sets(row, "tr_level", text(r, "level"));
        for (col, key) in cols {
            row.remove(col);
            setf(row, col, num(r, key));
        }
    }

    fn maxmet(&mut self, r: &J) {
        let Some(d) = day(r, "calendarDate") else {
            self.report.skipped += 1;
            return;
        };
        if r.get("sport").and_then(|s| s.as_str()) != Some("RUNNING") {
            return;
        }
        let Some(v) = num(r, "vo2MaxValue") else {
            self.report.skipped += 1;
            return;
        };
        let ts = time_ms(r, "updateTimestamp").unwrap_or(i64::MIN);
        if self.pick("maxmet", d, ts, false) {
            setf(self.day_row(d), "vo2max", Some(v));
        }
    }

    fn acclim(&mut self, r: &J) {
        let Some(d) = day(r, "calendarDate") else {
            self.report.skipped += 1;
            return;
        };
        let ts = time_ms(r, "timestamp").unwrap_or(i64::MIN);
        if !self.pick("acclim", d, ts, false) {
            return;
        }
        let row = self.day_row(d);
        row.remove("heat_acclim_pct");
        row.remove("altitude_acclim_m");
        setf(row, "heat_acclim_pct", num(r, "heatAcclimationPercentage"));
        setf(row, "altitude_acclim_m", num(r, "altitudeAcclimation"));
    }

    fn lifestyle(&mut self, r: &J) {
        let Some(list) = r.get("dailyLogList").and_then(|l| l.as_array()) else {
            self.report.skipped += 1;
            return;
        };
        for e in list {
            let (Some(d), Some(name)) = (day(e, "calendarDate"), text(e, "behaviourName")) else {
                self.report.skipped += 1;
                continue;
            };
            let value = e
                .get("amount")
                .and_then(to_num)
                .map(num_text)
                .or_else(|| text(e, "status"));
            let mut row = Row::new();
            setf(&mut row, "day", Some(d as f64));
            row.insert("name", Val::S(name));
            sets(&mut row, "value", value);
            row.insert("source", Val::S("garmin_lifestyle".into()));
            self.tags.push(row);
        }
    }

    fn activities_json(&mut self, v: &J) {
        let mut out = vec![];
        flatten(v, &mut out);
        for s in out {
            if s.get("parent").and_then(|p| p.as_bool()) == Some(true) {
                continue;
            }
            let id = match s.get("activityId") {
                Some(J::Number(n)) => n
                    .as_i64()
                    .map(|i| i.to_string())
                    .or_else(|| n.as_f64().map(|f| (f.round() as i64).to_string())),
                Some(J::String(x)) => Some(x.clone()),
                _ => None,
            };
            let (Some(id), Some(start)) = (id, time_ms(s, "startTimeGmt")) else {
                self.report.skipped += 1;
                continue;
            };
            let id = format!("garmin:{id}");
            if self.activities.contains_key(&id) {
                continue;
            }
            let mut row = Row::new();
            row.insert("id", Val::S(id.clone()));
            setf(&mut row, "start", Some(start as f64));
            setf(
                &mut row,
                "end",
                num(s, "duration").map(|d| (start as f64 + d).round()),
            );
            let off =
                time_ms(s, "startTimeLocal").map(|l| ((l - start) as f64 / MS_MIN as f64).round());
            setf(&mut row, "utc_offset_min", off);
            let type_key = match s.get("activityType") {
                Some(J::String(t)) => Some(t.clone()),
                Some(o @ J::Object(_)) => text(o, "typeKey"),
                _ => None,
            };
            sets(
                &mut row,
                "sport",
                Some(sport(type_key.as_deref().unwrap_or("")).to_string()),
            );
            sets(&mut row, "source_sport", type_key);
            sets(&mut row, "name", text(s, "name"));
            setf(&mut row, "load", num(s, "activityTrainingLoad"));
            setf(&mut row, "avg_hr", num(s, "avgHr"));
            setf(&mut row, "max_hr", num(s, "maxHr"));
            self.activities.insert(id.clone(), row);
            self.act_starts.push((start, id));
            self.act_dirty = true;
        }
    }
}

/// effort-curves `flatten`: arrays, `summarizedActivitiesExport` wrappers and objects with an `activityId`.
fn flatten<'a>(v: &'a J, out: &mut Vec<&'a J>) {
    match v {
        J::Array(a) => a.iter().for_each(|x| flatten(x, out)),
        J::Object(o) => {
            if let Some(x) = o.get("summarizedActivitiesExport") {
                flatten(x, out);
            } else if o.contains_key("activityId") {
                out.push(v);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch() {
        let k = |p: &str| json_kind(p);
        assert_eq!(
            k("DI_CONNECT/DI-Connect-Aggregator/UDSFile_2026-07-28_2026-11-05.json"),
            JsonKind::Uds
        );
        assert_eq!(
            k("DI_CONNECT\\DI-Connect-Wellness\\2026-07-29_2026-11-06_1_sleepData.json"),
            JsonKind::SleepJson
        );
        assert_eq!(
            k("x/TrainingReadinessDTO_20260913_20261222_1.json"),
            JsonKind::Readiness
        );
        assert_eq!(
            k("x/MetricsHeatAltitudeAcclimation_1.json"),
            JsonKind::Acclim
        );
        assert_eq!(k("x/MetricsMaxMetData_1.json"), JsonKind::Maxmet);
        assert_eq!(k("x/1_LifestyleLogging.json"), JsonKind::Lifestyle);
        assert_eq!(k("x/1_healthStatusData.json"), JsonKind::HealthStatus);
        assert_eq!(
            k("x/a@b.com_0_summarizedActivities.json"),
            JsonKind::ActivitiesJson
        );
        assert_eq!(k("x/HydrationLogFile_1.json"), JsonKind::Ignored);
        assert_eq!(k("x/UDSFile_1.csv"), JsonKind::Ignored);
    }
}
