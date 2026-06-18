# Tests & Benchmarks

## Tests

```bash
uv run pytest          # Python tests
cargo test --lib       # Rust tests
```

## Benchmarks

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
