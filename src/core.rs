//! Core logic for JSON/Protobuf conversion (pure Rust, no Python dependencies)

use prost::Message;
use prost_reflect::{
    DescriptorPool, DeserializeOptions, DynamicMessage, MessageDescriptor, SerializeOptions,
};
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
pub fn compile_proto(proto_path: &str, include_paths: Option<Vec<String>>) -> Result<Vec<u8>, String> {
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
    let file_descriptor_set = protox::compile(&[&proto_path], &include_paths_ref)
        .map_err(|e| format!("Failed to compile proto file: {}", e))?;

    // Serialize to bytes
    let mut descriptor_bytes = Vec::new();
    file_descriptor_set
        .encode(&mut descriptor_bytes)
        .map_err(|e| format!("Failed to encode descriptor set: {}", e))?;

    Ok(descriptor_bytes)
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
    pool.get_message_by_name(message_type).ok_or_else(|| {
        format!("Message type '{}' not found in descriptor", message_type)
    })
}

/// Convert a JSON string to Protobuf bytes using an already-resolved descriptor.
pub fn json_to_protobuf_bytes_with_descriptor(
    json_str: &str,
    message_descriptor: &MessageDescriptor,
) -> Result<Vec<u8>, String> {
    // Deserialize JSON straight into a DynamicMessage using prost-reflect's
    // proto3 JSON mapping (no intermediate serde_json::Value tree).
    let mut deserializer = serde_json::Deserializer::from_str(json_str);
    let dynamic_message = DynamicMessage::deserialize_with_options(
        message_descriptor.clone(),
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

/// Convert Protobuf bytes to a JSON string using an already-resolved descriptor.
pub fn protobuf_to_json_string_with_descriptor(
    protobuf_bytes: &[u8],
    message_descriptor: &MessageDescriptor,
    pretty: bool,
) -> Result<String, String> {
    // Decode bytes to DynamicMessage
    let dynamic_message = DynamicMessage::decode(message_descriptor.clone(), protobuf_bytes)
        .map_err(|e| format!("Decoding error: {}", e))?;

    // Serialize directly with prost-reflect (no intermediate serde_json::Value tree).
    let options = serialize_options();
    let buf = Vec::new();
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

    String::from_utf8(bytes).map_err(|e| format!("JSON serialization error: {}", e))
}

/// Convert a JSON string to a Protobuf message (bytes)
pub fn json_to_protobuf_bytes(
    json_str: &str,
    descriptor_bytes: &[u8],
    message_type: &str,
) -> Result<Vec<u8>, String> {
    let pool = load_descriptor_pool(descriptor_bytes)?;
    let message_descriptor = get_message_descriptor(&pool, message_type)?;
    json_to_protobuf_bytes_with_descriptor(json_str, &message_descriptor)
}

/// Convert a Protobuf message (bytes) to a JSON string
pub fn protobuf_to_json_string(
    protobuf_bytes: &[u8],
    descriptor_bytes: &[u8],
    pretty: bool,
    message_type: &str,
) -> Result<String, String> {
    let pool = load_descriptor_pool(descriptor_bytes)?;
    let message_descriptor = get_message_descriptor(&pool, message_type)?;
    protobuf_to_json_string_with_descriptor(protobuf_bytes, &message_descriptor, pretty)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value as JsonValue;

    fn get_test_descriptor() -> Vec<u8> {
        let proto_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/proto/message.proto");
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
        let protobuf_bytes = json_to_protobuf_bytes(json_input, &descriptor, "message.Message").unwrap();
        assert!(!protobuf_bytes.is_empty());

        // Protobuf -> JSON
        let json_output = protobuf_to_json_string(&protobuf_bytes, &descriptor, false, "message.Message").unwrap();

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
            protobuf_to_json_string(&protobuf_bytes, &descriptor, false, "message.Message").unwrap();

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

        let protobuf_bytes = json_to_protobuf_bytes(json_input, &descriptor, "message.Message").unwrap();
        assert!(!protobuf_bytes.is_empty());
    }

    #[test]
    fn test_protobuf_to_json_pretty() {
        let descriptor = get_test_descriptor();
        let json_input = r#"{"id": "1", "content": "Pretty test"}"#;

        let protobuf_bytes = json_to_protobuf_bytes(json_input, &descriptor, "message.Message").unwrap();
        let pretty_json = protobuf_to_json_string(&protobuf_bytes, &descriptor, true, "message.Message").unwrap();

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

        let protobuf_bytes = json_to_protobuf_bytes(json_input, &descriptor, "message.Message").unwrap();
        let json_output = protobuf_to_json_string(&protobuf_bytes, &descriptor, true, "message.Message").unwrap();

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

        let protobuf_bytes = json_to_protobuf_bytes(json_input, &descriptor, "message.Message").unwrap();
        let json_output = protobuf_to_json_string(&protobuf_bytes, &descriptor, false, "message.Message").unwrap();

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
            protobuf_to_json_string(&from_bytes, &descriptor, false, "message.Message").unwrap();
        assert_eq!(json_desc, json_bytes);
    }

    #[test]
    fn test_with_descriptor_invalid_json() {
        let descriptor = get_test_descriptor();
        let pool = load_descriptor_pool(&descriptor).unwrap();
        let desc = get_message_descriptor(&pool, "message.Message").unwrap();

        assert!(json_to_protobuf_bytes_with_descriptor("not valid json", &desc).is_err());
    }
}
