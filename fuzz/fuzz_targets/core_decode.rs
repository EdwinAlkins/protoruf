//! Arbitrary bytes -> JSON (prost-reflect): no panic; and when they decode, the JSON
//! encodes back, and that encoding gives the same JSON again.
#![no_main]

mod schema;

use libfuzzer_sys::fuzz_target;
use protoruf::core::{
    json_to_protobuf_bytes_with_descriptor, protobuf_to_json_string_with_descriptor,
};

fuzz_target!(|data: &[u8]| {
    let node = schema::node();
    let Ok(json) = protobuf_to_json_string_with_descriptor(data, &node, false) else {
        return;
    };
    let bytes = json_to_protobuf_bytes_with_descriptor(&json, &node)
        .unwrap_or_else(|e| panic!("decoded JSON does not encode back: {e}\n{json}"));
    let again = protobuf_to_json_string_with_descriptor(&bytes, &node, false).unwrap();
    assert_eq!(json, again);
});
