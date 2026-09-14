# protoruf — LLM Usage Prompt (Rust)

<div class="llm-copy-mount"></div>

> Feed this file to an LLM (or paste it into a system prompt) so it can use the
> **Rust** core of `protoruf` correctly with no other context. It is a complete,
> self-contained reference for version **0.2.0**.

## What protoruf is

`protoruf` converts between **JSON and Protobuf** *dynamically* — no `protoc`, no
generated structs, no codegen step. You compile a `.proto` to a **descriptor set**
once, then convert JSON ↔ Protobuf wire bytes in both directions. The Rust crate
**is** the engine that powers the Python, Node, and WASM bindings; all conversion
logic lives in the `core` module.

Core idea: a `descriptor` (compiled schema, `Vec<u8>`/`&[u8]`) + a `message_type`
(fully qualified name like `"user.User"`) are required for every conversion.

## Add the dependency

Not published to crates.io as of 0.2.0 — depend on it by git or path:

```toml
# Cargo.toml
[dependencies]
protoruf = { git = "https://github.com/EdwinAlkins/protoruf" }
# or, in a local checkout:
# protoruf = { path = "../protoruf" }
```

The default build has **no binding features** enabled — that's exactly what you
want when using it as a Rust library. The `python` / `node` / `wasm` features
exist only to build FFI artifacts and pull in PyO3 / napi / wasm-bindgen; do not
enable them for pure-Rust use.

Everything lives under the `core` module:

```rust
use protoruf::core;
```

## Error convention

Every `core` function returns `Result<_, String>` — the `Err` carries a
human-readable message (compile error, invalid JSON, unknown message type,
decode/serialize failure). There is no custom error enum.

## Complete public API (`protoruf::core`)

### Compilation

```rust
pub fn compile_proto(
    proto_path: &str,
    include_paths: Option<Vec<String>>,
) -> Result<Vec<u8>, String>;
```
Compile a `.proto` **from disk** to a serialized descriptor set.
`include_paths` defaults to the parent directory of `proto_path`.

```rust
pub fn compile_proto_from_sources(
    files: HashMap<String, String>,
    root: &str,
    include_imports: bool,
) -> Result<Vec<u8>, String>;
```
Compile `.proto` sources held **in memory** (no filesystem — works anywhere,
including WASM). `files` maps a logical filename → its source text; `import`s
resolve by name within the map, and Google well-known types are provided
automatically. `root` is the entry file. `include_imports = true` embeds imported
files so the descriptor is self-contained; `false` yields a smaller,
faster-decoding descriptor when you don't need well-known types.

### Conversion (one-shot)

```rust
pub fn json_to_protobuf_bytes(
    json_str: &str,
    descriptor_bytes: &[u8],
    message_type: &str,
) -> Result<Vec<u8>, String>;

pub fn protobuf_to_json_string(
    protobuf_bytes: &[u8],
    descriptor_bytes: &[u8],
    message_type: &str,
    pretty: bool,
) -> Result<String, String>;
```
`message_type` is the **fully qualified** name `"<package>.<Message>"`, e.g.
`"user.User"`. Note the argument order: `message_type` comes **before** `pretty`.

### Conversion (cached / high-throughput)

Decode the descriptor pool once, resolve each message type once, then call the
`*_with_descriptor` helpers in your hot loop. This is exactly what each binding's
`DescriptorCache` does internally.

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
(`*_with_descriptor_owned` variants taking `MessageDescriptor` by value also
exist and are what the bindings call.) `DescriptorPool` and `MessageDescriptor`
come from the [`prost-reflect`](https://docs.rs/prost-reflect) crate. protoruf
does **not** re-export them, so to name these types in your own code add
`prost-reflect` as a direct dependency of your crate (match protoruf's version,
`0.16`).

## Canonical examples

### Minimal round-trip

```rust
use protoruf::core;

fn main() -> Result<(), String> {
    // From disk (None => include path defaults to the .proto's parent dir):
    let descriptor = core::compile_proto("message.proto", None)?;

    let json = r#"{"id":"123","content":"Hello"}"#;
    let wire = core::json_to_protobuf_bytes(json, &descriptor, "message.Message")?;
    let out  = core::protobuf_to_json_string(&wire, &descriptor, "message.Message", true)?;
    println!("{out}");
    Ok(())
}
```

### Compile from in-memory sources

```rust
use protoruf::core;
use std::collections::HashMap;

let files = HashMap::from([(
    "message.proto".to_string(),
    r#"syntax = "proto3"; package message; message Message { string id = 1; string content = 2; }"#.to_string(),
)]);
let descriptor = core::compile_proto_from_sources(files, "message.proto", true)?;
```

### High-throughput loop (decode pool once)

```rust
use protoruf::core;

let descriptor = core::compile_proto("schema.proto", None)?;
let pool = core::load_descriptor_pool(&descriptor)?;           // expensive step, once
let md   = core::get_message_descriptor(&pool, "message.Message")?; // resolve once

for json_data in json_stream {
    let wire = core::json_to_protobuf_bytes_with_descriptor(&json_data, &md)?;
    process(wire);
}
```

## JSON shape — READ THIS (non-standard)

protoruf uses a **specific, non-default** JSON mapping (configured via
`prost-reflect` `SerializeOptions`: `use_proto_field_name(true)`,
`use_enum_numbers(true)`, `stringify_64_bit_integers(false)`). When generating or
validating JSON, follow these rules exactly:

- **Field names are proto names (`snake_case`)**, NOT the camelCase of canonical
  protobuf JSON. Use `created_at`, not `createdAt`.
- **Enums are emitted as numbers** (their integer value), not names.
- **64-bit integers (`int64`/`uint64`/…) are JSON numbers, not strings.**
  ⚠️ Values above 2^53 need a big-integer-aware JSON reader on the consumer side
  to avoid precision loss.
- **`bytes` fields** map to standard base64-encoded strings.
- **Fields at their proto3 default (0, `""`, `false`, empty repeated/map) are
  omitted** from the output JSON. Don't rely on them being present after a
  round-trip; re-parsing yields the default anyway.
- Standard proto3 JSON applies otherwise (nested messages → objects, repeated →
  arrays, map → object, `oneof` → the single set field, well-known types like
  `Timestamp`/`Duration`/`Struct` use their canonical JSON forms).

## Rules for the LLM

1. Use `protoruf::core::*`; return types are `Result<_, String>` — propagate with
   `?` or handle the `String` error.
2. Every conversion needs both `descriptor_bytes` (`&[u8]`) and a fully-qualified
   `message_type` (`"<package>.<Message>"`); a wrong name is an `Err`.
3. Argument order for `protobuf_to_json_string` is
   `(protobuf_bytes, descriptor_bytes, message_type, pretty)` — `message_type`
   before `pretty`.
4. For more than a few conversions, call `load_descriptor_pool` +
   `get_message_descriptor` once and reuse the `*_with_descriptor` helpers in the
   loop; don't re-decode the pool per message.
5. Do **not** enable the `python`/`node`/`wasm` cargo features for library use —
   they only build FFI artifacts.
6. Emit JSON in protoruf's shape (snake_case fields, numeric enums, numeric
   64-bit ints) — do not assume canonical camelCase protobuf JSON.
