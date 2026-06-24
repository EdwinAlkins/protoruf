//! Core logic for JSON/Protobuf conversion (pure Rust, no Python dependencies)

use crate::pool_cache;
use prost::Message;
use prost_reflect::{
    DescriptorPool, DeserializeOptions, DynamicMessage, MessageDescriptor, SerializeOptions,
};
use protox::file::{ChainFileResolver, File, FileResolver, GoogleFileResolver};
use protox::{Compiler, Error as ProtoxError};
use std::collections::HashMap;
use std::path::PathBuf;

/// Serialization options matching protoruf's historical JSON shape:
/// proto field names (snake_case), 64-bit integers as JSON numbers, and enums
/// as their numeric value.
fn serialize_options() -> SerializeOptions {
    SerializeOptions::new()
        .use_proto_field_name(true)
        .stringify_64_bit_integers(false)
        .use_enum_numbers(true)
}

/// Compile a .proto file to a descriptor set (bytes)
///
/// Reads the filesystem, so it is only wired up by the Python and Node bindings;
/// the WASM target deliberately omits it (use [`compile_proto_from_sources`]).
/// Allow it to be unused in wasm-only builds without a warning.
#[cfg_attr(not(any(feature = "python", feature = "node", test)), allow(dead_code))]
pub fn compile_proto(
    proto_path: &str,
    include_paths: Option<Vec<String>>,
) -> Result<Vec<u8>, String> {
    let proto_path = PathBuf::from(proto_path);

    // Determine include paths
    let include_paths: Vec<String> = include_paths.unwrap_or_else(|| {
        proto_path
            .parent()
            .map(|p| vec![p.to_string_lossy().to_string()])
            .unwrap_or_default()
    });

    // Convert to &str references for protox
    let include_paths_ref: Vec<&str> = include_paths.iter().map(|s| s.as_str()).collect();

    // Compile using protox
    let file_descriptor_set = protox::compile([&proto_path], &include_paths_ref)
        .map_err(|e| format!("Failed to compile proto file: {}", e))?;

    // Serialize to bytes
    let mut descriptor_bytes = Vec::new();
    file_descriptor_set
        .encode(&mut descriptor_bytes)
        .map_err(|e| format!("Failed to encode descriptor set: {}", e))?;

    Ok(descriptor_bytes)
}

/// A [`FileResolver`] that serves `.proto` sources held in memory (name -> source).
///
/// This is what makes compilation work without any filesystem access (e.g. in a
/// browser/WASM context): `import` statements between protos are resolved by name
/// against this map.
struct InMemoryResolver {
    files: HashMap<String, String>,
}

impl FileResolver for InMemoryResolver {
    fn open_file(&self, name: &str) -> Result<File, ProtoxError> {
        match self.files.get(name) {
            Some(source) => File::from_source(name, source),
            None => Err(ProtoxError::file_not_found(name)),
        }
    }
}

/// Compile a set of `.proto` files provided in memory into a descriptor set (bytes).
///
/// `files` maps each file's logical name (e.g. `"user.proto"`, `"common/types.proto"`)
/// to its source text. `root` is the entry file to compile (must be a key of `files`).
///
/// Unlike [`compile_proto`], this performs **no filesystem access**, so it works in
/// sandboxed targets such as WASM/the browser. Google well-known types
/// (`google/protobuf/*.proto`) are still resolved automatically. The resulting bytes
/// are identical to what [`compile_proto`] would produce for the same sources.
pub fn compile_proto_from_sources(
    files: HashMap<String, String>,
    root: &str,
    include_imports: bool,
) -> Result<Vec<u8>, String> {
    let mut resolver = ChainFileResolver::new();
    resolver.add(InMemoryResolver { files }); // user-provided protos take priority
    resolver.add(GoogleFileResolver::new()); // embedded well-known types

    let mut compiler = Compiler::with_file_resolver(resolver);
    // When true, transitively-imported files (and well-known types) are embedded so
    // the descriptor set is self-contained. When false, the output is smaller and
    // decodes faster for callers that do not need Google well-known types.
    compiler.include_imports(include_imports);
    compiler
        .open_file(root)
        .map_err(|e| format!("Failed to compile proto file: {}", e))?;
    Ok(compiler.encode_file_descriptor_set())
}

/// Decode a serialized `FileDescriptorSet` into a reusable [`DescriptorPool`].
///
/// This is the expensive step: callers that convert many messages should decode
/// the pool once and reuse it via the `*_with_descriptor` helpers below.
pub fn load_descriptor_pool(descriptor_bytes: &[u8]) -> Result<DescriptorPool, String> {
    DescriptorPool::decode(descriptor_bytes)
        .map_err(|e| format!("Failed to load descriptor pool: {}", e))
}

/// Look up a message descriptor by its fully-qualified name in a pool.
pub fn get_message_descriptor(
    pool: &DescriptorPool,
    message_type: &str,
) -> Result<MessageDescriptor, String> {
    pool.get_message_by_name(message_type)
        .ok_or_else(|| format!("Message type '{}' not found in descriptor", message_type))
}

/// Convert a JSON string to Protobuf bytes using an owned message descriptor.
pub fn json_to_protobuf_bytes_with_descriptor_owned(
    json_str: &str,
    message_descriptor: MessageDescriptor,
) -> Result<Vec<u8>, String> {
    // Deserialize JSON straight into a DynamicMessage using prost-reflect's
    // proto3 JSON mapping (no intermediate serde_json::Value tree).
    let mut deserializer = serde_json::Deserializer::from_str(json_str);
    let dynamic_message = DynamicMessage::deserialize_with_options(
        message_descriptor,
        &mut deserializer,
        &DeserializeOptions::new(),
    )
    .map_err(|e| format!("Invalid JSON: {}", e))?;
    deserializer
        .end()
        .map_err(|e| format!("Invalid JSON: {}", e))?;

    // Encode to bytes, pre-allocating the exact size to avoid reallocations
    let mut buf = Vec::with_capacity(dynamic_message.encoded_len());
    dynamic_message
        .encode(&mut buf)
        .map_err(|e| format!("Encoding error: {}", e))?;

    Ok(buf)
}

/// Convert a JSON string to Protobuf bytes using an already-resolved descriptor.
pub fn json_to_protobuf_bytes_with_descriptor(
    json_str: &str,
    message_descriptor: &MessageDescriptor,
) -> Result<Vec<u8>, String> {
    json_to_protobuf_bytes_with_descriptor_owned(json_str, message_descriptor.clone())
}

/// Convert Protobuf bytes to a JSON string using an owned message descriptor.
pub fn protobuf_to_json_string_with_descriptor_owned(
    protobuf_bytes: &[u8],
    message_descriptor: MessageDescriptor,
    pretty: bool,
) -> Result<String, String> {
    let dynamic_message = DynamicMessage::decode(message_descriptor, protobuf_bytes)
        .map_err(|e| format!("Decoding error: {}", e))?;

    let options = serialize_options();
    // Pre-allocate: JSON is typically a few times larger than the protobuf wire
    // format, so this avoids repeated reallocations as the buffer grows.
    let estimated_capacity = protobuf_bytes.len().saturating_mul(3).max(128);
    let buf = Vec::with_capacity(estimated_capacity);
    let bytes = if pretty {
        let mut serializer = serde_json::Serializer::pretty(buf);
        dynamic_message
            .serialize_with_options(&mut serializer, &options)
            .map_err(|e| format!("JSON serialization error: {}", e))?;
        serializer.into_inner()
    } else {
        let mut serializer = serde_json::Serializer::new(buf);
        dynamic_message
            .serialize_with_options(&mut serializer, &options)
            .map_err(|e| format!("JSON serialization error: {}", e))?;
        serializer.into_inner()
    };

    // SAFETY: `serde_json::Serializer` only emits valid UTF-8.
    Ok(unsafe { String::from_utf8_unchecked(bytes) })
}

/// Convert Protobuf bytes to a JSON string using an already-resolved descriptor.
pub fn protobuf_to_json_string_with_descriptor(
    protobuf_bytes: &[u8],
    message_descriptor: &MessageDescriptor,
    pretty: bool,
) -> Result<String, String> {
    protobuf_to_json_string_with_descriptor_owned(
        protobuf_bytes,
        message_descriptor.clone(),
        pretty,
    )
}

/// Convert a JSON string to a Protobuf message (bytes)
pub fn json_to_protobuf_bytes(
    json_str: &str,
    descriptor_bytes: &[u8],
    message_type: &str,
) -> Result<Vec<u8>, String> {
    let pool = pool_cache::load_descriptor_pool_cached(descriptor_bytes)?;
    let message_descriptor = get_message_descriptor(&pool, message_type)?;
    json_to_protobuf_bytes_with_descriptor_owned(json_str, message_descriptor)
}

/// Convert a Protobuf message (bytes) to a JSON string
pub fn protobuf_to_json_string(
    protobuf_bytes: &[u8],
    descriptor_bytes: &[u8],
    message_type: &str,
    pretty: bool,
) -> Result<String, String> {
    let pool = pool_cache::load_descriptor_pool_cached(descriptor_bytes)?;
    let message_descriptor = get_message_descriptor(&pool, message_type)?;
    protobuf_to_json_string_with_descriptor_owned(protobuf_bytes, message_descriptor, pretty)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value as JsonValue;

    fn get_test_descriptor() -> Vec<u8> {
        let proto_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/proto/message.proto");
        compile_proto(proto_path.to_str().unwrap(), None).unwrap()
    }

    #[test]
    fn test_compile_proto() {
        let descriptor = get_test_descriptor();
        assert!(!descriptor.is_empty());
        assert!(descriptor.len() > 100); // Descriptor should have some size
    }

    #[test]
    fn test_json_to_protobuf_roundtrip() {
        let descriptor = get_test_descriptor();

        let json_input = r#"{
            "id": "test-123",
            "content": "Hello World",
            "priority": 5,
            "tags": ["rust", "test"],
            "metadata": {
                "author": "TestUser",
                "created_at": 1234567890,
                "attributes": {"env": "test", "version": "1.0"}
            }
        }"#;

        // JSON -> Protobuf
        let protobuf_bytes =
            json_to_protobuf_bytes(json_input, &descriptor, "message.Message").unwrap();
        assert!(!protobuf_bytes.is_empty());

        // Protobuf -> JSON
        let json_output =
            protobuf_to_json_string(&protobuf_bytes, &descriptor, "message.Message", false)
                .unwrap();

        // Verify roundtrip
        let result: JsonValue = serde_json::from_str(&json_output).unwrap();
        assert_eq!(result["id"], "test-123");
        assert_eq!(result["content"], "Hello World");
        assert_eq!(result["priority"], 5);
        assert_eq!(result["tags"], serde_json::json!(["rust", "test"]));
        assert_eq!(result["metadata"]["author"], "TestUser");
        assert_eq!(result["metadata"]["created_at"], 1234567890);
        // Map fields must survive the roundtrip (regression test for map bug).
        assert_eq!(result["metadata"]["attributes"]["env"], "test");
        assert_eq!(result["metadata"]["attributes"]["version"], "1.0");
    }

    #[test]
    fn test_map_field_roundtrip() {
        let descriptor = get_test_descriptor();
        let json_input = r#"{
            "id": "map-test",
            "metadata": {
                "attributes": {"a": "1", "b": "2", "c": "3"}
            }
        }"#;

        let protobuf_bytes =
            json_to_protobuf_bytes(json_input, &descriptor, "message.Message").unwrap();
        let json_output =
            protobuf_to_json_string(&protobuf_bytes, &descriptor, "message.Message", false)
                .unwrap();

        let result: JsonValue = serde_json::from_str(&json_output).unwrap();
        let attrs = &result["metadata"]["attributes"];
        assert_eq!(attrs["a"], "1");
        assert_eq!(attrs["b"], "2");
        assert_eq!(attrs["c"], "3");
    }

    #[test]
    fn test_json_to_protobuf_simple_message() {
        let descriptor = get_test_descriptor();
        let json_input = r#"{"id": "1", "content": "Simple"}"#;

        let protobuf_bytes =
            json_to_protobuf_bytes(json_input, &descriptor, "message.Message").unwrap();
        assert!(!protobuf_bytes.is_empty());
    }

    #[test]
    fn test_protobuf_to_json_pretty() {
        let descriptor = get_test_descriptor();
        let json_input = r#"{"id": "1", "content": "Pretty test"}"#;

        let protobuf_bytes =
            json_to_protobuf_bytes(json_input, &descriptor, "message.Message").unwrap();
        let pretty_json =
            protobuf_to_json_string(&protobuf_bytes, &descriptor, "message.Message", true).unwrap();

        // Pretty JSON should contain newlines
        assert!(pretty_json.contains('\n'));
    }

    #[test]
    fn test_invalid_json() {
        let descriptor = get_test_descriptor();
        let invalid_json = "not valid json";

        let result = json_to_protobuf_bytes(invalid_json, &descriptor, "message.Message");
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_message_type() {
        let descriptor = get_test_descriptor();
        let json_input = r#"{"id": "1"}"#;

        let result = json_to_protobuf_bytes(json_input, &descriptor, "nonexistent.Message");
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_descriptor() {
        let empty_descriptor: Vec<u8> = vec![];
        let json_input = r#"{"id": "1"}"#;

        let result = json_to_protobuf_bytes(json_input, &empty_descriptor, "message.Message");
        assert!(result.is_err());
    }

    #[test]
    fn test_message_with_all_fields() {
        let descriptor = get_test_descriptor();

        let json_input = r#"{
            "id": "full-test",
            "content": "Complete message",
            "priority": 10,
            "tags": ["a", "b", "c"],
            "metadata": {
                "author": "FullUser",
                "created_at": 9999999999,
                "attributes": {
                    "key1": "value1",
                    "key2": "value2"
                }
            }
        }"#;

        let protobuf_bytes =
            json_to_protobuf_bytes(json_input, &descriptor, "message.Message").unwrap();
        let json_output =
            protobuf_to_json_string(&protobuf_bytes, &descriptor, "message.Message", true).unwrap();

        let result: JsonValue = serde_json::from_str(&json_output).unwrap();
        assert_eq!(result["id"], "full-test");
        assert_eq!(result["content"], "Complete message");
        assert_eq!(result["priority"], 10);
        assert_eq!(result["tags"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_message_with_minimal_fields() {
        let descriptor = get_test_descriptor();
        let json_input = r#"{"id": "minimal"}"#;

        let protobuf_bytes =
            json_to_protobuf_bytes(json_input, &descriptor, "message.Message").unwrap();
        let json_output =
            protobuf_to_json_string(&protobuf_bytes, &descriptor, "message.Message", false)
                .unwrap();

        let result: JsonValue = serde_json::from_str(&json_output).unwrap();
        assert_eq!(result["id"], "minimal");
    }

    // --- Tests for the cached/descriptor-based path (backing DescriptorCache) ---

    #[test]
    fn test_load_descriptor_pool_valid() {
        let descriptor = get_test_descriptor();
        let pool = load_descriptor_pool(&descriptor).unwrap();
        assert!(pool.get_message_by_name("message.Message").is_some());
    }

    #[test]
    fn test_load_descriptor_pool_invalid() {
        assert!(load_descriptor_pool(b"not a descriptor").is_err());
    }

    #[test]
    fn test_load_descriptor_pool_empty_is_valid_but_has_no_messages() {
        // Empty bytes decode to a valid, empty FileDescriptorSet (no messages).
        let pool = load_descriptor_pool(&[]).unwrap();
        assert!(get_message_descriptor(&pool, "message.Message").is_err());
    }

    #[test]
    fn test_get_message_descriptor_found_and_missing() {
        let descriptor = get_test_descriptor();
        let pool = load_descriptor_pool(&descriptor).unwrap();

        let desc = get_message_descriptor(&pool, "message.Message").unwrap();
        assert_eq!(desc.full_name(), "message.Message");

        assert!(get_message_descriptor(&pool, "message.DoesNotExist").is_err());
    }

    #[test]
    fn test_with_descriptor_roundtrip() {
        let descriptor = get_test_descriptor();
        let pool = load_descriptor_pool(&descriptor).unwrap();
        let desc = get_message_descriptor(&pool, "message.Message").unwrap();

        let json_input = r#"{
            "id": "cache-test",
            "content": "via descriptor",
            "metadata": {"attributes": {"k": "v"}}
        }"#;

        let protobuf_bytes = json_to_protobuf_bytes_with_descriptor(json_input, &desc).unwrap();
        let json_output =
            protobuf_to_json_string_with_descriptor(&protobuf_bytes, &desc, false).unwrap();

        let result: JsonValue = serde_json::from_str(&json_output).unwrap();
        assert_eq!(result["id"], "cache-test");
        assert_eq!(result["content"], "via descriptor");
        assert_eq!(result["metadata"]["attributes"]["k"], "v");
    }

    #[test]
    fn test_with_descriptor_reuse_for_many_messages() {
        // A single resolved descriptor (as DescriptorCache holds) handles many messages.
        let descriptor = get_test_descriptor();
        let pool = load_descriptor_pool(&descriptor).unwrap();
        let desc = get_message_descriptor(&pool, "message.Message").unwrap();

        for i in 0..50 {
            let json_input = format!(r#"{{"id": "{}", "priority": {}}}"#, i, i);
            let protobuf_bytes =
                json_to_protobuf_bytes_with_descriptor(&json_input, &desc).unwrap();
            let json_output =
                protobuf_to_json_string_with_descriptor(&protobuf_bytes, &desc, false).unwrap();
            let result: JsonValue = serde_json::from_str(&json_output).unwrap();
            assert_eq!(result["id"], i.to_string());
        }
    }

    #[test]
    fn test_with_descriptor_matches_byte_functions() {
        // The descriptor-based path must produce identical output to the byte-based one.
        let descriptor = get_test_descriptor();
        let pool = load_descriptor_pool(&descriptor).unwrap();
        let desc = get_message_descriptor(&pool, "message.Message").unwrap();

        let json_input = r#"{"id": "x", "content": "y", "tags": ["a"]}"#;

        let from_desc = json_to_protobuf_bytes_with_descriptor(json_input, &desc).unwrap();
        let from_bytes =
            json_to_protobuf_bytes(json_input, &descriptor, "message.Message").unwrap();
        assert_eq!(from_desc, from_bytes);

        let json_desc = protobuf_to_json_string_with_descriptor(&from_desc, &desc, false).unwrap();
        let json_bytes =
            protobuf_to_json_string(&from_bytes, &descriptor, "message.Message", false).unwrap();
        assert_eq!(json_desc, json_bytes);
    }

    #[test]
    fn test_with_descriptor_invalid_json() {
        let descriptor = get_test_descriptor();
        let pool = load_descriptor_pool(&descriptor).unwrap();
        let desc = get_message_descriptor(&pool, "message.Message").unwrap();

        assert!(json_to_protobuf_bytes_with_descriptor("not valid json", &desc).is_err());
    }

    // --- Tests for in-memory compilation (backing the WASM target) ---

    #[test]
    fn test_compile_from_sources_single_file() {
        let proto = r#"syntax = "proto3"; package user; message User { string id = 1; repeated string tags = 2; }"#;
        let files = HashMap::from([("user.proto".to_string(), proto.to_string())]);

        let descriptor = compile_proto_from_sources(files, "user.proto", true).unwrap();
        assert!(!descriptor.is_empty());

        // The descriptor must be usable for a normal round-trip.
        let pb =
            json_to_protobuf_bytes(r#"{"id":"123","tags":["a","b"]}"#, &descriptor, "user.User")
                .unwrap();
        let json = protobuf_to_json_string(&pb, &descriptor, "user.User", false).unwrap();
        let result: JsonValue = serde_json::from_str(&json).unwrap();
        assert_eq!(result["id"], "123");
        assert_eq!(result["tags"], serde_json::json!(["a", "b"]));
    }

    #[test]
    fn test_compile_from_sources_with_import() {
        // Two in-memory files, one importing the other (resolved by name, no FS).
        let common = r#"syntax = "proto3"; package common; message Id { string value = 1; }"#;
        let root = r#"syntax = "proto3"; package user; import "common.proto"; message User { common.Id id = 1; }"#;
        let files = HashMap::from([
            ("common.proto".to_string(), common.to_string()),
            ("user.proto".to_string(), root.to_string()),
        ]);

        let descriptor = compile_proto_from_sources(files, "user.proto", true).unwrap();
        let pb =
            json_to_protobuf_bytes(r#"{"id":{"value":"x"}}"#, &descriptor, "user.User").unwrap();
        let json = protobuf_to_json_string(&pb, &descriptor, "user.User", false).unwrap();
        let result: JsonValue = serde_json::from_str(&json).unwrap();
        assert_eq!(result["id"]["value"], "x");
    }

    #[test]
    fn test_compile_from_sources_with_google_wellknown() {
        // google/protobuf/* imports must resolve from the embedded GoogleFileResolver.
        let root = r#"syntax = "proto3"; package ev; import "google/protobuf/timestamp.proto"; message Event { google.protobuf.Timestamp at = 1; }"#;
        let files = HashMap::from([("ev.proto".to_string(), root.to_string())]);

        let descriptor = compile_proto_from_sources(files, "ev.proto", true).unwrap();
        assert!(!descriptor.is_empty());
        let pool = load_descriptor_pool(&descriptor).unwrap();
        assert!(get_message_descriptor(&pool, "ev.Event").is_ok());
    }

    #[test]
    fn test_compile_from_sources_matches_compile_proto() {
        // In-memory compilation must produce a descriptor equivalent to the FS-based one.
        let proto_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/proto/message.proto");
        let source = std::fs::read_to_string(&proto_path).unwrap();
        let files = HashMap::from([("message.proto".to_string(), source)]);

        let from_sources = compile_proto_from_sources(files, "message.proto", true).unwrap();

        // Both descriptors must resolve the same message and round-trip identically.
        let json_input = r#"{"id":"1","content":"hi"}"#;
        let a = json_to_protobuf_bytes(json_input, &from_sources, "message.Message").unwrap();
        let b =
            json_to_protobuf_bytes(json_input, &get_test_descriptor(), "message.Message").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn test_compile_from_sources_missing_root() {
        let files = HashMap::new();
        assert!(compile_proto_from_sources(files, "nope.proto", true).is_err());
    }

    #[test]
    fn test_compile_from_sources_invalid_syntax() {
        let files = HashMap::from([(
            "bad.proto".to_string(),
            "this is not valid proto".to_string(),
        )]);
        assert!(compile_proto_from_sources(files, "bad.proto", true).is_err());
    }
}
