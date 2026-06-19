# Rust core

protoruf is, at heart, a **Rust library**. Every language target (Python, Node.js, the
browser) is a thin binding over the same pure-Rust engine — so the conversion logic is written
once and behaves identically everywhere.

## Architecture

```
src/
├── core.rs     # pure-Rust engine: .proto compilation, JSON ↔ Protobuf, descriptor pool
├── lib.rs      # module hub (feature-gated bindings)
├── python.rs   # PyO3 binding        (feature "python")  -> CPython extension
├── node.rs     # napi-rs binding     (feature "node")    -> Node.js .node addon
└── wasm.rs     # wasm-bindgen binding (feature "wasm")   -> .wasm + JS glue
```

- **`core.rs` is dependency-free of any FFI.** It exposes plain functions returning
  `Result<_, String>` and is fully unit-tested with `cargo test`.
- **Each binding only translates types & errors** around `core::*`. They are gated behind Cargo
  feature flags, so building one target never pulls in another's dependencies (PyO3, napi, …).

The engine is built on [`protox`](https://crates.io/crates/protox) (in-process `.proto`
compilation, no `protoc`), [`prost`](https://crates.io/crates/prost) and
[`prost-reflect`](https://crates.io/crates/prost-reflect) (dynamic messages), and
[`serde_json`](https://crates.io/crates/serde_json).

## Using the core from Rust

The crate is the foundation of the published bindings. To use it directly in a Rust project,
depend on it by git (or path) and call `core::*`:

```toml
# Cargo.toml
[dependencies]
protoruf = { git = "https://github.com/EdwinAlkins/protoruf" }
```

```rust
use protoruf::core;

let descriptor = core::compile_proto("schema.proto", None)?;
let pb = core::json_to_protobuf_bytes(r#"{"id":"123"}"#, &descriptor, "user.User")?;
let json = core::protobuf_to_json_string(&pb, &descriptor, true, "user.User")?;
```

For high-throughput loops, decode the pool once and reuse the resolved descriptor via the
`*_with_descriptor` helpers (see the [API reference](api.md)).

## Building the bindings

```bash
cargo build --features python    # CPython extension (via maturin in practice)
cargo build --features node      # Node.js addon (via @napi-rs/cli)
cargo build --features wasm      # WASM module (via wasm-pack)
cargo test --lib                 # core unit tests (no feature needed)
```

## Design notes

In-depth analyses of each binding (trade-offs, FFI boundary, memory ownership, security) live
in the repository's developer notes:

- [JavaScript / TypeScript (napi + WASM)](https://github.com/EdwinAlkins/protoruf/blob/main/dev-docs/javascript-typescript.md)
- [Java](https://github.com/EdwinAlkins/protoruf/blob/main/dev-docs/java.md)
- [C / C++](https://github.com/EdwinAlkins/protoruf/blob/main/dev-docs/c-cpp.md)
