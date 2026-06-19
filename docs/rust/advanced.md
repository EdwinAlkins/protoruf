# Performance & Patterns (Rust)

The cached conversion path, building a reusable cache, and threading.

## The cost model

`json_to_protobuf_bytes` and `protobuf_to_json_string` are convenient one-shots, but they
**decode the descriptor pool on every call** — the dominant cost when converting many messages.
For hot loops, decode the pool once and reuse the resolved descriptor.

## The cached path

```rust
use protoruf::core;

// 1. Decode the descriptor pool a single time.
let pool = core::load_descriptor_pool(&descriptor)?;

// 2. Resolve each message descriptor once.
let desc = core::get_message_descriptor(&pool, "message.Message")?;

// 3. Reuse the resolved descriptor in your hot loop.
for json in json_stream {
    let wire = core::json_to_protobuf_bytes_with_descriptor(json, &desc)?;
    process(&wire);
}

// Round-trip back to JSON.
let restored = core::protobuf_to_json_string_with_descriptor(&wire, &desc, false)?;
```

The `*_with_descriptor` helpers skip both the pool decode and the message lookup. Output is
byte-for-byte identical to the one-shot functions — this is exactly what each language binding's
`DescriptorCache` does internally.

## Building a reusable cache

Wrap the pool and memoize resolved descriptors, mirroring the bindings' `DescriptorCache`:

```rust
use std::collections::HashMap;
use parking_lot::RwLock;
use prost_reflect::{DescriptorPool, MessageDescriptor};
use protoruf::core;

pub struct DescriptorCache {
    pool: DescriptorPool,
    descriptors: RwLock<HashMap<String, MessageDescriptor>>,
}

impl DescriptorCache {
    pub fn new(descriptor_bytes: &[u8]) -> Result<Self, String> {
        Ok(Self {
            pool: core::load_descriptor_pool(descriptor_bytes)?,
            descriptors: RwLock::new(HashMap::new()),
        })
    }

    fn resolve(&self, message_type: &str) -> Result<MessageDescriptor, String> {
        if let Some(d) = self.descriptors.read().get(message_type) {
            return Ok(d.clone());
        }
        let d = core::get_message_descriptor(&self.pool, message_type)?;
        self.descriptors.write().insert(message_type.to_string(), d.clone());
        Ok(d)
    }

    pub fn json_to_protobuf(&self, json: &str, message_type: &str) -> Result<Vec<u8>, String> {
        core::json_to_protobuf_bytes_with_descriptor(json, &self.resolve(message_type)?)
    }

    pub fn protobuf_to_json(&self, bytes: &[u8], message_type: &str, pretty: bool) -> Result<String, String> {
        core::protobuf_to_json_string_with_descriptor(bytes, &self.resolve(message_type)?, pretty)
    }
}
```

## Threading

`MessageDescriptor` and `DescriptorPool` are `Send + Sync`. The cache above uses an `RwLock`, so
concurrent conversions take a shared read lock and run in parallel; the write lock is only taken
the first time each message type is resolved. Share a single cache across threads behind an
`Arc`.

## Other tips

- **Pre-compile descriptors** — compile `.proto` once at build time and load the bytes at
  runtime instead of recompiling.
- **Skip pretty printing** in hot paths (`pretty = false`).
- **Reserve capacity** — the engine already pre-sizes output buffers from the input length.

## Next Steps

- [API Reference](api.md)
