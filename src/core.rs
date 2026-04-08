//! Core logic for JSON/Protobuf conversion (pure Rust, no Python dependencies)

use prost::Message;
use prost_reflect::{DescriptorPool, DynamicMessage, Kind, MapKey, ReflectMessage, Value};
use serde_json::Value as JsonValue;
use std::path::PathBuf;

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

/// Convert a JSON string to a Protobuf message (bytes)
pub fn json_to_protobuf_bytes(
    json_str: &str,
    descriptor_bytes: &[u8],
    message_type: &str,
) -> Result<Vec<u8>, String> {
    // Parse JSON
    let json_value: JsonValue = serde_json::from_str(json_str)
        .map_err(|e| format!("Invalid JSON: {}", e))?;

    // Load descriptor pool from provided bytes
    let pool = DescriptorPool::decode(descriptor_bytes)
        .map_err(|e| format!("Failed to load descriptor pool: {}", e))?;

    // Get the Message descriptor
    let message_descriptor = pool.get_message_by_name(message_type).ok_or_else(|| {
        format!("Message type '{}' not found in descriptor", message_type)
    })?;

    // Convert JSON to DynamicMessage
    let dynamic_message = json_to_dynamic_message(&message_descriptor, &json_value)?;

    // Encode to bytes
    let mut buf = Vec::new();
    dynamic_message
        .encode(&mut buf)
        .map_err(|e| format!("Encoding error: {}", e))?;

    Ok(buf)
}

/// Convert a Protobuf message (bytes) to a JSON string
pub fn protobuf_to_json_string(
    protobuf_bytes: &[u8],
    descriptor_bytes: &[u8],
    pretty: bool,
    message_type: &str,
) -> Result<String, String> {
    // Load descriptor pool from provided bytes
    let pool = DescriptorPool::decode(descriptor_bytes)
        .map_err(|e| format!("Failed to load descriptor pool: {}", e))?;

    // Get the Message descriptor
    let message_descriptor = pool.get_message_by_name(message_type).ok_or_else(|| {
        format!("Message type '{}' not found in descriptor", message_type)
    })?;

    // Decode bytes to DynamicMessage
    let dynamic_message = DynamicMessage::decode(message_descriptor.clone(), protobuf_bytes)
        .map_err(|e| format!("Decoding error: {}", e))?;

    // Convert to JSON Value
    let json_value = dynamic_message_to_json(&dynamic_message);

    // Serialize to JSON string
    if pretty {
        serde_json::to_string_pretty(&json_value).map_err(|e| format!("JSON serialization error: {}", e))
    } else {
        serde_json::to_string(&json_value).map_err(|e| format!("JSON serialization error: {}", e))
    }
}

fn json_to_dynamic_message(
    descriptor: &prost_reflect::MessageDescriptor,
    value: &JsonValue,
) -> Result<DynamicMessage, String> {
    let obj = value
        .as_object()
        .ok_or_else(|| "Expected JSON object".to_string())?;

    let mut message = DynamicMessage::new(descriptor.clone());

    for (key, val) in obj {
        let field = descriptor.get_field_by_name(key).ok_or_else(|| {
            format!("Unknown field: {}", key)
        })?;

        let field_value = json_to_prost_value(&field, val)?;
        message.set_field(&field, field_value);
    }

    Ok(message)
}

fn json_to_prost_value(
    field: &prost_reflect::FieldDescriptor,
    value: &JsonValue,
) -> Result<Value, String> {
    Ok(match value {
        JsonValue::Null => Value::default_value_for_field(field),
        JsonValue::Bool(b) => Value::Bool(*b),
        JsonValue::Number(n) => {
            if field.is_map() || field.is_list() {
                return Err("Invalid type for repeated/map field".to_string());
            }
            match field.kind() {
                Kind::Int32 | Kind::Sint32 | Kind::Sfixed32 => {
                    n.as_i64()
                        .map(|i| Value::I32(i as i32))
                        .ok_or_else(|| "Invalid int32".to_string())?
                }
                Kind::Int64 | Kind::Sint64 | Kind::Sfixed64 => {
                    n.as_i64().map(Value::I64).ok_or_else(|| "Invalid int64".to_string())?
                }
                Kind::Uint32 | Kind::Fixed32 => {
                    n.as_u64()
                        .map(|u| Value::U32(u as u32))
                        .ok_or_else(|| "Invalid uint32".to_string())?
                }
                Kind::Uint64 | Kind::Fixed64 => {
                    n.as_u64().map(Value::U64).ok_or_else(|| "Invalid uint64".to_string())?
                }
                Kind::Float => {
                    n.as_f64()
                        .map(|f| Value::F32(f as f32))
                        .ok_or_else(|| "Invalid float".to_string())?
                }
                Kind::Double => {
                    n.as_f64().map(Value::F64).ok_or_else(|| "Invalid double".to_string())?
                }
                _ => {
                    return Err(format!("Invalid numeric type for field {}", field.name()));
                }
            }
        }
        JsonValue::String(s) => {
            // Check if this is an enum field - try to parse the string as an enum value
            if let Kind::Enum(enum_desc) = field.kind() {
                // Try to find the enum value by name
                if let Some(enum_value) = enum_desc.get_value_by_name(s) {
                    Value::EnumNumber(enum_value.number())
                } else {
                    // Collect valid enum names for error message
                    let valid_names: Vec<String> =
                        enum_desc.values().map(|v| v.name().to_string()).collect();
                    return Err(format!(
                        "Invalid enum value '{}' for field '{}'. Valid values: {:?}",
                        s,
                        field.name(),
                        valid_names
                    ));
                }
            } else {
                Value::String(s.clone())
            }
        }
        JsonValue::Array(arr) => {
            let values: Vec<_> = arr
                .iter()
                .map(|v| json_to_prost_value(field, v))
                .collect::<Result<_, _>>()?;
            Value::List(values)
        }
        JsonValue::Object(obj) => {
            if let Kind::Message(msg_desc) = field.kind() {
                let mut msg = DynamicMessage::new(msg_desc.clone());
                for (k, v) in obj {
                    if let Some(f) = msg_desc.get_field_by_name(k) {
                        let val = json_to_prost_value(&f, v).unwrap();
                        msg.set_field(&f, val);
                    }
                }
                Value::Message(msg)
            } else {
                return Err(format!("Expected message type for field {}", field.name()));
            }
        }
    })
}

fn dynamic_message_to_json(message: &DynamicMessage) -> JsonValue {
    let mut obj = serde_json::Map::new();

    for field in message.descriptor().fields() {
        if !message.has_field(&field) {
            continue;
        }

        let value = message.get_field(&field);
        obj.insert(field.name().to_string(), prost_value_to_json(&value));
    }

    JsonValue::Object(obj)
}

fn prost_value_to_json(value: &Value) -> JsonValue {
    match value {
        Value::Bool(b) => JsonValue::Bool(*b),
        Value::I32(i) => JsonValue::Number((*i).into()),
        Value::I64(i) => JsonValue::Number((*i).into()),
        Value::U32(u) => JsonValue::Number((*u).into()),
        Value::U64(u) => JsonValue::Number((*u).into()),
        Value::F32(f) => serde_json::Number::from_f64((*f).into())
            .map(JsonValue::Number)
            .unwrap_or(JsonValue::Null),
        Value::F64(f) => serde_json::Number::from_f64(*f)
            .map(JsonValue::Number)
            .unwrap_or(JsonValue::Null),
        Value::String(s) => JsonValue::String(s.clone()),
        Value::EnumNumber(e) => JsonValue::Number((*e).into()),
        Value::Message(msg) => dynamic_message_to_json(msg),
        Value::List(list) => {
            JsonValue::Array(list.iter().map(|v| prost_value_to_json(v)).collect())
        }
        Value::Map(map) => {
            let obj: serde_json::Map<_, _> = map
                .iter()
                .map(|(k, v)| {
                    let key = match k {
                        MapKey::String(s) => s.clone(),
                        MapKey::I32(i) => i.to_string(),
                        MapKey::I64(i) => i.to_string(),
                        MapKey::U32(u) => u.to_string(),
                        MapKey::U64(u) => u.to_string(),
                        MapKey::Bool(b) => b.to_string(),
                    };
                    (key, prost_value_to_json(v))
                })
                .collect();
            JsonValue::Object(obj)
        }
        _ => JsonValue::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
