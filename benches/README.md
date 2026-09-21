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
cargo bench free_global_lru_json_to_proto
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
| `free_global_lru_json_to_proto` | JSON → Protobuf | Free function with a warmed global LRU |
| `hot_json_to_proto` | JSON → Protobuf | Reuses a pre-built `DescriptorResolver` |
| `free_global_lru_proto_to_json` | Protobuf → JSON | Free function with a warmed global LRU |
| `hot_proto_to_json` | Protobuf → JSON | Reuses a pre-built `DescriptorResolver` |
| `compile_proto_from_sources` | `.proto` sources → descriptor set | In-memory compilation |

These use the small fixture. The large-descriptor group creates 10,000 padding
messages and measures three separate paths:

| Benchmark | What it measures |
| --- | --- |
| `large_descriptor/decode_pool` | A genuinely cold `DescriptorPool::decode` |
| `large_descriptor/free_global_lru_hit` | Free-function conversion after warming the LRU |
| `large_descriptor/explicit_cache` | Conversion with a pre-resolved message descriptor |

The cold measurement isolates pool decoding; the two warmed measurements include
conversion of the same small payload. Compare the warmed paths to estimate
per-call descriptor hashing, lookup and locking costs. No benchmark here clears
the global LRU before every conversion.

## Notes

- `black_box` (`std::hint::black_box`) wraps the inputs so the optimizer cannot
  constant-fold the work away.
- These are Rust-only benches. End-to-end binding benchmarks (Python, Node, WASM)
  live under `tests/benchmark/`.
