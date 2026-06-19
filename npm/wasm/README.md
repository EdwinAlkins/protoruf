# @protoruf/wasm

<p align="center">
  <strong>High-performance JSON ↔ Protobuf conversion for the browser (WebAssembly), powered by Rust.</strong>
</p>

<p align="center">
  <a href="https://EdwinAlkins.github.io/protoruf/">📖 Documentation</a> •
  <a href="https://github.com/EdwinAlkins/protoruf">💻 Source</a> •
  <a href="https://www.npmjs.com/package/@protoruf/wasm">📦 npm</a>
</p>

---

WebAssembly build (wasm-bindgen) for converting between **JSON and Protobuf** dynamically,
**entirely in the browser** — no `protoc`, no server round-trip, no generated classes.
Runs in a sandboxed WASM module, so even hostile input can't reach the filesystem or network.

This is the browser target of [protoruf](https://github.com/EdwinAlkins/protoruf); the same
Rust core also ships for [Python](https://pypi.org/project/protoruf/),
[Node.js](https://www.npmjs.com/package/@protoruf/node) and Rust.

## Install

```bash
npm i @protoruf/wasm
```

Works with Vite, webpack and other bundlers, as well as native ESM (`<script type="module">`).

## Usage

The WASM module initialises **asynchronously**: call `init()` once before anything else.

```ts
import init, {
  compileProtoFromSources,
  jsonToProtobuf,
  protobufToJson,
  DescriptorCache,
} from "@protoruf/wasm";

await init(); // load & instantiate the .wasm module

// No filesystem in the browser -> compile from in-memory sources.
const descriptor = compileProtoFromSources(
  { "message.proto": 'syntax="proto3"; package message; message Message { string id = 1; }' },
  "message.proto",
);

const pb = jsonToProtobuf('{"id":"123"}', descriptor, "message.Message");
const json = protobufToJson(pb, descriptor, /* pretty */ false, "message.Message");
```

### High throughput: `DescriptorCache`

```ts
const cache = new DescriptorCache(descriptor);
const pb = cache.jsonToProtobuf('{"id":"123"}', "message.Message");
const json = cache.protobufToJson(pb, "message.Message", false);
cache.free(); // WASM objects own linear-memory state — free them when done
```

## Notes

- **No filesystem.** `compileProto(path)` is **not** exposed. Compile from in-memory sources,
  or load a pre-compiled descriptor's bytes (e.g. via `fetch`).
- **Binary type.** Functions return and accept `Uint8Array`.
- **Memory.** `DescriptorCache` owns native (linear-memory) state: call `cache.free()` (or use
  `using cache = new DescriptorCache(...)`) when done.
- **64-bit integers.** `int64`/`uint64` are emitted as JSON numbers; values above 2^53 lose
  precision under `JSON.parse`.

Full API and guides: **<https://EdwinAlkins.github.io/protoruf/>**

## License

MIT
