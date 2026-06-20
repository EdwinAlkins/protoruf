# Node.js benchmarks

Same 2×2 grid as the Python benchmarks, for the **Node.js native addon**
(`@protoruf/node`): **small vs large** message × **free functions vs
`DescriptorCache`**.

| File | protoruf scenario | Message | Descriptor decoded |
| --- | --- | --- | --- |
| `benchmark.mjs` | Free functions | small | on **every** call |
| `benchmark_hot_loop.mjs` | `DescriptorCache` | small | **once**, outside the loop |
| `benchmark_large.mjs` | Free functions | **5 000 records** | on **every** call |
| `benchmark_large_hot_loop.mjs` | `DescriptorCache` | **5 000 records** | **once**, outside the loop |

Shared timing logic lives in [`common.mjs`](common.mjs) (the JS counterpart of the
Python [`benchmark_utils.py`](../python/benchmark_utils.py)).

## Build first

The benchmarks import the compiled addon from `dist/`. From the repo root:

```bash
npm run build        # release addon -> dist/index.js (+ .node)
```

> A **release** build matters: a debug addon is several times slower and makes the
> numbers meaningless — exactly like `maturin develop --release` for Python.

## Run

Pass `--expose-gc` so the harness can control garbage collection between runs
(without it, the benchmarks still run but GC is left uncontrolled):

```bash
node --expose-gc tests/benchmark/node/benchmark.mjs > node_benchmark.txt               # small, "cold" (decode per call)
node --expose-gc tests/benchmark/node/benchmark_hot_loop.mjs > node_benchmark_hot_loop.txt       # small, hot loop (cached pool)
node --expose-gc tests/benchmark/node/benchmark_large.mjs > node_benchmark_large.txt          # large, "cold" (decode per call)
node --expose-gc tests/benchmark/node/benchmark_large_hot_loop.mjs > node_benchmark_large_hot_loop.txt # large, hot loop (cached pool)
```

## Methodology

Each script prints system info (CPU, OS, Node, Rust, protobufjs) and follows the
same protocol as the Python benchmarks:

| Parameter | Small messages | Large messages |
| --- | --- | --- |
| Warmup | 1 000 iterations | 10 iterations |
| Measured runs | 20 | 10 |
| Conversions per run | 10 000 | 200 |
| GC | `global.gc()` before each run (`--expose-gc`) | same |
| Reported stats | median, p95, stddev | median, p95, stddev |
| Throughput | msg/s | msg/s **and** MB/s |
| Memory | RSS Δ + heap Δ (`process.memoryUsage`) | same |

Before any timing, each script verifies a JSON ↔ Protobuf round-trip on the test
payload. During timing, each scenario prints live progress (warmup, run *n*/*N*,
memory sampling) so long runs do not appear stuck.

**What is measured:** the full **JSON ↔ Protobuf conversion stack** — JSON parsing,
encode/decode, and JSON formatting — **not** raw protobuf encoding alone. The large
dataset uses a **fixed** record count (`N_RECORDS = 5 000`) for reproducibility; the
actual JSON size is printed at runtime.

Do **not** extrapolate results linearly to arbitrary message counts: cache effects,
CPU boost, memory pressure, and GC behaviour are not linear.

## Optional comparison: protobufjs

Like the Python benchmarks compare against `google.protobuf`, these optionally
compare against [protobufjs](https://github.com/protobufjs/protobuf.js) (the JS
reference for dynamic JSON ↔ Protobuf). It is loaded from the same `.proto` source
and is **skipped gracefully** if not installed:

```bash
npm i -D protobufjs
```

The reported `speedup (median)` compares the full JSON ↔ Protobuf conversion path,
not protobuf encode alone.

## Notes

- Binary values are Node `Buffer`.
- The schemas are compiled **in memory** (`compileProtoFromSources`) so the files are
  self-contained and symmetric with the WASM benchmarks. The large message is a
  `bench.Dataset` of nested records (scalars, enum, repeated, map, vector of doubles),
  byte-for-byte identical to the Python generator.
