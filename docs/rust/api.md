# API Reference (Rust core)

The engine lives in the `core` module (`src/core.rs`). All functions return
`Result<_, String>` (the `Err` carries a human-readable message).

## Compilation

```rust
pub fn compile_proto(
    proto_path: &str,
    include_paths: Option<Vec<String>>,
) -> Result<Vec<u8>, String>;
```

Compile a `.proto` file **from disk** to a serialized descriptor set. `include_paths` defaults
to the parent directory of `proto_path`.

```rust
pub fn compile_proto_from_sources(
    files: HashMap<String, String>,
    root: &str,
) -> Result<Vec<u8>, String>;
```

Compile `.proto` sources **held in memory** — no filesystem access (used by the WASM target).
`files` maps each logical file name to its source; `import`s are resolved by name, and Google
well-known types are provided automatically. `root` is the entry file.

## Conversion (one-shot)

```rust
pub fn json_to_protobuf_bytes(
    json_str: &str,
    descriptor_bytes: &[u8],
    message_type: &str,
) -> Result<Vec<u8>, String>;

pub fn protobuf_to_json_string(
    protobuf_bytes: &[u8],
    descriptor_bytes: &[u8],
    pretty: bool,
    message_type: &str,
) -> Result<String, String>;
```

These decode the descriptor pool on every call. For repeated conversions, prefer the cached
path below.

## Conversion (cached / high-throughput)

```rust
pub fn load_descriptor_pool(descriptor_bytes: &[u8]) -> Result<DescriptorPool, String>;

pub fn get_message_descriptor(
    pool: &DescriptorPool,
    message_type: &str,
) -> Result<MessageDescriptor, String>;

pub fn json_to_protobuf_bytes_with_descriptor(
    json_str: &str,
    message_descriptor: &MessageDescriptor,
) -> Result<Vec<u8>, String>;

pub fn protobuf_to_json_string_with_descriptor(
    protobuf_bytes: &[u8],
    message_descriptor: &MessageDescriptor,
    pretty: bool,
) -> Result<String, String>;
```

Decode the pool once with `load_descriptor_pool`, resolve each message type once with
`get_message_descriptor`, then call the `*_with_descriptor` helpers in your hot loop. This is
exactly what the `DescriptorCache` of each binding does internally.

## JSON shape

Serialization matches protoruf's historical shape: proto field names (snake_case), enums as
**numbers**, and 64-bit integers as JSON **numbers** (not strings). Consumers that need exact
`int64`/`uint64` above 2^53 must parse with a big-integer-aware reader.
