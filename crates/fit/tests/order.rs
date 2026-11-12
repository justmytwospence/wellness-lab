//! JSON first: an activity FIT before the summaries is refused; after them it is matched by start time and its
//! beat-to-beat intervals kept.

mod fitgen;

use fitgen::activity_fit;
use wellness_lab_fit::{col, Column, ExportBuilder, FitError, FitKind};

const START: i64 = 1_793_833_608_000; // 2026-11-04T21:46:48Z

fn summaries() -> String {
    format!(
        r#"[{{"summarizedActivitiesExport": [
          {{"activityId": 24969979708, "name": "Run", "activityType": "running", "startTimeGmt": {s}.0,
            "startTimeLocal": {l}.0, "duration": 3270728.5, "activityTrainingLoad": 186.1, "avgHr": 155.0, "maxHr": 172.0}},
          {{"activityId": 1, "activityType": {{"typeKey": "multi_sport"}}, "parent": true, "startTimeGmt": {s}.0}},
          {{"activityId": 2, "activityType": "yoga", "startTimeGmt": {o}.0, "startTimeLocal": {o}.0, "duration": 60000}}
        ]}}]"#,
        s = START,
        l = START - 480 * 60_000,
        o = START + 86_400_000
    )
}

const UDS: &str = r#"[{"calendarDate": "2026-11-04", "wellnessStartTimeGmt": "2026-11-04T08:00:00.0",
  "wellnessStartTimeLocal": "2026-11-04T00:00:00.0"}]"#;

#[test]
fn activity_fit_needs_json_first() {
    let rr: Vec<i64> = (0..600).map(|i| 800 + i % 50).collect();
    let fit = activity_fit(START + 3000, 30, &rr);
    let mut b = ExportBuilder::new();
    assert!(matches!(b.add_fit("a.fit", &fit), Err(FitError::JsonFirst)));
    b.add_json("x/UDSFile_1.json", UDS).unwrap();
    b.add_json("x/a_0_summarizedActivities.json", &summaries())
        .unwrap();
    assert_eq!(b.add_fit("a.fit", &fit).unwrap(), FitKind::ActivityFit);
    // a second recording of the same start finds no unused summary
    assert_eq!(b.add_fit("b.fit", &fit).unwrap(), FitKind::ActivityFit);
    let (t, rep) = b.finish();
    assert_eq!(rep.skipped, 1);
    assert_eq!(rep.activities_with_rr, 1);
    let rr_out = &t.rr["garmin:24969979708"];
    assert_eq!(rr_out.len(), 600);
    assert_eq!(rr_out[0], 800.0);
    assert_eq!(rr_out[49], 849.0);

    let a = &t.activities;
    assert_eq!(
        wellness_lab_fit::rows(a),
        2,
        "the multisport parent is skipped"
    );
    match col(a, "id").unwrap() {
        Column::Str(v) => assert_eq!(v[0].as_deref(), Some("garmin:24969979708")),
        _ => panic!(),
    }
    assert_eq!(col(a, "start").unwrap().f64s()[0], START as f64);
    assert_eq!(col(a, "end").unwrap().f64s()[0], (START + 3_270_729) as f64);
    assert_eq!(col(a, "utc_offset_min").unwrap().f64s(), vec![-480.0, 0.0]);
    match col(a, "sport").unwrap() {
        Column::Str(v) => assert_eq!(
            v.clone(),
            vec![Some("run".to_string()), Some("other".to_string())]
        ),
        _ => panic!(),
    }
    assert_eq!(col(a, "rr_beats").unwrap().f64s(), vec![600.0, 0.0]);
    assert_eq!(col(a, "max_hr").unwrap().f64s()[0], 172.0);
}

#[test]
fn padding_and_short_recordings() {
    let mut b = ExportBuilder::new();
    b.add_json("x/UDSFile_1.json", UDS).unwrap();
    b.add_json("x/a_0_summarizedActivities.json", &summaries())
        .unwrap();
    // 0xFFFF pads the five-slot hrv arrays and is no beat; Garmin's 5000 ms gap marker is kept
    let mut rr: Vec<i64> = vec![900; 503];
    rr[10] = 0xFFFF;
    rr[20] = 5000;
    b.add_fit("a.fit", &activity_fit(START, 10, &rr)).unwrap();
    // under 500 beats: not kept
    b.add_fit("y.fit", &activity_fit(START + 86_400_000, 10, &[900; 100]))
        .unwrap();
    let (t, rep) = b.finish();
    assert_eq!(rep.activities_with_rr, 1);
    let out = &t.rr["garmin:24969979708"];
    // 503 slots less the 0xFFFF one; the two padding slots of the last message are dropped too
    assert_eq!(out.len(), 502);
    assert_eq!(out[10], 900.0);
    assert_eq!(out[19], 5000.0);
    assert_eq!(out.iter().filter(|&&x| x == 900.0).count(), 501);
}
