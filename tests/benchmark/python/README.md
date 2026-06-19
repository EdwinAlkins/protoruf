# Tests & Benchmarks

## Tests

```bash
uv run pytest          # Python tests
cargo test --lib       # Rust tests
```

## Benchmarks

> ⚠️ **Build in release mode first — otherwise your numbers are meaningless.**
>
> `uv run maturin develop` (and the editable install via `uv`) compile the Rust
> extension in **debug** mode by default. A debug build has **no optimizations**
> (no inlining, overflow checks enabled, …) and runs **~10-50× slower** on the
> serde/prost code path. Benchmarking a debug `.so` makes protoruf look *slower*
> than `google.protobuf`; with a release build it is several times faster.
>
> Always rebuild in release before measuring:
>
> ```bash
> uv run maturin develop --release
> ```
>
> Sanity check the installed artifact — release is ~3 MB, debug is ~32 MB:
>
> ```bash
> ls -la python/protoruf/_protoruf*.so   # ~3 MB => release, ~32 MB => debug
> file   python/protoruf/_protoruf*.so   # debug shows "with debug_info"
> ```

Four benchmarks, deliberately **kept separate** to stay transparent about what is
measured. They form a 2×2 grid: **small vs ~1 MB** message, and **free functions
(re-decode each call) vs `DescriptorCache` (decoded once)**.

| File | protoruf scenario | Message | Descriptor decoded |
| --- | --- | --- | --- |
| `benchmark.py` | Free functions `json_to_protobuf` / `protobuf_to_json` | small (`tests/proto/message.proto`) | on **every** call |
| `benchmark_hot_loop.py` | `DescriptorCache` | small (`tests/proto/message.proto`) | **once**, outside the loop |
| `benchmark_large.py` | Free functions `json_to_protobuf` / `protobuf_to_json` | **~1 MB** (`proto/large.proto`) | on **every** call |
| `benchmark_large_hot_loop.py` | `DescriptorCache` | **~1 MB** (`proto/large.proto`) | **once**, outside the loop |

```bash
uv run python tests/benchmark/python/benchmark.py                  # small, "cold" (decode per call)
uv run python tests/benchmark/python/benchmark_hot_loop.py         # small, hot loop (cached pool)
uv run python tests/benchmark/python/benchmark_large.py            # ~1 MB, "cold" (decode per call)
uv run python tests/benchmark/python/benchmark_large_hot_loop.py   # ~1 MB, hot loop (cached pool)
```

- **`benchmark.py`** reflects the simplest usage: every conversion re-decodes the
  `DescriptorPool`. On a *small* message that fixed cost dominates, and protoruf
  there is roughly on par with (slightly below) `google.protobuf`.
- **`benchmark_hot_loop.py`** reflects the recommended usage for high throughput:
  the pool is decoded once via `DescriptorCache` and then reused. On the
  `google.protobuf` side the factory/pool is likewise built once outside the
  measured loop (google already caches its pool) — so the comparison is fair.
  protoruf there is roughly **5× faster on writes** and **~11× on reads**.
- **`benchmark_large.py` / `benchmark_large_hot_loop.py`** target the regime where
  protoruf shines most: with a ~1 MB message the **payload conversion** dominates
  the fixed per-call costs — so protoruf can show a real gain **even without** the
  cache (`benchmark_large.py`), and the cache adds less on top than it does for
  small messages. Both use a dedicated schema, [`proto/large.proto`](proto/large.proto)
  (`bench.Dataset`: thousands of nested records with scalars, an enum, a repeated
  field, a map and a vector of doubles), round-trip it for correctness, and report
  throughput in **MB/s**.

> The `google.protobuf` benchmark is optional: install it with
> `uv sync --group benchmark` (otherwise only protoruf is measured).
