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

Two benchmarks, deliberately **kept separate** to stay transparent about what is
measured. Both share the same message, the same descriptor, and the same number
of iterations (`ITERATIONS`): **only protoruf's descriptor strategy changes**.

| File | protoruf scenario | Descriptor decoded |
| --- | --- | --- |
| `benchmark.py` | Free functions `json_to_protobuf` / `protobuf_to_json` | on **every** call |
| `benchmark_hot_loop.py` | `DescriptorCache` | **once**, outside the loop |

```bash
uv run python tests/benchmark.py            # "cold" call (decode per call)
uv run python tests/benchmark_hot_loop.py   # hot loop (cached pool)
```

- **`benchmark.py`** reflects the simplest usage: every conversion re-decodes the
  `DescriptorPool`. That cost dominates, and protoruf there is roughly on par with
  (slightly below) `google.protobuf`.
- **`benchmark_hot_loop.py`** reflects the recommended usage for high throughput:
  the pool is decoded once via `DescriptorCache` and then reused. On the
  `google.protobuf` side the factory/pool is likewise built once outside the
  measured loop (google already caches its pool) — so the comparison is fair.
  protoruf there is roughly **5× faster on writes** and **~11× on reads**.

> The `google.protobuf` benchmark is optional: install it with
> `uv sync --group benchmark` (otherwise only protoruf is measured).
