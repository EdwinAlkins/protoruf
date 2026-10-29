# Changelog

All notable changes to **protoruf** are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.1] - Pending

### Security

- **Merged sub-messages decode in linear time.** A singular message field repeated on the
  wire was merged by decoding the concatenation again at each occurrence: quadratic, so a
  128 KB request took 5.8 s of CPU (and a 4 MB one, gRPC's default limit, hours). The
  occurrences are now collected and decoded once (4 MB: under 0.1 s).
- **Unknown enum numbers are no longer cached** on the `OpenIntEnum` class: each distinct
  number a peer sent used to add a pseudo-member for the life of the process. Pseudo-members
  still compare equal to their int and re-encode unchanged; compare them with `==`, not
  `is`.

### Fixed

- Field tags are checked: field number 0 or above 2^29 − 1 is refused (a number above 2^32
  used to be truncated and read as another field), as are groups and wire types 6 and 7,
  everywhere a tag is read (messages, map entries, well-known types).
- `unknown_enum="error"` also refuses undeclared enum numbers on encoding, not only on
  decoding.

## [0.5.0] - 2026-10-29

### Added

- **`Codec`**, the native codec's public API (`protoruf.Codec`), created with
  `Schema(descriptor).codec(message_type, model, *, mode, field_names, unknown_enum,
  naive_datetime)`. `NativeCodec` (0.3 / 0.4) remains as an alias in `protoruf._protoruf`.
- `mode="dict"`: plain dicts even when a model is given.
- `field_names={message full name: {field number: Python name}}`: Python names that
  differ from the proto ones, in every message of the tree (`from` → `from_`), in every
  mode.
- `google.type.Date` ↔ `datetime.date`. Encoding also takes the `{"year", "month",
  "day"}` dict of 0.4; a `datetime` is refused rather than losing its time.

### Changed

- **`google.type.Date` decodes to `datetime.date`** instead of a `{"year", "month",
  "day"}` dict; a partial date (zero month or day) raises `ValueError`.
- Validated decoding passes `by_name=True` to Pydantic: models whose fields have aliases
  decode without needing `populate_by_name`.
- **oneof**: encoding two members of the same oneof raises `ValueError`; decoding keeps the
  last member read and unsets the others, as the reference implementations do.

### Fixed

- A singular message field that appears several times on the wire is merged (scalars from
  the last occurrence, repeated fields concatenated, sub-messages merged recursively),
  instead of keeping only the last occurrence.

### Tests & CI

- Differential test against `google.protobuf` (upb): 400 seeded random messages covering
  every scalar type and its bounds, open enums, packed and unpacked lists, maps, nesting,
  oneofs and well-known types; bytes identical (map entries compared as a set) and decoded
  back to the same values.
- Robustness test: random and mutated inputs through every decoding mode raise
  `ValueError` and never panic; nesting deeper than 100 is refused.
- The Python CI installs the `benchmark` group (protobuf), so the tests that compare with
  `google.protobuf` run instead of being skipped.

## [0.4.0] - 2026-10-29

### Added

- `compile_protos(roots, include_paths, sources, *, include_imports, source_info,
  standard_protos, output_path)` (Python): several root files, in-memory sources and
  include paths in one compilation. `source_info` (comments) is off by default.
- Standard protos embedded in the library: `google/rpc/*`, `google/type/*`,
  `grpc/health/v1` and `grpc/reflection/{v1,v1alpha}` (Apache-2.0, unmodified copies
  of `third_party/proto`), resolved after the user's files so they can be overridden.
- `Schema(descriptor)`: immutable, typed view of files, messages, fields, enums,
  services and comments, plus gRPC reflection helpers (`file_descriptor_proto`,
  `file_containing_symbol`, `file_dependencies`). Backed by the pure-Rust
  `describe::describe_descriptor_set` and `describe::split_descriptor_set`.

## [0.3.0] - 2026-10-28

### Added

- `include_default_values` option on `protobuf_to_json` (free function and
  `DescriptorCache`, Python binding): emits fields without presence even at their
  default value. Fields with presence (`optional`, messages, `oneof` members) are
  still emitted only when set. Off by default: the JSON output is unchanged.
- Experimental `NativeCodec` (spike, API not final): protobuf bytes <-> Python
  objects and Pydantic models without the JSON round-trip. Maps the well-known
  types to native values (`Timestamp` <-> aware `datetime`, `Duration` <->
  `timedelta`, wrappers <-> scalar or `None`, `Struct`/`Value`/`ListValue` <-> JSON
  values, `FieldMask` <-> `list[str]`), and offers a `mode="construct"` that builds
  model instances (nested models and enum members included) without validation.
- `OpenIntEnum`: an `IntEnum` that keeps undeclared values as cached
  `UNRECOGNIZED_<n>` pseudo-members, matching proto3 open enums.

### Changed

- Proto compile errors include the source location for every binding:
  `Failed to compile proto file: e.proto:4:3: name 'Missing' is not defined`
  (the location used to be dropped). The message prefix is unchanged.

### Fixed

- `protobuf_to_pydantic` no longer reports a required field holding a zero value
  (`False`, `0`, `""`, empty list/map) as missing: the helpers now request the
  default values from the JSON conversion.
- `pydantic_to_protobuf` / `protobuf_to_pydantic` handle `bytes` fields: they are
  written and read as base64, as the protobuf JSON mapping requires. Previously
  binary data failed to encode (`invalid base64` / invalid UTF-8) and base64 text
  was returned as-is on decode. Models without `bytes` fields keep the direct JSON
  path.

### CI

- `cargo clippy` also runs with the `python` feature, so the Python binding is linted.
- Standard protos not embedded by protox (`google/rpc`, `google/type`, gRPC health and
  reflection) are vendored under `third_party/proto/` (Apache-2.0) for the test suite.

## [0.2.0] - 2026-10-21

### Added

- Node.js and browser/WASM bindings over the shared Rust conversion core.
- In-memory proto compilation for all bindings, with optional imported-file inclusion.
- A process-wide, 64-entry LRU for decoded descriptor pools used by free functions.

### Fixed before publication

- Tag pushes start GitHub assets and PyPI publication independently, with a shared
  version check. Python wheels cover CPython 3.12–3.14 on Linux x86_64, Windows x64,
  macOS Intel, and macOS arm64 using the `dist` profile.
- Cache hits no longer copy descriptor bytes; misses decode outside the global lock.
- The browser example uses the correct `protobufToJson` argument order.
- Performance docs now describe the global LRU, and `include_imports=False` is
  documented as unsuitable for standalone conversion when imports are required.

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
  default). The profile adds `strip = true`; the publication workflow still
  used `--release` at the time and switches to `--profile dist` for 0.2.0.
  `panic = "abort"` is deliberately left unset to preserve PyO3's
  panic-to-exception handling.

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
