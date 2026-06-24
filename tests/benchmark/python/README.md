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
measured. They form a 2×2 grid: **small vs large** message, and **free functions
(re-decode each call) vs `DescriptorCache` (decoded once)**.

| File | protoruf scenario | Message | Descriptor decoded |
| --- | --- | --- | --- |
| `benchmark.py` | Free functions `json_to_protobuf` / `protobuf_to_json` | small (`tests/proto/message.proto`) | on **every** call |
| `benchmark_hot_loop.py` | `DescriptorCache` | small (`tests/proto/message.proto`) | **once**, outside the loop |
| `benchmark_large.py` | Free functions `json_to_protobuf` / `protobuf_to_json` | **5 000 records** (`proto/large.proto`) | on **every** call |
| `benchmark_large_hot_loop.py` | `DescriptorCache` | **5 000 records** (`proto/large.proto`) | **once**, outside the loop |

```bash
uv run python tests/benchmark/python/benchmark.py > benchmark.txt # small, "cold" (decode per call)
uv run python tests/benchmark/python/benchmark_hot_loop.py > benchmark_hot_loop.txt    # small, hot loop (cached pool)
uv run python tests/benchmark/python/benchmark_large.py > benchmark_large.txt    # large, "cold" (decode per call)
uv run python tests/benchmark/python/benchmark_large_hot_loop.py > benchmark_large_hot_loop.txt   # large, hot loop (cached pool)
```

Shared timing logic lives in [`benchmark_utils.py`](benchmark_utils.py).

### Methodology

Each script prints system info (CPU, OS, Python, Rust, protobuf) and follows the
same protocol:

| Parameter | Small messages | Large messages |
| --- | --- | --- |
| Warmup | 1 000 iterations | 10 iterations |
| Measured runs | 20 | 10 |
| Conversions per run | 100 000 | 200 |
| GC | disabled during measured runs | disabled during measured runs |
| Reported stats | median, p95, stddev | median, p95, stddev |
| Throughput | msg/s | msg/s **and** MB/s |
| Memory | peak traced (tracemalloc) + RSS Δ on Linux | same |

Before any timing, each script verifies a JSON ↔ Protobuf round-trip on the test
payload. During timing, each scenario prints live progress (warmup, run *n*/*N*,
memory sampling) so long runs do not appear stuck.

**What is measured:** the full **JSON ↔ Protobuf conversion stack** — JSON parsing,
serde/prost encode/decode, and JSON formatting — **not** raw protobuf encoding alone.
Both sides use the same descriptor compiled by protoruf; on the `google.protobuf`
side the factory/pool is built once outside the loop (google already caches its pool).

Do **not** extrapolate results linearly to arbitrary message counts: cache effects,
CPU boost, memory pressure, and allocator behaviour are not linear. The scripts report
msg/s and MB/s only — there is no "projection for 1 M messages".

The large benchmarks use a **fixed** dataset size (`N_RECORDS = 5 000`) for
reproducibility; the actual JSON size (~1.4 MB) is printed at runtime.

> The `google.protobuf` benchmark is optional: install it with
> `uv sync --group benchmark` (otherwise only protoruf is measured).

### Interpreting results

- **`benchmark.py`** reflects the simplest usage: every conversion re-decodes the
  `DescriptorPool`. On a *small* message that fixed cost dominates, and protoruf
  there is roughly on par with (slightly below) `google.protobuf`.
- **`benchmark_hot_loop.py`** reflects the recommended usage for high throughput:
  the pool is decoded once via `DescriptorCache` and then reused. protoruf there
  is typically several times faster on writes and reads (exact ratio depends on
  your machine — run the benchmark locally).
- **`benchmark_large.py` / `benchmark_large_hot_loop.py`** target the regime where
  the **payload conversion** dominates the fixed per-call costs — protoruf can show
  a real gain **even without** the cache. Both use [`proto/large.proto`](proto/large.proto)
  (`bench.Dataset`: nested records with scalars, enum, repeated field, map and vector).

When citing speedup numbers, always specify the scenario (small/large, cached/uncached)
and clarify that the comparison is JSON ↔ Protobuf conversion, not protobuf encode alone.
