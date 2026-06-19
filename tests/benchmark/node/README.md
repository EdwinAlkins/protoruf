# Node.js benchmarks

Same 2×2 grid as the Python benchmarks, for the **Node.js native addon**
(`@protoruf/node`): **small vs ~1 MB** message × **free functions vs
`DescriptorCache`**.

| File | protoruf scenario | Message | Descriptor decoded |
| --- | --- | --- | --- |
| `benchmark.mjs` | Free functions | small | on **every** call |
| `benchmark_hot_loop.mjs` | `DescriptorCache` | small | **once**, outside the loop |
| `benchmark_large.mjs` | Free functions | **~1 MB** | on **every** call |
| `benchmark_large_hot_loop.mjs` | `DescriptorCache` | **~1 MB** | **once**, outside the loop |

## Build first

The benchmarks import the compiled addon from `dist/`. From the repo root:

```bash
npm run build        # release addon -> dist/index.js (+ .node)
```

> A **release** build matters: a debug addon is several times slower and makes the
> numbers meaningless — exactly like `maturin develop --release` for Python.

## Run

```bash
node tests/benchmark/node/benchmark.mjs
node tests/benchmark/node/benchmark_hot_loop.mjs
node tests/benchmark/node/benchmark_large.mjs
node tests/benchmark/node/benchmark_large_hot_loop.mjs
```

Each run prints a schema-agnostic round-trip check, then `protoruf` write/read with
**ms/msg** and **MB/s** throughput.

## Optional comparison: protobufjs

Like the Python benchmarks compare against `google.protobuf`, these optionally
compare against [protobufjs](https://github.com/protobufjs/protobuf.js) (the JS
reference for dynamic JSON ↔ Protobuf). It is loaded from the same descriptor and
is **skipped gracefully** if not installed:

```bash
npm i -D protobufjs
```

## Notes

- Binary values are Node `Buffer`.
- The schemas are compiled **in memory** (`compileProtoFromSources`) so the files are
  self-contained and symmetric with the WASM benchmarks. The ~1 MB message is a
  `bench.Dataset` of thousands of nested records (scalars, enum, repeated, map, vector
  of doubles).
