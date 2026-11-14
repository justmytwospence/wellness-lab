//! The engine's entry point: a `Lab` holds the data, labels and settings, and runs jobs described as JSON. This list of jobs
//! is the contract the app (and the Python bindings) program against.

use crate::baseline::{baseline, BaselineSpec, Center, Scale, Transform};
use crate::data::{Data, Series, Table};
use crate::dfa::Windows;
use crate::events::{event_study, Controls};
use crate::experiments::{analyze, ExperimentDef};
use crate::labels::{excluded_days, Label, LabelKind, Proposal, Settings};
use crate::nights::{channels_with, night_features};
use crate::render::{bin_matrix, domain_for, m4, raster, typical, Align, Colormap};
use crate::stats::{mean, quantile_linear};
use crate::{
    beats, bench, circadian, compare, coverage, detect, dfa, hrv4t, kalman, registry, seasonal,
    teardown,
};
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;

/// A numeric array returned beside a job's JSON (large outputs travel as typed arrays, never JSON).
#[derive(Clone, Debug)]
pub enum ArrayData {
    F64(Vec<f64>),
    F32(Vec<f32>),
    I32(Vec<i32>),
    U8(Vec<u8>),
}

#[derive(Default, Debug)]
pub struct JobResult {
    pub json: String,
    pub arrays: Vec<(String, ArrayData)>,
}

#[derive(Deserialize, Debug, Default)]
pub struct ControlsIn {
    pub n: Option<usize>,
    pub doy: Option<i32>,
    pub same_weekday: Option<bool>,
    pub same_load_class: Option<bool>,
    pub exclude: Option<i32>,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Job {
    Hrv4t {
        #[serde(default = "d_range")]
        range_days: usize,
        #[serde(default = "d_kd")]
        k_daily: f64,
        #[serde(default = "d_ks")]
        k_swc: f64,
    },
    Sessions,
    Beats {
        id: String,
        #[serde(default)]
        window_min: Option<usize>,
    },
    /// DFA a1 windows, thresholds, history and durability for every recording
    Dfa,
    /// Deviation from a centred one-year median by day of year, for the By year and Radial views
    Seasonal {
        from: i32,
        to: i32,
        source: String,
        column: String,
    },
    Nights {
        from: Option<i32>,
        to: Option<i32>,
    },
    Night {
        day: i32,
        #[serde(default)]
        pad_min: Option<i64>,
    },
    Downsample {
        series: String,
        t0: f64,
        t1: f64,
        px: usize,
    },
    Raster {
        metric: String,
        #[serde(default)]
        align: Option<String>,
        from: i32,
        to: i32,
        #[serde(default)]
        bin_min: Option<i64>,
    },
    Typical {
        metric: String,
        #[serde(default)]
        align: Option<String>,
        a: [i32; 2],
        #[serde(default)]
        b: Option<[i32; 2]>,
        #[serde(default)]
        bin_min: Option<i64>,
    },
    Baseline {
        metric: String,
        #[serde(default)]
        window: Option<usize>,
        #[serde(default)]
        k_night: Option<f64>,
        #[serde(default)]
        k_short: Option<f64>,
        #[serde(default)]
        exclude_labelled: Option<bool>,
    },
    Circadian {
        from: i32,
        to: i32,
    },
    Actogram {
        from: i32,
        to: i32,
        #[serde(default)]
        period_h: Option<f64>,
    },
    Detect,
    EventStudy {
        #[serde(default)]
        label_kind: Option<LabelKind>,
        #[serde(default)]
        label_ids: Option<Vec<String>>,
        metrics: Vec<String>,
        #[serde(default)]
        window: Option<[i32; 2]>,
        #[serde(default)]
        controls: Option<ControlsIn>,
        #[serde(default)]
        n_boot: Option<usize>,
    },
    Compare {
        sel: Vec<[i32; 2]>,
        #[serde(default)]
        base: Option<Vec<[i32; 2]>>,
    },
    Bench {
        #[serde(default)]
        metrics: Option<Vec<String>>,
    },
    Teardown,
    Dfm,
    Experiment {
        def: ExperimentDef,
        #[serde(default)]
        adherence: BTreeMap<i32, bool>,
    },
    Coverage,
    Days {
        from: i32,
        to: i32,
        columns: Vec<String>,
    },
    Registry,
}

fn d_range() -> usize {
    60
}
fn d_kd() -> f64 {
    1.5
}
fn d_ks() -> f64 {
    0.5
}

fn json<T: serde::Serialize>(v: &T) -> Result<JobResult, String> {
    Ok(JobResult {
        json: serde_json::to_string(v).map_err(|e| e.to_string())?,
        arrays: vec![],
    })
}

fn align_of(s: &Option<String>) -> Align {
    if s.as_deref() == Some("clock") {
        Align::Clock
    } else {
        Align::Onset
    }
}

fn expand(r: &[[i32; 2]]) -> Vec<i32> {
    r.iter().flat_map(|p| p[0]..=p[1]).collect()
}

fn rows_between(t: &Table, from: i32, to: i32) -> Table {
    let days = t.col("day");
    let idx: Vec<usize> = (0..t.n)
        .filter(|&i| days[i] >= from as f64 && days[i] <= to as f64)
        .collect();
    let mut o = Table::new(idx.len());
    for (k, v) in &t.num {
        o.num.insert(k.clone(), idx.iter().map(|&i| v[i]).collect());
    }
    for (k, v) in &t.txt {
        o.txt
            .insert(k.clone(), idx.iter().map(|&i| v[i].clone()).collect());
    }
    o.n = idx.len();
    o
}

#[derive(Default)]
pub struct Lab {
    pub data: Data,
    pub labels: Vec<Label>,
    pub proposals: Vec<Proposal>,
    pub settings: Settings,
    cache: BTreeMap<String, String>,
    a1_cache: BTreeMap<String, Windows>,
    nights_cache: Option<Table>,
}

impl Lab {
    pub fn new() -> Self {
        Lab::default()
    }
    pub fn with_data(data: Data) -> Self {
        Lab {
            data,
            ..Default::default()
        }
    }
    fn invalidate(&mut self) {
        self.cache.clear();
        self.nights_cache = None;
    }
    pub fn load_table(&mut self, name: &str, table: Table) {
        if let Some(t) = self.data.table_mut(name) {
            *t = table;
        }
        self.invalidate();
    }
    pub fn load_series(&mut self, name: &str, s: Series) {
        self.data.series.insert(name.to_string(), s);
        self.invalidate();
    }
    pub fn load_rr(&mut self, id: &str, rr: Vec<f64>) {
        self.data.rr.insert(id.to_string(), rr);
        self.a1_cache.remove(id);
        self.invalidate();
    }
    pub fn set_labels(&mut self, labels: Vec<Label>) {
        self.labels = labels;
        self.invalidate();
    }
    pub fn set_proposals(&mut self, p: Vec<Proposal>) {
        self.proposals = p;
        self.cache.clear();
    }
    pub fn set_settings(&mut self, s: Settings) {
        self.settings = s;
        self.invalidate();
    }

    pub fn nights(&mut self) -> &Table {
        if self.nights_cache.is_none() {
            self.nights_cache = Some(night_features(&self.data, &self.labels, &self.settings));
        }
        self.nights_cache.as_ref().unwrap()
    }

    /// Run a job given as JSON; results are memoised until anything is loaded or changed.
    pub fn run(&mut self, job_json: &str) -> Result<JobResult, String> {
        if let Some(j) = self.cache.get(job_json) {
            return Ok(JobResult {
                json: j.clone(),
                arrays: vec![],
            });
        }
        let job: Job = serde_json::from_str(job_json).map_err(|e| format!("bad job: {e}"))?;
        let res = self.dispatch(job)?;
        if res.arrays.is_empty() {
            self.cache.insert(job_json.to_string(), res.json.clone());
        }
        Ok(res)
    }

    fn dispatch(&mut self, job: Job) -> Result<JobResult, String> {
        match job {
            Job::Hrv4t {
                range_days,
                k_daily,
                k_swc,
            } => json(&hrv4t::run(&self.data, range_days, k_daily, k_swc)),
            Job::Sessions => json(&beats::sessions(&self.data)),
            Job::Beats { id, window_min } => json(
                &beats::detail(&self.data, &id, window_min)
                    .ok_or_else(|| format!("no beat intervals for {id}"))?,
            ),
            Job::Dfa => json(&dfa::a1(&self.data, &mut self.a1_cache)),
            Job::Nights { from, to } => {
                let t = self.nights().clone();
                json(&rows_between(
                    &t,
                    from.unwrap_or(i32::MIN / 2),
                    to.unwrap_or(i32::MAX / 2),
                ))
            }
            Job::Night { day, pad_min } => {
                self.nights();
                json(
                    &channels_with(
                        &self.data,
                        self.nights_cache.as_ref().unwrap(),
                        day,
                        pad_min.unwrap_or(60),
                    )
                    .ok_or_else(|| format!("no sleep on day {day}"))?,
                )
            }
            Job::Downsample { series, t0, t1, px } => {
                let s = self
                    .data
                    .s(&series)
                    .ok_or_else(|| format!("no series {series}"))?;
                let (a, b) = s.range(t0 as i64, t1 as i64);
                let t: Vec<f64> = s.t[a..b].iter().map(|&t| t as f64).collect();
                let (ot, ov) = m4(&t, &s.v()[a..b], t0, t1, px);
                Ok(JobResult {
                    json: json!({"points": ot.len()}).to_string(),
                    arrays: vec![
                        ("t".into(), ArrayData::F64(ot)),
                        ("v".into(), ArrayData::F64(ov)),
                    ],
                })
            }
            Job::Raster {
                metric,
                align,
                from,
                to,
                bin_min,
            } => {
                let bin = bin_min.unwrap_or(if to - from > 400 { 10 } else { 5 });
                let (rows, days, span) =
                    bin_matrix(&self.data, &metric, align_of(&align), from, to, bin);
                let (dom, log, map) = domain_for(&metric, &rows);
                let img = raster(&rows, map, dom, log);
                let meta = json!({"width": img.width, "height": img.height, "days": days, "bin_min": bin, "span_min": span, "domain": [dom.0, dom.1, dom.2], "log": log, "metric": metric});
                Ok(JobResult {
                    json: meta.to_string(),
                    arrays: vec![("rgba".into(), ArrayData::U8(img.data))],
                })
            }
            Job::Typical {
                metric,
                align,
                a,
                b,
                bin_min,
            } => {
                let bin = bin_min.unwrap_or(10);
                let al = align_of(&align);
                let ta = typical(
                    &bin_matrix(&self.data, &metric, al, a[0], a[1], bin).0,
                    bin,
                    5,
                );
                let tb = b.map(|b| {
                    typical(
                        &bin_matrix(&self.data, &metric, al, b[0], b[1], bin).0,
                        bin,
                        5,
                    )
                });
                json(&json!({"a": ta, "b": tb}))
            }
            Job::Baseline {
                metric,
                window,
                k_night,
                k_short,
                exclude_labelled,
            } => {
                let t = self.nights().clone();
                let days: Vec<i32> = t.col("day").iter().map(|&d| d as i32).collect();
                let spec = BaselineSpec {
                    transform: Transform::None,
                    center: Center::Median,
                    scale: Scale::Mad,
                    window: window.unwrap_or(self.settings.baseline_window),
                    min_n: 14,
                    short: 3,
                    short_min: 2,
                    k_night: k_night.unwrap_or(2.0),
                    k_short: k_short.unwrap_or(1.0),
                    exclude_labelled: exclude_labelled.unwrap_or(true),
                };
                let b = baseline(&t.col(&metric), &days, &excluded_days(&self.labels), &spec);
                let mut o = Table::new(days.len());
                o.set("day", days.iter().map(|&d| d as f64).collect());
                for (k, v) in [
                    ("x", &b.x),
                    ("short", &b.short),
                    ("center", &b.center),
                    ("scale", &b.scale),
                    ("lo", &b.lo),
                    ("hi", &b.hi),
                    ("z", &b.z),
                ] {
                    o.set(k, v.clone());
                }
                o.set("state", b.state.iter().map(|&s| s as f64).collect());
                json(&o)
            }
            Job::Circadian { from, to } => {
                self.nights();
                let c = circadian::run_with(
                    &self.data,
                    self.nights_cache.as_ref().unwrap(),
                    &self.settings,
                    from,
                    to,
                );
                json(
                    &json!({"nonparametric": c.nonparametric, "cosinor": c.cosinor, "sri": c.sri, "chronotype": c.chronotype, "sleep_debt": c.sleep_debt, "hr_rhythm": c.hr_rhythm, "dlmo": c.dlmo}),
                )
            }
            Job::Actogram { from, to, period_h } => {
                let g = circadian::grid(&self.data, from, to);
                let (rows, days) = circadian::actogram(&g, period_h.unwrap_or(24.0));
                let all: Vec<f64> = rows
                    .iter()
                    .flatten()
                    .copied()
                    .filter(|v| v.is_finite() && *v > 0.0)
                    .collect();
                let hi = if all.is_empty() {
                    1.0
                } else {
                    quantile_linear(&all, 0.97)
                };
                let img = raster(&rows, Colormap::Night, (0.0, None, hi), false);
                let asleep: Vec<u8> = {
                    let per = (period_h.unwrap_or(24.0) * 60.0).round() as usize;
                    rows.iter()
                        .enumerate()
                        .flat_map(|(r, row)| (0..row.len()).map(move |k| (r * per + k, ())))
                        .map(|(m, _)| g.asleep.get(m).copied().unwrap_or(false) as u8)
                        .collect()
                };
                let meta = json!({"width": img.width, "height": img.height, "days": days, "period_h": period_h.unwrap_or(24.0)});
                Ok(JobResult {
                    json: meta.to_string(),
                    arrays: vec![
                        ("rgba".into(), ArrayData::U8(img.data)),
                        ("asleep".into(), ArrayData::U8(asleep)),
                    ],
                })
            }
            Job::Detect => {
                self.nights();
                let props = detect::detect_with(
                    &self.data,
                    self.nights_cache.as_ref().unwrap(),
                    &self.labels,
                    &self.settings,
                    &self.proposals,
                );
                json(&json!({"proposals": props, "model_version": crate::classify::MODEL_VERSION}))
            }
            Job::EventStudy {
                label_kind,
                label_ids,
                metrics,
                window,
                controls,
                n_boot,
            } => {
                let t = self.nights().clone();
                let eps: Vec<&Label> = self
                    .labels
                    .iter()
                    .filter(|l| {
                        l.live()
                            && label_kind.map(|k| k == l.kind).unwrap_or(true)
                            && label_ids
                                .as_ref()
                                .map(|ids| ids.contains(&l.id))
                                .unwrap_or(true)
                    })
                    .collect();
                let c = controls.unwrap_or_default();
                let def = Controls::default();
                let ctl = Controls {
                    n: c.n.unwrap_or(def.n),
                    doy_window: c.doy.unwrap_or(def.doy_window),
                    same_weekday: c.same_weekday.unwrap_or(def.same_weekday),
                    same_load_class: c.same_load_class.unwrap_or(def.same_load_class),
                    exclude_days: c.exclude.unwrap_or(def.exclude_days),
                };
                let w = window.unwrap_or([-7, 14]);
                json(&event_study(
                    &t,
                    &self.labels,
                    &eps,
                    &metrics,
                    (w[0], w[1]),
                    &ctl,
                    n_boot.unwrap_or(1000),
                    1,
                ))
            }
            Job::Compare { sel, base } => {
                let t = self.nights().clone();
                let (s, b) = (expand(&sel), base.as_ref().map(|b| expand(b)));
                json(&compare::compare(&t, &self.labels, &s, b.as_deref()))
            }
            Job::Bench { metrics } => {
                let t = self.nights().clone();
                json(&bench::bench(&t, &self.labels, metrics.as_deref(), 200))
            }
            Job::Teardown => {
                let t = self.nights().clone();
                json(&teardown::teardown(&self.data, &t))
            }
            Job::Dfm => {
                let t = self.nights().clone();
                json(&self.dfm(&t))
            }
            Job::Experiment { def, adherence } => {
                let t = self.nights().clone();
                let swc = bench::bench(
                    &t,
                    &self.labels,
                    Some(std::slice::from_ref(&def.outcome)),
                    50,
                )
                .first()
                .map(|r| r.swc)
                .filter(|s| s.is_finite())
                .unwrap_or(0.0);
                json(&analyze(&def, &t, &adherence, swc))
            }
            Job::Coverage => json(&coverage::coverage(&self.data)),
            Job::Days { from, to, columns } => {
                let mut t = rows_between(&self.data.days, from, to);
                t.num.retain(|k, _| k == "day" || columns.contains(k));
                t.txt.retain(|k, _| columns.contains(k));
                json(&t)
            }
            Job::Seasonal {
                from,
                to,
                source,
                column,
            } => {
                let t = match source.as_str() {
                    "days" => self.data.days.clone(),
                    "nights" => self.nights().clone(),
                    s => return Err(format!("seasonal: unknown source {s}")),
                };
                json(&seasonal::seasonal(
                    &t.col("day"),
                    &t.col(&column),
                    from,
                    to,
                ))
            }
            Job::Registry => Ok(JobResult {
                json: registry::registry_json(),
                arrays: vec![],
            }),
        }
    }

    /// One latent "strain" factor behind the nightly signals, with training, travel, alcohol and illness as inputs.
    fn dfm(&self, t: &Table) -> serde_json::Value {
        let days: Vec<i32> = t.col("day").iter().map(|&d| d as i32).collect();
        let excl = excluded_days(&self.labels);
        let spec = BaselineSpec {
            transform: Transform::None,
            center: Center::Median,
            scale: Scale::Mad,
            window: self.settings.baseline_window,
            min_n: 14,
            short: 3,
            short_min: 2,
            k_night: 2.0,
            k_short: 1.0,
            exclude_labelled: true,
        };
        let mut names = vec![];
        let mut ys = vec![];
        for (col, sign, floor) in [
            ("hr_min30", 1.0, 1.0),
            ("ln_rmssd", -1.0, 0.05),
            ("resp_mean", 1.0, 0.3),
            ("sleep_stress", 1.0, 2.0),
            ("skin_temp", 1.0, 0.1),
            ("rest_frac", -1.0, 0.05),
        ] {
            let v = t.col(col);
            if v.iter().filter(|x| x.is_finite()).count() < t.n / 2 {
                continue;
            }
            let b = baseline(&v, &days, &excl, &spec);
            ys.push(
                (0..t.n)
                    .map(|i| {
                        if v[i].is_finite() && b.center[i].is_finite() {
                            sign * (v[i] - b.center[i]) / b.scale[i].max(floor)
                        } else {
                            f64::NAN
                        }
                    })
                    .collect::<Vec<f64>>(),
            );
            names.push(col);
        }
        let has = |kind: LabelKind, d: i32| {
            self.labels
                .iter()
                .any(|l| l.live() && l.kind == kind && d >= l.start_day && d <= l.end_day)
                as u8 as f64
        };
        let xs: Vec<Vec<f64>> = vec![
            t.col("load_prev")
                .iter()
                .map(|v| if v.is_finite() { v / 100.0 } else { 0.0 })
                .collect(),
            t.col("tz_change")
                .iter()
                .map(|v| (v.is_finite() && *v != 0.0) as u8 as f64)
                .collect(),
            days.iter().map(|&d| has(LabelKind::Alcohol, d)).collect(),
            days.iter().map(|&d| has(LabelKind::Illness, d)).collect(),
        ];
        match kalman::dfm_fit(&ys, &xs, 0) {
            Some(f) => {
                json!({"signals": names, "inputs": ["training load", "time zone change", "alcohol", "illness"], "days": days, "fit": f})
            }
            None => json!({"signals": names, "error": "too little data"}),
        }
    }
}

#[allow(dead_code)]
fn _mean(v: &[f64]) -> f64 {
    mean(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::synth;

    fn lab() -> Lab {
        let (d, truth) = synth(3, 200, 19_723);
        let mut l = Lab::with_data(d);
        l.set_labels(truth);
        l
    }

    #[test]
    fn seasonal_job_rejects_unknown_source() {
        let e = lab()
            .run(r#"{"kind":"seasonal","from":0,"to":1,"source":"x","column":"rhr"}"#)
            .unwrap_err();
        assert!(e.contains("unknown source"), "{e}");
    }

    #[test]
    fn every_job_runs_on_synthetic_data() {
        let mut l = lab();
        let from = 19_723;
        let to = from + 199;
        let jobs = vec![
            json!({"kind": "hrv4t"}),
            json!({"kind": "nights"}),
            json!({"kind": "night", "day": from + 100}),
            json!({"kind": "baseline", "metric": "hr_min30"}),
            json!({"kind": "typical", "metric": "hr", "a": [from, to]}),
            json!({"kind": "circadian", "from": from, "to": from + 100}),
            json!({"kind": "detect"}),
            json!({"kind": "event_study", "label_kind": "illness", "metrics": ["hr_min30"], "window": [-2, 4], "n_boot": 50}),
            json!({"kind": "compare", "sel": [[from + 50, from + 60]]}),
            json!({"kind": "bench", "metrics": ["hr_min30"]}),
            json!({"kind": "teardown"}),
            json!({"kind": "dfm"}),
            json!({"kind": "coverage"}),
            json!({"kind": "days", "from": from, "to": to, "columns": ["rhr", "steps"]}),
            json!({"kind": "seasonal", "from": from, "to": to, "source": "days", "column": "rhr"}),
            json!({"kind": "seasonal", "from": from, "to": to, "source": "nights", "column": "resp_mean"}),
            json!({"kind": "registry"}),
            json!({"kind": "sessions"}),
            json!({"kind": "dfa"}),
        ];
        for j in jobs {
            let r = l.run(&j.to_string()).unwrap_or_else(|e| panic!("{j}: {e}"));
            assert!(!r.json.is_empty(), "{j}");
            let v: serde_json::Value = serde_json::from_str(&r.json).unwrap();
            assert!(!v.is_null(), "{j}");
        }
        // arrays: a raster and a downsampled line
        let r = l.run(&json!({"kind": "raster", "metric": "hr", "from": from, "to": to, "align": "onset"}).to_string()).unwrap();
        let meta: serde_json::Value = serde_json::from_str(&r.json).unwrap();
        let (w, h) = (
            meta["width"].as_u64().unwrap() as usize,
            meta["height"].as_u64().unwrap() as usize,
        );
        match &r.arrays[0].1 {
            ArrayData::U8(v) => assert_eq!(v.len(), w * h * 4),
            _ => panic!("rgba"),
        }
        let r = l.run(&json!({"kind": "downsample", "series": "hr", "t0": (from as f64) * 86_400_000.0, "t1": ((from + 2) as f64) * 86_400_000.0, "px": 300}).to_string()).unwrap();
        assert_eq!(r.arrays.len(), 2);
        // an unknown job is an error, not a panic
        assert!(l.run("{\"kind\": \"nope\"}").is_err());
    }

    #[test]
    fn results_are_memoised_until_labels_change() {
        let mut l = lab();
        let j = json!({"kind": "nights"}).to_string();
        let a = l.run(&j).unwrap().json;
        assert_eq!(a, l.run(&j).unwrap().json);
        l.set_labels(vec![]);
        let b = l.run(&j).unwrap().json;
        assert_ne!(a, b); // the excluded flag changed
    }
}
