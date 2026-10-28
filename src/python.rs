//! Python bindings (PyO3) for JSON/Protobuf conversion.
//!
//! The core logic lives in [`crate::core`]; this module only translates types and
//! errors between Rust and Python.

use crate::core;
use crate::describe;
use crate::descriptor_resolver::DescriptorResolver;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::collections::HashMap;

/// Compile a .proto file to a descriptor set (bytes)
#[pyfunction]
#[pyo3(signature = (proto_path, include_paths = None))]
fn compile_proto(proto_path: &str, include_paths: Option<Vec<String>>) -> PyResult<Vec<u8>> {
    core::compile_proto(proto_path, include_paths)
        .map_err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>)
}

/// Compile a set of .proto files provided in memory (no filesystem access)
#[pyfunction]
#[pyo3(signature = (files, root, include_imports = true))]
fn compile_proto_from_sources(
    files: HashMap<String, String>,
    root: &str,
    include_imports: bool,
) -> PyResult<Vec<u8>> {
    core::compile_proto_from_sources(files, root, include_imports)
        .map_err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>)
}

/// Compile several root .proto files from memory and/or disk, with the standard protos
#[pyfunction]
#[pyo3(signature = (
    roots,
    include_paths = Vec::new(),
    sources = HashMap::new(),
    include_imports = true,
    source_info = false,
    standard_protos = true,
))]
fn compile_protos(
    py: Python<'_>,
    roots: Vec<String>,
    include_paths: Vec<String>,
    sources: HashMap<String, String>,
    include_imports: bool,
    source_info: bool,
    standard_protos: bool,
) -> PyResult<Vec<u8>> {
    let options = core::CompileOptions {
        include_imports,
        source_info,
        standard_protos,
    };
    py.detach(|| core::compile_protos(&roots, &include_paths, sources, options))
        .map_err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>)
}

/// Describe a descriptor set (files, messages, enums, services) as a JSON document
#[pyfunction]
fn describe_descriptor_set(py: Python<'_>, descriptor_bytes: &[u8]) -> PyResult<String> {
    py.detach(|| describe::describe_descriptor_set(descriptor_bytes))
        .map_err(PyErr::new::<pyo3::exceptions::PyValueError, _>)
}

/// Split a descriptor set into (file name, FileDescriptorProto bytes), dependencies first
#[pyfunction]
fn split_descriptor_set(
    py: Python<'_>,
    descriptor_bytes: &[u8],
) -> PyResult<Vec<(String, Vec<u8>)>> {
    py.detach(|| describe::split_descriptor_set(descriptor_bytes))
        .map_err(PyErr::new::<pyo3::exceptions::PyValueError, _>)
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
        .map_err(PyErr::new::<pyo3::exceptions::PyValueError, _>)?;

    Ok(PyBytes::new(py, &protobuf_bytes))
}

/// Convert a Protobuf message (bytes) to a JSON string
#[pyfunction]
#[pyo3(signature = (protobuf_bytes, descriptor_bytes, message_type, pretty = false, include_default_values = false))]
fn protobuf_to_json<'py>(
    py: Python<'py>,
    protobuf_bytes: &[u8],
    descriptor_bytes: &[u8],
    message_type: &str,
    pretty: bool,
    include_default_values: bool,
) -> PyResult<String> {
    let options = core::JsonOutputOptions {
        pretty,
        include_default_values,
    };
    py.detach(|| {
        core::protobuf_to_json_string_with_options(
            protobuf_bytes,
            descriptor_bytes,
            message_type,
            options,
        )
    })
    .map_err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>)
}

/// A reusable, pre-decoded descriptor pool.
///
/// Holds a decoded pool and memoized message descriptors for repeated conversions.
/// Free functions also reuse pools through a process-wide LRU.
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
            .map_err(PyErr::new::<pyo3::exceptions::PyValueError, _>)?;

        Ok(PyBytes::new(py, &protobuf_bytes))
    }

    /// Convert a Protobuf message (bytes) to a JSON string.
    #[pyo3(signature = (protobuf_bytes, message_type, pretty = false, include_default_values = false))]
    fn protobuf_to_json(
        &self,
        py: Python<'_>,
        protobuf_bytes: &[u8],
        message_type: &str,
        pretty: bool,
        include_default_values: bool,
    ) -> PyResult<String> {
        let options = core::JsonOutputOptions {
            pretty,
            include_default_values,
        };
        py.detach(|| {
            let desc = self.resolver.resolve(message_type)?;
            core::protobuf_to_json_string_with_descriptor_options(protobuf_bytes, desc, options)
        })
        .map_err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>)
    }
}

/// Python module for protoruf
#[pymodule]
fn _protoruf(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(compile_proto, m)?)?;
    m.add_function(wrap_pyfunction!(compile_proto_from_sources, m)?)?;
    m.add_function(wrap_pyfunction!(compile_protos, m)?)?;
    m.add_function(wrap_pyfunction!(describe_descriptor_set, m)?)?;
    m.add_function(wrap_pyfunction!(split_descriptor_set, m)?)?;
    m.add_function(wrap_pyfunction!(json_to_protobuf, m)?)?;
    m.add_function(wrap_pyfunction!(protobuf_to_json, m)?)?;
    m.add_class::<DescriptorCache>()?;
    m.add_class::<crate::python_native::NativeCodec>()?;
    Ok(())
}
