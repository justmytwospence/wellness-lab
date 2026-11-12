//! Writes a small Garmin-export-shaped zip for tests and e2e: 14 days of daily summaries (UDS), sleepData,
//! TrainingReadinessDTO, LifestyleLogging and summarizedActivities JSON, plus monitoring_b, sleep (49), HRV status (68)
//! and one activity FIT (with `hrv` messages), nested in `DI-Connect-Uploaded-Files/UploadedFiles_0-_Part1.zip`.
//! Everything is synthetic and deterministic.
//!
//! `cargo run -p wellness-lab-fit --example make_fake_export -- <out.zip>`
#![allow(dead_code)]

#[path = "../tests/fitgen.rs"]
mod fitgen;

use serde_json::{json, Value};
use std::io::{Cursor, Write};
use std::path::Path;
use wellness_lab_core::time::{iso, parse_iso, MS_DAY, MS_HOUR, MS_MIN};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

const DAYS: i32 = 14;
const OFFSET_MIN: i64 = -420;

fn gmt(ms: i64) -> String {
    let day = ms.div_euclid(MS_DAY) as i32;
    let r = ms.rem_euclid(MS_DAY);
    format!(
        "{}T{:02}:{:02}:{:02}.0",
        iso(day),
        r / MS_HOUR,
        r / MS_MIN % 60,
        r / 1000 % 60
    )
}

/// UTC ms of local midnight of a day.
fn midnight(day: i32) -> i64 {
    day as i64 * MS_DAY - OFFSET_MIN * MS_MIN
}

fn json_files(first: i32) -> Vec<(String, Value)> {
    let range = format!("{}_{}", iso(first), iso(first + DAYS));
    let compact = |d: i32| iso(d).replace('-', "");
    let mut uds = vec![];
    let mut sleep = vec![];
    let mut readiness = vec![];
    let mut logs = vec![];
    for i in 0..DAYS {
        let d = first + i;
        let m = midnight(d);
        uds.push(json!({
            "calendarDate": iso(d), "totalSteps": 23_800 + 100 * i,
            "wellnessStartTimeGmt": gmt(m), "wellnessStartTimeLocal": gmt(d as i64 * MS_DAY),
            "wellnessEndTimeGmt": gmt(m + MS_DAY), "wellnessEndTimeLocal": gmt((d + 1) as i64 * MS_DAY),
            "minHeartRate": 44, "maxHeartRate": 150, "restingHeartRate": 50 + i % 3,
            "moderateIntensityMinutes": 10, "vigorousIntensityMinutes": 5,
            "averageSpo2Value": 95.0, "lowestSpo2Value": 89,
            "averageMonitoringEnvironmentAltitude": 2000.0,
            "allDayStress": {"aggregatorList": [{"type": "TOTAL", "averageStressLevel": 30 + i, "maxStressLevel": 90,
                "restDuration": 30000, "lowDuration": 20000, "mediumDuration": 10000, "highDuration": 2000,
                "activityDuration": 3000, "uncategorizedDuration": 4000}]},
            "bodyBattery": {"chargedValue": 60, "drainedValue": 55, "bodyBatteryStatList": [
                {"bodyBatteryStatType": "HIGHEST", "statsValue": 90},
                {"bodyBatteryStatType": "LOWEST", "statsValue": 20},
                {"bodyBatteryStatType": "SLEEPEND", "statsValue": 85}]},
            "respiration": {"avgWakingRespirationValue": 14.0, "highestRespirationValue": 20.0,
                "lowestRespirationValue": 9.0}
        }));
        // the night ending this morning: 23:00 to 07:00 local
        let (s, e) = (m - MS_HOUR, m + 7 * MS_HOUR);
        sleep.push(json!({
            "sleepStartTimestampGMT": gmt(s), "sleepEndTimestampGMT": gmt(e), "calendarDate": iso(d),
            "sleepWindowConfirmationType": "ENHANCED_CONFIRMED_FINAL",
            "deepSleepSeconds": 5400, "lightSleepSeconds": 16200, "remSleepSeconds": 6000, "awakeSleepSeconds": 1200,
            "unmeasurableSeconds": 0, "awakeCount": 1, "restlessMomentCount": 30, "avgSleepStress": 15.0,
            "averageRespiration": 14.0, "lowestRespiration": 10.0, "highestRespiration": 19.0, "retro": false,
            "spo2SleepSummary": {"averageSPO2": 95.0, "lowestSPO2": 90, "averageHR": 52.0},
            "sleepScores": {"overallScore": 80 + i % 10, "qualityScore": 80, "durationScore": 85, "recoveryScore": 75,
                "deepScore": 90, "remScore": 80, "lightScore": 85, "awakeningsCountScore": 90, "awakeTimeScore": 90,
                "restfulnessScore": 70, "interruptionsScore": 80, "feedback": "POSITIVE_OPTIMAL_STRUCTURE"}
        }));
        readiness.push(json!({
            "calendarDate": iso(d), "timestamp": gmt(m + 8 * MS_HOUR), "level": "MODERATE", "score": 60 + i,
            "sleepScoreFactorPercent": 70, "recoveryTimeFactorPercent": 90, "acwrFactorPercent": 100,
            "stressHistoryFactorPercent": 60, "hrvFactorPercent": 50, "sleepHistoryFactorPercent": 70,
            "acuteLoad": 300, "recoveryTime": 600
        }));
        logs.push(
            json!({"behaviourName": "Alcohol", "status": if i % 4 == 0 { "YES" } else { "NO" },
            "calendarDate": [2026, 10, 1 + i]}),
        );
    }
    let acts: Vec<Value> = [2, 7, 12]
        .iter()
        .map(|&i| {
            let s = midnight(first + i) + 17 * MS_HOUR;
            json!({"activityId": 1_000_000 + i, "name": "Run", "activityType": "running", "sportType": "RUNNING",
                "startTimeGmt": s as f64, "startTimeLocal": (s + OFFSET_MIN * MS_MIN) as f64, "duration": 1_800_000.0,
                "activityTrainingLoad": 120.0, "avgHr": 150.0, "maxHr": 172.0})
        })
        .collect();
    vec![
        (
            format!("DI_CONNECT/DI-Connect-Aggregator/UDSFile_{range}.json"),
            Value::Array(uds),
        ),
        (
            format!("DI_CONNECT/DI-Connect-Wellness/{range}_1_sleepData.json"),
            Value::Array(sleep),
        ),
        (
            format!(
                "DI_CONNECT/DI-Connect-Metrics/TrainingReadinessDTO_{}_{}_1.json",
                compact(first),
                compact(first + DAYS)
            ),
            Value::Array(readiness),
        ),
        (
            "DI_CONNECT/DI-Connect-Wellness/1_LifestyleLogging.json".into(),
            json!([{"dailyLogList": logs}]),
        ),
        (
            "DI_CONNECT/DI-Connect-Fitness/user@example.com_0_summarizedActivities.json".into(),
            json!([{"summarizedActivitiesExport": acts}]),
        ),
    ]
}

fn fit_files(first: i32) -> Vec<(String, Vec<u8>)> {
    let mut out = vec![];
    let mut n = 100;
    let mut name = || {
        n += 1;
        format!("user@example.com_{n}.fit")
    };
    for i in 0..DAYS {
        let d = first + i;
        let m = midnight(d);
        // two hours of monitoring from local midnight (07:00 UTC, so fitgen's monitoring_info local time, which is its
        // UTC start, has the same date)
        out.push((name(), fitgen::wellness_fit(m, 120)));
        let stages = [
            (20, "light"),
            (90, "deep"),
            (120, "light"),
            (30, "rem"),
            (10, "awake"),
            (120, "light"),
            (60, "rem"),
            (30, "light"),
        ];
        out.push((name(), fitgen::sleep_fit(m - MS_HOUR, &stages)));
        let values: Vec<f64> = (0..8).map(|k| 55.0 + (k + i) as f64 % 7.0).collect();
        out.push((name(), fitgen::hrv_status_fit(m + 7 * MS_HOUR, &values)));
    }
    let start = midnight(first + 2) + 17 * MS_HOUR;
    let rr: Vec<i64> = (0..600).map(|k| 400 + (k * 37 % 40)).collect();
    out.push((name(), fitgen::activity_fit(start, 120, &rr)));
    out
}

/// Write the fake export to `out`.
pub fn make_fake_export(out: &Path) -> std::io::Result<()> {
    let first = parse_iso("2026-10-01").unwrap();
    let opts = SimpleFileOptions::default();
    let mut inner = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in fit_files(first) {
        inner.start_file(name, opts)?;
        inner.write_all(&bytes)?;
    }
    let inner = inner.finish()?.into_inner();
    let mut z = ZipWriter::new(std::fs::File::create(out)?);
    for (name, v) in json_files(first) {
        z.start_file(name, opts)?;
        z.write_all(serde_json::to_string(&v)?.as_bytes())?;
    }
    z.start_file(
        "DI_CONNECT/DI-Connect-Uploaded-Files/UploadedFiles_0-_Part1.zip",
        opts,
    )?;
    z.write_all(&inner)?;
    z.start_file("DI_CONNECT/DI-Connect-User/user_profile.json", opts)?;
    z.write_all(b"{}")?;
    z.finish()?;
    Ok(())
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .expect("usage: make_fake_export <out.zip>");
    make_fake_export(Path::new(&out)).expect("write the fake export");
    println!("{out}");
}
