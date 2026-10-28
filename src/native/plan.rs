//! Precomputed codec plans: one flat description per message type, built once
//! from a [`MessageDescriptor`] and reused for every encode/decode.

use super::wire::{WIRE_I32, WIRE_I64, WIRE_LEN, WIRE_VARINT};
use prost_reflect::{Cardinality, EnumDescriptor, FieldDescriptor, Kind, MessageDescriptor};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scalar {
    Double,
    Float,
    Int32,
    Int64,
    Uint32,
    Uint64,
    Sint32,
    Sint64,
    Fixed32,
    Fixed64,
    Sfixed32,
    Sfixed64,
    Bool,
    String,
    Bytes,
    /// Index of the enum plan in [`CodecPlan::enums`].
    Enum(u32),
}

impl Scalar {
    pub fn wire_type(self) -> u8 {
        match self {
            Scalar::Double | Scalar::Fixed64 | Scalar::Sfixed64 => WIRE_I64,
            Scalar::Float | Scalar::Fixed32 | Scalar::Sfixed32 => WIRE_I32,
            Scalar::String | Scalar::Bytes => WIRE_LEN,
            _ => WIRE_VARINT,
        }
    }

    /// Whether repeated values of this type may use the packed encoding.
    pub fn packable(self) -> bool {
        !matches!(self, Scalar::String | Scalar::Bytes)
    }
}

/// Well-known types mapped to native host values instead of generic messages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WellKnown {
    Timestamp,
    Duration,
    Struct,
    Value,
    ListValue,
    FieldMask,
    /// `google.protobuf.*Value` wrappers: the wrapped scalar, or nothing.
    Wrapper(Scalar),
}

impl WellKnown {
    pub fn from_full_name(name: &str) -> Option<Self> {
        Some(match name {
            "google.protobuf.Timestamp" => WellKnown::Timestamp,
            "google.protobuf.Duration" => WellKnown::Duration,
            "google.protobuf.Struct" => WellKnown::Struct,
            "google.protobuf.Value" => WellKnown::Value,
            "google.protobuf.ListValue" => WellKnown::ListValue,
            "google.protobuf.FieldMask" => WellKnown::FieldMask,
            "google.protobuf.DoubleValue" => WellKnown::Wrapper(Scalar::Double),
            "google.protobuf.FloatValue" => WellKnown::Wrapper(Scalar::Float),
            "google.protobuf.Int64Value" => WellKnown::Wrapper(Scalar::Int64),
            "google.protobuf.UInt64Value" => WellKnown::Wrapper(Scalar::Uint64),
            "google.protobuf.Int32Value" => WellKnown::Wrapper(Scalar::Int32),
            "google.protobuf.UInt32Value" => WellKnown::Wrapper(Scalar::Uint32),
            "google.protobuf.BoolValue" => WellKnown::Wrapper(Scalar::Bool),
            "google.protobuf.StringValue" => WellKnown::Wrapper(Scalar::String),
            "google.protobuf.BytesValue" => WellKnown::Wrapper(Scalar::Bytes),
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Value {
    Scalar(Scalar),
    /// Index of the nested message plan in [`CodecPlan::messages`].
    Message(usize),
    WellKnown(WellKnown),
}

/// The declared values of an enum, used to reject unknown numbers on request and
/// to encode an enum given by name.
#[derive(Clone, Debug)]
pub struct EnumPlan {
    pub full_name: String,
    /// Declared numbers, sorted and deduplicated (aliases share a number).
    pub numbers: Vec<i32>,
    /// Every declared name, aliases included.
    pub names: Vec<(String, i32)>,
}

impl EnumPlan {
    pub fn contains(&self, number: i32) -> bool {
        self.numbers.binary_search(&number).is_ok()
    }

    pub fn number_of(&self, name: &str) -> Option<i32> {
        self.names
            .iter()
            .find_map(|(value, number)| (value == name).then_some(*number))
    }
}

#[derive(Clone, Copy, Debug)]
pub enum FieldType {
    Single(Value),
    Repeated { value: Value, packed: bool },
    Map { key: Scalar, value: Value },
}

#[derive(Clone, Debug)]
pub struct FieldPlan {
    pub name: String,
    pub number: u32,
    pub ty: FieldType,
    /// Explicit presence (`optional`, message, oneof member): absent stays absent and
    /// zero values are still written. Otherwise zero values are omitted on encode
    /// and filled back on decode.
    pub explicit_presence: bool,
}

#[derive(Clone, Debug)]
pub struct MessagePlan {
    pub full_name: String,
    /// Sorted by field number, which is also the canonical encoding order.
    pub fields: Vec<FieldPlan>,
    by_number: NumberIndex,
}

#[derive(Clone, Debug)]
enum NumberIndex {
    /// Field numbers are small: direct table lookup.
    Dense(Vec<u16>),
    Sparse(HashMap<u32, usize>),
}

const DENSE_LIMIT: u32 = 1024;
const ABSENT: u16 = u16::MAX;

impl MessagePlan {
    #[inline]
    pub fn field_index(&self, number: u32) -> Option<usize> {
        match &self.by_number {
            NumberIndex::Dense(table) => match table.get(number as usize) {
                Some(&i) if i != ABSENT => Some(i as usize),
                _ => None,
            },
            NumberIndex::Sparse(map) => map.get(&number).copied(),
        }
    }
}

/// All the message and enum plans reachable from a root type. The root is at index 0.
#[derive(Clone, Debug)]
pub struct CodecPlan {
    pub messages: Vec<MessagePlan>,
    pub enums: Vec<EnumPlan>,
}

impl CodecPlan {
    pub fn build(root: &MessageDescriptor) -> Result<Self, String> {
        let mut builder = Builder {
            messages: Vec::new(),
            indices: HashMap::new(),
            enums: Vec::new(),
            enum_indices: HashMap::new(),
        };
        builder.message(root)?;
        Ok(Self {
            messages: builder.messages,
            enums: builder.enums,
        })
    }
}

struct Builder {
    messages: Vec<MessagePlan>,
    indices: HashMap<String, usize>,
    enums: Vec<EnumPlan>,
    enum_indices: HashMap<String, u32>,
}

impl Builder {
    /// Plan a message (once), returning its index. Recursive types are supported:
    /// the index is reserved before the fields are planned.
    fn message(&mut self, desc: &MessageDescriptor) -> Result<usize, String> {
        if let Some(&index) = self.indices.get(desc.full_name()) {
            return Ok(index);
        }
        let index = self.messages.len();
        self.indices.insert(desc.full_name().to_string(), index);
        self.messages.push(MessagePlan {
            full_name: desc.full_name().to_string(),
            fields: Vec::new(),
            by_number: NumberIndex::Sparse(HashMap::new()),
        });

        let mut fields = desc
            .fields()
            .map(|field| self.field(&field))
            .collect::<Result<Vec<_>, _>>()?;
        fields.sort_by_key(|f| f.number);

        let max = fields.last().map_or(0, |f| f.number);
        let by_number = if max < DENSE_LIMIT {
            let mut table = vec![ABSENT; max as usize + 1];
            for (i, f) in fields.iter().enumerate() {
                table[f.number as usize] = i as u16;
            }
            NumberIndex::Dense(table)
        } else {
            NumberIndex::Sparse(
                fields
                    .iter()
                    .enumerate()
                    .map(|(i, f)| (f.number, i))
                    .collect(),
            )
        };

        let plan = &mut self.messages[index];
        plan.fields = fields;
        plan.by_number = by_number;
        Ok(index)
    }

    fn value(&mut self, kind: &Kind) -> Result<Value, String> {
        Ok(match kind {
            Kind::Message(desc) => match WellKnown::from_full_name(desc.full_name()) {
                Some(wkt) => Value::WellKnown(wkt),
                None => Value::Message(self.message(desc)?),
            },
            other => Value::Scalar(self.scalar(other)?),
        })
    }

    fn scalar(&mut self, kind: &Kind) -> Result<Scalar, String> {
        match kind {
            Kind::Enum(desc) => Ok(Scalar::Enum(self.enumeration(desc))),
            other => scalar(other),
        }
    }

    fn enumeration(&mut self, desc: &EnumDescriptor) -> u32 {
        if let Some(&index) = self.enum_indices.get(desc.full_name()) {
            return index;
        }
        let names: Vec<(String, i32)> = desc
            .values()
            .map(|v| (v.name().to_string(), v.number()))
            .collect();
        let mut numbers: Vec<i32> = names.iter().map(|(_, number)| *number).collect();
        numbers.sort_unstable();
        numbers.dedup();
        let index = self.enums.len() as u32;
        self.enums.push(EnumPlan {
            full_name: desc.full_name().to_string(),
            numbers,
            names,
        });
        self.enum_indices
            .insert(desc.full_name().to_string(), index);
        index
    }

    fn field(&mut self, field: &FieldDescriptor) -> Result<FieldPlan, String> {
        let ty = if field.is_map() {
            let Kind::Message(entry) = field.kind() else {
                return Err(format!("map field {} has no entry type", field.full_name()));
            };
            FieldType::Map {
                key: self.scalar(&entry.map_entry_key_field().kind())?,
                value: self.value(&entry.map_entry_value_field().kind())?,
            }
        } else if field.cardinality() == Cardinality::Repeated {
            FieldType::Repeated {
                value: self.value(&field.kind())?,
                packed: field.is_packed(),
            }
        } else {
            FieldType::Single(self.value(&field.kind())?)
        };
        Ok(FieldPlan {
            name: field.name().to_string(),
            number: field.number(),
            ty,
            explicit_presence: field.supports_presence(),
        })
    }
}

fn scalar(kind: &Kind) -> Result<Scalar, String> {
    Ok(match kind {
        Kind::Double => Scalar::Double,
        Kind::Float => Scalar::Float,
        Kind::Int32 => Scalar::Int32,
        Kind::Int64 => Scalar::Int64,
        Kind::Uint32 => Scalar::Uint32,
        Kind::Uint64 => Scalar::Uint64,
        Kind::Sint32 => Scalar::Sint32,
        Kind::Sint64 => Scalar::Sint64,
        Kind::Fixed32 => Scalar::Fixed32,
        Kind::Fixed64 => Scalar::Fixed64,
        Kind::Sfixed32 => Scalar::Sfixed32,
        Kind::Sfixed64 => Scalar::Sfixed64,
        Kind::Bool => Scalar::Bool,
        Kind::String => Scalar::String,
        Kind::Bytes => Scalar::Bytes,
        Kind::Enum(desc) => {
            return Err(format!(
                "{} is an enum, planned by the builder",
                desc.full_name()
            ));
        }
        Kind::Message(desc) => {
            return Err(format!("{} is a message, not a scalar", desc.full_name()));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core;
    use std::collections::HashMap as Map;

    fn plan_for(source: &str, message: &str) -> CodecPlan {
        let files = Map::from([("t.proto".to_string(), source.to_string())]);
        let bytes = core::compile_proto_from_sources(files, "t.proto", true).unwrap();
        let pool = core::load_descriptor_pool(&bytes).unwrap();
        CodecPlan::build(&core::get_message_descriptor(&pool, message).unwrap()).unwrap()
    }

    #[test]
    fn plans_nested_repeated_map_and_presence() {
        let plan = plan_for(
            r#"syntax = "proto3"; package t;
               message Child { string name = 1; }
               message Root {
                 repeated int32 ids = 3;
                 optional string note = 2;
                 Child child = 1;
                 map<string, Child> by_name = 4;
                 repeated string tags = 5;
               }"#,
            "t.Root",
        );
        let root = &plan.messages[0];
        let numbers: Vec<u32> = root.fields.iter().map(|f| f.number).collect();
        assert_eq!(numbers, [1, 2, 3, 4, 5]);
        assert!(root.fields[0].explicit_presence); // message
        assert!(root.fields[1].explicit_presence); // optional
        assert!(matches!(
            root.fields[2].ty,
            FieldType::Repeated {
                value: Value::Scalar(Scalar::Int32),
                packed: true
            }
        ));
        assert!(matches!(
            root.fields[3].ty,
            FieldType::Map {
                key: Scalar::String,
                value: Value::Message(1)
            }
        ));
        assert_eq!(root.field_index(4), Some(3));
        assert_eq!(root.field_index(9), None);
        assert_eq!(plan.messages[1].full_name, "t.Child");
    }

    #[test]
    fn plans_well_known_types_and_enums() {
        let plan = plan_for(
            r#"syntax = "proto3"; package t;
               import "google/protobuf/timestamp.proto";
               import "google/protobuf/wrappers.proto";
               import "google/protobuf/struct.proto";
               enum Color { option allow_alias = true; COLOR_UNSPECIFIED = 0; COLOR_RED = 1; COLOR_ROUGE = 1; }
               message Root {
                 google.protobuf.Timestamp at = 1;
                 repeated google.protobuf.StringValue names = 2;
                 map<string, google.protobuf.Struct> docs = 3;
                 Color color = 4;
                 repeated Color colors = 5;
               }"#,
            "t.Root",
        );
        // Well-known types are not planned as messages.
        assert_eq!(plan.messages.len(), 1);
        let fields = &plan.messages[0].fields;
        assert!(matches!(
            fields[0].ty,
            FieldType::Single(Value::WellKnown(WellKnown::Timestamp))
        ));
        assert!(matches!(
            fields[1].ty,
            FieldType::Repeated {
                value: Value::WellKnown(WellKnown::Wrapper(Scalar::String)),
                ..
            }
        ));
        assert!(matches!(
            fields[2].ty,
            FieldType::Map {
                value: Value::WellKnown(WellKnown::Struct),
                ..
            }
        ));
        // Both enum fields share one plan; aliases share a number and keep their name.
        assert!(matches!(
            fields[3].ty,
            FieldType::Single(Value::Scalar(Scalar::Enum(0)))
        ));
        assert_eq!(plan.enums.len(), 1);
        let color = &plan.enums[0];
        assert_eq!(color.numbers, [0, 1]);
        assert!(color.contains(1) && !color.contains(2));
        assert_eq!(color.number_of("COLOR_RED"), Some(1));
        assert_eq!(color.number_of("COLOR_ROUGE"), Some(1));
        assert_eq!(color.number_of("RED"), None);
    }

    #[test]
    fn plans_recursive_messages() {
        let plan = plan_for(
            r#"syntax = "proto3"; package t; message Node { Node next = 1; }"#,
            "t.Node",
        );
        assert_eq!(plan.messages.len(), 1);
        assert!(matches!(
            plan.messages[0].fields[0].ty,
            FieldType::Single(Value::Message(0))
        ));
    }
}
