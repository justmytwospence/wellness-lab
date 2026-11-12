//! Python bindings: the engine for the parity checks against the notebook and garmin-mcp.

use numpy::IntoPyArray;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::path::Path;
use wellness_lab_core::lab::{ArrayData, Lab as CoreLab};

fn err<E: std::fmt::Display>(e: E) -> PyErr {
    PyRuntimeError::new_err(e.to_string())
}

#[pyclass]
struct Lab {
    inner: CoreLab,
}

#[pymethods]
impl Lab {
    #[new]
    fn new() -> Self {
        Lab {
            inner: CoreLab::new(),
        }
    }

    /// Load a wellness-v1 directory.
    fn load_dir(&mut self, path: &str) -> PyResult<()> {
        let data = wellness_lab_io::read_dir(Path::new(path)).map_err(err)?;
        self.inner = CoreLab::with_data(data);
        Ok(())
    }

    /// Run a job (a dict or a JSON string): returns (result as a dict, arrays by name).
    fn run<'py>(
        &mut self,
        py: Python<'py>,
        job: &Bound<'py, PyAny>,
    ) -> PyResult<(Py<PyAny>, Bound<'py, PyDict>)> {
        let json = py.import("json")?;
        let text: String = if job.is_instance_of::<pyo3::types::PyString>() {
            job.extract()?
        } else {
            json.call_method1("dumps", (job,))?.extract()?
        };
        let res = self.inner.run(&text).map_err(err)?;
        let value = json.call_method1("loads", (res.json,))?.unbind();
        let arrays = PyDict::new(py);
        for (name, a) in res.arrays {
            match a {
                ArrayData::F64(v) => arrays.set_item(name, v.into_pyarray(py))?,
                ArrayData::F32(v) => arrays.set_item(name, v.into_pyarray(py))?,
                ArrayData::I32(v) => arrays.set_item(name, v.into_pyarray(py))?,
                ArrayData::U8(v) => arrays.set_item(name, v.into_pyarray(py))?,
            }
        }
        Ok((value, arrays))
    }
}

#[pyfunction]
fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[pymodule]
fn wellness_lab(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Lab>()?;
    m.add_function(wrap_pyfunction!(version, m)?)?;
    Ok(())
}
