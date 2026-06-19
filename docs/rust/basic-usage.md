# Basic Usage (Rust)

Converting between JSON and Protobuf with the `core` module.

## Core Functions

| Function | Description |
|----------|-------------|
| `compile_proto()` | Compile a `.proto` file from disk to a descriptor |
| `compile_proto_from_sources()` | Compile `.proto` sources held in memory |
| `json_to_protobuf_bytes()` | Convert a JSON string to Protobuf bytes |
| `protobuf_to_json_string()` | Convert Protobuf bytes to a JSON string |
| `load_descriptor_pool()` / `get_message_descriptor()` / `*_with_descriptor()` | The cached, high-throughput path ([Performance](advanced.md)) |

All functions return `Result<_, String>`; the `Err` carries a human-readable message. Binary
values are `Vec<u8>` (returned) / `&[u8]` (borrowed).

## Compiling

```rust
use protoruf::core;

let descriptor = core::compile_proto("schema.proto", None)?;
```

See [Proto Compilation](proto-files.md) for imports and in-memory compilation.

## JSON → Protobuf

```rust
let wire = core::json_to_protobuf_bytes(
    r#"{"id":"123","name":"Alice"}"#,
    &descriptor,
    "user.User",
)?;
```

- `json_str: &str`, `descriptor_bytes: &[u8]`, `message_type: &str`
- **returns** `Vec<u8>`

## Protobuf → JSON

```rust
let json = core::protobuf_to_json_string(&wire, &descriptor, /* pretty */ true, "user.User")?;
```

- `protobuf_bytes: &[u8]`, `descriptor_bytes: &[u8]`, `pretty: bool`, `message_type: &str`
- **returns** `String`

## Supported Protobuf Features

The conversion is fully dynamic, so all proto3 constructs are supported. Input/output is JSON
text; the schema is whatever you compiled.

### Scalars, nested, repeated, maps

```protobuf
message Address { string street = 1; string city = 2; }
message User {
  string name = 1;
  int32 age = 2;
  int64 id = 3;
  bool active = 4;
  Address address = 5;
  repeated string tags = 6;
  map<string, string> labels = 7;
}
```

```rust
let json = r#"{
  "name": "Alice", "age": 30, "id": 123456789, "active": true,
  "address": { "street": "123 Main St", "city": "Springfield" },
  "tags": ["a", "b"], "labels": { "env": "prod" }
}"#;
let wire = core::json_to_protobuf_bytes(json, &descriptor, "user.User")?;
```

### Enums

On **input**, enums accept the name or the number; on **output** they are emitted as the
**numeric value** (the engine serializes with `use_enum_numbers(true)`):

```protobuf
enum Priority { LOW = 0; MEDIUM = 1; HIGH = 2; }
message Task { string title = 1; Priority priority = 2; }
```

```rust
core::json_to_protobuf_bytes(r#"{"title":"x","priority":"HIGH"}"#, &d, "Task")?; // name in
// protobuf_to_json_string(...) -> {"title":"x","priority":2}                    // number out
```

### Oneof & defaults

Set exactly one oneof arm; only the set field appears on output. proto3 default values
(empty string, `0`, `false`, enum `0`, empty list/map) are **omitted** from the output JSON.

## Error Handling

```rust
match core::json_to_protobuf_bytes("not json", &descriptor, "user.User") {
    Ok(bytes) => { /* ... */ }
    Err(e) => eprintln!("conversion failed: {e}"), // "Invalid JSON: ..."
}
```

Errors are plain `String`s with a descriptive prefix (`Invalid JSON: …`,
`Message type '…' not found in descriptor`, `Decoding error: …`,
`Failed to compile proto file: …`). Bindings map these to their host's exception/error type.

## Next Steps

- [Proto Compilation](proto-files.md)
- [Performance & Patterns](advanced.md)
- [API Reference](api.md)
