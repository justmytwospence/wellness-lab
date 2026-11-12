//! Browser bindings for the engine. Numbers cross as typed arrays; NaN is missing.

use js_sys::{Array, Float32Array, Float64Array, Int32Array, Uint8Array};
use std::collections::BTreeMap;
use wasm_bindgen::prelude::*;
use wellness_lab_core::data::{Series, Table};
use wellness_lab_core::lab::{ArrayData, JobResult, Lab as CoreLab};

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

fn jserr<E: std::fmt::Display>(e: E) -> JsValue {
    JsValue::from_str(&e.to_string())
}

#[wasm_bindgen]
pub struct Lab {
    inner: CoreLab,
}

impl Default for Lab {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl Lab {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Lab {
        Lab { inner: CoreLab::new() }
    }

    /// Replace a table. `numeric[i]` is the column `numeric_names[i]` (NaN for missing); `strings_json` is
    /// `{column: (string|null)[]}`.
    #[wasm_bindgen(js_name = loadTable)]
    pub fn load_table(&mut self, name: &str, numeric_names: Vec<String>, numeric: Vec<Float64Array>, strings_json: &str) -> Result<(), JsValue> {
        let mut t = Table::new(0);
        for (n, a) in numeric_names.iter().zip(numeric.iter()) {
            t.num.insert(n.clone(), a.to_vec());
            t.n = a.length() as usize;
        }
        let strings: BTreeMap<String, Vec<Option<String>>> = serde_json::from_str(strings_json).map_err(jserr)?;
        for (n, v) in strings {
            t.n = t.n.max(v.len());
            t.txt.insert(n, v);
        }
        if self.inner.data.table_mut(name).is_none() {
            return Err(jserr(format!("unknown table {name}")));
        }
        self.inner.load_table(name, t);
        Ok(())
    }

    /// Replace a series. `t` is Unix ms as float64 (exact below 2^53); `col_names[i]` names `cols[i]` (`v` first, then
    /// whatever the schema lists: `q`, `conf`, `mode`, `intensity`, `day`, `v2`). NaN is missing.
    #[wasm_bindgen(js_name = loadSeries)]
    pub fn load_series(&mut self, name: &str, t: Float64Array, col_names: Vec<String>, cols: Vec<Float64Array>) {
        let mut s = Series { t: t.to_vec().into_iter().map(|t| t as i64).collect(), ..Default::default() };
        for (n, c) in col_names.into_iter().zip(cols) {
            s.cols.insert(n, c.to_vec());
        }
        self.inner.load_series(name, s);
    }

    #[wasm_bindgen(js_name = loadRr)]
    pub fn load_rr(&mut self, id: &str, rr: Float32Array) {
        self.inner.load_rr(id, rr.to_vec().into_iter().map(f64::from).collect());
    }

    #[wasm_bindgen(js_name = setLabels)]
    pub fn set_labels(&mut self, json: &str) -> Result<(), JsValue> {
        self.inner.set_labels(serde_json::from_str(json).map_err(jserr)?);
        Ok(())
    }

    #[wasm_bindgen(js_name = setProposals)]
    pub fn set_proposals(&mut self, json: &str) -> Result<(), JsValue> {
        self.inner.set_proposals(serde_json::from_str(json).map_err(jserr)?);
        Ok(())
    }

    #[wasm_bindgen(js_name = setSettings)]
    pub fn set_settings(&mut self, json: &str) -> Result<(), JsValue> {
        self.inner.set_settings(serde_json::from_str(json).map_err(jserr)?);
        Ok(())
    }

    pub fn run(&mut self, job: &str) -> Result<RunResult, JsValue> {
        self.inner.run(job).map(|r| RunResult { r }).map_err(jserr)
    }
}

#[wasm_bindgen]
pub struct RunResult {
    r: JobResult,
}

impl RunResult {
    fn get(&self, name: &str) -> Result<&ArrayData, JsValue> {
        self.r.arrays.iter().find(|a| a.0 == name).map(|a| &a.1).ok_or_else(|| jserr(format!("no array {name}")))
    }
}

#[wasm_bindgen]
impl RunResult {
    pub fn json(&self) -> String {
        self.r.json.clone()
    }
    pub fn names(&self) -> Array {
        self.r.arrays.iter().map(|a| JsValue::from_str(&a.0)).collect()
    }
    pub fn f64(&self, name: &str) -> Result<Float64Array, JsValue> {
        match self.get(name)? {
            ArrayData::F64(v) => Ok(Float64Array::from(v.as_slice())),
            _ => Err(jserr(format!("{name} is not f64"))),
        }
    }
    pub fn f32(&self, name: &str) -> Result<Float32Array, JsValue> {
        match self.get(name)? {
            ArrayData::F32(v) => Ok(Float32Array::from(v.as_slice())),
            _ => Err(jserr(format!("{name} is not f32"))),
        }
    }
    pub fn i32(&self, name: &str) -> Result<Int32Array, JsValue> {
        match self.get(name)? {
            ArrayData::I32(v) => Ok(Int32Array::from(v.as_slice())),
            _ => Err(jserr(format!("{name} is not i32"))),
        }
    }
    pub fn u8(&self, name: &str) -> Result<Uint8Array, JsValue> {
        match self.get(name)? {
            ArrayData::U8(v) => Ok(Uint8Array::from(v.as_slice())),
            _ => Err(jserr(format!("{name} is not u8"))),
        }
    }
}

#[wasm_bindgen]
pub fn registry() -> String {
    wellness_lab_core::registry::registry_json()
}

#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// A Lab preloaded with synthetic data and its true labels (the demo).
#[wasm_bindgen]
pub fn synth(seed: f64, days: i32) -> Lab {
    let (d, labels) = wellness_lab_core::synth::synth(seed as u64, days, 19_723);
    let mut lab = CoreLab::with_data(d);
    lab.set_labels(labels);
    Lab { inner: lab }
}

/// The synthetic data as plain table and series arrays (so the app can write it as wellness-v1 files for e2e).
#[wasm_bindgen(js_name = synthLabels)]
pub fn synth_labels(seed: f64, days: i32) -> String {
    let (_, labels) = wellness_lab_core::synth::synth(seed as u64, days, 19_723);
    serde_json::to_string(&labels).unwrap_or_default()
}
