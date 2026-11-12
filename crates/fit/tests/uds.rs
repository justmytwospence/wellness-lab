//! A daily summary (UDS) record becomes a `days` row and sets the day's UTC offset.

use wellness_lab_core::time::parse_iso;
use wellness_lab_fit::{col, Column, ExportBuilder, JsonKind};

const UDS: &str = r#"[{"userProfilePK": 1, "calendarDate": "2026-11-02", "totalSteps": 6594,
 "wellnessStartTimeGmt": "2026-11-02T08:00:00.0", "wellnessStartTimeLocal": "2026-11-02T00:00:00.0",
 "moderateIntensityMinutes": 1, "vigorousIntensityMinutes": 2, "minHeartRate": 43, "maxHeartRate": 133,
 "restingHeartRate": 48, "averageSpo2Value": 95.0, "lowestSpo2Value": 88,
 "allDayStress": {"aggregatorList": [
   {"type": "AWAKE", "averageStressLevel": 42, "maxStressLevel": 97},
   {"type": "TOTAL", "averageStressLevel": 34, "maxStressLevel": 97, "restDuration": 34800, "activityDuration": 4740,
    "uncategorizedDuration": 4920, "lowDuration": 23640, "mediumDuration": 14580, "highDuration": 3300}]},
 "bodyBattery": {"chargedValue": 61, "drainedValue": 46, "bodyBatteryStatList": [
   {"bodyBatteryStatType": "HIGHEST", "statsValue": 65}, {"bodyBatteryStatType": "LOWEST", "statsValue": 7},
   {"bodyBatteryStatType": "SLEEPEND", "statsValue": 64}, {"bodyBatteryStatType": "ENDOFDAY", "statsValue": 22}]},
 "averageMonitoringEnvironmentAltitude": 1067.0,
 "respiration": {"avgWakingRespirationValue": 13.0, "highestRespirationValue": 22.0, "lowestRespirationValue": 6.0}},
 {"retro": false}]"#;

fn f(c: Option<&Column>) -> Option<f32> {
    match c? {
        Column::F32(v) => v[0],
        _ => panic!("not FLOAT"),
    }
}

fn i(c: Option<&Column>) -> Option<i32> {
    match c? {
        Column::I32(v) => v[0],
        _ => panic!("not INTEGER"),
    }
}

#[test]
fn uds_row_and_offset() {
    let mut b = ExportBuilder::new();
    let k = b
        .add_json(
            "DI_CONNECT/DI-Connect-Aggregator/UDSFile_2026-07-28_2026-11-05.json",
            UDS,
        )
        .unwrap();
    assert_eq!(k, JsonKind::Uds);
    let (t, rep) = b.finish();
    let d = &t.days;
    assert_eq!(wellness_lab_fit::rows(d), 1);
    assert_eq!(rep.skipped, 1);
    assert_eq!(rep.json_files["uds"], 1);
    let day = parse_iso("2026-11-02").unwrap();
    assert_eq!(rep.days, Some((day, day)));
    assert_eq!(i(col(d, "day")), Some(day));
    assert_eq!(i(col(d, "utc_offset_min")), Some(-480));
    assert_eq!(f(col(d, "rhr")), Some(48.0));
    assert_eq!(f(col(d, "hr_min")), Some(43.0));
    assert_eq!(f(col(d, "hr_max")), Some(133.0));
    assert_eq!(i(col(d, "steps")), Some(6594));
    assert_eq!(i(col(d, "mod_min")), Some(1));
    assert_eq!(i(col(d, "vig_min")), Some(2));
    assert_eq!(f(col(d, "stress_avg")), Some(34.0));
    assert_eq!(f(col(d, "stress_max")), Some(97.0));
    assert_eq!(i(col(d, "rest_s")), Some(34800));
    assert_eq!(i(col(d, "low_s")), Some(23640));
    assert_eq!(i(col(d, "med_s")), Some(14580));
    assert_eq!(i(col(d, "high_s")), Some(3300));
    assert_eq!(i(col(d, "activity_s")), Some(4740));
    assert_eq!(i(col(d, "uncat_s")), Some(4920));
    assert_eq!(f(col(d, "bb_charged")), Some(61.0));
    assert_eq!(f(col(d, "bb_drained")), Some(46.0));
    assert_eq!(f(col(d, "bb_high")), Some(65.0));
    assert_eq!(f(col(d, "bb_low")), Some(7.0));
    assert_eq!(f(col(d, "bb_wake")), Some(64.0));
    assert_eq!(f(col(d, "spo2_avg")), Some(95.0));
    assert_eq!(f(col(d, "spo2_low")), Some(88.0));
    assert_eq!(f(col(d, "resp_awake_avg")), Some(13.0));
    assert_eq!(f(col(d, "resp_low")), Some(6.0));
    assert_eq!(f(col(d, "resp_high")), Some(22.0));
    assert_eq!(f(col(d, "altitude_avg")), Some(1067.0));
    assert_eq!(f(col(d, "sleep_score")), None);
    assert_eq!(f(col(d, "rhr7")), None);
}

#[test]
fn ignored_and_broken_json() {
    let mut b = ExportBuilder::new();
    assert_eq!(
        b.add_json(
            "DI_CONNECT/DI-Connect-Aggregator/HydrationLogFile_1.json",
            "["
        )
        .unwrap(),
        JsonKind::Ignored
    );
    assert!(b.add_json("x/UDSFile_1.json", "[").is_err());
    let (_, rep) = b.finish();
    assert_eq!(rep.json_files["ignored"], 1);
    assert_eq!(rep.errors.len(), 1);
}
