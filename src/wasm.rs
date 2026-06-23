//! WebAssembly / browser binding (wasm-bindgen) for JSON/Protobuf conversion.
//!
//! Thin wrapper around `core::*`. There is no filesystem in the browser, so
//! `compile_proto(path)` is intentionally **not** exposed — callers compile from
//! in-memory sources via [`compile_proto_from_sources`], or load a pre-compiled
//! descriptor's bytes. Binary payloads cross as `Uint8Array` (Rust `Vec<u8>`/`&[u8]`).

use crate::core;
use crate::descriptor_resolver::DescriptorResolver;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

/// Compile `.proto` sources provided in memory into a descriptor set.
///
/// `files` is a JS object mapping each file's logical name to its source text
/// (e.g. `{ "user.proto": "syntax=\"proto3\"; ..." }`); `root` is the entry file.
/// Google well-known types are resolved automatically.
#[wasm_bindgen(js_name = compileProtoFromSources)]
pub fn compile_proto_from_sources(
    files: JsValue,
    root: &str,
    include_imports: Option<bool>,
) -> Result<Vec<u8>, JsError> {
    let files: HashMap<String, String> =
        serde_wasm_bindgen::from_value(files).map_err(|e| JsError::new(&e.to_string()))?;
    core::compile_proto_from_sources(files, root, include_imports.unwrap_or(true))
        .map_err(|e| JsError::new(&e))
}

/// Convert a JSON string to a Protobuf message.
#[wasm_bindgen(js_name = jsonToProtobuf)]
pub fn json_to_protobuf(
    json_str: &str,
    descriptor_bytes: &[u8],
    message_type: &str,
) -> Result<Vec<u8>, JsError> {
    core::json_to_protobuf_bytes(json_str, descriptor_bytes, message_type)
        .map_err(|e| JsError::new(&e))
}

/// Convert a Protobuf message to a JSON string.
#[wasm_bindgen(js_name = protobufToJson)]
pub fn protobuf_to_json(
    protobuf_bytes: &[u8],
    descriptor_bytes: &[u8],
    pretty: bool,
    message_type: &str,
) -> Result<String, JsError> {
    core::protobuf_to_json_string(protobuf_bytes, descriptor_bytes, pretty, message_type)
        .map_err(|e| JsError::new(&e))
}

/// A reusable, pre-decoded descriptor pool (equivalent of the Python class).
#[wasm_bindgen]
pub struct DescriptorCache {
    resolver: DescriptorResolver,
}

#[wasm_bindgen]
impl DescriptorCache {
    #[wasm_bindgen(constructor)]
    pub fn new(descriptor_bytes: &[u8]) -> Result<DescriptorCache, JsError> {
        let resolver = DescriptorResolver::from_descriptor_bytes(descriptor_bytes)
            .map_err(|e| JsError::new(&e))?;
        Ok(Self { resolver })
    }

    #[wasm_bindgen(js_name = jsonToProtobuf)]
    pub fn json_to_protobuf(&self, json_str: &str, message_type: &str) -> Result<Vec<u8>, JsError> {
        let desc = self
            .resolver
            .resolve(message_type)
            .map_err(|e| JsError::new(&e))?;
        core::json_to_protobuf_bytes_with_descriptor_owned(json_str, desc)
            .map_err(|e| JsError::new(&e))
    }

    #[wasm_bindgen(js_name = protobufToJson)]
    pub fn protobuf_to_json(
        &self,
        protobuf_bytes: &[u8],
        message_type: &str,
        pretty: Option<bool>,
    ) -> Result<String, JsError> {
        let desc = self
            .resolver
            .resolve(message_type)
            .map_err(|e| JsError::new(&e))?;
        core::protobuf_to_json_string_with_descriptor_owned(
            protobuf_bytes,
            desc,
            pretty.unwrap_or(false),
        )
        .map_err(|e| JsError::new(&e))
    }
}
