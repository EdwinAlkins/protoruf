# protoruf

<p align="center">
  <img src="docs/assets/logo.png" alt="protoruf logo" width="200">
  <img src="docs/assets/logo_js.png" alt="protoruf logo" width="200">
</p>

<p align="center">
  <strong>High-performance JSON ↔ Protobuf conversion, powered by Rust —<br>for Python, Node.js, the browser (WASM) and Rust.</strong>
</p>

<p align="center">
  <a href="https://EdwinAlkins.github.io/protoruf/">📖 Documentation</a> •
  <a href="https://github.com/EdwinAlkins/protoruf">💻 Source</a> •
  <a href="https://pypi.org/project/protoruf/">📦 PyPI</a> •
  <a href="https://github.com/EdwinAlkins/protoruf/releases">📦 Releases (node &amp; wasm)</a>
</p>

---

protoruf converts between **JSON and Protobuf** dynamically — no `protoc`, no generated
classes. You compile a `.proto` to a descriptor once, then convert in both directions at
Rust speed.

A single **pure-Rust core** (`src/core.rs`) powers every target through a thin binding
layer, so the conversion logic is shared, never duplicated:

| Target | Package | Install | Docs |
|---|---|---|---|
| **Python** | [`protoruf`](https://pypi.org/project/protoruf/) | `pip install protoruf` | [Python guide](https://EdwinAlkins.github.io/protoruf/) |
| **Node.js** | `@protoruf/node` | [GitHub Releases](https://github.com/EdwinAlkins/protoruf/releases) by URL — npm coming soon | [Node guide](https://EdwinAlkins.github.io/protoruf/) |
| **Browser (WASM)** | `@protoruf/wasm` | [GitHub Releases](https://github.com/EdwinAlkins/protoruf/releases) by URL — npm coming soon | [Browser guide](https://EdwinAlkins.github.io/protoruf/) |
| **Rust** | `protoruf` core crate | — | [Rust guide](https://EdwinAlkins.github.io/protoruf/) |

> **Node & browser install (v0.2.0):** the npm registry release is coming soon. For now, grab the prebuilt tarballs from the [Releases page](https://github.com/EdwinAlkins/protoruf/releases) and install by URL:
> ```bash
> npm install https://github.com/EdwinAlkins/protoruf/releases/download/v0.2.0/protoruf-node-0.2.0.tgz
> npm install https://github.com/EdwinAlkins/protoruf/releases/download/v0.2.0/protoruf-wasm-0.2.0.tgz
> ```

## Features

- ⚡ Fast JSON ↔ Protobuf conversion powered by Rust
- 🔁 One shared core, four targets (Python / Node / Browser / Rust)
- 🔒 Type-safe with explicit message type specification
- 📦 Built-in `.proto` compilation via [`protox`](https://crates.io/crates/protox) — no `protoc`, no codegen
- 🧠 Reusable `DescriptorCache` for high-throughput conversion

## Quick start

**Python**

```python
from protoruf import compile_proto, json_to_protobuf, protobuf_to_json

descriptor = compile_proto("message.proto")
pb = json_to_protobuf('{"id": "123"}', descriptor, message_type="message.Message")
print(protobuf_to_json(pb, descriptor, message_type="message.Message", pretty=True))
```

**Node.js**

```js
import { compileProto, jsonToProtobuf, protobufToJson } from "@protoruf/node";

const descriptor = compileProto("message.proto");
const pb = jsonToProtobuf('{"id":"123"}', descriptor, "message.Message");
console.log(protobufToJson(pb, descriptor, true, "message.Message"));
```

**Browser (WASM)**

```js
import init, { compileProtoFromSources, jsonToProtobuf } from "@protoruf/wasm";

await init();                                   // load & instantiate the .wasm
const descriptor = compileProtoFromSources(
  { "message.proto": 'syntax="proto3"; package message; message Message { string id = 1; }' },
  "message.proto",
);
const pb = jsonToProtobuf('{"id":"123"}', descriptor, "message.Message");
```

> `compileProto(path)` reads the filesystem — available in Python and Node only. In the
> browser there is no filesystem, so compile from in-memory sources with
> `compileProtoFromSources`.

See the full guides in the [documentation](https://EdwinAlkins.github.io/protoruf/) and runnable
samples under [`examples/`](examples/) (Python) and [`examples/js/`](examples/js/) (Node, WASM, browser).

## Architecture

```
src/
├── core.rs     # pure-Rust engine: .proto compilation, JSON ↔ Protobuf, descriptor pool
├── lib.rs      # module hub (feature-gated bindings)
├── python.rs   # PyO3 binding        (feature "python")  -> CPython extension
├── node.rs     # napi-rs binding     (feature "node")    -> Node.js .node addon
└── wasm.rs     # wasm-bindgen binding (feature "wasm")   -> .wasm + JS glue
```

Each binding only translates types & errors around `core::*`; building one target never
pulls in another's dependencies. Design notes for each binding live in [`dev-docs/`](dev-docs/).

## Development

```bash
# Rust core
cargo test --lib

# Python (maturin reads features = ["python"] from pyproject.toml)
uv sync --group dev
uv run maturin develop
uv run pytest

# JS/TS bindings (Node + WASM), see dev-docs/npm-build.md
npm install
npm run test:js            # rebuilds the napi + wasm bindings, then runs Vitest
```

## License

MIT
