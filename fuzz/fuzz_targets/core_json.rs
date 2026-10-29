//! Arbitrary text -> protobuf (prost-reflect's JSON mapping): no panic; and what encodes
//! decodes back to JSON.
#![no_main]

mod schema;

use libfuzzer_sys::fuzz_target;
use protoruf::core::{
    json_to_protobuf_bytes_with_descriptor, protobuf_to_json_string_with_descriptor,
};

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let node = schema::node();
    if let Ok(bytes) = json_to_protobuf_bytes_with_descriptor(text, &node) {
        protobuf_to_json_string_with_descriptor(&bytes, &node, false)
            .unwrap_or_else(|e| panic!("encoded JSON does not decode: {e}\n{text}"));
    }
});
