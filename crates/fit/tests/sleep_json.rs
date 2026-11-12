//! A sleepData record becomes a main-sleep row, its nap a nap row, and its scores the day's sleep scores.

use wellness_lab_core::time::parse_iso;
use wellness_lab_fit::{col, Column, ExportBuilder, JsonKind};

const SLEEP: &str = r#"[{"sleepStartTimestampGMT": "2026-07-30T05:54:27.0", "sleepEndTimestampGMT": "2026-07-30T14:45:27.0",
 "calendarDate": "2026-07-30", "sleepWindowConfirmationType": "ENHANCED_CONFIRMED_FINAL",
 "napList": [{"napTimeSec": 840, "napStartTimestampGMT": "2026-07-31T01:54:00.0",
              "napEndTimestampGMT": "2026-07-31T02:08:00.0", "calendarDate": "2026-07-30"}],
 "deepSleepSeconds": 6360, "lightSleepSeconds": 17220, "remSleepSeconds": 5100, "awakeSleepSeconds": 180,
 "unmeasurableSeconds": 3000, "averageRespiration": 14.0, "lowestRespiration": 9.0, "highestRespiration": 20.0,
 "retro": false, "awakeCount": 0, "avgSleepStress": 14.07,
 "spo2SleepSummary": {"averageSPO2": 96.71, "averageHR": 54.0, "lowestSPO2": 92},
 "breathingDisruptionSeverity": "NONE",
 "sleepScores": {"overallScore": 91, "qualityScore": 88, "durationScore": 100, "recoveryScore": 84, "deepScore": 100,
   "remScore": 74, "lightScore": 87, "awakeningsCountScore": 100, "awakeTimeScore": 100, "combinedAwakeScore": 100,
   "restfulnessScore": 74, "interruptionsScore": 94, "feedback": "POSITIVE_OPTIMAL_STRUCTURE"},
 "restlessMomentCount": 51},
 {"retro": false}]"#;

fn val(c: Option<&Column>, i: usize) -> Option<f64> {
    match c? {
        Column::I32(v) => v[i].map(|x| x as f64),
        Column::I64(v) => v[i].map(|x| x as f64),
        Column::F32(v) => v[i].map(|x| x as f64),
        Column::Str(_) => panic!("text"),
    }
}

fn txt(c: Option<&Column>, i: usize) -> Option<String> {
    match c? {
        Column::Str(v) => v[i].clone(),
        _ => panic!("not text"),
    }
}

#[test]
fn sleep_row_nap_and_scores() {
    let mut b = ExportBuilder::new();
    let k = b
        .add_json(
            "DI_CONNECT/DI-Connect-Wellness/2026-07-29_2026-11-06_1_sleepData.json",
            SLEEP,
        )
        .unwrap();
    assert_eq!(k, JsonKind::SleepJson);
    let (t, rep) = b.finish();
    assert_eq!(rep.skipped, 1);
    let s = &t.sleeps;
    assert_eq!(wellness_lab_fit::rows(s), 2);
    let day = parse_iso("2026-07-30").unwrap() as f64;
    let start = 1_785_390_867_000i64; // 2026-07-30T05:54:27Z
    assert_eq!(txt(col(s, "id"), 0), Some(format!("main:{start}")));
    assert_eq!(val(col(s, "day"), 0), Some(day));
    assert_eq!(val(col(s, "kind"), 0), Some(0.0));
    assert_eq!(val(col(s, "start"), 0), Some(start as f64));
    assert_eq!(
        val(col(s, "end"), 0),
        Some((start + (8 * 3600 + 51 * 60) * 1000) as f64)
    );
    assert_eq!(
        txt(col(s, "confirmation"), 0).as_deref(),
        Some("ENHANCED_CONFIRMED_FINAL")
    );
    assert_eq!(val(col(s, "sleep_s"), 0), Some(6360.0 + 17220.0 + 5100.0));
    assert_eq!(val(col(s, "deep_s"), 0), Some(6360.0));
    assert_eq!(val(col(s, "light_s"), 0), Some(17220.0));
    assert_eq!(val(col(s, "rem_s"), 0), Some(5100.0));
    assert_eq!(val(col(s, "awake_s"), 0), Some(180.0));
    assert_eq!(val(col(s, "unmeasurable_s"), 0), Some(3000.0));
    assert_eq!(val(col(s, "awake_count"), 0), Some(0.0));
    assert_eq!(val(col(s, "restless_count"), 0), Some(51.0));
    assert!((val(col(s, "avg_stress"), 0).unwrap() - 14.07).abs() < 1e-5);
    assert_eq!(val(col(s, "avg_hr"), 0), Some(54.0));
    assert_eq!(val(col(s, "rhr"), 0), None);
    assert_eq!(val(col(s, "resp_avg"), 0), Some(14.0));
    assert_eq!(val(col(s, "resp_low"), 0), Some(9.0));
    assert_eq!(val(col(s, "resp_high"), 0), Some(20.0));
    assert!((val(col(s, "spo2_avg"), 0).unwrap() - 96.71).abs() < 1e-4);
    assert_eq!(val(col(s, "spo2_low"), 0), Some(92.0));
    assert_eq!(
        txt(col(s, "breathing_disruption"), 0).as_deref(),
        Some("NONE")
    );
    // the nap
    let nap = 1_785_462_840_000i64; // 2026-07-31T01:54:00Z
    assert_eq!(txt(col(s, "id"), 1), Some(format!("nap:{nap}")));
    assert_eq!(val(col(s, "kind"), 1), Some(1.0));
    assert_eq!(val(col(s, "day"), 1), Some(day));
    assert_eq!(val(col(s, "end"), 1), Some((nap + 840_000) as f64));
    assert_eq!(val(col(s, "sleep_s"), 1), Some(840.0));
    // the day's scores
    let d = &t.days;
    assert_eq!(val(col(d, "day"), 0), Some(day));
    for (c, x) in [
        ("sleep_score", 91.0),
        ("sleep_quality_score", 88.0),
        ("sleep_duration_score", 100.0),
        ("sleep_recovery_score", 84.0),
        ("sleep_deep_score", 100.0),
        ("sleep_rem_score", 74.0),
        ("sleep_light_score", 87.0),
        ("sleep_awakenings_score", 100.0),
        ("sleep_awake_time_score", 100.0),
        ("sleep_restfulness_score", 74.0),
        ("sleep_interruptions_score", 94.0),
    ] {
        assert_eq!(val(col(d, c), 0), Some(x), "{c}");
    }
    assert_eq!(
        txt(col(d, "sleep_feedback"), 0).as_deref(),
        Some("POSITIVE_OPTIMAL_STRUCTURE")
    );
}
