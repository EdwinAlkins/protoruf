# Changelog

All notable changes to **protoruf** are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.5] - 2026-06-18

A follow-up to 0.1.4 focused on **concurrency** and **build ergonomics**: the
descriptor cache now reads in parallel, and heavy link-time optimization no
longer slows down every local rebuild.

### Changed

- **`DescriptorCache` now uses an `RwLock` instead of a `Mutex`** for its
  memoized descriptor lookups (via `parking_lot`). Concurrent conversions sharing
  a cache now take a shared read lock on the fast path and only escalate to a
  write lock the first time each message type is resolved (with a double-check to
  avoid races). This improves throughput when a single cache is shared across
  many threads.
- **LTO and `codegen-units = 1` moved from the `release` profile to a new opt-in
  `dist` profile.** Full LTO is expensive at compile time and was slowing down
  every local rebuild (`uv sync` / `maturin develop`, which build in release by
  default). Published wheels are now built with `maturin build --profile dist`,
  which also adds `strip = true`. `panic = "abort"` is deliberately left unset to
  preserve PyO3's panic-to-exception handling.

### Performance

- `protobuf_to_json` now pre-allocates its output buffer based on the input size
  (~3× the protobuf wire size), avoiding repeated reallocations as the JSON
  string grows.

### Dependencies

- Added `parking_lot` 0.12.

### Compatibility

- No breaking changes. The public API is unchanged from 0.1.4.

## [0.1.4] - 2026-06-18

This release focuses on **performance** and **correctness**: a new cached
descriptor pool that makes protoruf 5–10× faster than `google.protobuf` in hot
loops, and a fix for a silent data-loss bug affecting `map` fields.

### Added

- **`DescriptorCache`** — a reusable, pre-decoded descriptor pool. The free
  functions re-decode the descriptor set on every call, which dominates the cost
  in hot loops. Build a `DescriptorCache` once and reuse it across conversions
  for a **~7–14× speedup**:

  ```python
  from protoruf import compile_proto, DescriptorCache

  cache = DescriptorCache(compile_proto("schema.proto"))
  pb = cache.json_to_protobuf(json_str, "message.Message")
  js = cache.protobuf_to_json(pb, "message.Message", pretty=True)
  ```

  A single instance handles every message type in the descriptor and is safe to
  share across threads.
- New example `examples/05_descriptor_cache.py` and documentation covering the
  cache in the API reference and the advanced/performance guide.

### Fixed

- **Map fields were silently dropped on `json_to_protobuf`.** A `map<k, v>` such
  as `{"env": "prod"}` round-tripped to `{"": ""}`, losing all entries. Map
  fields are now converted correctly. Regression tests added.

### Changed

- Internal JSON ↔ Protobuf conversion now uses **prost-reflect's native serde**
  implementation instead of a hand-written converter (~140 lines removed). This
  is what fixes the map bug. **The JSON output format is unchanged** — proto
  field names (snake_case), 64-bit integers as JSON numbers, and enums as their
  numeric value.
- Release builds now enable **LTO** and `codegen-units = 1` (~25–30% faster
  reads).
- Encode buffers are pre-allocated to the exact message size.

### Dependencies

- Bumped `pyo3` from 0.23 to **0.28**.
- Enabled the `serde` feature on `prost-reflect`.
- Minimum `pydantic` raised to 2.13.4.

### Compatibility

- No breaking changes. Existing code using `compile_proto`, `load_descriptor`,
  `json_to_protobuf`, `protobuf_to_json`, `pydantic_to_protobuf`, and
  `protobuf_to_pydantic` continues to work unchanged. `DescriptorCache` is an
  opt-in addition.

### Benchmarks

Measured at 100k iterations, varied messages, vs `google.protobuf` (protobuf
7.35.1) using a `DescriptorCache`:

| Operation              | google.protobuf | protoruf      | Speedup |
| ---------------------- | --------------- | ------------- | ------- |
| Serialize (JSON→Proto) | ~52k msg/s      | ~271k msg/s   | ~5.2×   |
| Parse (Proto→JSON)     | ~62k msg/s      | ~657k msg/s   | ~10.6×  |

Without the cache, protoruf is roughly at parity with (slightly slower than)
`google.protobuf`, which is why `DescriptorCache` is recommended for any
high-throughput workload.

[0.1.5]: https://github.com/EdwinAlkins/protoruf/releases/tag/v0.1.5
[0.1.4]: https://github.com/EdwinAlkins/protoruf/releases/tag/v0.1.4
