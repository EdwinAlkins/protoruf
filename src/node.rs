//! Node.js native addon (napi-rs) for JSON/Protobuf conversion.
//!
//! Mirrors the Python binding: a thin wrapper translating types & errors around
//! `core::*`. Binary payloads cross the boundary as Node `Buffer`s (which are
//! `Uint8Array`s at runtime). Filesystem access is available, so `compile_proto`
//! works exactly like in Python.

use crate::core;
use napi::bindgen_prelude::{Buffer, Error, Result};
use napi_derive::napi;
use prost_reflect::DescriptorPool;
use std::collections::HashMap;

/// Compile a `.proto` file from disk to a descriptor set.
#[napi]
pub fn compile_proto(
    proto_path: String,
    include_paths: Option<Vec<String>>,
) -> Result<Buffer> {
    core::compile_proto(&proto_path, include_paths)
        .map(Buffer::from)
        .map_err(|e| Error::from_reason(e))
}

/// Compile `.proto` sources provided in memory (no filesystem access).
///
/// `files` maps each file's logical name to its source; `root` is the entry file.
#[napi]
pub fn compile_proto_from_sources(
    files: HashMap<String, String>,
    root: String,
) -> Result<Buffer> {
    core::compile_proto_from_sources(files, &root)
        .map(Buffer::from)
        .map_err(|e| Error::from_reason(e))
}

/// Convert a JSON string to a Protobuf message.
#[napi]
pub fn json_to_protobuf(
    json_str: String,
    descriptor_bytes: Buffer,
    message_type: String,
) -> Result<Buffer> {
    core::json_to_protobuf_bytes(&json_str, &descriptor_bytes, &message_type)
        .map(Buffer::from)
        .map_err(|e| Error::from_reason(e))
}

/// Convert a Protobuf message to a JSON string.
#[napi]
pub fn protobuf_to_json(
    protobuf_bytes: Buffer,
    descriptor_bytes: Buffer,
    pretty: Option<bool>,
    message_type: String,
) -> Result<String> {
    core::protobuf_to_json_string(
        &protobuf_bytes,
        &descriptor_bytes,
        pretty.unwrap_or(false),
        &message_type,
    )
    .map_err(|e| Error::from_reason(e))
}

/// A reusable, pre-decoded descriptor pool (equivalent of the Python class).
///
/// Decoding the descriptor set is the dominant cost of every conversion; build
/// this once and reuse it across calls.
#[napi]
pub struct DescriptorCache {
    pool: DescriptorPool,
}

#[napi]
impl DescriptorCache {
    #[napi(constructor)]
    pub fn new(descriptor_bytes: Buffer) -> Result<Self> {
        let pool = core::load_descriptor_pool(&descriptor_bytes)
            .map_err(|e| Error::from_reason(e))?;
        Ok(Self { pool })
    }

    #[napi]
    pub fn json_to_protobuf(
        &self,
        json_str: String,
        message_type: String,
    ) -> Result<Buffer> {
        let desc = core::get_message_descriptor(&self.pool, &message_type)
            .map_err(|e| Error::from_reason(e))?;
        core::json_to_protobuf_bytes_with_descriptor(&json_str, &desc)
            .map(Buffer::from)
            .map_err(|e| Error::from_reason(e))
    }

    #[napi]
    pub fn protobuf_to_json(
        &self,
        protobuf_bytes: Buffer,
        message_type: String,
        pretty: Option<bool>,
    ) -> Result<String> {
        let desc = core::get_message_descriptor(&self.pool, &message_type)
            .map_err(|e| Error::from_reason(e))?;
        core::protobuf_to_json_string_with_descriptor(
            &protobuf_bytes,
            &desc,
            pretty.unwrap_or(false),
        )
        .map_err(|e| Error::from_reason(e))
    }
}
