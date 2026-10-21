# Installation (Browser / WASM)

The browser target is a WebAssembly module built with
[wasm-bindgen](https://rustwasm.github.io/wasm-bindgen/) / wasm-pack. It runs in the browser's
WASM sandbox — no filesystem, no network, no DOM access by default.

## Prerequisites

- A modern browser (or any WASM-capable runtime: Deno, Bun, edge runtimes)
- A bundler (Vite, webpack, …) or native ESM
- Rust toolchain is **NOT required** — the `.wasm` is published prebuilt

## Install

!!! note "v0.2.0 distribution"
    The npm registry release is coming soon. For now, install the prebuilt package from the
    [GitHub Releases page](https://github.com/EdwinAlkins/protoruf/releases) by URL:

```bash
npm install https://github.com/EdwinAlkins/protoruf/releases/download/v0.2.0/protoruf-wasm-0.2.0.tgz
```

The published package is the **`web`** target — it works with bundlers (Vite, webpack, …) and
with native ESM (`<script type="module">`). Once on npm, this becomes `npm i @protoruf/wasm`; the
code below is identical either way.

## Initialise

The WASM module loads **asynchronously**. Call the default-exported `init()` **once** before any
other call:

```ts
import init, { compileProtoFromSources } from "@protoruf/wasm";

await init(); // fetch + instantiate the .wasm
const descriptor = compileProtoFromSources({ "m.proto": 'syntax="proto3"; ...' }, "m.proto");
```

With a bundler, `init()` resolves the `.wasm` asset for you. If you serve the files yourself,
the module and the `.wasm` must be served over **HTTP(S)** — ES modules and `WebAssembly` fetch
do **not** work from `file://`.

!!! tip "Initialise once"
    Call `init()` a single time at app startup (e.g. await it in your entry module or a
    top-level `await`). Subsequent calls are cheap but unnecessary.

## Verify

```ts
import init, { compileProtoFromSources, jsonToProtobuf } from "@protoruf/wasm";

await init();
const d = compileProtoFromSources(
  { "m.proto": 'syntax="proto3"; package m; message M { string id = 1; }' },
  "m.proto",
);
console.log(jsonToProtobuf('{"id":"123"}', d, "m.M").length, "bytes");
```

## Build from source

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
# bundler target (Vite/webpack):
wasm-pack build --target bundler --out-dir dist/wasm-web -- --features wasm
# or --target web (native ESM <script>) / --target nodejs (Node/SSR)
```

| wasm-pack target | Use it for |
|---|---|
| `bundler` | Vite, webpack, Rollup (recommended for apps) |
| `web` | native ESM via `<script type="module">` |
| `nodejs` | Node.js / SSR (synchronous init, no `await init()`) |

See the build guide: <https://github.com/EdwinAlkins/protoruf/blob/main/dev-docs/npm-build.md>.

## Next Steps

- Follow the [Quick Start](quick-start.md)
- Read [Basic Usage](basic-usage.md) for every supported Protobuf feature
