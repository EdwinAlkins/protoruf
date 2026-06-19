# Quick Start (Rust)

Use the protoruf engine directly from a Rust project.

## Step 1: Add the dependency

The crate is the engine behind every binding. Depend on it by git (or a local path):

```toml
# Cargo.toml
[dependencies]
protoruf = { git = "https://github.com/EdwinAlkins/protoruf" }
```

```toml
# or, working in a checkout
protoruf = { path = "../protoruf" }
```

All conversion functions live in the `core` module and return `Result<_, String>`.

## Step 2: Compile a schema

```rust
use protoruf::core;

// From disk:
let descriptor = core::compile_proto("message.proto", None)?;

// ...or fully in memory (no file, works anywhere including WASM):
use std::collections::HashMap;
let files = HashMap::from([(
    "message.proto".to_string(),
    r#"syntax = "proto3"; package message; message Message { string id = 1; string content = 2; }"#.to_string(),
)]);
let descriptor = core::compile_proto_from_sources(files, "message.proto")?;
```

## Step 3: JSON → Protobuf → JSON

```rust
let json = r#"{"id":"123","content":"Hello"}"#;

// JSON -> Protobuf bytes
let wire = core::json_to_protobuf_bytes(json, &descriptor, "message.Message")?;

// Protobuf bytes -> JSON (pretty)
let out = core::protobuf_to_json_string(&wire, &descriptor, true, "message.Message")?;
println!("{out}");
```

## Complete example

```rust
use protoruf::core;

fn main() -> Result<(), String> {
    let descriptor = core::compile_proto("message.proto", None)?;

    let json = r#"{"id":"123","content":"Hello"}"#;
    let wire = core::json_to_protobuf_bytes(json, &descriptor, "message.Message")?;
    let out = core::protobuf_to_json_string(&wire, &descriptor, true, "message.Message")?;

    println!("{out}");
    Ok(())
}
```

## What's Next?

- [Basic Usage](basic-usage.md) — every supported Protobuf feature
- [Proto Compilation](proto-files.md) — imports, well-known types, in-memory compilation
- [Performance & Patterns](advanced.md) — the cached path for hot loops
- [API Reference](api.md)
