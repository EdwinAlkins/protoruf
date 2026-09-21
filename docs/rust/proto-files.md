# Proto Compilation (Rust)

Compiling `.proto` files to descriptor sets — from disk or fully in memory.

## From disk: `compile_proto`

```rust
pub fn compile_proto(
    proto_path: &str,
    include_paths: Option<Vec<String>>,
) -> Result<Vec<u8>, String>;
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `proto_path` | `&str` | Path to the `.proto` file |
| `include_paths` | `Option<Vec<String>>` | Include directories for `import`s (default: the parent directory of `proto_path`) |

**Returns** the serialized descriptor set (`Vec<u8>`).

```rust
use protoruf::core;

let descriptor = core::compile_proto("protos/api/service.proto", Some(vec!["protos".into()]))?;
```

Imports are resolved against `include_paths` and included in the descriptor — one descriptor is
self-contained.

## In memory: `compile_proto_from_sources`

No filesystem access — works in sandboxed targets (this is what the WASM binding uses).

```rust
pub fn compile_proto_from_sources(
    files: HashMap<String, String>,
    root: &str,
) -> Result<Vec<u8>, String>;
```

`files` maps each logical file name to its source; `import`s resolve **by name** within the
map; `root` is the entry file. Google well-known types (`google/protobuf/*.proto`) are resolved
automatically.

```rust
use std::collections::HashMap;
use protoruf::core;

let files = HashMap::from([
    ("common.proto".into(), r#"syntax="proto3"; package common; message Id { string value = 1; }"#.into()),
    ("user.proto".into(), r#"syntax="proto3"; package user; import "common.proto"; message User { common.Id id = 1; }"#.into()),
]);
let descriptor = core::compile_proto_from_sources(files, "user.proto")?;
```

!!! note "Equivalent output"
    For the same sources, `compile_proto_from_sources` produces a descriptor equivalent to
    `compile_proto`. The in-memory compiler enables `include_imports`, so the descriptor is
    self-contained.

## Saving & loading descriptors

A descriptor is just bytes — persist and reload with `std::fs`:

```rust
use std::fs;

// build step
fs::write("schema.desc", core::compile_proto("schema.proto", None)?).map_err(|e| e.to_string())?;

// runtime (no .proto needed)
let descriptor = fs::read("schema.desc").map_err(|e| e.to_string())?;
```

## Troubleshooting

| Error message | Fix |
|---|---|
| `Failed to compile proto file: ... not found` | Add the right `include_paths`, or include the file in `compile_proto_from_sources` |
| `Message type '...' not found in descriptor` | Check the type matches `package` + message name |
| `Failed to load descriptor pool: ...` | Regenerate the descriptor; the bytes are corrupt/truncated |

## Next Steps

- [Performance & Patterns](advanced.md)
- [API Reference](api.md)
