//! Python bindings (PyO3) for JSON/Protobuf conversion.
//!
//! The core logic lives in [`crate::core`]; this module only translates types and
//! errors between Rust and Python.

use crate::core;
use crate::descriptor_resolver::DescriptorResolver;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::collections::HashMap;

/// Compile a .proto file to a descriptor set (bytes)
#[pyfunction]
#[pyo3(signature = (proto_path, include_paths = None))]
fn compile_proto(proto_path: &str, include_paths: Option<Vec<String>>) -> PyResult<Vec<u8>> {
    core::compile_proto(proto_path, include_paths).map_err(|e| {
        PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e)
    })
}

/// Compile a set of .proto files provided in memory (no filesystem access)
#[pyfunction]
#[pyo3(signature = (files, root, include_imports = true))]
fn compile_proto_from_sources(
    files: HashMap<String, String>,
    root: &str,
    include_imports: bool,
) -> PyResult<Vec<u8>> {
    core::compile_proto_from_sources(files, root, include_imports).map_err(|e| {
        PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e)
    })
}

/// Convert a JSON string to a Protobuf message (bytes)
#[pyfunction]
fn json_to_protobuf<'py>(
    py: Python<'py>,
    json_str: &str,
    descriptor_bytes: &[u8],
    message_type: &str,
) -> PyResult<Bound<'py, PyBytes>> {
    let protobuf_bytes = py
        .detach(|| core::json_to_protobuf_bytes(json_str, descriptor_bytes, message_type))
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e))?;

    Ok(PyBytes::new(py, &protobuf_bytes))
}

/// Convert a Protobuf message (bytes) to a JSON string
#[pyfunction]
fn protobuf_to_json<'py>(
    py: Python<'py>,
    protobuf_bytes: &[u8],
    descriptor_bytes: &[u8],
    pretty: bool,
    message_type: &str,
) -> PyResult<String> {
    py.detach(|| {
        core::protobuf_to_json_string(protobuf_bytes, descriptor_bytes, pretty, message_type)
    })
    .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e))
}

/// A reusable, pre-decoded descriptor pool.
///
/// Decoding the descriptor set (`DescriptorPool::decode`) is the dominant cost of
/// every conversion. Instantiate this once and reuse it across calls to avoid
/// re-decoding the pool (and re-resolving message descriptors) on every message.
#[pyclass]
struct DescriptorCache {
    resolver: DescriptorResolver,
}

#[pymethods]
impl DescriptorCache {
    #[new]
    fn new(descriptor_bytes: &[u8]) -> PyResult<Self> {
        let resolver = DescriptorResolver::from_descriptor_bytes(descriptor_bytes)
            .map_err(PyErr::new::<pyo3::exceptions::PyValueError, _>)?;
        Ok(Self { resolver })
    }

    /// Convert a JSON string to a Protobuf message (bytes).
    fn json_to_protobuf<'py>(
        &self,
        py: Python<'py>,
        json_str: &str,
        message_type: &str,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let protobuf_bytes = py
            .detach(|| {
                let desc = self.resolver.resolve(message_type)?;
                core::json_to_protobuf_bytes_with_descriptor_owned(json_str, desc)
            })
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e))?;

        Ok(PyBytes::new(py, &protobuf_bytes))
    }

    /// Convert a Protobuf message (bytes) to a JSON string.
    #[pyo3(signature = (protobuf_bytes, message_type, pretty = false))]
    fn protobuf_to_json(
        &self,
        py: Python<'_>,
        protobuf_bytes: &[u8],
        message_type: &str,
        pretty: bool,
    ) -> PyResult<String> {
        py.detach(|| {
            let desc = self.resolver.resolve(message_type)?;
            core::protobuf_to_json_string_with_descriptor_owned(protobuf_bytes, desc, pretty)
        })
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e))
    }
}

/// Python module for protoruf
#[pymodule]
fn _protoruf(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(compile_proto, m)?)?;
    m.add_function(wrap_pyfunction!(compile_proto_from_sources, m)?)?;
    m.add_function(wrap_pyfunction!(json_to_protobuf, m)?)?;
    m.add_function(wrap_pyfunction!(protobuf_to_json, m)?)?;
    m.add_class::<DescriptorCache>()?;
    Ok(())
}
