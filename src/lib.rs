//! Rust JSON Protobuf - Python bindings
//! 
//! This module provides Python bindings for JSON/Protobuf conversion.
//! The core logic is in the `core` module, this module just provides PyO3 bindings.

mod core;

use pyo3::prelude::*;
use pyo3::types::PyBytes;

/// Compile a .proto file to a descriptor set (bytes)
#[pyfunction]
#[pyo3(signature = (proto_path, include_paths = None))]
fn compile_proto(proto_path: &str, include_paths: Option<Vec<String>>) -> PyResult<Vec<u8>> {
    core::compile_proto(proto_path, include_paths).map_err(|e| {
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
    let protobuf_bytes = core::json_to_protobuf_bytes(json_str, descriptor_bytes, message_type)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e))?;
    
    Ok(PyBytes::new(py, &protobuf_bytes))
}

/// Convert a Protobuf message (bytes) to a JSON string
#[pyfunction]
fn protobuf_to_json(
    protobuf_bytes: &[u8],
    descriptor_bytes: &[u8],
    pretty: bool,
    message_type: &str,
) -> PyResult<String> {
    core::protobuf_to_json_string(protobuf_bytes, descriptor_bytes, pretty, message_type)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e))
}

/// Python module for protoruf
#[pymodule]
fn _protoruf(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(compile_proto, m)?)?;
    m.add_function(wrap_pyfunction!(json_to_protobuf, m)?)?;
    m.add_function(wrap_pyfunction!(protobuf_to_json, m)?)?;
    Ok(())
}
