# WASM benchmarks

Same 2×2 grid as the Python/Node benchmarks, for the **WebAssembly module**
(`@protoruf/wasm`): **small vs large** message × **free functions vs
`DescriptorCache`**.

These run **in Node** using the wasm-pack `nodejs` target (synchronous init — no
`await init()` needed), so the numbers are comparable to the Node addon.

| File | protoruf scenario | Message | Descriptor decoded |
| --- | --- | --- | --- |
| `benchmark.mjs` | Free functions | small | on **every** call |
| `benchmark_hot_loop.mjs` | `DescriptorCache` | small | **once**, outside the loop |
| `benchmark_large.mjs` | Free functions | **5 000 records** | on **every** call |
| `benchmark_large_hot_loop.mjs` | `DescriptorCache` | **5 000 records** | **once**, outside the loop |

Shared timing logic lives in [`common.mjs`](common.mjs) (the JS counterpart of the
Python [`benchmark_utils.py`](../python/benchmark_utils.py)).

## Build first

The benchmarks import the compiled module from `dist/wasm/`. From the repo root:

```bash
npm run build:wasm   # wasm-pack (release + wasm-opt -Oz via Cargo.toml metadata)
```

This needs the `wasm32-unknown-unknown` target and `wasm-pack`:

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

> wasm-pack builds release by default and runs `wasm-opt` with bulk-memory /
> nontrapping-float-to-int enabled (see `[package.metadata.wasm-pack]` in
> `Cargo.toml`). A debug build would be much slower.

## Run

Pass `--expose-gc` so the harness can control garbage collection between runs
(without it, the benchmarks still run but GC is left uncontrolled):

```bash
node --expose-gc tests/benchmark/wasm/benchmark.mjs > wasm_benchmark.txt # small, "cold" (decode per call)
node --expose-gc tests/benchmark/wasm/benchmark_hot_loop.mjs > wasm_benchmark_hot_loop.txt       # small, hot loop (cached pool)
node --expose-gc tests/benchmark/wasm/benchmark_large.mjs > wasm_benchmark_large.txt          # large, "cold" (decode per call)
node --expose-gc tests/benchmark/wasm/benchmark_large_hot_loop.mjs > wasm_benchmark_large_hot_loop.txt # large, hot loop (cached pool)
```

## Methodology

Each script prints system info (CPU, OS, Node, Rust, protobufjs) and follows the
same protocol as the Python/Node benchmarks:

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

Optionally compares against [protobufjs](https://github.com/protobufjs/protobuf.js)
(loaded from the same `.proto` source), **skipped gracefully** if not installed:

```bash
npm i -D protobufjs
```

The reported `speedup (median)` compares the full JSON ↔ Protobuf conversion path,
not protobuf encode alone.

## Notes

- Binary values are `Uint8Array`.
- `DescriptorCache` owns WASM linear-memory state; the cached benchmarks call
  `cache.free()` when done.
- The schemas are compiled **in memory** (`compileProtoFromSources`) — the only option
  in WASM. The large message is a `bench.Dataset` of nested records (scalars, enum,
  repeated, map, vector of doubles), byte-for-byte identical to the Python generator.
