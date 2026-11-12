//! Python bindings: the engine for the parity checks against the notebook and garmin-mcp, the importer and the
//! cross-binding cases. Numbers cross as numpy arrays (NaN is missing); everything else as dicts and lists.

use numpy::{IntoPyArray, PyReadonlyArray1};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString};
use std::path::Path;
use wellness_lab_core::data::{Series, Table};
use wellness_lab_core::lab::{ArrayData, JobResult, Lab as CoreLab};
use wellness_lab_fit::{Column, Columns, ExportBuilder as FitBuilder, FitError, Tables};

fn err<E: std::fmt::Display>(e: E) -> PyErr {
    PyRuntimeError::new_err(e.to_string())
}

fn default_importer() -> String {
    format!("wellness-lab-import@{}", env!("CARGO_PKG_VERSION"))
}

/// A Python value as JSON text: a `str` is taken as JSON already, anything else goes through `json.dumps`.
fn to_json(py: Python<'_>, v: &Bound<'_, PyAny>) -> PyResult<String> {
    if v.is_instance_of::<PyString>() {
        return v.extract();
    }
    py.import("json")?.call_method1("dumps", (v,))?.extract()
}

fn from_json(py: Python<'_>, text: &str) -> PyResult<Py<PyAny>> {
    Ok(py.import("json")?.call_method1("loads", (text,))?.unbind())
}

/// Any array-like as a contiguous 1-D array of `dtype`, copied out.
fn vec_of<T: numpy::Element + Copy>(
    py: Python<'_>,
    v: &Bound<'_, PyAny>,
    dtype: &str,
) -> PyResult<Vec<T>> {
    let a = py
        .import("numpy")?
        .call_method1("ascontiguousarray", (v, dtype))?;
    let a: PyReadonlyArray1<T> = a
        .extract()
        .map_err(|e: PyErr| err(format!("expected a 1-D array: {e}")))?;
    Ok(a.as_array().to_vec())
}

fn arrays<'py>(py: Python<'py>, list: Vec<(String, ArrayData)>) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new(py);
    for (name, a) in list {
        match a {
            ArrayData::F64(v) => out.set_item(name, v.into_pyarray(py))?,
            ArrayData::F32(v) => out.set_item(name, v.into_pyarray(py))?,
            ArrayData::I32(v) => out.set_item(name, v.into_pyarray(py))?,
            ArrayData::U8(v) => out.set_item(name, v.into_pyarray(py))?,
        }
    }
    Ok(out)
}

#[pyclass]
struct Lab {
    inner: CoreLab,
}

impl Lab {
    fn exec(&mut self, py: Python<'_>, job: &Bound<'_, PyAny>) -> PyResult<JobResult> {
        let text = to_json(py, job)?;
        self.inner.run(&text).map_err(err)
    }
}

#[pymethods]
impl Lab {
    #[new]
    fn new() -> Self {
        Lab {
            inner: CoreLab::new(),
        }
    }

    /// Load a wellness-v1 directory (replaces all data; labels and settings stay).
    fn load_dir(&mut self, path: &str) -> PyResult<()> {
        let data = wellness_lab_io::read_dir(Path::new(path)).map_err(err)?;
        let labels = std::mem::take(&mut self.inner.labels);
        let proposals = std::mem::take(&mut self.inner.proposals);
        let settings = std::mem::take(&mut self.inner.settings);
        self.inner = CoreLab::with_data(data);
        self.inner.set_labels(labels);
        self.inner.set_proposals(proposals);
        self.inner.set_settings(settings);
        Ok(())
    }

    /// Replace a table: `numeric` maps column names to arrays (NaN for missing), `strings` to lists of str or None.
    #[pyo3(signature = (name, numeric, strings=None))]
    fn load_table(
        &mut self,
        py: Python<'_>,
        name: &str,
        numeric: &Bound<'_, PyDict>,
        strings: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        if self.inner.data.table_mut(name).is_none() {
            return Err(err(format!("unknown table {name}")));
        }
        let mut t = Table::new(0);
        for (k, v) in numeric.iter() {
            let v: Vec<f64> = vec_of(py, &v, "float64")?;
            t.n = t.n.max(v.len());
            t.num.insert(k.extract()?, v);
        }
        if let Some(s) = strings {
            for (k, v) in s.iter() {
                let v: Vec<Option<String>> = v.extract()?;
                t.n = t.n.max(v.len());
                t.txt.insert(k.extract()?, v);
            }
        }
        self.inner.load_table(name, t);
        Ok(())
    }

    /// Replace a series: `t` in Unix ms, `cols` maps `v` (and `q`, `conf`, ... as the schema lists) to arrays.
    fn load_series(
        &mut self,
        py: Python<'_>,
        name: &str,
        t: &Bound<'_, PyAny>,
        cols: &Bound<'_, PyDict>,
    ) -> PyResult<()> {
        let mut s = Series {
            t: vec_of(py, t, "int64")?,
            ..Default::default()
        };
        for (k, v) in cols.iter() {
            let v: Vec<f64> = vec_of(py, &v, "float64")?;
            if v.len() != s.t.len() {
                return Err(err(format!(
                    "column {k} has {} values for {} times",
                    v.len(),
                    s.t.len()
                )));
            }
            s.cols.insert(k.extract()?, v);
        }
        self.inner.load_series(name, s);
        Ok(())
    }

    /// Beat intervals (ms) of one activity; stored as float32, as the wellness-v1 files and the browser hold them.
    fn load_rr(&mut self, py: Python<'_>, id: &str, rr: &Bound<'_, PyAny>) -> PyResult<()> {
        let v: Vec<f32> = vec_of(py, rr, "float32")?;
        self.inner
            .load_rr(id, v.into_iter().map(f64::from).collect());
        Ok(())
    }

    /// Labels as a list of dicts (or its JSON text).
    fn set_labels(&mut self, py: Python<'_>, labels: &Bound<'_, PyAny>) -> PyResult<()> {
        self.inner
            .set_labels(serde_json::from_str(&to_json(py, labels)?).map_err(err)?);
        Ok(())
    }

    fn set_proposals(&mut self, py: Python<'_>, proposals: &Bound<'_, PyAny>) -> PyResult<()> {
        self.inner
            .set_proposals(serde_json::from_str(&to_json(py, proposals)?).map_err(err)?);
        Ok(())
    }

    fn set_settings(&mut self, py: Python<'_>, settings: &Bound<'_, PyAny>) -> PyResult<()> {
        self.inner
            .set_settings(serde_json::from_str(&to_json(py, settings)?).map_err(err)?);
        Ok(())
    }

    /// The labels in effect, as a list of dicts.
    fn labels(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        from_json(py, &serde_json::to_string(&self.inner.labels).map_err(err)?)
    }

    /// Run a job (a dict or a JSON string): returns (result as a dict, arrays by name).
    fn run<'py>(
        &mut self,
        py: Python<'py>,
        job: &Bound<'py, PyAny>,
    ) -> PyResult<(Py<PyAny>, Bound<'py, PyDict>)> {
        let res = self.exec(py, job)?;
        Ok((from_json(py, &res.json)?, arrays(py, res.arrays)?))
    }

    /// Like `run`, but the result stays the engine's JSON text (the cross-binding check compares it byte for byte).
    fn run_json<'py>(
        &mut self,
        py: Python<'py>,
        job: &Bound<'py, PyAny>,
    ) -> PyResult<(String, Bound<'py, PyDict>)> {
        let res = self.exec(py, job)?;
        Ok((res.json, arrays(py, res.arrays)?))
    }
}

// ---- importing a Garmin account export ----

#[pyclass]
struct ExportBuilder {
    inner: Option<FitBuilder>,
}

#[pymethods]
impl ExportBuilder {
    #[new]
    fn new() -> Self {
        ExportBuilder {
            inner: Some(FitBuilder::new()),
        }
    }

    /// Add one JSON file of the export; returns its kind.
    fn add_json(&mut self, path: &str, text: &str) -> PyResult<String> {
        let b = self.inner.as_mut().ok_or_else(|| err("finished"))?;
        b.add_json(path, text)
            .map(|k| k.as_str().to_string())
            .map_err(err)
    }

    /// Add one FIT file; returns its kind. Raises `RuntimeError('json-first')` when an activity file arrives before
    /// the JSON summaries.
    fn add_fit(&mut self, name: &str, data: &[u8]) -> PyResult<String> {
        let b = self.inner.as_mut().ok_or_else(|| err("finished"))?;
        match b.add_fit(name, data) {
            Ok(k) => Ok(k.as_str().to_string()),
            Err(FitError::JsonFirst) => Err(err("json-first")),
            Err(e) => Err(err(e)),
        }
    }

    fn finish(&mut self) -> PyResult<ExportResult> {
        let b = self.inner.take().ok_or_else(|| err("finished"))?;
        let (tables, report) = b.finish();
        Ok(ExportResult {
            tables,
            report: serde_json::to_string(&report).map_err(err)?,
        })
    }
}

#[pyclass]
struct ExportResult {
    tables: Tables,
    report: String,
}

fn columns<'a>(t: &'a Tables, name: &str) -> Option<&'a Columns> {
    t.table(name).or_else(|| t.series.get(name))
}

#[pymethods]
impl ExportResult {
    fn tables(&self) -> Vec<&'static str> {
        vec!["days", "sleeps", "stages", "activities", "tags", "devices"]
    }

    fn columns(&self, table: &str) -> Vec<String> {
        columns(&self.tables, table)
            .map(|c| c.iter().map(|(n, _)| n.clone()).collect())
            .unwrap_or_default()
    }

    /// A numeric column as float64 (NaN for null); a string or unknown column gives an empty array.
    fn numeric<'py>(
        &self,
        py: Python<'py>,
        table: &str,
        col: &str,
    ) -> Bound<'py, numpy::PyArray1<f64>> {
        columns(&self.tables, table)
            .and_then(|c| c.iter().find(|(n, _)| n == col))
            .map(|(_, c)| c.f64s())
            .unwrap_or_default()
            .into_pyarray(py)
    }

    /// A string column as a list of str or None.
    fn strings<'py>(
        &self,
        py: Python<'py>,
        table: &str,
        col: &str,
    ) -> PyResult<Bound<'py, PyList>> {
        match columns(&self.tables, table).and_then(|c| c.iter().find(|(n, _)| n == col)) {
            Some((_, Column::Str(v))) => PyList::new(py, v),
            _ => Ok(PyList::empty(py)),
        }
    }

    /// The series with rows.
    fn series(&self) -> Vec<String> {
        self.tables
            .series
            .iter()
            .filter(|(_, c)| !c.is_empty() && !c[0].1.is_empty())
            .map(|(n, _)| n.clone())
            .collect()
    }

    fn series_column<'py>(
        &self,
        py: Python<'py>,
        name: &str,
        col: &str,
    ) -> Bound<'py, numpy::PyArray1<f64>> {
        self.numeric(py, name, col)
    }

    /// Activity ids with beat intervals, as file stems (`garmin-<id>`).
    fn rr_ids(&self) -> Vec<String> {
        self.tables
            .rr
            .keys()
            .map(|k| k.replace("garmin:", "garmin-"))
            .collect()
    }

    fn rr<'py>(&self, py: Python<'py>, id: &str) -> Bound<'py, numpy::PyArray1<f32>> {
        let key = id.replacen("garmin-", "garmin:", 1);
        self.tables
            .rr
            .get(&key)
            .cloned()
            .unwrap_or_default()
            .into_pyarray(py)
    }

    fn report(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        from_json(py, &self.report)
    }

    /// Write the tables as a wellness-v1 directory; returns the manifest.
    #[pyo3(signature = (out_dir, importer=None, source_kind="garmin-export"))]
    fn write(
        &self,
        py: Python<'_>,
        out_dir: &str,
        importer: Option<String>,
        source_kind: &str,
    ) -> PyResult<Py<PyAny>> {
        let importer = importer.unwrap_or_else(default_importer);
        let m = wellness_lab_io::write_data(
            Path::new(out_dir),
            &self.tables.to_data(),
            &importer,
            source_kind,
        )
        .map_err(err)?;
        from_json(py, &serde_json::to_string(&m).map_err(err)?)
    }
}

/// Import a Garmin account export zip into the wellness-v1 directory `out_dir`; returns the import report.
#[pyfunction]
#[pyo3(signature = (zip_path, out_dir, importer=None))]
fn import_export(
    py: Python<'_>,
    zip_path: &str,
    out_dir: &str,
    importer: Option<String>,
) -> PyResult<Py<PyAny>> {
    let importer = importer.unwrap_or_else(default_importer);
    let (zip, out) = (
        Path::new(zip_path).to_path_buf(),
        Path::new(out_dir).to_path_buf(),
    );
    let report = py
        .detach(|| wellness_lab_io::import_zip(&zip, &out, &importer))
        .map_err(err)?;
    from_json(py, &serde_json::to_string(&report).map_err(err)?)
}

/// A Lab preloaded with `days` days of synthetic data from 2024-01-01 and the true labels of its injected episodes.
#[pyfunction]
fn synth(seed: u64, days: i32) -> Lab {
    let (d, labels) = wellness_lab_core::synth::synth(seed, days, 19_723);
    let mut lab = CoreLab::with_data(d);
    lab.set_labels(labels);
    Lab { inner: lab }
}

/// The ground-truth labels of `synth(seed, days)`.
#[pyfunction]
fn synth_labels(py: Python<'_>, seed: u64, days: i32) -> PyResult<Py<PyAny>> {
    let (_, labels) = wellness_lab_core::synth::synth(seed, days, 19_723);
    from_json(py, &serde_json::to_string(&labels).map_err(err)?)
}

/// The synthetic data as an export result (the same tables the browser's `synthExport` gives).
#[pyfunction]
fn synth_export(seed: u64, days: i32) -> ExportResult {
    let (d, _) = wellness_lab_core::synth::synth(seed, days, 19_723);
    ExportResult {
        tables: Tables::from_data(&d),
        report: "{}".into(),
    }
}

/// Write `synth(seed, days)` as a wellness-v1 directory with the native writer; returns the manifest.
#[pyfunction]
#[pyo3(signature = (out_dir, seed, days, importer=None))]
fn write_synth(
    py: Python<'_>,
    out_dir: &str,
    seed: u64,
    days: i32,
    importer: Option<String>,
) -> PyResult<Py<PyAny>> {
    let (d, _) = wellness_lab_core::synth::synth(seed, days, 19_723);
    let importer =
        importer.unwrap_or_else(|| format!("wellness-lab-synth@{}", env!("CARGO_PKG_VERSION")));
    let m = wellness_lab_io::write_data(Path::new(out_dir), &d, &importer, "synth").map_err(err)?;
    from_json(py, &serde_json::to_string(&m).map_err(err)?)
}

/// The method registry (citations and validity grades).
#[pyfunction]
fn registry(py: Python<'_>) -> PyResult<Py<PyAny>> {
    from_json(py, &wellness_lab_core::registry::registry_json())
}

#[pyfunction]
fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[pymodule]
fn wellness_lab(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Lab>()?;
    m.add_class::<ExportBuilder>()?;
    m.add_class::<ExportResult>()?;
    for f in [
        wrap_pyfunction!(import_export, m)?,
        wrap_pyfunction!(synth, m)?,
        wrap_pyfunction!(synth_labels, m)?,
        wrap_pyfunction!(synth_export, m)?,
        wrap_pyfunction!(write_synth, m)?,
        wrap_pyfunction!(registry, m)?,
        wrap_pyfunction!(version, m)?,
    ] {
        m.add_function(f)?;
    }
    Ok(())
}
