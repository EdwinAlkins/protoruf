//! Describe a compiled descriptor set as plain data (files, messages, fields, enums,
//! services, comments), to generate code from `.proto` files or check models against
//! them, and split a set per file for gRPC server reflection.

use crate::core::load_descriptor_pool;
use prost::Message;
use prost_reflect::{
    Cardinality, DescriptorPool, EnumDescriptor, FieldDescriptor, Kind, MessageDescriptor,
    MethodDescriptor, ServiceDescriptor, Syntax,
};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize)]
pub struct SchemaDescription {
    pub files: Vec<FileDescription>,
    /// Every message, nested ones and map entries included.
    pub messages: Vec<MessageDescription>,
    /// Every enum, nested ones included.
    pub enums: Vec<EnumDescription>,
    pub services: Vec<ServiceDescription>,
}

#[derive(Serialize)]
pub struct FileDescription {
    pub name: String,
    pub package: String,
    pub syntax: &'static str,
    pub dependencies: Vec<String>,
    /// Full names of the top-level messages, enums and services.
    pub messages: Vec<String>,
    pub enums: Vec<String>,
    pub services: Vec<String>,
}

/// Comments attached to an element; only present when compiled with source info.
#[derive(Clone, Serialize)]
pub struct Comments {
    pub leading: Option<String>,
    pub trailing: Option<String>,
    pub detached: Vec<String>,
}

#[derive(Serialize)]
pub struct MessageDescription {
    pub name: String,
    pub full_name: String,
    pub file: String,
    /// Full name of the enclosing message, for nested messages.
    pub parent: Option<String>,
    pub fields: Vec<FieldDescription>,
    /// Real `oneof`s only: the synthetic ones of proto3 `optional` are left out.
    pub oneofs: Vec<OneofDescription>,
    /// Inclusive `(start, end)` pairs, as written in `reserved 15 to 20;`.
    pub reserved_ranges: Vec<(i64, i64)>,
    pub reserved_names: Vec<String>,
    pub is_map_entry: bool,
    pub deprecated: bool,
    pub comments: Option<Comments>,
}

#[derive(Serialize)]
pub struct OneofDescription {
    pub name: String,
    pub fields: Vec<String>,
    pub comments: Option<Comments>,
}

#[derive(Serialize)]
pub struct FieldDescription {
    pub name: String,
    pub number: u32,
    pub json_name: String,
    /// Proto type name: `"int32"`, `"string"`, ..., `"enum"` or `"message"`.
    pub kind: &'static str,
    /// Full name of the enum or message type.
    pub type_name: Option<String>,
    /// `"optional"`, `"required"` or `"repeated"`.
    pub cardinality: &'static str,
    pub is_map: bool,
    pub map_key_kind: Option<&'static str>,
    pub map_value_kind: Option<&'static str>,
    pub map_value_type_name: Option<String>,
    pub is_packed: bool,
    /// Explicit presence: messages, `optional`, oneof members, proto2 singular fields.
    pub has_presence: bool,
    pub proto3_optional: bool,
    /// Name of the real `oneof` the field belongs to.
    pub oneof: Option<String>,
    /// proto2 `[default = ...]`, as written in the `.proto`.
    pub default_value: Option<String>,
    pub deprecated: bool,
    pub comments: Option<Comments>,
}

#[derive(Serialize)]
pub struct EnumDescription {
    pub name: String,
    pub full_name: String,
    pub file: String,
    pub parent: Option<String>,
    pub values: Vec<EnumValueDescription>,
    pub reserved_ranges: Vec<(i64, i64)>,
    pub reserved_names: Vec<String>,
    pub deprecated: bool,
    pub comments: Option<Comments>,
}

#[derive(Serialize)]
pub struct EnumValueDescription {
    pub name: String,
    pub number: i32,
    pub deprecated: bool,
    pub comments: Option<Comments>,
}

#[derive(Serialize)]
pub struct ServiceDescription {
    pub name: String,
    pub full_name: String,
    pub file: String,
    pub methods: Vec<MethodDescription>,
    pub deprecated: bool,
    pub comments: Option<Comments>,
}

#[derive(Serialize)]
pub struct MethodDescription {
    pub name: String,
    pub full_name: String,
    pub input_type: String,
    pub output_type: String,
    pub client_streaming: bool,
    pub server_streaming: bool,
    pub deprecated: bool,
    pub comments: Option<Comments>,
}

/// Comments of every element, by file name and `SourceCodeInfo` path.
struct CommentIndex(HashMap<String, HashMap<Vec<i32>, Comments>>);

impl CommentIndex {
    fn new(pool: &DescriptorPool) -> Self {
        let mut files = HashMap::new();
        for file in pool.files() {
            let Some(info) = &file.file_descriptor_proto().source_code_info else {
                continue;
            };
            let paths = info
                .location
                .iter()
                .filter(|l| {
                    l.leading_comments.is_some()
                        || l.trailing_comments.is_some()
                        || !l.leading_detached_comments.is_empty()
                })
                .map(|l| {
                    let comments = Comments {
                        leading: l.leading_comments.clone(),
                        trailing: l.trailing_comments.clone(),
                        detached: l.leading_detached_comments.clone(),
                    };
                    (l.path.clone(), comments)
                })
                .collect();
            files.insert(file.name().to_string(), paths);
        }
        Self(files)
    }

    fn get(&self, file: &str, path: &[i32]) -> Option<Comments> {
        self.0.get(file)?.get(path).cloned()
    }
}

fn syntax_name(syntax: Syntax) -> &'static str {
    match syntax {
        Syntax::Proto2 => "proto2",
        Syntax::Proto3 => "proto3",
    }
}

fn kind_name(kind: &Kind) -> &'static str {
    match kind {
        Kind::Double => "double",
        Kind::Float => "float",
        Kind::Int32 => "int32",
        Kind::Int64 => "int64",
        Kind::Uint32 => "uint32",
        Kind::Uint64 => "uint64",
        Kind::Sint32 => "sint32",
        Kind::Sint64 => "sint64",
        Kind::Fixed32 => "fixed32",
        Kind::Fixed64 => "fixed64",
        Kind::Sfixed32 => "sfixed32",
        Kind::Sfixed64 => "sfixed64",
        Kind::Bool => "bool",
        Kind::String => "string",
        Kind::Bytes => "bytes",
        Kind::Enum(_) => "enum",
        Kind::Message(_) => "message",
    }
}

fn type_name(kind: &Kind) -> Option<String> {
    match kind {
        Kind::Enum(e) => Some(e.full_name().to_string()),
        Kind::Message(m) => Some(m.full_name().to_string()),
        _ => None,
    }
}

fn describe_field(field: &FieldDescriptor, comments: &CommentIndex) -> FieldDescription {
    let proto = field.field_descriptor_proto();
    let kind = field.kind();
    let (map_key_kind, map_value_kind, map_value_type_name) = match &kind {
        Kind::Message(entry) if field.is_map() => {
            let value = entry.map_entry_value_field().kind();
            (
                Some(kind_name(&entry.map_entry_key_field().kind())),
                Some(kind_name(&value)),
                type_name(&value),
            )
        }
        _ => (None, None, None),
    };
    FieldDescription {
        name: field.name().to_string(),
        number: field.number(),
        json_name: field.json_name().to_string(),
        kind: kind_name(&kind),
        type_name: type_name(&kind),
        cardinality: match field.cardinality() {
            Cardinality::Optional => "optional",
            Cardinality::Required => "required",
            Cardinality::Repeated => "repeated",
        },
        is_map: field.is_map(),
        map_key_kind,
        map_value_kind,
        map_value_type_name,
        is_packed: field.is_packed(),
        has_presence: field.supports_presence(),
        proto3_optional: proto.proto3_optional(),
        oneof: field
            .containing_oneof()
            .filter(|o| !o.is_synthetic())
            .map(|o| o.name().to_string()),
        default_value: proto.default_value.clone(),
        deprecated: proto.options.as_ref().is_some_and(|o| o.deprecated()),
        comments: comments.get(field.parent_file().name(), field.path()),
    }
}

fn describe_message(message: &MessageDescriptor, comments: &CommentIndex) -> MessageDescription {
    let file = message.parent_file().name().to_string();
    let proto = message.descriptor_proto();
    MessageDescription {
        name: message.name().to_string(),
        full_name: message.full_name().to_string(),
        parent: message.parent_message().map(|p| p.full_name().to_string()),
        fields: message
            .fields()
            .map(|f| describe_field(&f, comments))
            .collect(),
        oneofs: message
            .oneofs()
            .filter(|o| !o.is_synthetic())
            .map(|o| OneofDescription {
                name: o.name().to_string(),
                fields: o.fields().map(|f| f.name().to_string()).collect(),
                comments: comments.get(&file, o.path()),
            })
            .collect(),
        reserved_ranges: message
            .reserved_ranges()
            .map(|r| (i64::from(r.start), i64::from(r.end) - 1))
            .collect(),
        reserved_names: message.reserved_names().map(str::to_string).collect(),
        is_map_entry: message.is_map_entry(),
        deprecated: proto.options.as_ref().is_some_and(|o| o.deprecated()),
        comments: comments.get(&file, message.path()),
        file,
    }
}

fn describe_enum(enumeration: &EnumDescriptor, comments: &CommentIndex) -> EnumDescription {
    let file = enumeration.parent_file().name().to_string();
    let proto = enumeration.enum_descriptor_proto();
    EnumDescription {
        name: enumeration.name().to_string(),
        full_name: enumeration.full_name().to_string(),
        parent: enumeration
            .parent_message()
            .map(|p| p.full_name().to_string()),
        values: enumeration
            .values()
            .map(|v| EnumValueDescription {
                name: v.name().to_string(),
                number: v.number(),
                deprecated: v
                    .enum_value_descriptor_proto()
                    .options
                    .as_ref()
                    .is_some_and(|o| o.deprecated()),
                comments: comments.get(&file, v.path()),
            })
            .collect(),
        reserved_ranges: enumeration
            .reserved_ranges()
            .map(|r| (i64::from(*r.start()), i64::from(*r.end())))
            .collect(),
        reserved_names: enumeration.reserved_names().map(str::to_string).collect(),
        deprecated: proto.options.as_ref().is_some_and(|o| o.deprecated()),
        comments: comments.get(&file, enumeration.path()),
        file,
    }
}

fn describe_method(method: &MethodDescriptor, comments: &CommentIndex) -> MethodDescription {
    MethodDescription {
        name: method.name().to_string(),
        full_name: method.full_name().to_string(),
        input_type: method.input().full_name().to_string(),
        output_type: method.output().full_name().to_string(),
        client_streaming: method.is_client_streaming(),
        server_streaming: method.is_server_streaming(),
        deprecated: method
            .method_descriptor_proto()
            .options
            .as_ref()
            .is_some_and(|o| o.deprecated()),
        comments: comments.get(method.parent_file().name(), method.path()),
    }
}

fn describe_service(service: &ServiceDescriptor, comments: &CommentIndex) -> ServiceDescription {
    let file = service.parent_file().name().to_string();
    ServiceDescription {
        name: service.name().to_string(),
        full_name: service.full_name().to_string(),
        methods: service
            .methods()
            .map(|m| describe_method(&m, comments))
            .collect(),
        deprecated: service
            .service_descriptor_proto()
            .options
            .as_ref()
            .is_some_and(|o| o.deprecated()),
        comments: comments.get(&file, service.path()),
        file,
    }
}

/// Describe every file, message, enum and service of a decoded pool.
pub fn describe_pool(pool: &DescriptorPool) -> SchemaDescription {
    let comments = CommentIndex::new(pool);
    SchemaDescription {
        files: pool
            .files()
            .map(|file| FileDescription {
                name: file.name().to_string(),
                package: file.package_name().to_string(),
                syntax: syntax_name(file.syntax()),
                dependencies: file.dependencies().map(|d| d.name().to_string()).collect(),
                messages: file.messages().map(|m| m.full_name().to_string()).collect(),
                enums: file.enums().map(|e| e.full_name().to_string()).collect(),
                services: file.services().map(|s| s.full_name().to_string()).collect(),
            })
            .collect(),
        messages: pool
            .all_messages()
            .map(|m| describe_message(&m, &comments))
            .collect(),
        enums: pool
            .all_enums()
            .map(|e| describe_enum(&e, &comments))
            .collect(),
        services: pool
            .services()
            .map(|s| describe_service(&s, &comments))
            .collect(),
    }
}

/// Describe a serialized `FileDescriptorSet` as a JSON document (see
/// [`SchemaDescription`]).
pub fn describe_descriptor_set(descriptor_bytes: &[u8]) -> Result<String, String> {
    let pool = load_descriptor_pool(descriptor_bytes)?;
    serde_json::to_string(&describe_pool(&pool))
        .map_err(|e| format!("Failed to describe descriptor set: {e}"))
}

/// Split a serialized `FileDescriptorSet` into `(file name, FileDescriptorProto bytes)`,
/// dependencies first: what gRPC server reflection sends for each file.
pub fn split_descriptor_set(descriptor_bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    let pool = load_descriptor_pool(descriptor_bytes)?;
    Ok(pool
        .files()
        .map(|file| {
            (
                file.name().to_string(),
                file.file_descriptor_proto().encode_to_vec(),
            )
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{CompileOptions, compile_protos};
    use serde_json::Value;

    const SOURCE: &str = r#"syntax = "proto3";
package d.v1;
import "google/protobuf/timestamp.proto";

// An order.
message Order {
  // Line of an order.
  message Line { string sku = 1; }
  enum Kind { KIND_UNSPECIFIED = 0; KIND_GIFT = 1 [deprecated = true]; }
  reserved 9, 15 to 20;
  reserved "legacy";
  string id = 1 [json_name = "orderId"]; // Public id.
  optional string note = 2;
  oneof payment { string card = 3; string iban = 4; }
  map<string, Line> lines = 5;
  repeated Kind kinds = 6;
  google.protobuf.Timestamp at = 7 [deprecated = true];
}

// Order management.
service Orders {
  // Watch orders.
  rpc Watch(Order) returns (stream Order);
}
"#;

    fn describe(source_info: bool) -> Value {
        let descriptor = compile_protos(
            &["d.proto".to_string()],
            &[],
            HashMap::from([("d.proto".to_string(), SOURCE.to_string())]),
            CompileOptions {
                source_info,
                ..CompileOptions::default()
            },
        )
        .unwrap();
        serde_json::from_str(&describe_descriptor_set(&descriptor).unwrap()).unwrap()
    }

    fn find<'a>(items: &'a Value, full_name: &str) -> &'a Value {
        items
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["full_name"] == full_name)
            .unwrap()
    }

    #[test]
    fn describes_files_messages_fields_and_services() {
        let schema = describe(true);
        let file = schema["files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["name"] == "d.proto")
            .unwrap();
        assert_eq!(file["package"], "d.v1");
        assert_eq!(file["syntax"], "proto3");
        assert_eq!(file["dependencies"][0], "google/protobuf/timestamp.proto");
        assert_eq!(file["messages"], serde_json::json!(["d.v1.Order"]));
        assert_eq!(file["services"], serde_json::json!(["d.v1.Orders"]));

        let order = find(&schema["messages"], "d.v1.Order");
        assert_eq!(
            order["reserved_ranges"],
            serde_json::json!([[9, 9], [15, 20]])
        );
        assert_eq!(order["reserved_names"], serde_json::json!(["legacy"]));
        assert_eq!(order["oneofs"][0]["name"], "payment");
        assert_eq!(order["oneofs"].as_array().unwrap().len(), 1);
        assert_eq!(order["comments"]["leading"], " An order.\n");

        let fields = order["fields"].as_array().unwrap();
        let field = |name: &str| fields.iter().find(|f| f["name"] == name).unwrap();
        assert_eq!(field("id")["json_name"], "orderId");
        assert_eq!(field("id")["comments"]["trailing"], " Public id.\n");
        assert_eq!(field("note")["has_presence"], true);
        assert_eq!(field("note")["proto3_optional"], true);
        assert_eq!(field("note")["oneof"], Value::Null);
        assert_eq!(field("card")["oneof"], "payment");
        let lines = field("lines");
        assert_eq!(lines["is_map"], true);
        assert_eq!(lines["map_key_kind"], "string");
        assert_eq!(lines["map_value_type_name"], "d.v1.Order.Line");
        assert_eq!(field("kinds")["kind"], "enum");
        assert_eq!(field("kinds")["type_name"], "d.v1.Order.Kind");
        assert_eq!(field("kinds")["cardinality"], "repeated");
        assert_eq!(field("kinds")["is_packed"], true);
        assert_eq!(field("at")["deprecated"], true);
        assert_eq!(field("at")["type_name"], "google.protobuf.Timestamp");

        let line = find(&schema["messages"], "d.v1.Order.Line");
        assert_eq!(line["parent"], "d.v1.Order");
        assert_eq!(line["comments"]["leading"], " Line of an order.\n");
        let entry = find(&schema["messages"], "d.v1.Order.LinesEntry");
        assert_eq!(entry["is_map_entry"], true);

        let kind = find(&schema["enums"], "d.v1.Order.Kind");
        assert_eq!(kind["parent"], "d.v1.Order");
        assert_eq!(kind["values"][1]["deprecated"], true);

        let orders = find(&schema["services"], "d.v1.Orders");
        let watch = &orders["methods"][0];
        assert_eq!(watch["full_name"], "d.v1.Orders.Watch");
        assert_eq!(watch["input_type"], "d.v1.Order");
        assert_eq!(watch["client_streaming"], false);
        assert_eq!(watch["server_streaming"], true);
        assert_eq!(watch["comments"]["leading"], " Watch orders.\n");
    }

    #[test]
    fn comments_need_source_info() {
        let schema = describe(false);
        assert_eq!(
            find(&schema["messages"], "d.v1.Order")["comments"],
            Value::Null
        );
    }

    #[test]
    fn splits_per_file_dependencies_first() {
        let descriptor = compile_protos(
            &["d.proto".to_string()],
            &[],
            HashMap::from([("d.proto".to_string(), SOURCE.to_string())]),
            CompileOptions::default(),
        )
        .unwrap();
        let files = split_descriptor_set(&descriptor).unwrap();
        let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["google/protobuf/timestamp.proto", "d.proto"]);
        let set =
            prost_reflect::prost_types::FileDescriptorSet::decode(descriptor.as_slice()).unwrap();
        for ((_, bytes), expected) in files.iter().zip(&set.file) {
            assert_eq!(
                prost_reflect::prost_types::FileDescriptorProto::decode(bytes.as_slice()).unwrap(),
                *expected
            );
        }
    }
}
