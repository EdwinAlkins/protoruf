# Rust benchmarks

[Criterion](https://github.com/bheisler/criterion.rs)-based micro-benchmarks for
the core conversion functions in `src/core.rs`. They measure the raw Rust path
only — no Python/Node/WASM binding overhead is involved.

## Running

```bash
cargo bench
```

Run a single benchmark by name (substring match):

```bash
cargo bench cold_json_to_proto
```

HTML reports (plots, statistics, regression vs. the previous run) are written to
`target/criterion/`; open `target/criterion/report/index.html` in a browser.

> Note: `cargo bench` builds the `conversion` bench with `harness = false`
> (see `Cargo.toml`). The same target is also exercised — without timing — by
> `cargo test --all-targets`, which is why CI compiles it too.

## What is measured

The fixtures come from `tests/proto/message.proto` and a small inline JSON
payload (`sample_json`). Each benchmark isolates one direction of conversion.

| Benchmark | Direction | Path |
| --- | --- | --- |
| `cold_json_to_proto` | JSON → Protobuf | Decodes the descriptor set on every call (`json_to_protobuf_bytes`) |
| `hot_json_to_proto` | JSON → Protobuf | Reuses a pre-built `DescriptorResolver` (`*_with_descriptor_owned`) |
| `cold_proto_to_json` | Protobuf → JSON | Decodes the descriptor set on every call (`protobuf_to_json_string`) |
| `hot_proto_to_json` | Protobuf → JSON | Reuses a pre-built `DescriptorResolver` (`*_with_descriptor_owned`) |
| `compile_proto_from_sources` | `.proto` sources → descriptor set | In-memory compilation (`compile_proto_from_sources`) |

### Cold vs. hot

- **Cold** paths reflect a one-shot call where the descriptor pool must be
  decoded from the descriptor-set bytes for each conversion.
- **Hot** paths reflect a steady-state loop where the descriptor is resolved
  once (via `DescriptorResolver`) and reused, isolating the cost of the
  serialization/deserialization itself.

Comparing the two shows how much of a conversion's cost is descriptor handling
versus the actual encode/decode work.

## Notes

- `black_box` (`std::hint::black_box`) wraps the inputs so the optimizer cannot
  constant-fold the work away.
- These are Rust-only benches. End-to-end binding benchmarks (Python, Node, WASM)
  live under `tests/benchmark/`.
