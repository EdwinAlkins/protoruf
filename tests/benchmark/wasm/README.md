# WASM benchmarks

Same 2×2 grid as the Python/Node benchmarks, for the **WebAssembly module**
(`@protoruf/wasm`): **small vs ~1 MB** message × **free functions vs
`DescriptorCache`**.

These run **in Node** using the wasm-pack `nodejs` target (synchronous init — no
`await init()` needed), so the numbers are comparable to the Node addon.

| File | protoruf scenario | Message | Descriptor decoded |
| --- | --- | --- | --- |
| `benchmark.mjs` | Free functions | small | on **every** call |
| `benchmark_hot_loop.mjs` | `DescriptorCache` | small | **once**, outside the loop |
| `benchmark_large.mjs` | Free functions | **~1 MB** | on **every** call |
| `benchmark_large_hot_loop.mjs` | `DescriptorCache` | **~1 MB** | **once**, outside the loop |

## Build first

The benchmarks import the compiled module from `dist/wasm/`. From the repo root:

```bash
npm run build:wasm   # wasm-pack build --target nodejs --out-dir dist/wasm -- --features wasm
```

This needs the `wasm32-unknown-unknown` target and `wasm-pack`:

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

> wasm-pack builds release by default — good. (A debug build would be much slower.)

## Run

```bash
node tests/benchmark/wasm/benchmark.mjs
node tests/benchmark/wasm/benchmark_hot_loop.mjs
node tests/benchmark/wasm/benchmark_large.mjs
node tests/benchmark/wasm/benchmark_large_hot_loop.mjs
```

Each run prints a schema-agnostic round-trip check, then `protoruf` write/read with
**ms/msg** and **MB/s** throughput.

## Optional comparison: protobufjs

Optionally compares against [protobufjs](https://github.com/protobufjs/protobuf.js)
(loaded from the same descriptor), **skipped gracefully** if not installed:

```bash
npm i -D protobufjs
```

## Notes

- Binary values are `Uint8Array`.
- `DescriptorCache` owns WASM linear-memory state; the cached benchmarks call
  `cache.free()` when done.
- The schemas are compiled **in memory** (`compileProtoFromSources`) — the only option
  in WASM. The ~1 MB message is a `bench.Dataset` of thousands of nested records
  (scalars, enum, repeated, map, vector of doubles).
