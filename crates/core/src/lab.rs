//! The engine's entry point: a `Lab` holds the data, labels and settings, and runs jobs described as JSON.

use crate::data::{Data, Series, Table};
use crate::hrv4t;
use crate::labels::{Label, Proposal, Settings};
use serde::Deserialize;
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

#[derive(Default)]
pub struct Lab {
    pub data: Data,
    pub labels: Vec<Label>,
    pub proposals: Vec<Proposal>,
    pub settings: Settings,
    cache: BTreeMap<String, String>,
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
    pub fn load_table(&mut self, name: &str, table: Table) {
        if let Some(t) = self.data.table_mut(name) {
            *t = table;
        }
        self.cache.clear();
    }
    pub fn load_series(&mut self, name: &str, s: Series) {
        self.data.series.insert(name.to_string(), s);
        self.cache.clear();
    }
    pub fn load_rr(&mut self, id: &str, rr: Vec<f64>) {
        self.data.rr.insert(id.to_string(), rr);
        self.cache.clear();
    }
    pub fn set_labels(&mut self, labels: Vec<Label>) {
        self.labels = labels;
        self.cache.clear();
    }
    pub fn set_proposals(&mut self, p: Vec<Proposal>) {
        self.proposals = p;
        self.cache.clear();
    }
    pub fn set_settings(&mut self, s: Settings) {
        self.settings = s;
        self.cache.clear();
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
        let res = match job {
            Job::Hrv4t {
                range_days,
                k_daily,
                k_swc,
            } => {
                let r = hrv4t::run(&self.data, range_days, k_daily, k_swc);
                JobResult {
                    json: serde_json::to_string(&r).map_err(|e| e.to_string())?,
                    arrays: vec![],
                }
            }
        };
        if res.arrays.is_empty() {
            self.cache.insert(job_json.to_string(), res.json.clone());
        }
        Ok(res)
    }
}
