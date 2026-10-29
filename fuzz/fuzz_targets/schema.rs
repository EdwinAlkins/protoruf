//! The message the `core_*` targets work on: every scalar type, packed and unpacked lists,
//! maps, recursion, a oneof, optional fields and the well-known types.

use std::collections::HashMap;
use std::sync::OnceLock;

use prost_reflect::MessageDescriptor;
use protoruf::core::{compile_proto_from_sources, get_message_descriptor, load_descriptor_pool};

const PROTO: &str = r#"
syntax = "proto3";
package fuzz;
import "google/protobuf/struct.proto";
import "google/protobuf/timestamp.proto";
import "google/protobuf/duration.proto";
import "google/protobuf/wrappers.proto";
import "google/protobuf/field_mask.proto";
import "google/protobuf/empty.proto";

enum Kind { KIND_UNSPECIFIED = 0; KIND_A = 1; KIND_B = 2; }

message Node {
  double f64 = 1;
  float f32 = 2;
  int32 i32 = 3;
  int64 i64 = 4;
  uint32 u32 = 5;
  uint64 u64 = 6;
  sint32 s32 = 7;
  sint64 s64 = 8;
  fixed32 x32 = 9;
  fixed64 x64 = 10;
  sfixed32 sx32 = 11;
  sfixed64 sx64 = 12;
  bool flag = 13;
  string text = 14;
  bytes blob = 15;
  Kind kind = 16;
  repeated int64 packed = 17;
  repeated int64 unpacked = 18 [packed = false];
  repeated string names = 19;
  repeated Kind kinds = 20;
  map<string, Node> children = 21;
  map<int64, string> labels = 22;
  Node next = 23;
  oneof pick { string choice = 24; Node other = 25; }
  optional int32 maybe = 26;
  google.protobuf.Struct meta = 27;
  google.protobuf.Value value = 28;
  google.protobuf.ListValue list = 29;
  google.protobuf.Timestamp at = 30;
  google.protobuf.Duration span = 31;
  google.protobuf.StringValue wrapped = 32;
  google.protobuf.FieldMask mask = 33;
  google.protobuf.Empty empty = 34;
}
"#;

pub fn node() -> MessageDescriptor {
    static NODE: OnceLock<MessageDescriptor> = OnceLock::new();
    NODE.get_or_init(|| {
        let files = HashMap::from([("fuzz.proto".to_owned(), PROTO.to_owned())]);
        let descriptor = compile_proto_from_sources(files, "fuzz.proto", true).unwrap();
        let pool = load_descriptor_pool(&descriptor).unwrap();
        get_message_descriptor(&pool, "fuzz.Node").unwrap()
    })
    .clone()
}
