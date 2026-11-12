//! HRV status files (type 68): five-minute values go to `hrv5` with the night's day, the summary to `days`.
//! Sleep files (type 49): stage changes become stages ending at the next change or at the end of the main sleep.

mod fitgen;

use fitgen::{hrv_status_fit, sleep_fit};
use wellness_lab_core::time::{parse_iso, MS_DAY, MS_HOUR};
use wellness_lab_fit::{col, Column, ExportBuilder, FitKind};

const UDS: &str = r#"[{"calendarDate": "2026-11-02", "wellnessStartTimeGmt": "2026-11-02T08:00:00.0",
  "wellnessStartTimeLocal": "2026-11-02T00:00:00.0"},
 {"calendarDate": "2026-11-03", "wellnessStartTimeGmt": "2026-11-03T08:00:00.0",
  "wellnessStartTimeLocal": "2026-11-03T00:00:00.0"}]"#;

#[test]
fn hrv_values_and_summary() {
    let mut b = ExportBuilder::new();
    b.add_json("x/UDSFile_1.json", UDS).unwrap();
    // 06:00 UTC on the 3rd is 22:00 local on the 2nd at -08:00
    let end = parse_iso("2026-11-03").unwrap() as i64 * MS_DAY + 6 * MS_HOUR;
    let bytes = hrv_status_fit(end, &[50.0, 55.5, 61.0]);
    assert_eq!(b.add_fit("h.fit", &bytes).unwrap(), FitKind::HrvFit);
    let (t, rep) = b.finish();
    assert_eq!(rep.fit_files["68"], 1);
    let day = parse_iso("2026-11-02").unwrap();

    let h = &t.series["hrv5"];
    assert_eq!(
        col(h, "t").unwrap(),
        &Column::I64(vec![
            Some(end - 900_000),
            Some(end - 600_000),
            Some(end - 300_000)
        ])
    );
    assert_eq!(col(h, "v").unwrap().f64s(), vec![50.0, 55.5, 61.0]);
    assert_eq!(col(h, "day").unwrap(), &Column::I32(vec![Some(day); 3]));

    let d = &t.days;
    let i = col(d, "day")
        .unwrap()
        .f64s()
        .iter()
        .position(|&x| x == day as f64)
        .unwrap();
    let get = |c: &str| col(d, c).unwrap().f64s()[i];
    assert_eq!(get("hrv_night"), 61.0);
    assert_eq!(get("hrv_week"), 60.0);
    assert_eq!(get("hrv_5min_high"), 80.0);
    assert_eq!(get("hrv_low_upper"), 40.0);
    assert_eq!(get("hrv_bal_lo"), 45.0);
    assert_eq!(get("hrv_bal_hi"), 70.0);
    match col(d, "hrv_status").unwrap() {
        Column::Str(v) => assert_eq!(v[i].as_deref(), Some("LOW")),
        _ => panic!(),
    }
}

#[test]
fn sleep_stages() {
    let sleep = r#"[{"sleepStartTimestampGMT": "2026-11-03T06:00:00.0", "sleepEndTimestampGMT": "2026-11-03T14:00:00.0",
      "calendarDate": "2026-11-03"}]"#;
    let mut b = ExportBuilder::new();
    b.add_json("x/1_sleepData.json", sleep).unwrap();
    let start = parse_iso("2026-11-03").unwrap() as i64 * MS_DAY + 6 * MS_HOUR;
    let bytes = sleep_fit(
        start,
        &[(10, "awake"), (60, "light"), (90, "deep"), (20, "rem")],
    );
    assert_eq!(b.add_fit("s.fit", &bytes).unwrap(), FitKind::SleepFit);
    let (t, _) = b.finish();
    let s = &t.stages;
    let m = 60_000;
    assert_eq!(
        col(s, "start").unwrap(),
        &Column::I64(vec![
            Some(start),
            Some(start + 10 * m),
            Some(start + 70 * m),
            Some(start + 160 * m)
        ])
    );
    // the last change lasts until the end of the main sleep
    assert_eq!(
        col(s, "end").unwrap(),
        &Column::I64(vec![
            Some(start + 10 * m),
            Some(start + 70 * m),
            Some(start + 160 * m),
            Some(start + 8 * 60 * m)
        ])
    );
    assert_eq!(col(s, "stage").unwrap().f64s(), vec![3.0, 1.0, 0.0, 2.0]);
    let day = parse_iso("2026-11-03").unwrap();
    assert_eq!(col(s, "day").unwrap(), &Column::I32(vec![Some(day); 4]));
}
