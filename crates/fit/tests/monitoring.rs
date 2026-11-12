//! A monitoring_b FIT built with the test writer: HR, stress quality codes, steps increments, `timestamp_16`
//! resolution across a 16-bit wrap, respiration and the undocumented altitude message 279.

mod fitgen;

use fitgen::{fit_ts, one, wellness_into, Base, FitWriter, FIT_EPOCH_S};
use wellness_lab_core::time::{parse_iso, MS_DAY};
use wellness_lab_fit::{col, Column, Columns, ExportBuilder, FitKind};

fn f64s(cols: &Columns, name: &str) -> Vec<f64> {
    col(cols, name).unwrap().f64s()
}

fn ts(cols: &Columns) -> Vec<i64> {
    match col(cols, "t").unwrap() {
        Column::I64(v) => v.iter().map(|x| x.unwrap()).collect(),
        _ => panic!("t is BIGINT"),
    }
}

#[test]
fn monitoring_series() {
    let day = parse_iso("2026-11-02").unwrap();
    let t0_ms = day as i64 * MS_DAY;
    let t0 = fit_ts(t0_ms) as i64;
    let mut w = FitWriter::new();
    wellness_into(&mut w, t0_ms, 10);
    // stress: activity (-2) and unmeasurable (-1)
    w.message(
        227,
        &[(0, Base::S16, one(-2)), (1, Base::U32, one(t0 + 600))],
    );
    w.message(
        227,
        &[(0, Base::S16, one(-1)), (1, Base::U32, one(t0 + 660))],
    );
    // a full timestamp 16 s before a 16-bit boundary, then two messages with only timestamp_16
    let tw = ((t0 + 7200) & !0xFFFF) + 0xFFF0;
    w.message(55, &[(253, Base::U32, one(tw)), (27, Base::U8, one(70))]);
    // a later full timestamp on another message (an event) is not the base of the monitoring timestamp_16
    w.message(
        21,
        &[
            (253, Base::U32, one(tw + 300)),
            (0, Base::Enum, one(0)),
            (1, Base::Enum, one(0)),
        ],
    );
    for (k, hr) in [(60, 71), (120, 72)] {
        w.message(
            55,
            &[
                (26, Base::U16, one((tw + k) & 0xFFFF)),
                (27, Base::U8, one(hr)),
            ],
        );
    }
    let bytes = w.to_bytes();

    let mut b = ExportBuilder::new();
    assert_eq!(b.add_fit("m.fit", &bytes).unwrap(), FitKind::Monitoring);
    let (t, rep) = b.finish();
    assert_eq!(rep.fit_files["monitoring_b"], 1);
    assert!(rep.errors.is_empty(), "{:?}", rep.errors);
    let ms = |s: i64| (s + FIT_EPOCH_S) * 1000;

    let hr = &t.series["hr"];
    let mut want_t: Vec<i64> = (0..10).map(|i| ms(t0 + i * 60)).collect();
    want_t.extend([ms(tw), ms(tw + 60), ms(tw + 120)]);
    want_t.sort();
    assert_eq!(ts(hr), want_t);
    let v = f64s(hr, "v");
    let by_t: std::collections::BTreeMap<i64, f64> = ts(hr).into_iter().zip(v).collect();
    for i in 0..10 {
        assert_eq!(by_t[&ms(t0 + i * 60)], 60.0 + i as f64);
    }
    // timestamp_16 resolved across the wrap
    assert_eq!(by_t[&ms(tw + 60)], 71.0);
    assert_eq!(by_t[&ms(tw + 120)], 72.0);

    let st = &t.series["stress"];
    assert_eq!(rows(st), 12);
    let (sv, sq) = (f64s(st, "v"), f64s(st, "q"));
    for i in 0..10 {
        assert_eq!((sv[i], sq[i]), (20.0 + i as f64, 0.0));
    }
    assert!(sv[10].is_nan() && sq[10] == 2.0, "-2 is activity");
    assert!(sv[11].is_nan() && sq[11] == 1.0, "-1 is unmeasurable");
    assert!(matches!(col(st, "q"), Some(Column::I32(_))));

    let steps = &t.series["steps"];
    assert_eq!(
        ts(steps),
        (0..10).map(|i| ms(t0 + i * 60)).collect::<Vec<_>>()
    );
    let mut want = vec![0.0];
    want.extend([200.0; 9]);
    assert_eq!(f64s(steps, "v"), want);

    let resp = &t.series["resp"];
    assert_eq!(f64s(resp, "v"), vec![15.0; 10]);

    let alt = &t.series["altitude"];
    assert_eq!(
        ts(alt),
        (0..10)
            .step_by(2)
            .map(|i| ms(t0 + i * 60))
            .collect::<Vec<_>>()
    );
    assert_eq!(f64s(alt, "v"), vec![2000.0; 5]);
    assert_eq!(rep.series_rows["hr"], 13);

    // the creator device of the file day
    let d = &t.devices;
    assert_eq!(rows(d), 1);
    assert_eq!(f64s(d, "first_day"), vec![day as f64]);
    match col(d, "serial").unwrap() {
        Column::Str(v) => assert_eq!(v[0].as_deref(), Some("12345")),
        _ => panic!(),
    }
}

fn rows(c: &Columns) -> usize {
    wellness_lab_fit::rows(c)
}

#[test]
fn steps_from_the_activity_type_component() {
    // `current_activity_type_intensity` (field 24: walking, intensity 3) instead of `activity_type`: the steps
    // sub-field then depends on the component, which the decoder may expand before or after `cycles`
    let t0_ms = parse_iso("2026-11-03").unwrap() as i64 * MS_DAY;
    let t0 = fit_ts(t0_ms) as i64;
    let mut w = FitWriter::new();
    fitgen::file_id(&mut w, 32, t0 as u32);
    w.message(103, &[(253, Base::U32, one(t0)), (0, Base::U32, one(t0))]);
    for (i, cum) in [100, 250, 240, 400].into_iter().enumerate() {
        w.message(
            55,
            &[
                (253, Base::U32, one(t0 + 60 + 60 * i as i64)),
                (3, Base::U32, one(cum)),
                (24, Base::U8, one(6 | (3 << 5))),
            ],
        );
    }
    let mut b = ExportBuilder::new();
    b.add_fit("m.fit", &w.to_bytes()).unwrap();
    let (t, _) = b.finish();
    let steps = &t.series["steps"];
    // increments of the cumulative count over its highest value so far (a lower sample adds nothing)
    assert_eq!(f64s(steps, "v"), vec![100.0, 150.0, 0.0, 150.0]);
    assert_eq!(f64s(steps, "intensity"), vec![3.0; 4]);
}

#[test]
fn undecodable_files_are_counted() {
    let mut b = ExportBuilder::new();
    assert!(b.add_fit("bad.fit", b"not a fit file").is_err());
    let (_, rep) = b.finish();
    assert_eq!(rep.errors.len(), 1);
}
