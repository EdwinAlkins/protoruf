# @protoruf/node

<p align="center">
  <img src="https://raw.githubusercontent.com/EdwinAlkins/protoruf/main/docs/assets/logo_js.png" alt="protoruf logo" width="200">
</p>

<p align="center">
  <strong>High-performance JSON ↔ Protobuf conversion for Node.js, powered by Rust.</strong>
</p>

<p align="center">
  <a href="https://EdwinAlkins.github.io/protoruf/">📖 Documentation</a> •
  <a href="https://github.com/EdwinAlkins/protoruf">💻 Source</a> •
  <a href="https://www.npmjs.com/package/@protoruf/node">📦 npm</a>
</p>

---

Native Node.js addon (napi-rs) for converting between **JSON and Protobuf** dynamically —
no `protoc`, no generated classes. Compile a `.proto` to a descriptor once, then convert in
both directions at Rust speed.

This is the Node.js target of [protoruf](https://github.com/EdwinAlkins/protoruf); the same
Rust core also ships for [Python](https://pypi.org/project/protoruf/), the
[browser (WASM)](https://www.npmjs.com/package/@protoruf/wasm) and Rust.

## Install

```bash
npm i @protoruf/node
```

Prebuilt binaries are published per platform (Linux / macOS / Windows × x64 / arm64) and
selected automatically at install time.

## Usage

```ts
import {
  compileProto,
  compileProtoFromSources,
  jsonToProtobuf,
  protobufToJson,
  DescriptorCache,
} from "@protoruf/node";

// Compile a .proto from disk (Node can read the filesystem)...
const descriptor = compileProto("message.proto");
// ...or from in-memory sources (no disk access):
// const descriptor = compileProtoFromSources({ "message.proto": "syntax=\"proto3\"; ..." }, "message.proto");

// JSON -> Protobuf
const pb = jsonToProtobuf('{"id":"123","tags":["a","b"]}', descriptor, "message.Message");

// Protobuf -> JSON
const json = protobufToJson(pb, descriptor, /* pretty */ true, "message.Message");
```

### High throughput: `DescriptorCache`

Decoding the descriptor set is the dominant cost of each conversion. Decode it once and
reuse it:

```ts
const cache = new DescriptorCache(descriptor);
const pb = cache.jsonToProtobuf('{"id":"123"}', "message.Message");
const json = cache.protobufToJson(pb, "message.Message", false);
```

## Notes

- **Binary type.** Functions return and accept Node `Buffer` (a `Uint8Array` at runtime).
- **`compileProto(path)`** reads the filesystem — Node/native only. For sandboxed contexts
  use `compileProtoFromSources`.
- **64-bit integers.** `int64`/`uint64` are emitted as JSON numbers; values above 2^53 lose
  precision under `JSON.parse`. See the docs for options.

Full API and guides: **<https://EdwinAlkins.github.io/protoruf/>**

## License

MIT
