use kitt_native_engine::model::{BlockReplaceRequest, EditRequest, SearchOptions};
use kitt_native_engine::{NativeEngine, compress_process_output_with_budget};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pythonize::pythonize;
use serde::Serialize;

fn to_py<'py, T: Serialize>(py: Python<'py>, value: &T) -> PyResult<Bound<'py, PyAny>> {
    pythonize(py, value).map_err(|error| PyRuntimeError::new_err(error.to_string()))
}

fn runtime_error(error: impl std::fmt::Display) -> PyErr {
    PyRuntimeError::new_err(error.to_string())
}

#[pyclass]
struct Engine {
    inner: NativeEngine,
}

#[pymethods]
impl Engine {
    #[new]
    fn new(root: String) -> PyResult<Self> {
        NativeEngine::new(root)
            .map(|inner| Self { inner })
            .map_err(runtime_error)
    }

    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature=(query, regex=false, case_sensitive=false, max_results=50, max_per_file=8, context_lines=1, token_budget=1200))]
    fn search<'py>(
        &self,
        py: Python<'py>,
        query: String,
        regex: bool,
        case_sensitive: bool,
        max_results: usize,
        max_per_file: usize,
        context_lines: usize,
        token_budget: usize,
    ) -> PyResult<Bound<'py, PyAny>> {
        let options = SearchOptions {
            regex,
            case_sensitive,
            max_results,
            max_per_file,
            context_lines,
            token_budget,
            include_hidden: false,
        };
        let value = py
            .allow_threads(|| self.inner.search(&query, options))
            .map_err(runtime_error)?;
        to_py(py, &value)
    }

    #[pyo3(signature=(query, limit=50))]
    fn find_symbols<'py>(
        &self,
        py: Python<'py>,
        query: String,
        limit: usize,
    ) -> PyResult<Bound<'py, PyAny>> {
        let value = py
            .allow_threads(|| self.inner.find_symbols(&query, limit))
            .map_err(runtime_error)?;
        to_py(py, &value)
    }

    fn read_symbol<'py>(
        &self,
        py: Python<'py>,
        symbol_id: String,
    ) -> PyResult<Bound<'py, PyAny>> {
        let value = py
            .allow_threads(|| self.inner.read_symbol(&symbol_id))
            .map_err(runtime_error)?;
        to_py(py, &value)
    }

    #[pyo3(signature=(symbol_id, limit=100))]
    fn references<'py>(
        &self,
        py: Python<'py>,
        symbol_id: String,
        limit: usize,
    ) -> PyResult<Bound<'py, PyAny>> {
        let value = py
            .allow_threads(|| self.inner.references(&symbol_id, limit))
            .map_err(runtime_error)?;
        to_py(py, &value)
    }

    #[pyo3(signature=(max_symbols=10000))]
    fn dependency_edges<'py>(
        &self,
        py: Python<'py>,
        max_symbols: usize,
    ) -> PyResult<Bound<'py, PyAny>> {
        let value = py
            .allow_threads(|| self.inner.dependency_edges(max_symbols))
            .map_err(runtime_error)?;
        to_py(py, &value)
    }

    #[pyo3(signature=(symbol_id, replacement, expected_hash=None, validate_syntax=true))]
    fn replace_symbol<'py>(
        &self,
        py: Python<'py>,
        symbol_id: String,
        replacement: String,
        expected_hash: Option<String>,
        validate_syntax: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        let request = EditRequest {
            symbol_id,
            replacement,
            expected_hash,
            validate_syntax,
        };
        let value = py
            .allow_threads(|| self.inner.replace_symbol(request))
            .map_err(runtime_error)?;
        to_py(py, &value)
    }

    #[pyo3(signature=(path, search, replacement, expected_file_hash=None, validate_syntax=true))]
    fn replace_block<'py>(
        &self,
        py: Python<'py>,
        path: String,
        search: String,
        replacement: String,
        expected_file_hash: Option<String>,
        validate_syntax: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        let request = BlockReplaceRequest {
            path,
            search,
            replacement,
            expected_file_hash,
            validate_syntax,
        };
        let value = py
            .allow_threads(|| self.inner.replace_block(request))
            .map_err(runtime_error)?;
        to_py(py, &value)
    }

    #[pyo3(signature=(path, start_line=1, end_line=None, max_bytes=4194304, token_budget=1200))]
    fn read_file<'py>(
        &self,
        py: Python<'py>,
        path: String,
        start_line: usize,
        end_line: Option<usize>,
        max_bytes: usize,
        token_budget: usize,
    ) -> PyResult<Bound<'py, PyAny>> {
        let value = py
            .allow_threads(|| {
                self.inner
                    .read_file(&path, start_line, end_line, max_bytes, token_budget)
            })
            .map_err(runtime_error)?;
        to_py(py, &value)
    }

    #[pyo3(signature=(path, limit=100, token_budget=600))]
    fn list_files<'py>(
        &self,
        py: Python<'py>,
        path: String,
        limit: usize,
        token_budget: usize,
    ) -> PyResult<Bound<'py, PyAny>> {
        let value = py
            .allow_threads(|| self.inner.list_files(&path, limit, token_budget))
            .map_err(runtime_error)?;
        to_py(py, &value)
    }
}

#[pyfunction]
#[pyo3(signature=(argv, stdout, stderr, returncode, token_budget=1200))]
fn compress_output<'py>(
    py: Python<'py>,
    argv: Vec<String>,
    stdout: String,
    stderr: String,
    returncode: i32,
    token_budget: usize,
) -> PyResult<Bound<'py, PyAny>> {
    let value = py.allow_threads(|| {
        compress_process_output_with_budget(&argv, &stdout, &stderr, returncode, token_budget)
    });
    to_py(py, &value)
}

#[pymodule]
fn kitt_native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Engine>()?;
    m.add_function(wrap_pyfunction!(compress_output, m)?)?;
    m.add("ENGINE_VERSION", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
