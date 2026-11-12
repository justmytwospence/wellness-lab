//! Every table and series the builder emits has exactly the columns and types of `schema/wellness-v1.json`, and
//! `Tables::from_data` / `to_data` round-trip `core::synth` data.

mod fitgen;

use serde_json::Value;
use wellness_lab_core::synth::synth;
use wellness_lab_core::time::{parse_iso, MS_DAY};
use wellness_lab_fit::{Column, Columns, ExportBuilder, Tables};

const SCHEMA: &str = include_str!("../../../schema/wellness-v1.json");

fn ty(c: &Column) -> &'static str {
    match c {
        Column::I32(_) => "INTEGER",
        Column::I64(_) => "BIGINT",
        Column::F32(_) => "FLOAT",
        Column::Str(_) => "VARCHAR",
    }
}

fn shape(cols: &Columns) -> Vec<(String, String)> {
    cols.iter()
        .map(|(n, c)| (n.clone(), ty(c).to_string()))
        .collect()
}

fn want(v: &Value) -> Vec<(String, String)> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p[0].as_str().unwrap().to_string(),
                p[1].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn check(t: &Tables) {
    let s: Value = serde_json::from_str(SCHEMA).unwrap();
    for (name, cols) in s["tables"].as_object().unwrap() {
        assert_eq!(shape(t.table(name).unwrap()), want(cols), "table {name}");
    }
    for (name, cols) in &t.series {
        assert_eq!(shape(cols), want(&s["series"][name]), "series {name}");
    }
}

#[test]
fn builder_columns_equal_the_schema() {
    let mut b = ExportBuilder::new();
    let t0 = parse_iso("2026-11-02").unwrap() as i64 * MS_DAY;
    b.add_fit("m.fit", &fitgen::wellness_fit(t0, 5)).unwrap();
    b.add_fit("h.fit", &fitgen::hrv_status_fit(t0, &[50.0, 60.0]))
        .unwrap();
    let (t, _) = b.finish();
    assert!(t.series.contains_key("hr") && t.series.contains_key("hrv5"));
    check(&t);
    check(&ExportBuilder::new().finish().0);
}

#[test]
fn synth_round_trip() {
    let (data, _) = synth(7, 60, 20_000);
    let t = Tables::from_data(&data);
    check(&t);
    assert_eq!(wellness_lab_fit::rows(&t.days), 60);
    let back = Tables::from_data(&t.to_data());
    assert_eq!(back, t);
    let d = t.to_data();
    assert_eq!(d.series["hr"].t, data.series["hr"].t);
    assert_eq!(d.days.col("rhr").len(), 60);
}
