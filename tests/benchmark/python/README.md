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

Four benchmarks form a 2×2 grid: **small vs large** message, and **free
functions using the process-wide LRU vs explicit `DescriptorCache`**. These
scripts do not measure a cold descriptor decode. Earlier results labeled
"cold" should be rerun and relabeled before publication.

| File | protoruf scenario | Message | Pool reuse |
| --- | --- | --- | --- |
| `benchmark.py` | Free functions `json_to_protobuf` / `protobuf_to_json` | small (`tests/proto/message.proto`) | global LRU hit after warmup |
| `benchmark_hot_loop.py` | `DescriptorCache` | small (`tests/proto/message.proto`) | held by the cache |
| `benchmark_large.py` | Free functions `json_to_protobuf` / `protobuf_to_json` | **5 000 records** (`proto/large.proto`) | global LRU hit after warmup |
| `benchmark_large_hot_loop.py` | `DescriptorCache` | **5 000 records** (`proto/large.proto`) | held by the cache |

```bash
uv run python tests/benchmark/python/benchmark.py > benchmark.txt # small, global LRU hit
uv run python tests/benchmark/python/benchmark_hot_loop.py > benchmark_hot_loop.txt    # small, hot loop (cached pool)
uv run python tests/benchmark/python/benchmark_large.py > benchmark_large.txt    # large, global LRU hit
uv run python tests/benchmark/python/benchmark_large_hot_loop.py > benchmark_large_hot_loop.txt   # large, hot loop (cached pool)
```

The free-function runs warm the process-wide LRU before timing; they do not measure a cold descriptor decode. The explicit cache runs avoid the descriptor hash and global LRU lock.

Shared timing logic lives in [`benchmark_utils.py`](benchmark_utils.py).

### Methodology

Each script prints system info (CPU, OS, Python, Rust, protobuf) and follows the
same protocol:

| Parameter | Small messages | Large messages |
| --- | --- | --- |
| Warmup | 1 000 free / 100 explicit-cache iterations | 10 iterations |
| Measured runs | 20 | 10 |
| Conversions per run | 10 000 free / 1 000 explicit-cache | 200 |
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

- **`benchmark.py`** measures free functions with a warmed global LRU. Each
  conversion still hashes the descriptor bytes and looks up the message type.
- **`benchmark_hot_loop.py`** holds the pool and message descriptors in an
  explicit `DescriptorCache`. Compare it with the free-function run only after
  accounting for their different iteration counts.
- **`benchmark_large.py` / `benchmark_large_hot_loop.py`** target the regime where
  the **payload conversion** dominates the fixed per-call costs. Both reuse a
  decoded pool, through the global LRU or an explicit cache. Both use [`proto/large.proto`](proto/large.proto)
  (`bench.Dataset`: nested records with scalars, enum, repeated field, map and vector).

When citing speedup numbers, always specify the scenario (small/large, global LRU/explicit cache)
and clarify that the comparison is JSON ↔ Protobuf conversion, not protobuf encode alone.
